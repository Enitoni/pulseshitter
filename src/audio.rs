use crossbeam::atomic::AtomicCell;
use crossbeam::channel::{unbounded, Receiver, Sender};
use lazy_static::lazy_static;
use regex::Regex;
use ringbuf::{HeapConsumer, HeapProducer, HeapRb};
use std::io::{BufReader, Read, Seek};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use songbird::input::reader::MediaSource;
use songbird::input::{Codec, Container, Input, Reader};

use crate::pulse::{Application, PulseAudio};

pub type AudioProducer = Arc<Mutex<HeapProducer<u8>>>;
pub type AudioConsumer = Arc<Mutex<HeapConsumer<u8>>>;
pub type CurrentAudioStatus = Arc<Mutex<AudioStatus>>;
pub type SelectedApp = Arc<Mutex<Option<Application>>>;

pub type AudioTime = Arc<AtomicCell<f32>>;
pub type AudioLatency = Arc<AtomicCell<u32>>;

const BUFFER_SIZE: usize = 4068;

/// Keeps track of the selected application and provides a reader to discord
pub struct AudioSystem {
    pub selected_app: SelectedApp,
    pub status: CurrentAudioStatus,

    pub latency: AudioLatency,
    pub time: AudioTime,

    pulse: Arc<PulseAudio>,
    child: Arc<Mutex<Option<Child>>>,

    sender: Sender<AudioMessage>,
    receiver: Receiver<AudioMessage>,

    audio_producer: AudioProducer,
    audio_consumer: AudioConsumer,
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

pub enum AudioMessage {
    /// Sent when a parec process has spawned
    StreamSpawned(ChildStdout, ChildStderr),

    /// Stop streaming
    Clear,
}

impl AudioSystem {
    pub fn new(pulse: Arc<PulseAudio>) -> Self {
        let (sender, receiver) = unbounded();

        let (audio_producer, audio_consumer) = HeapRb::new(BUFFER_SIZE).split();

        Self {
            pulse,
            child: Default::default(),
            status: Default::default(),

            latency: Default::default(),
            time: Default::default(),

            selected_app: Default::default(),

            sender,
            receiver,

            audio_producer: Mutex::from(audio_producer).into(),
            audio_consumer: Mutex::from(audio_consumer).into(),
        }
    }

    pub fn run(audio: Arc<AudioSystem>) {
        run_audio_thread(audio);
    }

    pub fn set_application(&self, app: Application) {
        *(self.selected_app.lock().unwrap()) = Some(app.clone());
        *(self.status.lock().unwrap()) = AudioStatus::Connecting(app.clone());

        thread::spawn({
            let device = self.pulse.device_name();
            let sender = self.sender.clone();
            let status = self.status.clone();
            let stored_child = self.child.clone();

            move || {
                match spawn_parec(device, app) {
                    Ok(mut child) => {
                        let stdout = child.stdout.take().expect("Take stdout from child");
                        let stderr = child.stderr.take().expect("Take stderr from child");

                        let mut stored_child = stored_child.lock().unwrap();

                        // Kill existing child if it exists
                        if let Some(stored_child) = stored_child.as_mut() {
                            stored_child.kill().expect("Kill child");
                        }

                        *stored_child = Some(child);

                        sender
                            .send(AudioMessage::StreamSpawned(stdout, stderr))
                            .unwrap();
                    }
                    Err(err) => *(status.lock().unwrap()) = AudioStatus::Failed(err),
                };
            }
        });
    }

    pub fn clear(&self) {
        *(self.status.lock().unwrap()) = AudioStatus::Idle;
        *(self.selected_app.lock().unwrap()) = None;

        if let Some(child) = self.child.lock().unwrap().as_mut() {
            child.kill().expect("Kill child");
        }

        self.sender.send(AudioMessage::Clear).unwrap();
    }

    pub fn stream(&self) -> AudioStream {
        AudioStream(self.audio_consumer.clone())
    }
}

#[derive(Clone)]
pub struct AudioStream(AudioConsumer);

impl AudioStream {
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

impl Read for AudioStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut consumer = self.0.lock().unwrap();
        let mut read = 0;

        loop {
            if read == buf.len() {
                break;
            }

            read += consumer.read(&mut buf[read..]).unwrap_or_default();
            thread::sleep(Duration::from_millis(1));
        }

        Ok(buf.len())
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

fn spawn_parec(device: String, app: Application) -> Result<Child, String> {
    Command::new("parec")
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
        .map_err(|err| format!("Could not spawn Parec instance: {}", err))
}

fn run_audio_thread(audio: Arc<AudioSystem>) {
    thread::Builder::new()
        .name("audio".to_string())
        .spawn(move || {
            let mut stdout = None;
            let stderr = Arc::new(Mutex::new(None));

            let receiver = audio.receiver.clone();
            let producer = audio.audio_producer.clone();
            let status = audio.status.clone();
            let pulse = audio.pulse.clone();

            // Update applications periodically
            thread::Builder::new()
                .name("pulse-app-polling".to_string())
                .spawn(move || loop {
                    pulse.update_applications();
                    thread::sleep(Duration::from_secs(1));
                })
                .unwrap();

            // Listen for events
            thread::Builder::new()
                .name("audio-events".to_string())
                .spawn({
                    let stderr = stderr.clone();
                    let sender = audio.sender.clone();

                    move || {
                        loop {
                            let event = stderr.try_lock().ok().and_then(|mut f| {
                                f.as_mut()
                                    .and_then(read_from_parec_stderr)
                                    .and_then(ParecEvent::parse)
                            });

                            if let Some(event) = event {
                                let correct_device = audio.pulse.device_name();

                                // Parec stream is no longer valid
                                if event.is_invalid(correct_device) {
                                    sender.send(AudioMessage::Clear).unwrap();
                                }

                                if let ParecEvent::Time(time, latency) = event {
                                    audio.time.store(time);
                                    audio.latency.store(latency);
                                }

                                if let ParecEvent::TimedOut = event {
                                    *(status.lock().unwrap()) =
                                        AudioStatus::Failed("Stream timed out.".to_string());
                                }

                                if let ParecEvent::Connected(_, _) = event {
                                    let application = audio.selected_app.lock().unwrap();

                                    if let Some(app) = &*application {
                                        *(status.lock().unwrap()) =
                                            AudioStatus::Connected(app.to_owned());
                                    }
                                }
                            }

                            thread::sleep(Duration::from_millis(1));
                        }
                    }
                })
                .unwrap();

            loop {
                while let Ok(event) = receiver.try_recv() {
                    match event {
                        AudioMessage::StreamSpawned(new_stdout, new_stderr) => {
                            stdout = Some(new_stdout);
                            *(stderr.lock().unwrap()) = Some(BufReader::new(new_stderr));
                        }
                        AudioMessage::Clear => {
                            stdout = None;
                            *(stderr.lock().unwrap()) = None;
                        }
                    }
                }

                // Read audio into buffer
                if let Some(stdout) = stdout.as_mut() {
                    let mut producer = producer.lock().unwrap();
                    let mut buf = [0; BUFFER_SIZE];

                    let read = stdout.read(&mut buf).unwrap_or_default();
                    producer.push_slice(&buf[..read]);
                }

                thread::sleep(Duration::from_millis(1));
            }
        })
        .unwrap();
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

    /// Returns true if Parec moved or connected to a different stream
    pub fn is_invalid(&self, correct_device: String) -> bool {
        match self {
            Self::Connected(device, _) => !device.contains(&correct_device),
            Self::StreamMoved => false,
            _ => false,
        }
    }
}

fn read_from_parec_stderr(buffer: &mut BufReader<ChildStderr>) -> Option<String> {
    let mut line = String::new();
    let mut c = [0; 1];

    loop {
        match buffer.read_exact(&mut c) {
            Ok(_) => {}
            Err(_) => {
                return None;
            }
        };

        match c {
            [13] | [10] => break,
            [c] => line.push(c as char),
        }
    }

    Some(line)
}
