//! Defines packet structures used by the Link Layer Control Protocol.

use crate::bytes::{self, *};
use crate::link::{channel_map::ChannelMap, comp_id::CompanyId, features::FeatureSet};
use crate::{time::Duration, utils::Hex, Error};
use core::{cmp, convert::TryInto};
use zerocopy::{AsBytes, FromBytes, Unaligned};

/// An undecoded LLCP PDU.
#[derive(Debug)]
pub struct RawPdu<T>(T);

impl<T: AsRef<[u8]>> RawPdu<T> {
    pub fn new(buf: T) -> Option<Self> {
        if buf.as_ref().len() < 1 {
            None
        } else {
            Some(RawPdu(buf))
        }
    }

    /// Decodes the LLCP opcode, returning a structured representation of the PDU.
    pub fn decode(&self) -> Option<PduRef<'_>> {
        let bytes = self.0.as_ref();
        let (opcode, data) = bytes.split_first()?;
        Some(match ControlOpcode::from(*opcode) {
            ControlOpcode::ConnectionUpdateReq => PduRef::ConnectionUpdateReq(data.decode_as()?),
            ControlOpcode::ChannelMapReq => PduRef::ChannelMapReq(data.decode_as()?),
            ControlOpcode::TerminateInd => PduRef::TerminateInd(data.decode_as()?),
            ControlOpcode::EncReq => PduRef::EncReq(data.decode_as()?),
            ControlOpcode::EncRsp => PduRef::EncRsp(data.decode_as()?),
            ControlOpcode::StartEncReq => PduRef::StartEncReq(data.decode_as()?),
            ControlOpcode::StartEncRsp => PduRef::StartEncRsp(data.decode_as()?),
            ControlOpcode::UnknownRsp => PduRef::UnknownRsp(data.decode_as()?),
            ControlOpcode::FeatureReq => PduRef::FeatureReq(data.decode_as()?),
            ControlOpcode::FeatureRsp => PduRef::FeatureRsp(data.decode_as()?),
            ControlOpcode::PauseEncReq => PduRef::PauseEncReq(data.decode_as()?),
            ControlOpcode::PauseEncRsp => PduRef::PauseEncRsp(data.decode_as()?),
            ControlOpcode::VersionInd => PduRef::VersionInd(data.decode_as()?),
            ControlOpcode::RejectInd => PduRef::RejectInd(data.decode_as()?),
            ControlOpcode::SlaveFeatureReq => PduRef::SlaveFeatureReq(data.decode_as()?),
            ControlOpcode::ConnectionParamReq => PduRef::ConnectionParamReq(data.decode_as()?),
            ControlOpcode::ConnectionParamRsp => PduRef::ConnectionParamRsp(data.decode_as()?),
            ControlOpcode::RejectIndExt => PduRef::RejectIndExt(data.decode_as()?),
            ControlOpcode::PingReq => PduRef::PingReq(data.decode_as()?),
            ControlOpcode::PingRsp => PduRef::PingRsp(data.decode_as()?),
            ControlOpcode::LengthReq => PduRef::LengthReq(data.decode_as()?),
            ControlOpcode::LengthRsp => PduRef::LengthRsp(data.decode_as()?),
            ControlOpcode::Unknown(_) => return None,
        })
    }

    pub fn opcode(&self) -> ControlOpcode {
        ControlOpcode::from(self.0.as_ref()[0])
    }
}

/// Reference to a structured LLCP PDU.
#[derive(Debug, Copy, Clone)]
pub enum PduRef<'a> {
    ConnectionUpdateReq(&'a ConnectionUpdateReq),
    ChannelMapReq(&'a ChannelMapReq),
    TerminateInd(&'a TerminateInd),
    EncReq(&'a EncReq),
    EncRsp(&'a EncRsp),
    StartEncReq(&'a StartEncReq),
    StartEncRsp(&'a StartEncRsp),
    UnknownRsp(&'a UnknownRsp),
    FeatureReq(&'a FeatureReq),
    FeatureRsp(&'a FeatureRsp),
    PauseEncReq(&'a PauseEncReq),
    PauseEncRsp(&'a PauseEncRsp),
    VersionInd(&'a VersionInd),
    RejectInd(&'a RejectInd),
    SlaveFeatureReq(&'a SlaveFeatureReq),
    ConnectionParamReq(&'a ConnectionParamReq),
    ConnectionParamRsp(&'a ConnectionParamRsp),
    RejectIndExt(&'a RejectIndExt),
    PingReq(&'a PingReq),
    PingRsp(&'a PingRsp),
    LengthReq(&'a LengthReq),
    LengthRsp(&'a LengthRsp),
}

impl<'a> PduRef<'a> {
    pub fn opcode(&self) -> ControlOpcode {
        match self {
            Self::ConnectionUpdateReq(_) => ControlOpcode::ConnectionUpdateReq,
            Self::ChannelMapReq(_) => ControlOpcode::ChannelMapReq,
            Self::TerminateInd(_) => ControlOpcode::TerminateInd,
            Self::EncReq(_) => ControlOpcode::EncReq,
            Self::EncRsp(_) => ControlOpcode::EncRsp,
            Self::StartEncReq(_) => ControlOpcode::StartEncReq,
            Self::StartEncRsp(_) => ControlOpcode::StartEncRsp,
            Self::UnknownRsp(_) => ControlOpcode::UnknownRsp,
            Self::FeatureReq(_) => ControlOpcode::FeatureReq,
            Self::FeatureRsp(_) => ControlOpcode::FeatureRsp,
            Self::PauseEncReq(_) => ControlOpcode::PauseEncReq,
            Self::PauseEncRsp(_) => ControlOpcode::PauseEncRsp,
            Self::VersionInd(_) => ControlOpcode::VersionInd,
            Self::RejectInd(_) => ControlOpcode::RejectInd,
            Self::SlaveFeatureReq(_) => ControlOpcode::SlaveFeatureReq,
            Self::ConnectionParamReq(_) => ControlOpcode::ConnectionParamReq,
            Self::ConnectionParamRsp(_) => ControlOpcode::ConnectionParamRsp,
            Self::RejectIndExt(_) => ControlOpcode::RejectIndExt,
            Self::PingReq(_) => ControlOpcode::PingReq,
            Self::PingRsp(_) => ControlOpcode::PingRsp,
            Self::LengthReq(_) => ControlOpcode::LengthReq,
            Self::LengthRsp(_) => ControlOpcode::LengthRsp,
        }
    }
}

/// Structured representation of an LLCP PDU.
#[derive(Debug, Copy, Clone)]
pub enum Pdu {
    ConnectionUpdateReq(ConnectionUpdateReq),
    ChannelMapReq(ChannelMapReq),
    TerminateInd(TerminateInd),
    EncReq(EncReq),
    EncRsp(EncRsp),
    StartEncReq(StartEncReq),
    StartEncRsp(StartEncRsp),
    UnknownRsp(UnknownRsp),
    FeatureReq(FeatureReq),
    FeatureRsp(FeatureRsp),
    PauseEncReq(PauseEncReq),
    PauseEncRsp(PauseEncRsp),
    VersionInd(VersionInd),
    RejectInd(RejectInd),
    SlaveFeatureReq(SlaveFeatureReq),
    ConnectionParamReq(ConnectionParamReq),
    ConnectionParamRsp(ConnectionParamRsp),
    RejectIndExt(RejectIndExt),
    PingReq(PingReq),
    PingRsp(PingRsp),
    LengthReq(LengthReq),
    LengthRsp(LengthRsp),
}

impl Pdu {
    pub fn opcode(&self) -> ControlOpcode {
        match self {
            Self::ConnectionUpdateReq(_) => ControlOpcode::ConnectionUpdateReq,
            Self::ChannelMapReq(_) => ControlOpcode::ChannelMapReq,
            Self::TerminateInd(_) => ControlOpcode::TerminateInd,
            Self::EncReq(_) => ControlOpcode::EncReq,
            Self::EncRsp(_) => ControlOpcode::EncRsp,
            Self::StartEncReq(_) => ControlOpcode::StartEncReq,
            Self::StartEncRsp(_) => ControlOpcode::StartEncRsp,
            Self::UnknownRsp(_) => ControlOpcode::UnknownRsp,
            Self::FeatureReq(_) => ControlOpcode::FeatureReq,
            Self::FeatureRsp(_) => ControlOpcode::FeatureRsp,
            Self::PauseEncReq(_) => ControlOpcode::PauseEncReq,
            Self::PauseEncRsp(_) => ControlOpcode::PauseEncRsp,
            Self::VersionInd(_) => ControlOpcode::VersionInd,
            Self::RejectInd(_) => ControlOpcode::RejectInd,
            Self::SlaveFeatureReq(_) => ControlOpcode::SlaveFeatureReq,
            Self::ConnectionParamReq(_) => ControlOpcode::ConnectionParamReq,
            Self::ConnectionParamRsp(_) => ControlOpcode::ConnectionParamRsp,
            Self::RejectIndExt(_) => ControlOpcode::RejectIndExt,
            Self::PingReq(_) => ControlOpcode::PingReq,
            Self::PingRsp(_) => ControlOpcode::PingRsp,
            Self::LengthReq(_) => ControlOpcode::LengthReq,
            Self::LengthRsp(_) => ControlOpcode::LengthRsp,
        }
    }

    fn ctr_data(&self) -> &[u8] {
        match self {
            Self::ConnectionUpdateReq(it) => it.as_bytes(),
            Self::ChannelMapReq(it) => it.as_bytes(),
            Self::TerminateInd(it) => it.as_bytes(),
            Self::EncReq(it) => it.as_bytes(),
            Self::EncRsp(it) => it.as_bytes(),
            Self::StartEncReq(it) => it.as_bytes(),
            Self::StartEncRsp(it) => it.as_bytes(),
            Self::UnknownRsp(it) => it.as_bytes(),
            Self::FeatureReq(it) => it.as_bytes(),
            Self::FeatureRsp(it) => it.as_bytes(),
            Self::PauseEncReq(it) => it.as_bytes(),
            Self::PauseEncRsp(it) => it.as_bytes(),
            Self::VersionInd(it) => it.as_bytes(),
            Self::RejectInd(it) => it.as_bytes(),
            Self::SlaveFeatureReq(it) => it.as_bytes(),
            Self::ConnectionParamReq(it) => it.as_bytes(),
            Self::ConnectionParamRsp(it) => it.as_bytes(),
            Self::RejectIndExt(it) => it.as_bytes(),
            Self::PingReq(it) => it.as_bytes(),
            Self::PingRsp(it) => it.as_bytes(),
            Self::LengthReq(it) => it.as_bytes(),
            Self::LengthRsp(it) => it.as_bytes(),
        }
    }
}

impl ToBytes for Pdu {
    fn to_bytes(&self, buffer: &mut ByteWriter<'_>) -> Result<(), Error> {
        buffer.write_u8(self.opcode().into())?;
        buffer.write_slice(self.ctr_data())?;
        Ok(())
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

/// `LL_CONNECTION_UPDATE_REQ` - Update connection parameters.
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct ConnectionUpdateReq {
    win_size: u8,
    win_offset: u16,
    interval: u16,
    latency: u16,
    timeout: u16,
    instant: u16,
}

impl ConnectionUpdateReq {
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

/// `LL_CHANNEL_MAP_REQ` - Update the channel map in use.
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct ChannelMapReq {
    map: [u8; 5],
    instant: u16,
}

impl ChannelMapReq {
    pub fn channel_map(&self) -> ChannelMap {
        ChannelMap::from_raw(self.map)
    }

    pub fn instant(&self) -> u16 {
        self.instant
    }
}

/// `LL_TERMINATE_IND` - Connection termination indication.
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct TerminateInd {
    error: u8,
}

impl TerminateInd {
    pub fn error_code(&self) -> u8 {
        self.error
    }
}

/// `LL_ENC_REQ`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct EncReq {
    rand: [u8; 8],
    ediv: u16,
    skdm: [u8; 8],
    ivm: [u8; 4],
}

/// `LL_ENC_RSP`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct EncRsp {
    sdks: [u8; 8],
    ivs: [u8; 4],
}

/// `LL_START_END_REQ`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct StartEncReq {
    _p: (),
}

/// `LL_START_ENC_RSP`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct StartEncRsp {
    _p: (),
}

/// `LL_UNKNOWN_RSP`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct UnknownRsp {
    unknown_type: u8,
}

impl UnknownRsp {
    pub fn new(unknown: ControlOpcode) -> Self {
        Self {
            unknown_type: unknown.into(),
        }
    }
}

/// `LL_FEATURE_REQ`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct FeatureReq {
    feature_set: u64,
}

impl FeatureReq {
    pub fn new(master_features: FeatureSet) -> Self {
        Self {
            feature_set: master_features.bits(),
        }
    }

    pub fn master_features(&self) -> FeatureSet {
        FeatureSet::from_bits_truncate(self.feature_set)
    }
}

/// `LL_FEATURE_RSP`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct FeatureRsp {
    feature_set: u64,
}

impl FeatureRsp {
    pub fn new(feature_set: FeatureSet) -> Self {
        Self {
            feature_set: feature_set.bits(),
        }
    }
}

/// `LL_PAUSE_END_REQ`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct PauseEncReq {
    _p: (),
}

/// `LL_PAUSE_END_RSP`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct PauseEncRsp {
    _p: (),
}

/// `LL_VERSION_IND`.
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct VersionInd {
    vers_nr: u8,
    comp_id: u16,
    sub_vers_nr: u16,
}

impl VersionInd {
    pub fn new(bt_vers: VersionNumber, comp_id: CompanyId, sub_vers_nr: u16) -> Self {
        Self {
            vers_nr: bt_vers.into(),
            comp_id: comp_id.as_u16(),
            sub_vers_nr,
        }
    }
}

/// `LL_REJECT_IND`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct RejectInd {
    error_code: u8,
}

/// `LL_SLAVE_FEATURE_REQ`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct SlaveFeatureReq {
    feature_set: u64,
}

impl SlaveFeatureReq {
    pub fn feature_set(&self) -> FeatureSet {
        FeatureSet::from_bits_truncate(self.feature_set)
    }
}

/// `LL_CONNECTION_PARAM_REQ` - A connection parameter update request.
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct ConnectionParamReq {
    interval_min: u16,
    interval_max: u16,
    slave_latency: u16,
    supervision_timeout: u16,
    /// `connInterval` is preferred to be a multiple of this value (in 1.25 ms steps).
    preferred_periodicity: u8,
    reference_conn_event_count: u16,
    offsets: [u16; 6],
}

impl ConnectionParamReq {
    /// Creates a new connection update request structure filled with default values.
    ///
    /// The returned structure will use conservative (maximally permissive) default values that will
    /// not usually result in a change in connection parameters, so users of this function likely
    /// want to call a setter afterwards.
    pub fn new() -> Self {
        Self {
            interval_min: 6,    // 7.5ms
            interval_max: 3200, // 4s
            slave_latency: 0,
            supervision_timeout: 100,      // FIXME (unsure; 1s)
            preferred_periodicity: 0,      // not valid
            reference_conn_event_count: 0, // irrelevant
            offsets: [0xFFFF; 6],          // none valid
        }
    }

    /// Sets the minimum and maximum requested connection interval.
    ///
    /// # Parameters
    ///
    /// * `min`: Minimum connection interval to request.
    /// * `max`: Maximum connection interval to request.
    ///
    /// Both `min` and `max` must be in range 7.5ms to 4s, or they will be constrained to lie in
    /// that range.
    ///
    /// Both `min` and `max` will be rounded down to units of 1.25 ms.
    ///
    /// # Panics
    ///
    /// This will panic if `min > max`.
    pub fn set_conn_interval(&mut self, min: Duration, max: Duration) {
        assert!(min <= max);

        // Convert and round to units of 1.25 ms.
        let max = max.as_micros() / 1_250;
        let min = min.as_micros() / 1_250;

        // Clamp to valid range of 6..=3200
        let min = cmp::min(cmp::max(min, 6), 3200);
        let max = cmp::min(cmp::max(max, 6), 3200);
        debug_assert!(min <= max);
        self.interval_min = min as u16;
        self.interval_max = max as u16;
    }

    /// Returns the minimum requested connection interval.
    pub fn min_conn_interval(&self) -> Duration {
        Duration::from_micros(u32::from(self.interval_min) * 1_250)
    }

    /// Returns the maximum requested connection interval.
    pub fn max_conn_interval(&self) -> Duration {
        Duration::from_micros(u32::from(self.interval_max) * 1_250)
    }

    /// Returns the slave latency in number of connection events.
    pub fn slave_latency(&self) -> u16 {
        self.slave_latency
    }

    /// Returns the supervision timeout.
    pub fn supervision_timeout(&self) -> Duration {
        Duration::from_millis(self.supervision_timeout * 10)
    }
}

impl<'a> bytes::FromBytes<'a> for ConnectionParamReq {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        Ok(Self {
            interval_min: bytes.read_u16_le()?,
            interval_max: bytes.read_u16_le()?,
            slave_latency: bytes.read_u16_le()?,
            supervision_timeout: bytes.read_u16_le()?,
            preferred_periodicity: bytes.read_u8()?,
            reference_conn_event_count: bytes.read_u16_le()?,
            offsets: [
                bytes.read_u16_le()?,
                bytes.read_u16_le()?,
                bytes.read_u16_le()?,
                bytes.read_u16_le()?,
                bytes.read_u16_le()?,
                bytes.read_u16_le()?,
            ],
        })
    }
}

impl ToBytes for ConnectionParamReq {
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        writer.write_u16_le(self.interval_min)?;
        writer.write_u16_le(self.interval_max)?;
        writer.write_u16_le(self.slave_latency)?;
        writer.write_u16_le(self.supervision_timeout)?;
        writer.write_u8(self.preferred_periodicity)?;
        writer.write_u16_le(self.reference_conn_event_count)?;
        let offsets = self.offsets;
        for offset in &offsets {
            writer.write_u16_le(*offset)?;
        }
        Ok(())
    }
}

/// `LL_CONNECTION_PARAM_RSP`
pub type ConnectionParamRsp = ConnectionParamReq;

/// `LL_REJECT_IND_EXT`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct RejectIndExt {
    reject_opcode: u8,
    error_code: u8,
}

/// `LL_PING_REQ`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct PingReq {
    _p: (),
}

/// `LL_PING_RSP`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct PingRsp {
    _p: (),
}

/// `LL_LENGTH_REQ`
#[derive(Debug, Copy, Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C, packed)]
pub struct LengthReq {
    max_rx_octets: u16,
    max_rx_time: u16,
    max_tx_octets: u16,
    max_tx_time: u16,
}

/// `LL_LENGTH_RSP`
pub type LengthRsp = LengthReq;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_req_set_conn_interval() {
        fn set(min: Duration, max: Duration) -> (Duration, Duration) {
            let mut req = ConnectionParamReq::new();
            req.set_conn_interval(min, max);

            (req.min_conn_interval(), req.max_conn_interval())
        }

        fn same(min: Duration, max: Duration) {
            let (min2, max2) = set(min, max);
            assert_eq!(min2, min);
            assert_eq!(max2, max);
        }

        same(Duration::from_secs(1), Duration::from_secs(1));
        same(Duration::from_micros(7_500), Duration::from_micros(7_500));
        same(Duration::from_micros(7_500), Duration::from_secs(4));
        same(Duration::from_secs(4), Duration::from_secs(4));

        let (min, max) = set(Duration::from_secs(8), Duration::from_secs(8));
        assert_eq!(min, Duration::from_secs(4));
        assert_eq!(max, Duration::from_secs(4));

        let (min, max) = set(Duration::from_secs(0), Duration::from_secs(8));
        assert_eq!(min, Duration::from_micros(7_500));
        assert_eq!(max, Duration::from_secs(4));

        let (min, max) = set(Duration::from_micros(7_501), Duration::from_micros(7_502));
        assert_eq!(min, Duration::from_micros(7_500));
        assert_eq!(max, Duration::from_micros(7_500));
    }

    #[test]
    #[should_panic(expected = "min <= max")]
    fn update_req_set_conn_interval_minmax() {
        let mut req = ConnectionParamReq::new();
        req.set_conn_interval(Duration::from_secs(8), Duration::from_secs(7));
    }
}
