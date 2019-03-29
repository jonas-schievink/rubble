//! The Logical Link Control and Adaptation Protocol (L2CAP).
//!
//! Note that LE and Classic Bluetooth differ quite a bit on this layer, even though they're
//! supposed to share L2CAP. We're only implementing the LE bits.
//!
//! L2CAP provides "channels" to the upper layers that are mapped to the physical transport below
//! the L2CAP layer (the LE Link Layer or whatever Classic Bluetooth does). A channel is identified
//! by a 16-bit ID (also see [`Channel`]), a few of which are reserved.
//!
//! A minimal implementation for Classic Bluetooth must support the L2CAP signaling channel
//! (`0x0001`). A minimal implementation for BLE has to support the L2CAP LE signaling channel
//! (`0x0005`), the Attribute Protocol channel (`0x0004`), and the LE Security Manager channel
//! (`0x0006`).
//!
//! [`Channel`]: struct.Channel.html

/// An L2CAP channel identifier (CID).
///
/// A number of channel identifiers are reserved for predefined functions:
///
/// * `0x0000`: The null identifier. Must never be used as a destination endpoint.
/// * `0x0001`: L2CAP signaling channel (Classic Bluetooth only).
/// * `0x0002`: Connectionless channel (Classic Bluetooth only).
/// * `0x0003`: AMP manager (not relevant for Classic and LE Bluetooth).
/// * `0x0004`: Attribute protocol (ATT). BLE only.
/// * `0x0005`: LE L2CAP signaling channel.
/// * `0x0006`: LE Security Manager protocol.
/// * `0x0007`: Classic Bluetooth Security Manager protocol.
/// * `0x0008`-`0x003E`: Reserved.
/// * `0x003F`: AMP test manager (not relevant for Classic and LE Bluetooth).
///
/// For BLE, channels `0x0040`-`0x007F` are dynamically allocated, while `0x0080` and beyond are
/// reserved and should not be used (as of *Bluetooth 4.2*).
///
/// For classic Bluetooth, all channels `0x0040`-`0xFFFF` are available for dynamic allocation.
#[derive(PartialEq, Eq, Copy, Clone)]
pub struct Channel(u16);

impl Channel {
    pub const NULL: Self = Channel(0x0000);

    /// Returns whether this channel is connection-oriented.
    ///
    /// L2CAP PDUs addressed to connection-oriented channels are called *B-frames*.
    pub fn is_connection_oriented(&self) -> bool {
        !self.is_connectionless()
    }

    /// Returns whether this channel is connectionless.
    ///
    /// L2CAP PDUs addressed to connectionless channels are called *G-frames*.
    pub fn is_connectionless(&self) -> bool {
        match self.0 {
            0x0002 | 0x0001 | 0x0005 => true,
            _ => false,
        }
    }
}
