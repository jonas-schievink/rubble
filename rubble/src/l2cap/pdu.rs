use crate::bytes::{self, *};
use crate::Error;
use core::fmt;
use zerocopy::{FromBytes, Unaligned};

/// An L2CAP channel identifier (CID).
///
/// Channels are basically like TCP ports. A `Protocol` can listen on a channel and is connected to
/// a channel on the other device to which all responses are addressed.
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
#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct Channel(u16);

impl Channel {
    /// The null channel identifier. Must not be used as a destination endpoint.
    pub const NULL: Self = Channel(0x0000);

    /// The channel used by the Attribute Protocol (ATT).
    pub const ATT: Self = Channel(0x0004);

    /// LE L2CAP signaling channel (connectionless).
    pub const LE_SIGNALING: Self = Channel(0x0005);

    /// LE Security Manager channel.
    pub const LE_SECURITY_MANAGER: Self = Channel(0x0006);

    /// Returns the channel identifier (CID) as a raw `u16`.
    pub fn as_raw(&self) -> u16 {
        self.0
    }

    /// Returns whether this channel is connection-oriented.
    ///
    /// L2CAP PDUs addressed to connection-oriented channels are called *B-frames* if the channel is
    /// in "Basic Mode", and can be either *S-frames* or *I-frames* if the channel is in
    /// retransmission/flow control/streaming modes.
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

impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#06X}", self.0)
    }
}

impl bytes::FromBytes<'_> for Channel {
    fn from_bytes(bytes: &mut ByteReader<'_>) -> Result<Self, Error> {
        Ok(Channel(bytes.read_u16_le()?))
    }
}

impl ToBytes for Channel {
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        writer.write_u16_le(self.0)
    }
}

/// Header used by *all* L2CAP PDUs.
#[derive(Debug, FromBytes, Unaligned)]
#[repr(C, packed)]
struct Header {
    /// Length of the payload following the length and channel fields (after reassembly).
    length: u16,
    /// Destination endpoint of the PDU.
    channel: u16,
}

impl Header {
    /// The size of an L2CAP message header in Bytes.
    const SIZE: u8 = 2 + 2;
}

impl<'a> bytes::FromBytes<'a> for Header {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        let length = bytes.read_u16_le()?;
        let channel = Channel::from_bytes(bytes)?;
        Ok(Self {
            length,
            channel: channel.as_raw(),
        })
    }
}

impl ToBytes for Header {
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        writer.write_u16_le(self.length)?;
        writer.write_u16_le(self.channel)?;
        Ok(())
    }
}

pub struct RawPdu<T: AsRef<[u8]>>(T);

impl<T: AsRef<[u8]>> RawPdu<T> {
    pub fn new(buf: T) -> Option<Self> {
        if buf.as_ref().len() < 4 {
            None
        } else {
            Some(RawPdu(buf))
        }
    }

    pub fn header(&self) -> Header {
        *self.0.as_ref()[..4].decode_as().unwrap()
    }
}
