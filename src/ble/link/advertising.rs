//! Advertising channel operations.
//!
//! This module defines PDUs, states and fields used by packets transmitted on the advertising
//! channels. Generally, this includes everything needed to advertise as and scan for slave devices
//! and to establish connections.
//!
//! Note that while the types in here do not completely eliminate illegal values to be created, they
//! do employ a range of sanity checks that prevent bogus packets from being sent by the stack.

use {
    super::{
        ad_structure::{AdStructure, Flags},
        AddressKind, DeviceAddress, MAX_PAYLOAD_SIZE,
    },
    crate::ble::{bytes::*, utils::Hex, Error},
    byteorder::{ByteOrder, LittleEndian},
    core::{fmt, iter},
};

mod private {
    #[derive(Debug, Copy, Clone)]
    pub struct Priv;
}

/// A parsed advertising channel PDU.
#[derive(Debug, Copy, Clone)]
pub struct Pdu<'a> {
    data: PduData,
    ad: Option<BytesOr<'a, [AdStructure<'a>]>>,
}

impl<'a> Pdu<'a> {
    /// Constructs a PDU by parsing `payload`.
    pub fn from_header_and_payload(header: Header, payload: &mut &'a [u8]) -> Result<Self, Error> {
        if usize::from(header.payload_length()) != payload.len() {
            return Err(Error::InvalidLength);
        }

        let data = match header.type_() {
            PduType::AdvInd => PduData::ConnectableUndirected {
                advertiser_addr: {
                    let kind = if header.tx_add() {
                        AddressKind::Random
                    } else {
                        AddressKind::Public
                    };
                    DeviceAddress::new(payload.read_array::<[u8; 6]>().ok_or(Error::Eof)?, kind)
                },
                _priv: private::Priv,
            },
            PduType::AdvDirectInd => PduData::ConnectableDirected {
                advertiser_addr: {
                    let kind = if header.tx_add() {
                        AddressKind::Random
                    } else {
                        AddressKind::Public
                    };
                    DeviceAddress::new(payload.read_array::<[u8; 6]>().ok_or(Error::Eof)?, kind)
                },
                initiator_addr: {
                    let kind = if header.rx_add() {
                        AddressKind::Random
                    } else {
                        AddressKind::Public
                    };
                    DeviceAddress::new(payload.read_array::<[u8; 6]>().ok_or(Error::Eof)?, kind)
                },
                _priv: private::Priv,
            },
            PduType::AdvNonconnInd => PduData::NonconnectableUndirected {
                advertiser_addr: {
                    let kind = if header.tx_add() {
                        AddressKind::Random
                    } else {
                        AddressKind::Public
                    };
                    DeviceAddress::new(payload.read_array::<[u8; 6]>().ok_or(Error::Eof)?, kind)
                },
                _priv: private::Priv,
            },
            PduType::AdvScanInd => PduData::ScannableUndirected {
                advertiser_addr: {
                    let kind = if header.tx_add() {
                        AddressKind::Random
                    } else {
                        AddressKind::Public
                    };
                    DeviceAddress::new(payload.read_array::<[u8; 6]>().ok_or(Error::Eof)?, kind)
                },
                _priv: private::Priv,
            },
            PduType::ScanReq => PduData::ScanRequest {
                scanner_addr: {
                    // Scanning device sends this PDU
                    let kind = if header.tx_add() {
                        AddressKind::Random
                    } else {
                        AddressKind::Public
                    };
                    DeviceAddress::new(payload.read_array::<[u8; 6]>().ok_or(Error::Eof)?, kind)
                },
                advertiser_addr: {
                    // Advertiser receives this PDU (when it broadcasts an advertisement that
                    // indicates that the device is scannable).
                    let kind = if header.rx_add() {
                        AddressKind::Random
                    } else {
                        AddressKind::Public
                    };
                    DeviceAddress::new(payload.read_array::<[u8; 6]>().ok_or(Error::Eof)?, kind)
                },
                _priv: private::Priv,
            },
            PduType::ScanRsp => PduData::ScanResponse {
                advertiser_addr: {
                    let kind = if header.tx_add() {
                        AddressKind::Random
                    } else {
                        AddressKind::Public
                    };
                    DeviceAddress::new(payload.read_array::<[u8; 6]>().ok_or(Error::Eof)?, kind)
                },
                _priv: private::Priv,
            },
            PduType::ConnectReq => PduData::ConnectRequest {
                // Initiator sends this PDU
                initiator_addr: {
                    // Scanning device sends this PDU
                    let kind = if header.tx_add() {
                        AddressKind::Random
                    } else {
                        AddressKind::Public
                    };
                    DeviceAddress::new(payload.read_array::<[u8; 6]>().ok_or(Error::Eof)?, kind)
                },
                // Advertiser receives this PDU (if it has sent a connectable advertisement)
                advertiser_addr: {
                    // Advertiser receives this PDU (when it broadcasts an advertisement that
                    // indicates that the device is scannable).
                    let kind = if header.rx_add() {
                        AddressKind::Random
                    } else {
                        AddressKind::Public
                    };
                    DeviceAddress::new(payload.read_array::<[u8; 6]>().ok_or(Error::Eof)?, kind)
                },
                _priv: private::Priv,
            },
            PduType::Unknown(_) => return Err(Error::InvalidValue),
        };
        let ad = if header.type_().allows_adv_data() {
            Some(BytesOr::from_bytes(payload)?)
        } else {
            None
        };
        Ok(Self { data, ad })
    }

    /// Returns the device address of the sender of this PDU.
    pub fn sender(&self) -> &DeviceAddress {
        use self::PduData::*;

        match &self.data {
            ConnectableUndirected {
                advertiser_addr, ..
            }
            | ConnectableDirected {
                advertiser_addr, ..
            }
            | NonconnectableUndirected {
                advertiser_addr, ..
            }
            | ScannableUndirected {
                advertiser_addr, ..
            }
            | ScanResponse {
                advertiser_addr, ..
            } => advertiser_addr,

            ScanRequest { scanner_addr, .. } => scanner_addr,

            ConnectRequest { initiator_addr, .. } => initiator_addr,
        }
    }

    /// Returns the intended receiver of this PDU.
    ///
    /// This may be `None` if the PDU doesn't have a fixed receiver.
    pub fn receiver(&self) -> Option<&DeviceAddress> {
        use self::PduData::*;

        match &self.data {
            ConnectableUndirected { .. }
            | NonconnectableUndirected { .. }
            | ScannableUndirected { .. }
            | ScanResponse { .. } => None,

            ConnectableDirected { initiator_addr, .. } => Some(initiator_addr),
            ScanRequest {
                advertiser_addr, ..
            }
            | ConnectRequest {
                advertiser_addr, ..
            } => Some(advertiser_addr),
        }
    }

    /// Returns a structured representation of all fixed data in this PDU.
    ///
    /// The returned `PduData` does not include the attached advertising data.
    /// To get that, call `advertising_data()`.
    pub fn data(&self) -> &PduData {
        &self.data
    }

    /// Returns the PDU type of `self`.
    pub fn ty(&self) -> PduType {
        use self::PduData::*;

        match self.data {
            ConnectableUndirected { .. } => PduType::AdvInd,
            ConnectableDirected { .. } => PduType::AdvDirectInd,
            NonconnectableUndirected { .. } => PduType::AdvNonconnInd,
            ScannableUndirected { .. } => PduType::AdvScanInd,
            ScanRequest { .. } => PduType::ScanReq,
            ScanResponse { .. } => PduType::ScanRsp,
            ConnectRequest { .. } => PduType::ConnectReq,
        }
    }

    /// Returns an iterator over all AD structures encoded in the PDU.
    ///
    /// If this PDU doesn't support attaching AD structures, this will return
    /// `None`.
    pub fn advertising_data(&self) -> Option<impl Iterator<Item = AdStructure<'a>>> {
        self.ad.map(|ad| ad.iter())
    }
}

/// Decodes an advertising channel PDU (consisting of header and payload) from
/// raw bytes.
impl<'a> FromBytes<'a> for Pdu<'a> {
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error> {
        let header = Header::from_bytes(bytes)?;
        Self::from_header_and_payload(header, bytes)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum PduData {
    /// Connectable and scannable advertisement.
    ConnectableUndirected {
        advertiser_addr: DeviceAddress,
        // + adv_data
        #[doc(hidden)]
        _priv: private::Priv,
    },
    ConnectableDirected {
        advertiser_addr: DeviceAddress,
        initiator_addr: DeviceAddress,

        #[doc(hidden)]
        _priv: private::Priv,
    },

    NonconnectableUndirected {
        advertiser_addr: DeviceAddress,
        // + adv_data
        #[doc(hidden)]
        _priv: private::Priv,
    },

    ScannableUndirected {
        advertiser_addr: DeviceAddress,
        // + adv_data
        #[doc(hidden)]
        _priv: private::Priv,
    },

    ScanRequest {
        scanner_addr: DeviceAddress,
        advertiser_addr: DeviceAddress,

        #[doc(hidden)]
        _priv: private::Priv,
    },

    ScanResponse {
        advertiser_addr: DeviceAddress,
        // + adv_data (or scan_response_data)
        #[doc(hidden)]
        _priv: private::Priv,
    },

    ConnectRequest {
        initiator_addr: DeviceAddress,
        advertiser_addr: DeviceAddress,

        #[doc(hidden)]
        _priv: private::Priv,
    },
}

/// Stores an advertising channel PDU.
///
/// This is an owned version of `Pdu` and should be used when *creating* a PDU
/// to be sent out.
pub struct PduBuf {
    /// 2-Byte header.
    header: Header,
    /// Fixed-size buffer that can store the largest PDU. Actual length is
    /// stored in the header.
    payload_buf: [u8; MAX_PAYLOAD_SIZE],
}

impl PduBuf {
    /// Builds a PDU buffer containing advertiser address and data.
    fn adv(
        ty: PduType,
        adv: DeviceAddress,
        adv_data: &mut Iterator<Item = &AdStructure>,
    ) -> Result<Self, Error> {
        let mut payload = [0; MAX_PAYLOAD_SIZE];
        let mut buf = ByteWriter::new(&mut payload[..]);
        buf.write_slice(adv.raw()).unwrap();
        for ad in adv_data {
            ad.to_bytes(&mut buf)?;
        }

        let left = buf.space_left();
        let used = payload.len() - left;
        let mut header = Header::new(ty);
        header.set_payload_length(used as u8);
        header.set_tx_add(adv.is_random());
        header.set_rx_add(false);
        Ok(Self {
            header,
            payload_buf: payload,
        })
    }

    /// Creates a connectable undirected advertising PDU (`ADV_IND`).
    ///
    /// # Parameters
    ///
    /// * `adv`: The advertiser address, the address of the device sending this
    ///   PDU.
    /// * `adv_data`: Additional advertising data to send.
    pub fn connectable_undirected(
        advertiser_addr: DeviceAddress,
        advertiser_data: &[AdStructure],
    ) -> Result<Self, Error> {
        Self::adv(
            PduType::AdvInd,
            advertiser_addr,
            &mut advertiser_data.iter(),
        )
    }

    /// Creates a connectable directed advertising PDU (`ADV_DIRECT_IND`).
    pub fn connectable_directed(
        advertiser_addr: DeviceAddress,
        initiator_addr: DeviceAddress,
    ) -> Self {
        let mut payload = [0; 37];
        payload[0..6].copy_from_slice(advertiser_addr.raw());
        payload[6..12].copy_from_slice(initiator_addr.raw());

        let mut header = Header::new(PduType::AdvDirectInd);
        header.set_payload_length(6 + 6);
        header.set_tx_add(advertiser_addr.is_random());
        header.set_rx_add(initiator_addr.is_random());

        Self {
            header,
            payload_buf: payload,
        }
    }

    /// Creates a non-connectable undirected advertising PDU
    /// (`ADV_NONCONN_IND`).
    ///
    /// This is equivalent to `PduBuf::beacon`, which should be preferred when
    /// building a beacon PDU to improve clarity.
    pub fn nonconnectable_undirected(
        advertiser_addr: DeviceAddress,
        advertiser_data: &[AdStructure],
    ) -> Result<Self, Error> {
        Self::adv(
            PduType::AdvNonconnInd,
            advertiser_addr,
            &mut advertiser_data.iter(),
        )
    }

    /// Creates a scannable undirected advertising PDU (`ADV_SCAN_IND`).
    ///
    /// Note that scanning is not supported at the moment.
    pub fn scannable_undirected(
        advertiser_addr: DeviceAddress,
        advertiser_data: &[AdStructure],
    ) -> Result<Self, Error> {
        Self::adv(
            PduType::AdvScanInd,
            advertiser_addr,
            &mut advertiser_data.iter(),
        )
    }

    /// Creates an advertising channel PDU suitable for building a simple
    /// beacon.
    ///
    /// This is mostly equivalent to `PduBuf::nonconnectable_undirected`, but it
    /// will automatically add a suitable `Flags` AD structure to the
    /// advertising data (this flags is mandatory).
    pub fn beacon(
        advertiser_addr: DeviceAddress,
        advertiser_data: &[AdStructure],
    ) -> Result<Self, Error> {
        Self::adv(
            PduType::AdvNonconnInd,
            advertiser_addr,
            &mut iter::once(&AdStructure::from(Flags::broadcast())).chain(advertiser_data),
        )
    }

    /// Creates an advertising PDU that makes this device "visible" for scanning
    /// devices that want to establish a connection.
    ///
    /// This should be used when this device would like to initiate pairing.
    ///
    /// This function is mostly equivalent to `PduBuf::connectable_undirected`,
    /// but will automatically add a suitable `Flags` AD structure to the
    /// advertising data.
    ///
    /// To establish a connection with an already paired device, a "directed"
    /// advertisement must be sent instead.
    pub fn discoverable(
        advertiser_addr: DeviceAddress,
        advertiser_data: &[AdStructure],
    ) -> Result<Self, Error> {
        // TODO what's the difference between "general" and "limited" discoverability?
        Self::adv(
            PduType::AdvInd,
            advertiser_addr,
            &mut iter::once(&AdStructure::from(Flags::discoverable())).chain(advertiser_data),
        )
    }

    /// Creates a scan request PDU.
    ///
    /// Note that scanning is not yet implemented.
    ///
    /// # Parameters
    ///
    /// * `scanner`: Device address of the device in scanning state (sender of
    ///   the request).
    /// * `adv`: Device address of the advertising device that this scan request
    ///   is directed towards.
    pub fn scan_request(_scanner: DeviceAddress, _adv: DeviceAddress) -> Result<Self, Error> {
        unimplemented!()
    }

    /// Creates a scan response PDU.
    ///
    /// Note that scanning is not yet implemented.
    pub fn scan_response(_adv: DeviceAddress, _scan_data: &[AdStructure]) -> Result<Self, Error> {
        unimplemented!()
    }

    pub fn header(&self) -> Header {
        self.header
    }

    pub fn payload(&self) -> &[u8] {
        let len = self.header.payload_length() as usize;
        &self.payload_buf[..len]
    }
}

impl fmt::Debug for PduBuf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({:?}, {:?})", self.header(), Hex(self.payload()))
    }
}

/// 16-bit Advertising Channel PDU header preceding the Payload.
///
/// The header looks like this:
///
/// ```notrust
/// LSB                                                                     MSB
/// +------------+------------+---------+---------+--------------+------------+
/// |  PDU Type  |     -      |  TxAdd  |  RxAdd  |    Length    |     -      |
/// |  (4 bits)  |  (2 bits)  | (1 bit) | (1 bit) |   (6 bits)   |  (2 bits)  |
/// +------------+------------+---------+---------+--------------+------------+
/// ```
///
/// The `TxAdd` and `RxAdd` field are only used for some payloads, for all others, they should be
/// set to 0.
///
/// Length may be in range 6 to 37 (inclusive). With the 2-Byte header this is exactly the max.
/// on-air packet size.
#[derive(Copy, Clone)]
pub struct Header(u16);

const TXADD_MASK: u16 = 0b00000000_01000000;
const RXADD_MASK: u16 = 0b00000000_10000000;

impl Header {
    /// Creates a new Advertising Channel PDU header specifying the Payload type `ty`.
    pub fn new(ty: PduType) -> Self {
        Header(u8::from(ty) as u16)
    }

    pub fn parse(raw: &[u8]) -> Self {
        Header(LittleEndian::read_u16(&raw))
    }

    /// Returns the raw representation of the header.
    ///
    /// The returned `u16` must be transmitted LSb first as the first 2 octets of the PDU.
    pub fn to_u16(&self) -> u16 {
        self.0
    }

    /// Sets all bits in the header that are set in `mask`.
    fn set_header_bits(&mut self, mask: u16) {
        self.0 |= mask;
    }

    /// Clears all bits in the header that are set in `mask`.
    fn clear_header_bits(&mut self, mask: u16) {
        self.0 &= !mask;
    }

    /// Returns the PDU type specified in the header.
    pub fn type_(&self) -> PduType {
        PduType::from((self.0 & 0b00000000_00001111) as u8)
    }

    /// Returns the state of the `TxAdd` field.
    pub fn tx_add(&self) -> bool {
        self.0 & TXADD_MASK != 0
    }

    /// Sets the `TxAdd` field's value.
    pub fn set_tx_add(&mut self, value: bool) {
        if value {
            self.set_header_bits(TXADD_MASK);
        } else {
            self.clear_header_bits(TXADD_MASK);
        }
    }

    /// Returns the state of the `RxAdd` field.
    pub fn rx_add(&self) -> bool {
        self.0 & RXADD_MASK != 0
    }

    /// Sets the `RxAdd` field's value.
    pub fn set_rx_add(&mut self, value: bool) {
        if value {
            self.set_header_bits(RXADD_MASK);
        } else {
            self.clear_header_bits(RXADD_MASK);
        }
    }

    /// Returns the length of the payload in octets as specified in the `Length` field.
    ///
    /// According to the spec, the length must be in range 6...37, but this isn't checked by this
    /// function.
    pub fn payload_length(&self) -> u8 {
        ((self.0 & 0b00111111_00000000) >> 8) as u8
    }

    /// Sets the payload length of this PDU.
    ///
    /// The `length` must be in range 6...37, otherwise this function panics.
    pub fn set_payload_length(&mut self, length: u8) {
        assert!(6 <= length && length <= 37);

        let header = self.0 & !0b00111111_00000000;
        self.0 = header | ((length as u16) << 8);
    }
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Header")
            .field("PDU Type", &self.type_())
            .field("TxAdd", &self.tx_add())
            .field("RxAdd", &self.rx_add())
            .field("len", &self.payload_length())
            .finish()
    }
}

impl<'a> FromBytes<'a> for Header {
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error> {
        let raw = bytes.read_u16::<LittleEndian>().ok_or(Error::Eof)?;
        Ok(Header(raw))
    }
}

impl ToBytes for Header {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u16::<LittleEndian>(self.0)
    }
}

enum_with_unknown! {
    /// 4-bit PDU type in [`Header`].
    ///
    /// For more details, see [`PduBuf`].
    ///
    /// [`Header`]: struct.Header.html
    /// [`PduBuf`]: struct.PduBuf.html
    #[derive(Debug)]
    pub enum PduType(u8) {
        /// Connectable undirected advertising event (`ADV_IND`).
        AdvInd = 0b0000,
        /// Connectable directed advertising event (`ADV_DIRECT_IND`).
        AdvDirectInd = 0b0001,
        /// Non-connectable undirected advertising event (`ADV_NONCONN_IND`).
        AdvNonconnInd = 0b0010,
        /// Scannable undirected advertising event (`ADV_SCAN_IND`).
        AdvScanInd = 0b0110,

        /// Scan request (`SCAN_REQ`).
        ///
        /// Sent by device in Scanning State, received by device in Advertising
        /// State.
        ScanReq = 0b0011,
        /// Scan response (`SCAN_RSP`).
        ///
        /// Sent by device in Advertising State, received by devicein Scanning
        /// State.
        ScanRsp = 0b0100,
        /// Connect request (`CONNECT_REQ`).
        ///
        /// Sent by device in Initiating State, received by device in
        /// Advertising State.
        ConnectReq = 0b0101,
    }
}

impl PduType {
    /// Whether AD structures can follow the fixed data in a PDU of this type.
    fn allows_adv_data(&self) -> bool {
        match self {
            PduType::AdvInd | PduType::AdvNonconnInd | PduType::AdvScanInd | PduType::ScanRsp => {
                true
            }
            PduType::AdvDirectInd
            | PduType::ScanReq
            | PduType::ConnectReq
            | PduType::Unknown(_) => false,
        }
    }
}
