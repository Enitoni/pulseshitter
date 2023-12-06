use self::analysis::{raw_samples_from_bytes, spawn_analysis_thread, StereoMeter};
use self::parec::{spawn_event_thread, spawn_parec, Stderr};
use self::pulse_old::{spawn_pulse_thread, Source};
use crossbeam::atomic::AtomicCell;
use crossbeam::channel::{unbounded, Receiver, Sender};
use pulse_old::PulseAudio;
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
mod pulse;
pub mod pulse_old;

pub type AudioProducer = Arc<Mutex<HeapProducer<u8>>>;
pub type AudioConsumer = Arc<Mutex<HeapConsumer<u8>>>;
pub type CurrentAudioStatus = Arc<Mutex<AudioStatus>>;

pub type AudioTime = Arc<AtomicCell<f32>>;
pub type AudioLatency = Arc<AtomicCell<u32>>;

pub type Sample = f32;

pub const SAMPLE_RATE: usize = 48000;
pub const SAMPLE_IN_BYTES: usize = 4;

const BUFFER_SIZE: usize = (SAMPLE_IN_BYTES * 2) * 2048;

/// Keeps track of the selected application and provides a reader to discord
pub struct AudioSystem {
    pub status: CurrentAudioStatus,
    pub latency: AudioLatency,
    pub time: AudioTime,

    pulse: Arc<PulseAudio>,
    child: Arc<Mutex<Option<Child>>>,

    sender: Sender<AudioMessage>,
    receiver: Receiver<AudioMessage>,

    audio_producer: AudioProducer,
    audio_consumer: AudioConsumer,

    meter: Arc<StereoMeter>,
}

/// Helper struct for passing around audio related data
#[derive(Clone)]
pub struct AudioContext {
    pub meter: Arc<StereoMeter>,
    pub pulse: Arc<PulseAudio>,
    pub status: CurrentAudioStatus,
    pub latency: AudioLatency,
    pub time: AudioTime,
}

#[derive(Default)]
pub enum AudioStatus {
    #[default]
    Idle,
    Connecting(Source),
    Connected(Source),
    Searching,
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
    pub fn new() -> Self {
        let (sender, receiver) = unbounded();

        let (audio_producer, audio_consumer) = HeapRb::new(BUFFER_SIZE).split();

        Self {
            pulse: PulseAudio::new().into(),
            child: Default::default(),
            status: Default::default(),

            latency: Default::default(),
            time: Default::default(),

            sender,
            receiver,

            audio_producer: Mutex::from(audio_producer).into(),
            audio_consumer: Mutex::from(audio_consumer).into(),

            meter: StereoMeter::new().into(),
        }
    }

    pub fn set_source(&self, source: Source) {
        self.pulse.set_current_source(source.clone());
        self.pulse.set_selected_source(source.clone());
        self.audio_consumer.lock().unwrap().clear();

        *(self.status.lock().unwrap()) = AudioStatus::Connecting(source.clone());

        match spawn_parec(self.pulse.current_device(), source) {
            Ok(mut child) => {
                let stdout = child.stdout.take().expect("Take stdout from child");
                let stderr = child.stderr.take().expect("Take stderr from child");

                let mut stored_child = self.child.lock().unwrap();

                // Kill existing child if it exists
                if let Some(stored_child) = stored_child.as_mut() {
                    stored_child.kill().expect("Kill child");
                    stored_child.wait().expect("Child killed properly");
                }

                *stored_child = Some(child);

                self.sender
                    .send(AudioMessage::StreamSpawned(stdout, stderr))
                    .unwrap();
            }
            Err(err) => *(self.status.lock().unwrap()) = AudioStatus::Failed(err),
        };
    }

    pub fn clear(&self) {
        self.pulse.clear();
        self.audio_consumer.lock().unwrap().clear();

        *(self.status.lock().unwrap()) = AudioStatus::Idle;

        if let Some(mut child) = self.child.lock().unwrap().take() {
            child.kill().expect("Kill child");
            child.wait().expect("Child killed properly");
        }

        self.sender.send(AudioMessage::Clear).unwrap();
    }

    fn invalid(&self) {
        *(self.status.lock().unwrap()) = AudioStatus::Searching;

        self.sender.send(AudioMessage::Clear).unwrap();
        self.pulse.invalid();
    }

    pub fn stream(&self) -> AudioStream {
        AudioStream(self.audio_consumer.clone())
    }

    pub fn context(&self) -> AudioContext {
        AudioContext {
            meter: self.meter.clone(),
            pulse: self.pulse.clone(),
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
        // Clear the stream to minimize latency
        self.0.lock().unwrap().clear();

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

        let stereo = SAMPLE_IN_BYTES * 2;
        let safe_length = buf.len() / stereo * stereo;

        consumer.read(&mut buf[..safe_length]).unwrap_or_default();

        Ok(safe_length)
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

fn normalize_volume(bytes: &[u8], incoming_volume: f32) -> Vec<u8> {
    let reciprocal = 1. / incoming_volume;
    let db_loudness = 10. * reciprocal.log(3.);
    let signal_factor = 10f32.powf(db_loudness / 20.);

    raw_samples_from_bytes(bytes)
        .into_iter()
        .map(|s| s * signal_factor)
        .flat_map(|s| s.to_le_bytes())
        .collect()
}

pub fn spawn_audio_thread(audio: Arc<AudioSystem>) {
    let run = move || {
        let mut stdout = None;
        let stderr = Stderr::default();

        let receiver = audio.receiver.clone();
        let producer = audio.audio_producer.clone();

        let time_to_wait = 1. / SAMPLE_RATE as f32;

        spawn_event_thread(audio.clone(), stderr.clone());
        spawn_analysis_thread(audio.clone());
        spawn_pulse_thread(audio.clone());

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

                let current_source_volume = audio
                    .pulse
                    .current_source()
                    .map(|s| s.volume())
                    .unwrap_or(1.);

                let bytes_read = stdout.read(&mut buf).unwrap_or_default();
                let normalized_bytes = normalize_volume(&buf[..bytes_read], current_source_volume);

                producer.lock().unwrap().push_slice(&normalized_bytes);
                audio.meter.write(&normalized_bytes);
            } else {
                audio.meter.write(&[0; SAMPLE_IN_BYTES * 4])
            }

            thread::sleep(Duration::from_secs_f32(time_to_wait));
        }
    };

    thread::Builder::new()
        .name("audio".to_string())
        .spawn(run)
        .unwrap();
}
