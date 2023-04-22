use self::analysis::StereoMeter;
use self::parec::{spawn_event_thread, spawn_parec, Stderr, PAREC_SAMPLE_RATE};
use crate::pulse::{Application, PulseAudio};
use crossbeam::atomic::AtomicCell;
use crossbeam::channel::{unbounded, Receiver, Sender};
use ringbuf::{HeapConsumer, HeapProducer, HeapRb};
use songbird::input::reader::MediaSource;
use songbird::input::{Codec, Container, Input, Reader};
use std::fmt::Display;
use std::io::{BufReader, Read, Seek};
use std::process::{Child, ChildStderr, ChildStdout};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

mod analysis;
mod parec;

pub type AudioProducer = Arc<Mutex<HeapProducer<u8>>>;
pub type AudioConsumer = Arc<Mutex<HeapConsumer<u8>>>;
pub type CurrentAudioStatus = Arc<Mutex<AudioStatus>>;
pub type SelectedApp = Arc<Mutex<Option<Application>>>;

pub type AudioTime = Arc<AtomicCell<f32>>;
pub type AudioLatency = Arc<AtomicCell<u32>>;

pub type Sample = f32;

pub const SAMPLE_RATE: usize = 48000;
pub const SAMPLE_IN_BYTES: usize = 4;

const BUFFER_SIZE: usize = (SAMPLE_IN_BYTES * 2) * SAMPLE_RATE;

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

    meter_buffer: Arc<parking_lot::Mutex<Vec<u8>>>,
    meter: Arc<StereoMeter>,
}

/// Helper struct for passing around audio related data
#[derive(Clone)]
pub struct AudioContext {
    pub meter: Arc<StereoMeter>,
    pub pulse: Arc<PulseAudio>,
    pub selected_app: SelectedApp,
    pub status: CurrentAudioStatus,
    pub latency: AudioLatency,
    pub time: AudioTime,
}

#[derive(Default)]
pub enum AudioStatus {
    #[default]
    Idle,
    Connecting(Application),
    Connected(Application),
    Searching(Application),
    Failed(AudioError),
}

#[derive(Debug, Clone, Copy)]
pub enum AudioError {
    TimedOut,
    ParecMissing,
}

impl Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            AudioError::TimedOut => "Stream timed out",
            AudioError::ParecMissing => "Cannot spawn Parec",
        };

        write!(f, "{}", message)
    }
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

            meter: StereoMeter::new().into(),
            meter_buffer: Default::default(),
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
        self.audio_consumer.lock().unwrap().clear();

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

    pub fn context(&self) -> AudioContext {
        AudioContext {
            meter: self.meter.clone(),
            pulse: self.pulse.clone(),
            selected_app: self.selected_app.clone(),
            status: self.status.clone(),
            latency: self.latency.clone(),
            time: self.time.clone(),
        }
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

fn run_audio_thread(audio: Arc<AudioSystem>) {
    thread::Builder::new()
        .name("audio".to_string())
        .spawn(move || {
            let mut stdout = None;
            let stderr = Stderr::default();

            let receiver = audio.receiver.clone();
            let producer = audio.audio_producer.clone();
            let pulse = audio.pulse.clone();

            let meter_buffer = audio.meter_buffer.clone();
            let meter = audio.meter.clone();

            let time_to_wait = 1. / SAMPLE_RATE as f32;

            spawn_event_thread(audio, stderr.clone());

            // Update applications periodically
            thread::Builder::new()
                .name("pulse-app-polling".to_string())
                .spawn(move || loop {
                    pulse.update_applications();
                    thread::sleep(Duration::from_secs(1));
                })
                .unwrap();

            thread::Builder::new()
                .name("audio-analysis".to_string())
                .spawn({
                    let meter_buffer = meter_buffer.clone();

                    move || loop {
                        let mut meter_buffer = meter_buffer.lock();
                        let buffer_len = meter_buffer.len();

                        // Ensure the analyser does not try to read misaligned floats
                        let remainder = buffer_len % SAMPLE_IN_BYTES * 2;
                        let safe_range = ..buffer_len - remainder;

                        let mut samples = raw_samples_from_bytes(&meter_buffer[safe_range]);

                        // Prevent freezing when there is no output
                        samples.resize(PAREC_SAMPLE_RATE, 0.);

                        meter.process(&samples);
                        meter_buffer.drain(safe_range);

                        drop(meter_buffer);
                        thread::sleep(Duration::from_secs_f32(time_to_wait));
                    }
                })
                .unwrap();

            loop {
                while let Ok(event) = receiver.try_recv() {
                    match event {
                        AudioMessage::StreamSpawned(new_stdout, new_stderr) => {
                            stdout = Some(new_stdout);
                            *(stderr.lock()) = Some(BufReader::new(new_stderr));
                        }
                        AudioMessage::Clear => {
                            stdout = None;
                            *(stderr.lock()) = None;
                        }
                    }
                }

                // Read audio into buffers
                if let Some(stdout) = stdout.as_mut() {
                    let mut buf = [0; BUFFER_SIZE];

                    let bytes_read = stdout.read(&mut buf).unwrap_or_default();
                    let new_bytes = &buf[..bytes_read];

                    producer.lock().unwrap().push_slice(new_bytes);
                    meter_buffer.lock().extend_from_slice(new_bytes);
                }

                thread::sleep(Duration::from_secs_f32(time_to_wait));
            }
        })
        .unwrap();
}

/// Converts a slice of bytes into a vec of [Sample].
pub fn raw_samples_from_bytes(bytes: &[u8]) -> Vec<Sample> {
    bytes
        .chunks_exact(SAMPLE_IN_BYTES)
        .map(|b| {
            let arr: [u8; SAMPLE_IN_BYTES] = [b[0], b[1], b[2], b[3]];
            Sample::from_le_bytes(arr)
        })
        .collect()
}
