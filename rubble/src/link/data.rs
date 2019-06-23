//! Data Channel structures.

use {
    crate::{
        bytes::*,
        link::{channel_map::ChannelMap, comp_id::CompanyId, FeatureSet, SeqNum},
        time::Duration,
        utils::Hex,
        Error,
    },
    byteorder::{ByteOrder, LittleEndian},
    core::fmt,
};

/// 16-bit data channel header preceding the payload.
///
/// Layout (in Bluetooth 4.2):
///
/// ```notrust
/// LSB                                                                MSB
/// +----------+---------+---------+---------+------------+--------------+
/// |   LLID   |  NESN   |   SN    |   MD    |     -      |    Length    |
/// | (2 bits) | (1 bit) | (1 bit) | (1 bit) |  (3 bits)  |   (8 bits)   |
/// +----------+---------+---------+---------+------------+--------------+
/// ```
///
/// Payload format depends on the value of the 2-bit `LLID` field:
///
/// * `0b00`: Reserved value.
/// * `0b01`: LL Data PDU Continuation fragment or empty PDU.
/// * `0b10`: LL Data PDU Start of L2CAP message (or complete message if no fragmentation
///   necessary).
/// * `0b11`: LL Control PDU.
///
/// The `NESN` field specifies the **N**ext **E**xpected **S**equence **N**umber. The `SN` field
/// specifies the **S**equence **N**umber of this PDU.
///
/// The `MD` field specifies that the device sending the packet has more data to send during this
/// *connection event*. When both slave and master send a packet with the `MD` bit set to 0, the
/// connection event ends.
///
/// The `Length` field specifies the length of payload **and `MIC`**. Prior to Bluetooth 4.2, this
/// was a 5-bit field, resulting in payloads + MICs of up to 31 Bytes. With Bluetooth 4.2, devices
/// can communicate their buffer sizes and optionally transmit larger packets.
///
/// ## Sequence Numbers
///
/// The `NESN` and `SN` fields are used for retransmission and acknowledgement. The link layer
/// stores two 1-bit parameters for an established connection, called `transmitSeqNum` and
/// `nextExpectedSeqNum`. When a connection is established, both start out as 0. Both parameters are
/// repeatedly incremented by 1 when data is transmitted, using wrapping arithmetic.
///
/// When a data channel packet is sent for the first time (ie. not retransmitted), the `SN` field is
/// set to `transmitSeqNum`. When the packet is resent, the `SN` field is not modified. In both
/// cases, the `NESN` bit is set to `nextExpectedSeqNum`.
///
/// The `NESN` bit tells the receiver whether its last packet has arrived: When a packet is
/// received with an `NESN` value equal to the receiver's `transmitSeqNum`, the receiver has already
/// sent a packet with the expected `SN`, but the other side hasn't received it yet. The receiver
/// must resend the last data channel PDU. No other data channel PDU must be sent by it.
///
/// When the received packet's `NESN` bit is different from `transmitSeqNum`, the last PDU has been
/// acknowledged and the receiver should increment `transmitSeqNum` by 1.
///
/// Similarly, the `SN` bit is used to distinguish retransmitted and new packets: When a packet is
/// received with an `SN` value equal to the receiver's `nextExpectedSeqNum` value, the packet is
/// new (not a retransmission), and `nextExpectedSeqNum` should be incremented by 1. If the value is
/// not equal to `nextExpectedSeqNum`, this packet is a retransmission, so `nextExpectedSeqNum`
/// should not be changed.
#[derive(Copy, Clone)]
pub struct Header(u16);

impl Header {
    /// Creates a header with the given LLID field and all other fields set to 0 (including the
    /// payload length).
    pub fn new(llid: Llid) -> Self {
        Header(llid as u16)
    }

    /// Parses a header from raw bytes.
    ///
    /// Panics when `raw` contains less than 2 Bytes.
    pub fn parse(raw: &[u8]) -> Self {
        Header(LittleEndian::read_u16(&raw))
    }

    /// Returns the raw representation of the header.
    ///
    /// The returned `u16` must be transmitted LSB and LSb first as the first 2 octets of the PDU.
    pub fn to_u16(&self) -> u16 {
        self.0
    }

    /// Returns the length of the payload in octets as specified in the `Length` field.
    pub fn payload_length(&self) -> u8 {
        ((self.0 & 0b11111111_00000000) >> 8) as u8
    }

    /// Sets the payload length field to `len`.
    ///
    /// Note that BLE <4.2 is restricted to 5-bit payload lengths.
    pub fn set_payload_length(&mut self, len: u8) {
        self.0 = (u16::from(len) << 8) | (self.0 & 0x00ff);
    }

    /// Returns the `LLID` field (PDU type).
    pub fn llid(&self) -> Llid {
        let bits = self.0 & 0b11;
        match bits {
            0b00 => Llid::Reserved,
            0b01 => Llid::DataCont,
            0b10 => Llid::DataStart,
            0b11 => Llid::Control,
            _ => unreachable!(),
        }
    }

    /// Returns the value of the `NESN` field (Next Expected Sequence Number).
    pub fn nesn(&self) -> SeqNum {
        let bit = self.0 & 0b0100;
        if bit == 0 {
            SeqNum::ZERO
        } else {
            SeqNum::ONE
        }
    }

    /// Sets the value of the `NESN` field.
    pub fn set_nesn(&mut self, nesn: SeqNum) {
        if nesn == SeqNum::ONE {
            self.0 |= 0b0100;
        } else {
            self.0 &= !0b0100;
        }
    }

    /// Returns the value of the `SN` field (Sequence Number).
    pub fn sn(&self) -> SeqNum {
        let bit = self.0 & 0b1000;
        if bit == 0 {
            SeqNum::ZERO
        } else {
            SeqNum::ONE
        }
    }

    /// Sets the value of the `SN` field.
    pub fn set_sn(&mut self, sn: SeqNum) {
        if sn == SeqNum::ONE {
            self.0 |= 0b1000;
        } else {
            self.0 &= !0b1000;
        }
    }

    /// Returns whether the `MD` field is set (More Data).
    pub fn md(&self) -> bool {
        let bit = self.0 & 0b10000;
        bit != 0
    }

    /// Sets the value of the `MD` field.
    pub fn set_md(&mut self, md: bool) {
        if md {
            self.0 |= 0b10000;
        } else {
            self.0 &= !0b10000;
        }
    }
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Header")
            .field("LLID", &self.llid())
            .field("NESN", &self.nesn())
            .field("SN", &self.sn())
            .field("MD", &self.md())
            .field("Length", &self.payload_length())
            .finish()
    }
}

/// Values of the LLID field in `Header`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Llid {
    /// Reserved for future use.
    Reserved = 0b00,

    /// Continuation of L2CAP message, or empty PDU.
    DataCont = 0b01,

    /// Start of L2CAP message.
    DataStart = 0b10,

    /// LL control PDU.
    Control = 0b11,
}

/// Structured representation of a data channel PDU.
#[derive(Debug)]
pub enum Pdu<'a, L> {
    /// Continuation of an L2CAP message (or empty PDU).
    DataCont { message: L },

    /// Start of an L2CAP message (must not be empty).
    DataStart { message: L },

    /// LL Control PDU for controlling the Link-Layer connection.
    Control { data: BytesOr<'a, ControlPdu<'a>> },
}

impl<'a> Pdu<'a, &'a [u8]> {
    /// Creates an empty PDU that carries no message.
    ///
    /// This PDU can be sent whenever there's no actual data to be transferred.
    pub fn empty() -> Self {
        Pdu::DataCont { message: &[] }
    }
}

impl<'a, L> Pdu<'a, L> {
    /// Returns the `LLID` field to use for this PDU.
    pub fn llid(&self) -> Llid {
        match self {
            Pdu::DataCont { .. } => Llid::DataCont,
            Pdu::DataStart { .. } => Llid::DataStart,
            Pdu::Control { .. } => Llid::Control,
        }
    }
}

impl<'a, L: FromBytes<'a> + ?Sized> Pdu<'a, L> {
    /// Parses a PDU from a `Header` and raw payload.
    pub fn parse(header: Header, payload: &'a [u8]) -> Result<Self, Error> {
        match header.llid() {
            Llid::DataCont => Ok(Pdu::DataCont {
                message: L::from_bytes(&mut ByteReader::new(payload))?,
            }),
            Llid::DataStart => Ok(Pdu::DataStart {
                message: L::from_bytes(&mut ByteReader::new(payload))?,
            }),
            Llid::Control => Ok(Pdu::Control {
                data: BytesOr::from_bytes(&mut ByteReader::new(payload))?,
            }),
            Llid::Reserved => Err(Error::InvalidValue),
        }
    }
}

impl<'a> From<&'a ControlPdu<'a>> for Pdu<'a, &'a [u8]> {
    fn from(c: &'a ControlPdu<'a>) -> Self {
        Pdu::Control { data: c.into() }
    }
}

/// Serializes the payload of the PDU to bytes.
///
/// The PDU header must be constructed using Link-Layer state (and `Pdu::llid`).
impl<'a, L: ToBytes> ToBytes for Pdu<'a, L> {
    fn to_bytes(&self, buffer: &mut ByteWriter<'_>) -> Result<(), Error> {
        match self {
            Pdu::DataCont { message } | Pdu::DataStart { message } => message.to_bytes(buffer),
            Pdu::Control { data } => data.to_bytes(buffer),
        }
    }
}

/// Data transmitted with an `LL_CONNECTION_UPDATE_REQ` Control PDU, containing a new set of
/// connection parameters.
#[derive(Debug, Copy, Clone)]
pub struct ConnectionUpdateData {
    win_size: u8,
    win_offset: u16,
    interval: u16,
    latency: u16,
    timeout: u16,
    instant: u16,
}

impl ConnectionUpdateData {
    /// Returns the size of the transmit window for the first PDU of the connection.
    pub fn win_size(&self) -> Duration {
        Duration::from_micros(u32::from(self.win_size) * 1_250)
    }

    /// Returns the offset of the transmit window, as a duration since the `instant`.
    pub fn win_offset(&self) -> Duration {
        Duration::from_micros(u32::from(self.win_offset) * 1_250)
    }

    /// Returns the duration between connection events.
    pub fn interval(&self) -> Duration {
        Duration::from_micros(u32::from(self.interval) * 1_250)
    }

    /// Returns the slave latency.
    pub fn latency(&self) -> u16 {
        self.latency
    }

    /// Returns the connection supervision timeout (`connSupervisionTimeout`).
    pub fn timeout(&self) -> Duration {
        Duration::from_micros(u32::from(self.timeout) * 10_000)
    }

    /// Returns the instant at which these changes should take effect.
    pub fn instant(&self) -> u16 {
        self.instant
    }
}

/// A structured representation of an LL Control PDU used by the Link Layer Control Protocol (LLCP).
#[derive(Debug, Copy, Clone)]
pub enum ControlPdu<'a> {
    /// `0x00`/`LL_CONNECTION_UPDATE_REQ` - Update connection parameters.
    ///
    /// Sent by the master. The slave does not send a response back.
    ConnectionUpdateReq(ConnectionUpdateData),

    /// `0x01`/`LL_CHANNEL_MAP_REQ` - Update the channel map.
    ///
    /// Sent by the master. The slave does not send a response back.
    ChannelMapReq { map: ChannelMap, instant: u16 },

    /// `0x02`/`LL_TERMINATE_IND` - Close the connection.
    ///
    /// Can be sent by master or slave.
    TerminateInd { error_code: Hex<u8> },

    /// `0x07`/`LL_UNKNOWN_RSP` - Response to unknown/unsupported LL Control PDUs.
    ///
    /// This is returned as a response to an incoming LL Control PDU when the opcode is
    /// unimplemented or unknown, or when the `CtrData` is invalid for the opcode.
    UnknownRsp {
        /// Opcode of the unknown PDU.
        unknown_type: ControlOpcode,
    },

    /// `0x08`/`LL_FEATURE_REQ` - Master requests slave's features.
    FeatureReq {
        /// Supported feature set of the master.
        features_master: FeatureSet,
    },

    /// `0x09`/`LL_FEATURE_RSP` - Slave answers `LL_FEATURE_REQ` with the used feature set.
    FeatureRsp {
        /// Features that will be used for the connection. Logical `AND` of master and slave
        /// features.
        features_used: FeatureSet,
    },

    /// `0x0C`/`LL_VERSION_IND` - Bluetooth version indication (sent by both master and slave).
    ///
    /// When either master or slave receive this PDU, they should respond with their version if they
    /// have not already sent this PDU during this data connection (FIXME do this).
    VersionInd {
        vers_nr: VersionNumber,
        comp_id: CompanyId,
        sub_vers_nr: Hex<u16>,
    },

    /// Catch-all variant for unsupported opcodes.
    Unknown {
        /// The opcode we don't support. This can also be the `Unknown` variant.
        opcode: ControlOpcode,

        /// Additional data depending on the opcode.
        ctr_data: &'a [u8],
    },
}

impl ControlPdu<'_> {
    /// Returns the opcode of this LL Control PDU.
    pub fn opcode(&self) -> ControlOpcode {
        match self {
            ControlPdu::ConnectionUpdateReq { .. } => ControlOpcode::ConnectionUpdateReq,
            ControlPdu::ChannelMapReq { .. } => ControlOpcode::ChannelMapReq,
            ControlPdu::TerminateInd { .. } => ControlOpcode::TerminateInd,
            ControlPdu::UnknownRsp { .. } => ControlOpcode::UnknownRsp,
            ControlPdu::FeatureReq { .. } => ControlOpcode::FeatureReq,
            ControlPdu::FeatureRsp { .. } => ControlOpcode::FeatureRsp,
            ControlPdu::VersionInd { .. } => ControlOpcode::VersionInd,
            ControlPdu::Unknown { opcode, .. } => *opcode,
        }
    }
}

impl<'a> FromBytes<'a> for ControlPdu<'a> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        let opcode = ControlOpcode::from(bytes.read_u8()?);
        Ok(match opcode {
            ControlOpcode::ConnectionUpdateReq => {
                ControlPdu::ConnectionUpdateReq(ConnectionUpdateData {
                    win_size: bytes.read_u8()?,
                    win_offset: bytes.read_u16_le()?,
                    interval: bytes.read_u16_le()?,
                    latency: bytes.read_u16_le()?,
                    timeout: bytes.read_u16_le()?,
                    instant: bytes.read_u16_le()?,
                })
            }
            ControlOpcode::ChannelMapReq => ControlPdu::ChannelMapReq {
                map: ChannelMap::from_raw(bytes.read_array()?),
                instant: bytes.read_u16_le()?,
            },
            ControlOpcode::TerminateInd => ControlPdu::TerminateInd {
                error_code: Hex(bytes.read_u8()?),
            },
            ControlOpcode::UnknownRsp => ControlPdu::UnknownRsp {
                unknown_type: ControlOpcode::from(bytes.read_u8()?),
            },
            ControlOpcode::FeatureReq => ControlPdu::FeatureReq {
                features_master: FeatureSet::from_bytes(bytes)?,
            },
            ControlOpcode::FeatureRsp => ControlPdu::FeatureRsp {
                features_used: FeatureSet::from_bytes(bytes)?,
            },
            ControlOpcode::VersionInd => ControlPdu::VersionInd {
                vers_nr: VersionNumber::from(bytes.read_u8()?),
                comp_id: CompanyId::from_raw(bytes.read_u16_le()?),
                sub_vers_nr: Hex(bytes.read_u16_le()?),
            },
            _ => ControlPdu::Unknown {
                opcode,
                ctr_data: bytes.read_rest(),
            },
        })
    }
}

impl<'a> ToBytes for ControlPdu<'a> {
    fn to_bytes(&self, buffer: &mut ByteWriter<'_>) -> Result<(), Error> {
        buffer.write_u8(self.opcode().into())?;
        match self {
            ControlPdu::ConnectionUpdateReq(data) => {
                buffer.write_u8(data.win_size)?;
                buffer.write_u16_le(data.win_offset)?;
                buffer.write_u16_le(data.interval)?;
                buffer.write_u16_le(data.latency)?;
                buffer.write_u16_le(data.timeout)?;
                buffer.write_u16_le(data.instant)?;
                Ok(())
            }
            ControlPdu::ChannelMapReq { map, instant } => {
                buffer.write_slice(&map.to_raw())?;
                buffer.write_u16_le(*instant)?;
                Ok(())
            }
            ControlPdu::TerminateInd { error_code } => {
                buffer.write_u8(error_code.0)?;
                Ok(())
            }
            ControlPdu::UnknownRsp { unknown_type } => {
                buffer.write_u8(u8::from(*unknown_type))?;
                Ok(())
            }
            ControlPdu::FeatureReq { features_master } => features_master.to_bytes(buffer),
            ControlPdu::FeatureRsp { features_used } => features_used.to_bytes(buffer),
            ControlPdu::VersionInd {
                vers_nr,
                comp_id,
                sub_vers_nr,
            } => {
                buffer.write_u8(u8::from(*vers_nr))?;
                buffer.write_u16_le(comp_id.as_u16())?;
                buffer.write_u16_le(sub_vers_nr.0)?;
                Ok(())
            }
            ControlPdu::Unknown { ctr_data, .. } => {
                buffer.write_slice(ctr_data)?;
                Ok(())
            }
        }
    }
}

enum_with_unknown! {
    /// Enumeration of all known LL Control PDU opcodes (not all of which might be supported).
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub enum ControlOpcode(u8) {
        ConnectionUpdateReq = 0x00,
        ChannelMapReq = 0x01,
        TerminateInd = 0x02,
        EncReq = 0x03,
        EncRsp = 0x04,
        StartEncReq = 0x05,
        StartEncRsp = 0x06,
        UnknownRsp = 0x07,
        FeatureReq = 0x08,
        FeatureRsp = 0x09,
        PauseEncReq = 0x0A,
        PauseEncRsp = 0x0B,
        VersionInd = 0x0C,
        RejectInd = 0x0D,
        SlaveFeatureReq = 0x0E,
        ConnectionParamReq = 0x0F,
        ConnectionParamRsp = 0x10,
        RejectIndExt = 0x11,
        PingReq = 0x12,
        PingRsp = 0x13,
        LengthReq = 0x14,
        LengthRsp = 0x15,
    }
}

enum_with_unknown! {
    /// Enumeration of all possible `VersNr` for `LL_VERSION_IND` PDUs.
    ///
    /// According to https://www.bluetooth.com/specifications/assigned-numbers/link-layer
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub enum VersionNumber(u8) {
        V4_0 = 6,
        V4_1 = 7,
        V4_2 = 8,
        V5_0 = 9,
        V5_1 = 10,
    }
}
