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
    selected_app: Mutex<Option<Application>>,
    pulse: PulseAudio,
    stream: AudioStream,
}

impl AudioSystem {
    pub fn new(pulse: PulseAudio) -> Self {
        Self {
            pulse,
            selected_app: Default::default(),
            stream: Default::default(),
        }
    }

    pub fn run(audio: Arc<AudioSystem>) {
        run_check_thread(audio.clone());
        run_respawn_thread(audio);
    }

    pub fn set_application(&self, app: Application) {
        {
            let mut selected_app = self.selected_app.lock().unwrap();
            *selected_app = Some(app.clone());
        }

        println!("Spawning recorder for {}", app.name);
        self.stream.respawn(self.pulse.device_name(), app)
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
    pub fn respawn(&self, device: String, app: Application) {
        let new_parec = Parec::new(device, app);

        let mut parec = self.parec.lock().unwrap();
        *parec = Some(new_parec);
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
        let mut bytes_read = 0;

        while bytes_read < buf.len() {
            let mut lock = self.parec.lock().unwrap();
            let parec = (*lock).as_mut();

            dbg!(&bytes_read);

            match parec {
                Some(parec) => bytes_read += parec.stdout.read(buf).unwrap_or_default(),
                None => continue,
            };
        }

        Ok(bytes_read)
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
    fn new(device: String, app: Application) -> Self {
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
            .expect("Could not spawn parec instance");

        let stdout = child.stdout.take().expect("Take stdout from child");
        let stderr = child.stderr.take().expect("Take stderr from child");

        Self {
            child: child.into(),
            stderr: Some(stderr),
            stdout,
        }
    }
}

// We must implement this otherwise when a Parec stream is dropped, the child will continue to live
impl Drop for Parec {
    fn drop(&mut self) {
        self.child.lock().unwrap().kill().expect("Child killed");
    }
}

/// Runs a thread to check when the parec stream moves or is incorrect
fn run_check_thread(audio: Arc<AudioSystem>) {
    thread::spawn(move || loop {
        let parec_stderr = {
            let mut parec = audio.stream.parec.lock().unwrap();
            (*parec).as_mut().and_then(|parec| parec.stderr.take())
        };

        match parec_stderr {
            Some(stderr) => {
                let mut reader = BufReader::new(stderr);
                let device = audio.pulse.device_name();

                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).expect("Read line");

                    eprint!("{}", line);

                    // Parec connected or moved to the wrong device
                    if line.contains(STREAM_CONNECTED_MESSAGE) && !line.contains(&device)
                        || line.contains(STREAM_MOVED_MESSAGE)
                    {
                        audio.stream.clear();
                        break;
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
                audio.pulse.update_applications();
                let app = audio
                    .pulse
                    .applications()
                    .into_iter()
                    .find(|app| app.sink_input_name == selected_app.sink_input_name);

                if let Some(app) = app {
                    audio.set_application(app);
                    break;
                }
            }
        }
    });
}

const STREAM_CONNECTED_MESSAGE: &str = "Connected to device";
const STREAM_MOVED_MESSAGE: &str = "Stream moved to";
