use crossbeam::atomic::AtomicCell;
use parking_lot::Mutex;

use super::{Sample, SAMPLE_RATE};

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
        let mut buffer = self.buffer.lock();
        let window_size = self.window_size.load();

        buffer.extend_from_slice(buf);

        let overflow = buffer.len() as isize + -(window_size as isize);
        let overflow = overflow.max(0) as usize;

        if overflow > 0 {
            buffer.drain(..overflow);
        }

        let current_value = self.current_value.load();
        let max_value = buffer.iter().fold(0f32, |acc, x| acc.max((*x).abs()));

        if max_value > current_value {
            self.current_value.store(max_value);
            self.samples_since_last_peak.store(0);
        } else {
            let samples_since_last_peak = self.samples_since_last_peak.load() as f32;

            let release = (1. - samples_since_last_peak / Self::SMOOTHING_RELEASE as f32)
                .max(0.)
                .powf(0.8);

            let max_smoothing =
                (1. - Self::MAX_SMOOTHING) + Self::MAX_SMOOTHING * Self::SMOOTHING_BOUNDARY;

            let min_smoothing =
                (1. - Self::MIN_SMOOTHING) + Self::MIN_SMOOTHING * Self::SMOOTHING_BOUNDARY;

            let smoothing = min_smoothing + (max_smoothing - min_smoothing) * release;
            dbg!(release, smoothing);

            self.samples_since_last_peak.fetch_add(buf.len());
            self.current_value.store(current_value * smoothing);
        }
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
