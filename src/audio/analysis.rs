use std::{sync::Arc, thread, time::Duration};

use crossbeam::atomic::AtomicCell;
use multiversion::multiversion;
use parking_lot::Mutex;

use super::{parec::PAREC_SAMPLE_RATE, AudioSystem, Sample, SAMPLE_IN_BYTES, SAMPLE_RATE};

/// Measures dBFS of a single channel
pub struct Meter {
    window_size: AtomicCell<usize>,

    current_value: AtomicCell<f32>,
    samples_since_last_peak: AtomicCell<usize>,

    buffer: Mutex<Vec<f32>>,
}

impl Meter {
    const DEFAULT_WINDOW_SIZE: usize = SAMPLE_RATE / 16;
    const DB_RANGE: f32 = 70.;

    /// The smoothing modifiers. Higher values equals less smoothing.
    const MAX_SMOOTHING: f32 = 0.02;
    const MIN_SMOOTHING: f32 = 0.2;

    /// The minimum smoothing boundary
    const SMOOTHING_BOUNDARY: f32 = 0.99;

    // How quickly in samples should the smoothing taper off after a peak
    const SMOOTHING_RELEASE: usize = SAMPLE_RATE * 10;

    pub fn new() -> Self {
        Self {
            window_size: Self::DEFAULT_WINDOW_SIZE.into(),
            samples_since_last_peak: Default::default(),
            current_value: Default::default(),
            buffer: Default::default(),
        }
    }

    pub fn process(&self, buf: &[Sample]) {
        process_multiversioned(self, buf)
    }

    pub fn value(&self) -> f32 {
        self.current_value.load()
    }

    pub fn dbfs(&self) -> f32 {
        self.value().log10() * 20.
    }

    pub fn value_ranged(&self) -> f32 {
        (Self::DB_RANGE + self.dbfs()) / Self::DB_RANGE
    }
}

#[multiversion(targets("x86_64+avx2"))]
fn process_multiversioned(meter: &Meter, buf: &[Sample]) {
    let mut buffer = meter.buffer.lock();
    let window_size = meter.window_size.load();

    buffer.extend_from_slice(buf);

    let overflow = buffer.len() as isize + -(window_size as isize);
    let overflow = overflow.max(0) as usize;

    if overflow > 0 {
        buffer.drain(..overflow);
    }

    let current_value = meter.current_value.load();
    let max_value = buffer
        .iter()
        .fold(0f32, |acc, x| faster_max(acc, (*x).abs()));

    if max_value > current_value {
        meter.current_value.store(max_value);
        meter.samples_since_last_peak.store(0);
    } else {
        let samples_since_last_peak = meter.samples_since_last_peak.load() as f32;

        let release = (1. - samples_since_last_peak / Meter::SMOOTHING_RELEASE as f32)
            .max(0.)
            .powf(0.8);

        let max_smoothing =
            (1. - Meter::MAX_SMOOTHING) + Meter::MAX_SMOOTHING * Meter::SMOOTHING_BOUNDARY;

        let min_smoothing =
            (1. - Meter::MIN_SMOOTHING) + Meter::MIN_SMOOTHING * Meter::SMOOTHING_BOUNDARY;

        let smoothing = min_smoothing + (max_smoothing - min_smoothing) * release;

        meter.samples_since_last_peak.fetch_add(buf.len());
        meter.current_value.store(current_value * smoothing);
    }
}

pub struct StereoMeter {
    left: Meter,
    right: Meter,
}

impl StereoMeter {
    pub fn new() -> Self {
        Self {
            left: Meter::new(),
            right: Meter::new(),
        }
    }

    pub fn process(&self, buf: &[Sample]) {
        let left: Vec<_> = buf.iter().step_by(2).copied().collect();
        let right: Vec<_> = buf.iter().skip(1).step_by(2).copied().collect();

        self.left.process(&left);
        self.right.process(&right);
    }

    pub fn value(&self) -> (f32, f32) {
        (self.left.value(), self.right.value())
    }

    pub fn dbfs(&self) -> (f32, f32) {
        (self.left.dbfs(), self.right.dbfs())
    }

    pub fn value_ranged(&self) -> (f32, f32) {
        (self.left.value_ranged(), self.right.value_ranged())
    }
}

impl Default for StereoMeter {
    fn default() -> Self {
        Self::new()
    }
}

fn faster_max(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
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

pub fn spawn_analysis_thread(audio: Arc<AudioSystem>) {
    let run = move || {
        let tick_rate = 1. / SAMPLE_RATE as f32;

        loop {
            let mut meter_buffer = audio.meter_buffer.lock();

            let meter = audio.meter.clone();
            let buffer_len = meter_buffer.len();

            // Ensure the analyser does not try to read misaligned floats
            let remainder = buffer_len % SAMPLE_IN_BYTES * 2;
            let safe_range = ..buffer_len - remainder;

            let mut samples = raw_samples_from_bytes(&meter_buffer[safe_range]);

            // Prevent the meter freezing when there is no output
            samples.resize(PAREC_SAMPLE_RATE, 0.);

            meter.process(&samples);
            meter_buffer.drain(safe_range);

            // Drop mutex immediately to avoid slowdown
            drop(meter_buffer);

            thread::sleep(Duration::from_secs_f32(tick_rate));
        }
    };

    thread::Builder::new()
        .name("audio-analysis".to_string())
        .spawn(run)
        .unwrap();
}
