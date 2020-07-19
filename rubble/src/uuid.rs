//! BLE UUIDs (16, 32 or 128 bits).
//!
//! Bluetooth assigns UUIDs to identify services and characteristics. In order to save space, many
//! common UUIDs can be represented and transmitted as 16- or 32-bit aliases instead of the full
//! 128 bits.
//!
//! The shorter UUIDs can be converted to their full 128-bit counterparts by making use of the
//! Bluetooth Base UUID, which is defined as `00000000-0000-1000-8000-00805F9B34FB`.
//!
//! A 16-bit UUID alias can be converted to its 32-bit equivalent by zero-extending it: `0xABCD`
//! becomes `0x0000ABCD`.
//!
//! A 32-bit UUID alias can then be converted to its full 128-bit equivalent by placing it in the
//! first 4 Bytes of the Base UUID. Hence `0x1234ABCD` would become
//! `1234ABCD-0000-1000-8000-00805F9B34FB`.

use crate::{bytes::*, Error};
use core::fmt;

// FIXME this could be more readable
const BASE_UUID: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00, /*-*/ 0x00, 0x00, /*-*/ 0x10, 00, /*-*/ 0x80, 0x00,
    /*-*/ 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB,
];

/// A 16-bit UUID alias.
///
/// Can be converted to its 32- and 128-bit equivalents via `.into()`.
#[derive(PartialEq, Eq, Copy, Clone)]
pub struct Uuid16(pub u16);

/// A 32-bit UUID alias.
///
/// Can be converted to its 128-bit equivalent via `.into()`.
#[derive(PartialEq, Eq, Copy, Clone)]
pub struct Uuid32(pub u32);

/// A full 128-bit UUID.
#[derive(PartialEq, Eq, Copy, Clone)]
pub struct Uuid128([u8; 16]);

impl Uuid128 {
    /// Creates a 128-bit UUID from 16 raw bytes (encoded in big-endian).
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

impl From<Uuid16> for Uuid32 {
    fn from(smol: Uuid16) -> Self {
        Uuid32(smol.0.into())
    }
}

impl From<Uuid16> for Uuid128 {
    fn from(uuid: Uuid16) -> Self {
        Uuid32::from(uuid).into()
    }
}

impl From<Uuid32> for Uuid128 {
    fn from(uuid: Uuid32) -> Self {
        let mut buf = BASE_UUID;
        buf[..4].copy_from_slice(&uuid.0.to_be_bytes());
        Uuid128(buf)
    }
}

impl ToBytes for Uuid16 {
    fn to_bytes(&self, buffer: &mut ByteWriter<'_>) -> Result<(), Error> {
        buffer.write_slice(&self.0.to_le_bytes())
    }
}

impl ToBytes for Uuid32 {
    fn to_bytes(&self, buffer: &mut ByteWriter<'_>) -> Result<(), Error> {
        buffer.write_slice(&self.0.to_le_bytes())
    }
}

impl ToBytes for Uuid128 {
    fn to_bytes(&self, buffer: &mut ByteWriter<'_>) -> Result<(), Error> {
        buffer.write_slice(&self.0)
    }
}

impl FromBytes<'_> for Uuid16 {
    fn from_bytes(bytes: &mut ByteReader<'_>) -> Result<Self, Error> {
        let array = bytes.read_array()?;
        Ok(Uuid16(u16::from_le_bytes(array)))
    }
}

impl FromBytes<'_> for Uuid32 {
    fn from_bytes(bytes: &mut ByteReader<'_>) -> Result<Self, Error> {
        let array = bytes.read_array()?;
        Ok(Uuid32(u32::from_le_bytes(array)))
    }
}

impl FromBytes<'_> for Uuid128 {
    fn from_bytes(bytes: &mut ByteReader<'_>) -> Result<Self, Error> {
        let array = bytes.read_array()?;
        Ok(Uuid128(array))
    }
}

impl fmt::Debug for Uuid16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Uuid16({:04x})", self.0)
    }
}

impl fmt::Debug for Uuid32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Uuid32({:08x})", self.0)
    }
}

impl fmt::Debug for Uuid128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [_0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15] = self.0;
        let a = u32::from_be_bytes([_0, _1, _2, _3]);
        let b = u16::from_be_bytes([_4, _5]);
        let c = u16::from_be_bytes([_6, _7]);
        let d = u16::from_be_bytes([_8, _9]);
        let e = u64::from_be_bytes([0, 0, _10, _11, _12, _13, _14, _15]);
        write!(f, "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", a, b, c, d, e)
    }
}

/// List of the supported UUID types.
#[derive(Debug, Copy, Clone)]
pub enum UuidKind {
    Uuid16,
    Uuid32,
    Uuid128,
}

/// Marker for UUID types.
///
/// This is useful when being generic over the specific type of UUID used. It
/// also brings in the `ToBytes` and `FromBytes` trait bounds that are likely
/// needed as well.
pub trait IsUuid: for<'a> FromBytes<'a> + ToBytes + Copy {
    const KIND: UuidKind;
}

impl IsUuid for Uuid16 {
    const KIND: UuidKind = UuidKind::Uuid16;
}

impl IsUuid for Uuid32 {
    const KIND: UuidKind = UuidKind::Uuid32;
}

impl IsUuid for Uuid128 {
    const KIND: UuidKind = UuidKind::Uuid128;
}
