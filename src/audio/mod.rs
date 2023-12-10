mod analysis;
pub mod pulse;
mod source;
mod system;

use std::sync::Arc;

use parking_lot::Mutex;
use ringbuf::{HeapConsumer, HeapProducer};
pub use source::*;
pub use system::*;

pub type Sample = f32;

pub type AudioProducer = Arc<Mutex<HeapProducer<u8>>>;
pub type AudioConsumer = Arc<Mutex<HeapConsumer<u8>>>;

pub const SAMPLE_RATE: usize = 48000;
pub const SAMPLE_IN_BYTES: usize = 4;

pub const LATENCY_IN_SECONDS: f32 = 0.05;
pub const BUFFER_SIZE: usize =
    (SAMPLE_IN_BYTES * 2) * (SAMPLE_RATE as f32 * LATENCY_IN_SECONDS) as usize;
