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
        att::{AttributeServer, Attributes, NoAttributes},
        bytes::*,
        link::{
            data::Llid,
            queue::{Consume, Producer},
        },
        security_manager::{NoSecurity, SecurityLevel, SecurityManager},
        utils::HexSlice,
        Error,
    },
    core::fmt,
    log::{debug, warn},
};

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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#06X}", self.0)
    }
}

impl FromBytes<'_> for Channel {
    fn from_bytes(bytes: &mut ByteReader) -> Result<Self, Error> {
        Ok(Channel(bytes.read_u16_le()?))
    }
}

impl ToBytes for Channel {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u16_le(self.0)
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
    response_channel: Channel,

    /// The protocol listening on this channel.
    protocol: &'a mut ProtocolObj,

    rsp_pdu: u8,
}

impl<'a> ChannelData<'a> {
    /// Returns the `Channel` to which the response should be sent.
    pub fn response_channel(&self) -> Channel {
        self.response_channel
    }

    /// Returns the protocol response size in Bytes.
    ///
    /// This is the minimal size in Bytes the protocol needs to have provided for its responses.
    /// `Protocol` implementations may make use of additional space as well, but this is the very
    /// minimum.
    ///
    /// The L2CAP implementation will not forward PDUs to the protocol unless this amount of space
    /// is available in the TX buffer.
    pub fn response_pdu_size(&self) -> u8 {
        self.rsp_pdu
    }

    /// Returns the protocol connected to the channel.
    pub fn protocol(&mut self) -> &mut ProtocolObj {
        self.protocol
    }
}

/// A fixed BLE channel map that provides only the required channel endpoints and does not allow
/// dynamic channels.
///
/// The channels are mapped as follows (no other channels are supported):
///
/// * `0x0004`: Attribute protocol (ATT).
/// * `0x0005`: LE L2CAP signaling channel.
/// * `0x0006`: LE Security Manager protocol.
pub struct BleChannelMap<A: Attributes, S: SecurityLevel> {
    att: AttributeServer<A>,
    sm: SecurityManager<S>,
}

impl BleChannelMap<NoAttributes, NoSecurity> {
    /// Creates a new channel map with no backing data for the connected protocols.
    ///
    /// This means:
    /// * The attribute server on channel `0x0004` will host an empty attribute set.
    /// * The security manager on channel `0x0006` will not support pairing or any security.
    pub fn empty() -> Self {
        Self {
            att: AttributeServer::new(NoAttributes),
            sm: SecurityManager::no_security(),
        }
    }
}

impl<A: Attributes> BleChannelMap<A, NoSecurity> {
    pub fn with_attributes(att: A) -> Self {
        Self {
            att: AttributeServer::new(att),
            sm: SecurityManager::no_security(),
        }
    }
}

impl<A: Attributes, S: SecurityLevel> ChannelMapper for BleChannelMap<A, S> {
    fn lookup(&mut self, channel: Channel) -> Option<ChannelData> {
        match channel {
            Channel::ATT => Some(ChannelData {
                response_channel: Channel::ATT,
                protocol: &mut self.att,
                rsp_pdu: AttributeServer::<A>::RSP_PDU_SIZE,
            }),
            Channel::LE_SECURITY_MANAGER => Some(ChannelData {
                response_channel: Channel::LE_SECURITY_MANAGER,
                protocol: &mut self.sm,
                rsp_pdu: SecurityManager::<S>::RSP_PDU_SIZE,
            }),
            // FIXME implement the rest
            _ => None,
        }
    }
}

/// Trait for protocols that sit on top of L2CAP (object-safe part).
///
/// A protocol can be connected to an L2CAP channel via a `ChannelMapper`.
pub trait ProtocolObj {
    /// Process a message sent to the protocol.
    ///
    /// The message is reassembled by L2CAP already, and the `responder` is guaranteed to fit a
    /// protocol payload of at least `Protocol::RSP_PDU_SIZE` Bytes, as defined by the protocol.
    ///
    /// # Errors
    ///
    /// This method should only return an error when a critical problem occurs that can not be
    /// recovered from and that can not be reported back to the connected device using the protocol.
    /// This means that only things like unrecoverable protocol parsing errors should return an
    /// error here.
    fn process_message(&mut self, message: &[u8], responder: L2CAPResponder) -> Result<(), Error>;
}

/// Trait for protocols that sit on top of L2CAP (non-object-safe part).
///
/// This extends the `ProtocolObj` trait with other protocol properties.
pub trait Protocol: ProtocolObj {
    /// Minimum size needed by PDUs sent by this protocol.
    ///
    /// Incoming PDUs will only be forwarded to the protocol if there is at least this much space in
    /// the TX buffer.
    const RSP_PDU_SIZE: u8;
}

/// Header used by *all* L2CAP PDUs.
#[derive(Debug)]
struct Header {
    /// Length of the payload following the length and channel fields (after reassembly).
    length: u16,
    /// Destination endpoint of the PDU.
    channel: Channel,
}

impl Header {
    /// The size of an L2CAP message header in Bytes.
    const SIZE: u8 = 2 + 2;
}

impl<'a> FromBytes<'a> for Header {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        let length = bytes.read_u16_le()?;
        let channel = Channel::from_bytes(bytes)?;
        Ok(Self { length, channel })
    }
}

impl ToBytes for Header {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u16_le(self.length)?;
        writer.write_u16_le(self.channel.as_raw())?;
        Ok(())
    }
}

struct Message<P> {
    header: Header,
    payload: P,
}

impl<'a, P: FromBytes<'a>> FromBytes<'a> for Message<P> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        let header = Header::from_bytes(bytes)?;
        assert_eq!(
            header.length as usize,
            bytes.bytes_left(),
            "L2CAP reassembly not yet implemented"
        );

        Ok(Self {
            header,
            payload: P::from_bytes(bytes)?,
        })
    }
}

impl<P: ToBytes> ToBytes for Message<P> {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        self.header.to_bytes(writer)?;
        self.payload.to_bytes(writer)?;
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

    /// Dispatches a fully reassembled L2CAP message to the protocol listening on the addressed
    /// channel.
    fn dispatch(&mut self, channel: Channel, payload: &[u8], tx: &mut Producer) -> Consume<()> {
        if let Some(mut chdata) = self.mapper.lookup(channel) {
            let free = tx.free_space();
            let needed = usize::from(chdata.response_pdu_size() + Header::SIZE);
            if free < needed {
                debug!(
                    "{} free bytes, need {}; waiting",
                    free,
                    chdata.response_pdu_size()
                );
                return Consume::never(Ok(()));
            }

            let resp_channel = chdata.response_channel();
            Consume::always(chdata.protocol().process_message(
                payload,
                L2CAPResponder {
                    tx,
                    channel: resp_channel,
                },
            ))
        } else {
            warn!(
                "ignoring message sent to unconnected channel {:?}: {:?}",
                channel,
                HexSlice(payload)
            );
            Consume::always(Ok(()))
        }
    }

    /// Process the start of a new L2CAP message (or a complete, unfragmented message).
    pub fn process_start(&mut self, message: &[u8], tx: &mut Producer) -> Consume<()> {
        let msg = match Message::<&[u8]>::from_bytes(&mut ByteReader::new(message)) {
            Ok(msg) => msg,
            Err(e) => return Consume::always(Err(e)),
        };

        if usize::from(msg.header.length) != msg.payload.len() {
            // Lengths mismatch => Reassembly needed
            unimplemented!("L2CAP reassembly");
        }

        self.dispatch(msg.header.channel, msg.payload, tx)
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
    /// L2CAP header (including the destination endpoint's channel) and the data channel PDU header
    /// will be added automatically.
    ///
    /// This will fail if there's not enough space left in the TX queue.
    pub fn respond<P: ToBytes>(&mut self, payload: P) -> Result<(), Error> {
        self.respond_with(|writer| payload.to_bytes(writer))
    }

    /// Respond with an L2CAP message encoded by a closure.
    ///
    /// L2CAP header and data channel PDU header will be added automatically. The closure `f` only
    /// has to write the protocol PDU to transmit over L2CAP.
    ///
    /// The L2CAP implementation will ensure that there are at least `Protocol::RSP_PDU_SIZE` Bytes
    /// available in the `ByteWriter` passed to the closure.
    pub fn respond_with<T, E>(
        &mut self,
        f: impl FnOnce(&mut ByteWriter) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<Error>,
    {
        // FIXME automatic fragmentation is not implemented

        // The payload length goes into the header, so we have to skip that part and write it later
        let channel = self.channel;
        let mut r = None;
        self.tx.produce_with(|writer| -> Result<_, E> {
            let mut header_writer = writer.split_off(usize::from(Header::SIZE))?;

            let left = writer.space_left();
            r = Some(f(writer)?);
            let used = left - writer.space_left();

            assert!(used < 0xFFFF);
            Header {
                length: used as u16,
                channel: channel,
            }
            .to_bytes(&mut header_writer)?;

            assert_eq!(header_writer.space_left(), 0);

            Ok(Llid::DataStart)
        })?;
        Ok(r.unwrap())
    }
}
