use crossbeam::atomic::AtomicCell;
use lazy_static::lazy_static;
use regex::Regex;
use std::io::{BufRead, BufReader, Read, Seek};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use songbird::input::reader::MediaSource;
use songbird::input::{Codec, Container, Input, Reader};

use crate::pulse::{Application, PulseAudio};

/// Keeps track of the selected application and provides a reader to discord
pub struct AudioSystem {
    pub selected_app: SelectedApp,
    pub status: CurrentAudioStatus,
    pub latency: AudioLatency,

    pulse: Arc<PulseAudio>,
    stream: AudioStream,
}

#[derive(Default)]
pub enum AudioStatus {
    #[default]
    Idle,
    Connecting(Application),
    Connected(Application),
    Searching(Application),
    Failed(String),
}

pub type CurrentAudioStatus = Arc<Mutex<AudioStatus>>;
pub type SelectedApp = Arc<Mutex<Option<Application>>>;
pub type AudioLatency = Arc<AtomicCell<u32>>;

impl AudioSystem {
    pub fn new(pulse: Arc<PulseAudio>) -> Self {
        Self {
            pulse,
            selected_app: Default::default(),
            status: Default::default(),
            stream: Default::default(),
            latency: Default::default(),
        }
    }

    pub fn run(audio: Arc<AudioSystem>) {
        poll_parec_events(audio.clone());
        //run_respawn_thread(audio);
    }

    pub fn set_application(&self, app: Application) {
        {
            let mut selected_app = self.selected_app.lock().unwrap();
            *selected_app = Some(app.clone());
        }

        self.stream
            .respawn(self.status.clone(), self.pulse.device_name(), app)
    }

    pub fn stream(&self) -> AudioStream {
        self.stream.clone()
    }
}

/// Provides the raw audio of a parec stream
#[derive(Clone)]
pub struct AudioStream {
    /// The current parec stream, if any
    parec: Arc<Mutex<Option<Parec>>>,
}

impl AudioStream {
    pub fn respawn(&self, status: CurrentAudioStatus, device: String, app: Application) {
        {
            *(status.lock().unwrap()) = AudioStatus::Connecting(app.clone());
        }

        match Parec::new(device, app) {
            Ok(new_parec) => {
                let mut parec = self.parec.lock().unwrap();
                *parec = Some(new_parec);
            }
            Err(err) => {
                *(status.lock().unwrap()) = AudioStatus::Failed(err);
            }
        }
    }

    pub fn clear(&self) {
        let mut parec = self.parec.lock().unwrap();
        *parec = None;
    }

    pub fn into_input(self) -> Input {
        Input::new(
            true,
            Reader::Extension(Box::new(self)),
            Codec::FloatPcm,
            Container::Raw,
            None,
        )
    }
}

impl Default for AudioStream {
    fn default() -> Self {
        Self {
            parec: Arc::new(Mutex::new(None)),
        }
    }
}

impl Read for AudioStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut lock = self.parec.lock().unwrap();
        let parec = (*lock).as_mut();

        match parec {
            Some(parec) => {
                let bytes_read = parec.stdout.read(buf).unwrap_or_default();

                // We don't want to tell songbird that the stream is over
                Ok(bytes_read.min(buf.len()))
            }
            None => Ok(buf.len()),
        }
    }
}

impl Seek for AudioStream {
    fn seek(&mut self, _: std::io::SeekFrom) -> std::io::Result<u64> {
        unreachable!()
    }
}

impl MediaSource for AudioStream {
    fn byte_len(&self) -> Option<u64> {
        None
    }

    fn is_seekable(&self) -> bool {
        false
    }
}

struct Parec {
    child: Mutex<Child>,
    stdout: ChildStdout,
    stderr: Option<ChildStderr>,
}

impl Parec {
    fn new(device: String, app: Application) -> Result<Self, String> {
        let mut child = Command::new("parec")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("--verbose")
            .arg("--device")
            .arg(device)
            .arg("--monitor-stream")
            .arg(app.sink_input_index.to_string())
            .arg("--format=float32le")
            .arg("--rate=48000")
            .arg("--channels=2")
            .arg("--latency=1")
            .arg("--process-time=1")
            .spawn()
            .map_err(|err| format!("Could not spawn Parec instance: {}", err))?;

        let stdout = child.stdout.take().expect("Take stdout from child");
        let stderr = child.stderr.take().expect("Take stderr from child");

        Ok(Self {
            child: child.into(),
            stderr: Some(stderr),
            stdout,
        })
    }
}

// We must implement this otherwise when a Parec stream is dropped, the child will continue to live
impl Drop for Parec {
    fn drop(&mut self) {
        self.child.lock().unwrap().kill().expect("Child killed");
    }
}

enum ParecEvent {
    TimedOut,
    /// Device name, index
    Connected(String, u32),
    /// Time, latency
    Time(f32, u32),
    StreamMoved,
}

lazy_static! {
    static ref CONNECTED_REGEX: Regex =
        Regex::new(r"Connected to device (.*?) \(index: (\d*), suspended: no\)").unwrap();
    static ref TIME_REGEX: Regex =
        Regex::new(r"Time: (\d+\.\d+) sec; Latency: (\d+) usec.").unwrap();
}

impl ParecEvent {
    const STREAM_MOVED_MESSAGE: &str = "Stream moved to";
    const STREAM_TIMEOUT: &str = "Stream error: Timeout";

    pub fn parse(line: String) -> Option<Self> {
        eprintln!("{}", &line);

        if let Some(captures) = CONNECTED_REGEX.captures(&line) {
            return Some(Self::Connected(
                captures[1].to_string(),
                captures[2].parse().expect("Parec gives valid index"),
            ));
        }

        if let Some(captures) = TIME_REGEX.captures(&line) {
            return Some(Self::Time(
                captures[1].parse().expect("Parec gives valid time"),
                captures[2].parse().expect("Parec gives valid latency"),
            ));
        }

        if line.contains(Self::STREAM_MOVED_MESSAGE) {
            return Some(Self::StreamMoved);
        }

        if line.contains(Self::STREAM_TIMEOUT) {
            return Some(Self::TimedOut);
        }

        None
    }
}

/// Runs a thread to check when the parec stream moves or is incorrect
fn poll_parec_events(audio: Arc<AudioSystem>) {
    thread::spawn(move || loop {
        let stderr = {
            let mut parec = audio.stream.parec.lock().unwrap();
            (*parec).as_mut().and_then(|parec| parec.stderr.take())
        };

        match stderr {
            Some(stderr) => {
                let mut reader = BufReader::new(stderr);

                let mut line = Vec::new();
                reader.read_until(0x13, &mut line).expect("Read line");

                let line = String::from_utf8(line).unwrap_or_default();
                let event = ParecEvent::parse(line);

                if let Some(event) = event {
                    let selected_app = audio.selected_app.lock().unwrap();

                    if let Some(selected_app) = &*selected_app {
                        match event {
                            ParecEvent::TimedOut => {
                                *(audio.status.lock().unwrap()) =
                                    AudioStatus::Failed("The stream timed out.".to_string());
                            }
                            ParecEvent::Connected(device, _) => {
                                if !device.contains(&audio.pulse.device_name()) {
                                    *(audio.status.lock().unwrap()) =
                                        AudioStatus::Searching(selected_app.clone());

                                    audio.stream.clear();
                                    break;
                                }

                                *(audio.status.lock().unwrap()) =
                                    AudioStatus::Connected(selected_app.clone());
                            }
                            ParecEvent::Time(_, latency) => {
                                dbg!(&latency);
                                audio.latency.store(latency);
                            }
                            ParecEvent::StreamMoved => {
                                *(audio.status.lock().unwrap()) =
                                    AudioStatus::Searching(selected_app.clone());

                                audio.stream.clear();
                                break;
                            }
                        }
                    }
                }
            }
            None => {
                thread::sleep(Duration::from_millis(100));
            }
        };
    });
}

/// Runs a thread that respawns parec when the selected application is ready again
fn run_respawn_thread(audio: Arc<AudioSystem>) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(100));

        let stream_cleared = {
            let parec = audio.stream.parec.lock().unwrap();
            parec.is_none()
        };

        let selected_app = {
            let selected_app = audio.selected_app.lock().unwrap();

            match (*selected_app).as_ref() {
                Some(app) => app.clone(),
                None => continue,
            }
        };

        if stream_cleared {
            loop {
                thread::sleep(Duration::from_millis(100));
                audio.pulse.update_applications();

                let apps = {
                    let mut a = audio.pulse.applications();
                    a.reverse();
                    a
                };

                let app = apps
                    .iter()
                    .find(|app| app.sink_input_name == selected_app.sink_input_name)
                    .or_else(|| apps.iter().find(|app| app.id == selected_app.id))
                    .or_else(|| {
                        apps.iter()
                            .find(|app| app.process_id == selected_app.process_id)
                    });

                if let Some(app) = app {
                    audio.set_application(app.to_owned());
                    break;
                }
            }
        }
    });
}
