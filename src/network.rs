use crate::time::Duration;

mod process;
pub use process::{Process, ProcessId, ProcessLogic};

mod link;
pub use link::Link;

/// Network latency in milliseconds
pub type Latency = Duration;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Bandwidth(u64);

impl Bandwidth {
    pub fn from_megabits_per_second(mbps: u64) -> Self {
        Self(mbps * 1024 * 1024)
    }

    pub fn into_bits_per_second(self) -> u64 {
        self.0
    }
}

pub trait NetworkMessage: 'static + Send {
    fn get_size(&self) -> u64;
}

pub fn get_size_delay(size: u64, bandwidth: Bandwidth) -> Duration {
    // Converts bandwidth bits / microsecond and size to bits
    let micros = (size * 8 * 1000 * 1000) / bandwidth.into_bits_per_second();

    Duration::from_micros(micros)
}
