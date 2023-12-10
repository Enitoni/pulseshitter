use std::{sync::Arc, thread, time::Duration};

use crossbeam::atomic::AtomicCell;
use multiversion::multiversion;
use parking_lot::Mutex;

use crate::interface::TARGET_FPS;

use super::{Sample, SAMPLE_IN_BYTES, SAMPLE_RATE};

/// Measures dBFS of a single channel
pub struct Meter {
    window_size: AtomicCell<usize>,

    current_value: AtomicCell<f32>,

    buffer: Mutex<Vec<f32>>,
}

impl Meter {
    const DEFAULT_WINDOW_SIZE: usize = SAMPLE_RATE / 24;
    const DB_RANGE: f32 = 100.;

    /// The smoothing modifiers. Higher values equals less smoothing.
    const MAX_SMOOTHING: f32 = 0.1 / 2.;
    const MIN_SMOOTHING: f32 = 0.2;

    /// The minimum smoothing boundary
    const SMOOTHING_BOUNDARY: f32 = 0.5;

    pub fn new() -> Self {
        Self {
            window_size: Self::DEFAULT_WINDOW_SIZE.into(),
            current_value: Default::default(),
            buffer: Default::default(),
        }
    }

    pub fn write(&self, buf: &[u8]) {
        let mut buffer = self.buffer.lock();
        let window_size = self.window_size.load();

        let samples = raw_samples_from_bytes(buf);

        buffer.extend_from_slice(&samples);

        let overflow = buffer.len() as isize + -(window_size as isize);
        let overflow = overflow.max(0) as usize;

        if overflow > 0 {
            buffer.drain(..overflow);
        }
    }

    pub fn drain(&self, amount: usize) {
        let mut buffer = self.buffer.lock();

        buffer.extend(vec![0.; amount]);
        buffer.drain(..amount);
    }

    pub fn process(&self) {
        process_multiversioned(self)
    }

    pub fn value(&self) -> f32 {
        self.current_value.load()
    }

    pub fn dbfs(&self) -> f32 {
        self.value().log10() * 20.
    }

    pub fn value_ranged(&self) -> f32 {
        let ranged = (Self::DB_RANGE + self.dbfs()) / Self::DB_RANGE;
        ranged.max(0.).powf(2.)
    }
}

#[multiversion(targets("x86_64+avx2"))]
fn process_multiversioned(meter: &Meter) {
    let buffer = meter.buffer.lock();

    let current_value = meter.current_value.load();
    let max_value = buffer
        .iter()
        .fold(0f32, |acc, x| faster_max(acc, (*x).abs()));

    let difference = (current_value - max_value).abs();

    if max_value > current_value {
        meter.current_value.store(max_value);
    } else {
        let max_smoothing =
            (1. - Meter::MAX_SMOOTHING) + Meter::MAX_SMOOTHING * Meter::SMOOTHING_BOUNDARY;

        let min_smoothing =
            (1. - Meter::MIN_SMOOTHING) + Meter::MIN_SMOOTHING * Meter::SMOOTHING_BOUNDARY;

        let flipped_difference = (-difference) + 1.;
        let smoothing_factor = flipped_difference * max_value.powf(0.1);

        let smoothing = min_smoothing + (max_smoothing - min_smoothing) * smoothing_factor;
        let result = current_value * smoothing;

        meter.current_value.store(result);
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

    pub fn write(&self, buf: &[u8]) {
        let samples = buf.chunks_exact(SAMPLE_IN_BYTES * 2);

        let mut left = vec![];
        let mut right = vec![];

        for chunk in samples {
            left.extend_from_slice(&chunk[..SAMPLE_IN_BYTES]);
            right.extend_from_slice(&chunk[SAMPLE_IN_BYTES..]);
        }

        self.left.write(&left);
        self.right.write(&right);
    }

    pub fn process(&self) {
        self.left.process();
        self.right.process();
    }

    pub fn drain(&self, amount: usize) {
        self.left.drain(amount);
        self.right.drain(amount);
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

pub fn spawn_analysis_thread(meter: Arc<StereoMeter>) {
    let run = move || {
        let tick_rate = 1. / TARGET_FPS as f32;
        let samples_to_drain = (SAMPLE_RATE as f32 * tick_rate) as usize;

        loop {
            let meter = meter.clone();
            meter.process();
            meter.drain(samples_to_drain);

            thread::sleep(Duration::from_secs_f32(tick_rate));
        }
    };

    thread::Builder::new()
        .name("audio-analysis".to_string())
        .spawn(run)
        .unwrap();
}
