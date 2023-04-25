use super::pulse::{Device, Source};

use super::{AudioError, AudioStatus, AudioSystem};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use regex::Regex;
use std::{
    io::{BufReader, Read},
    process::{Child, ChildStderr, Command, Stdio},
    sync::Arc,
    thread,
    time::Duration,
};

pub type Stderr = Arc<Mutex<Option<BufReader<ChildStderr>>>>;

// This is an estimation used to synchronize the meter
pub const PAREC_SAMPLE_RATE: usize = 256;

lazy_static! {
    static ref CONNECTED_REGEX: Regex =
        Regex::new(r"Connected to device (.*?) \(index: (\d*), suspended: no\)").unwrap();
    static ref TIME_REGEX: Regex =
        Regex::new(r"Time: (\d+\.\d+) sec; Latency: (\d+) usec.").unwrap();
}

#[derive(Debug)]
pub enum ParecEvent {
    TimedOut,
    /// Device name, index
    Connected(String, u32),
    /// Time, latency
    Time(f32, u32),
    StreamMoved,
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
    pub fn is_invalid(&self, correct_device: &Device) -> bool {
        match self {
            Self::Connected(device, _) => !device.contains(&correct_device.id()),
            Self::StreamMoved => true,
            _ => false,
        }
    }
}

pub fn spawn_parec(device: Device, source: Source) -> Result<Child, AudioError> {
    Command::new("parec")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--verbose")
        .arg("--device")
        .arg(format!("{}.monitor", device.id()))
        .arg("--monitor-stream")
        .arg(source.input_index().to_string())
        .arg("--format=float32le")
        .arg("--rate=48000")
        .arg("--channels=2")
        .arg("--latency=1")
        .arg("--process-time=1")
        .spawn()
        .map_err(|_| AudioError::ParecMissing)
}

pub fn read_from_parec_stderr(buffer: &mut BufReader<ChildStderr>) -> Option<String> {
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

/// Handle Parec's events and make corrections when a stream becomes invalid
pub fn spawn_event_thread(audio: Arc<AudioSystem>, stderr: Stderr) {
    let run = move || {
        let status = audio.status.clone();
        let device = audio.pulse.current_device();

        loop {
            let event = stderr.try_lock().and_then(|mut f| {
                f.as_mut()
                    .and_then(read_from_parec_stderr)
                    .and_then(ParecEvent::parse)
            });

            let handle_time = |time, latency| {
                audio.time.store(time);
                audio.latency.store(latency);
            };

            let handle_timed_out = || {
                *(status.lock().unwrap()) = AudioStatus::Failed(AudioError::TimedOut);
            };

            let handle_connected = || {
                let source = audio.pulse.current_source();

                if let Some(source) = source {
                    *(status.lock().unwrap()) = AudioStatus::Connected(source);
                }
            };

            if let Some(event) = event {
                if event.is_invalid(&device) {
                    audio.invalid();
                }

                match event {
                    ParecEvent::Time(time, latency) => handle_time(time, latency),
                    ParecEvent::TimedOut => handle_timed_out(),
                    ParecEvent::Connected(_, _) => handle_connected(),
                    _ => {}
                }
            }

            thread::sleep(Duration::from_millis(1));
        }
    };

    thread::Builder::new()
        .name("audio-events".to_string())
        .spawn(run)
        .unwrap();
}
