use crossbeam::atomic::AtomicCell;
use parking_lot::Mutex;

use super::{Sample, SAMPLE_RATE};

/// Measures dBFS of a single channel
pub struct Meter {
    window_size: AtomicCell<usize>,
    current_value: AtomicCell<f32>,
    buffer: Mutex<Vec<f32>>,
}

impl Meter {
    const DEFAULT_WINDOW_SIZE: usize = SAMPLE_RATE / 4;
    const DB_RANGE: f32 = 40.;

    pub fn new() -> Self {
        Self {
            window_size: Self::DEFAULT_WINDOW_SIZE.into(),
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
            buffer.drain(overflow..);
        }

        let current_value = self.current_value.load();
        let max_value = buffer.iter().fold(0f32, |acc, x| acc.max((*x).abs()));

        if max_value > current_value {
            self.current_value.store(max_value);
        } else {
            self.current_value.store(current_value * 0.995);
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
