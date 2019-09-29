//! Stack configuration trait.

use crate::{link::Transmitter, time::Timer};

// TODO: Use associated type defaults in the trait once stable

/// Trait for Rubble stack configurations.
///
/// This trait defines a number of types to be used throughout the layers of the BLE stack, which
/// define capabilities, data structures, data, and hardware interface types to be used.
///
/// Every application must define a type implementing this trait and supply it to the stack.
pub trait Config {
    /// A timesource with microsecond resolution.
    type Timer: Timer;

    /// The BLE packet transmitter.
    type Transmitter: Transmitter;
}
