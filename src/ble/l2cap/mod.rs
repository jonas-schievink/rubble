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
//! Establishing new connection-oriented channels, as well as transferring data over the
//! connectionless channel (`0x0002`) makes use of *Protocol/Service Multiplexers* (PSMs), which are
//! numbers identifying the protocol or service to use. These numbers are either defined by the
//! Bluetooth SIG or allocated dynamically for use with the Service Discovery Protocol (SDP). The
//! preallocated numbers are hosted online [here][l2c].
//!
//! [`Channel`]: struct.Channel.html
//! [l2c]: https://www.bluetooth.com/specifications/assigned-numbers/logical-link-control

use {
    crate::ble::{
        att::AttributeServer,
        bytes::*,
        link::{
            data,
            queue::{Consume, Producer},
        },
        utils::HexSlice,
        Error,
    },
    byteorder::LittleEndian,
    core::fmt,
    log::warn,
};

/// An L2CAP channel identifier (CID).
///
/// Channels are basically like TCP ports. A `Protocol` can listen on a channel and is connected to
/// a channel on the other device.
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#06X}", self.0)
    }
}

impl FromBytes<'_> for Channel {
    fn from_bytes(bytes: &mut &[u8]) -> Result<Self, Error> {
        Ok(Channel(bytes.read_u16::<LittleEndian>()?))
    }
}

impl ToBytes for Channel {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u16::<LittleEndian>(self.0)
    }
}

/// Trait for L2CAP channel mappers that provide access to the protocol or service behind a CID.
pub trait ChannelMapper {
    /// Look up what's connected to `channel` (eg. the `Protocol` to which to forward).
    fn lookup(&mut self, channel: Channel) -> Option<ChannelData>;
}

/// Data associated with a connected L2CAP channel.
pub struct ChannelData<'a> {
    /// Channel to which responses should be addressed.
    ///
    /// For fixed, predefined channels, this is always the same value for both devices, but
    /// dynamically allocated channels can have different CIDs on both devices.
    pub response_channel: Channel,

    /// The protocol listening on this channel.
    pub protocol: &'a mut Protocol,
}

/// A fixed BLE channel map that provides only the required channel endpoints and does not allow
/// dynamic channels.
///
/// The channels are mapped as follows (no other channels are supported):
///
/// * `0x0004`: Attribute protocol (ATT).
/// * `0x0005`: LE L2CAP signaling channel.
/// * `0x0006`: LE Security Manager protocol.
pub struct BleChannelMap {
    att: AttributeServer,
}

impl BleChannelMap {
    pub fn new() -> Self {
        Self {
            att: AttributeServer::empty(),
        }
    }
}

impl ChannelMapper for BleChannelMap {
    fn lookup(&mut self, channel: Channel) -> Option<ChannelData> {
        match channel {
            Channel::ATT => Some(ChannelData {
                response_channel: Channel::ATT,
                protocol: &mut self.att,
            }),
            // FIXME implement the rest
            _ => None,
        }
    }
}

/// Trait for protocols that sit on top of L2CAP.
///
/// A `Protocol` can be connected to an L2CAP channel.
pub trait Protocol {
    /// Process a message sent to the protocol.
    ///
    /// The message is reassembled by L2CAP already.
    fn process_message(&mut self, message: &[u8], responder: L2CAPResponder) -> Consume<()>;
}

struct Message<'a> {
    /// Length of the payload following the length and channel fields.
    length: u16,
    channel: Channel,
    payload: &'a [u8],
}

impl<'a> FromBytes<'a> for Message<'a> {
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error> {
        let length = bytes.read_u16::<LittleEndian>()?;
        let channel = Channel::from_bytes(bytes)?;
        assert_eq!(
            length as usize,
            bytes.len(),
            "L2CAP reassembly not yet implemented"
        );

        let payload = bytes.read_slice(usize::from(length))?;
        Ok(Self {
            length,
            channel,
            payload,
        })
    }
}

impl ToBytes for Message<'_> {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u16::<LittleEndian>(self.length)?;
        writer.write_u16::<LittleEndian>(self.channel.as_raw())?;
        writer.write_slice(self.payload)?;
        Ok(())
    }
}

/// L2CAP channel manager and responder.
pub struct L2CAPState<M: ChannelMapper> {
    mapper: M,
}

impl<M: ChannelMapper> L2CAPState<M> {
    pub fn new(mapper: M) -> Self {
        Self { mapper }
    }

    /// Process the start of a new L2CAP message (or a complete, unfragmented message).
    pub fn process_start(&mut self, mut message: &[u8], tx: &mut Producer) -> Consume<()> {
        let msg = match Message::from_bytes(&mut message) {
            Ok(msg) => msg,
            Err(e) => return Consume::always(Err(e)),
        };
        if let Some(chdata) = self.mapper.lookup(msg.channel) {
            chdata.protocol.process_message(
                msg.payload,
                L2CAPResponder {
                    tx,
                    channel: chdata.response_channel,
                },
            )
        } else {
            warn!(
                "ignoring message sent to unconnected channel {:?}: {:?}",
                msg.channel,
                HexSlice(msg.payload)
            );
            Consume::always(Ok(()))
        }
    }

    /// Process continuation of an L2CAP message.
    pub fn process_cont(&mut self, _data: &[u8], _tx: &mut Producer) -> Consume<()> {
        unimplemented!("reassembly")
    }
}

pub struct L2CAPResponder<'a> {
    /// Data PDU channel.
    tx: &'a mut Producer,

    /// Channel to which the response will be addressed.
    channel: Channel,
}

impl<'a> L2CAPResponder<'a> {
    /// Enqueues an L2CAP message to be sent over the data connection.
    ///
    /// This will fail if there's not enough space left in the PDU queue.
    pub fn respond(&mut self, payload: &[u8]) -> Result<(), Error> {
        // FIXME automatic fragmentation is not implemented

        // Build L2CAP message
        assert!(payload.len() < usize::from(u16::max_value()));
        let message = Message {
            length: payload.len() as u16,
            channel: self.channel,
            payload,
        };
        self.tx.produce_pdu(data::Pdu::DataStart { message })
    }
}
