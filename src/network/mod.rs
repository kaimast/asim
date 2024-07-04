/// Utilities to simulate a network
///
/// There are two important primitives in this module
///     * Nodes represent individual nodes in the network
///     * Links are connection between the nodes
use crate::time::Duration;

mod node;
pub use node::{DummyNodeCallback, DummyNodeData, Node, NodeCallback, NodeData};

mod link;
pub use link::{DummyLinkCallback, Link, LinkCallback};

mod object;
pub use object::{Object, ObjectId};

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

pub trait NetworkMessage: Clone + 'static {
    fn get_size(&self) -> u64;
}

#[derive(Default, Clone)]
pub struct DummyNetworkMessage {}

impl NetworkMessage for DummyNetworkMessage {
    fn get_size(&self) -> u64 {
        0
    }
}

pub fn get_size_delay(size: u64, bandwidth: Bandwidth) -> Duration {
    // Converts bandwidth bits / microsecond and size to bits
    let micros = (size * 8 * 1000 * 1000) / bandwidth.into_bits_per_second();

    Duration::from_micros(micros)
}
