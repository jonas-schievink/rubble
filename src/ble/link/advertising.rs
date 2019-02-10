//! Advertising channel operations.
//!
//! This module defines PDUs, states and fields used by packets transmitted on the advertising
//! channels. Generally, this includes everything needed to advertise as and scan for slave devices
//! and to establish connections.
//!
//! Note that while the types in here do not completely eliminate illegal values to be created, they
//! do employ a range of sanity checks that prevent bogus packets from being sent by the stack.

use super::DeviceAddress;
use super::ad_structure::AdStructure;

use byteorder::{ByteOrder, LittleEndian};
use core::fmt;

/// Higher-level representation of an advertising channel PDU.
pub enum StructuredPdu<'a> {
    /// Connectable undirected advertising event.
    AdvInd {
        /// Advertiser address.
        advertiser_address: DeviceAddress,
        /// Advertising data (may be empty). Up to 31 octets / 15 AD structures.
        advertiser_data: &'a [AdStructure<'a>],
    },

    /// Connectable directed advertising event.
    ///
    /// Sent from an advertiser to an initiator.
    AdvDirectInd {
        advertiser_address: DeviceAddress,
        initiator_address: DeviceAddress,
    },

    /// Non-connectable undirected advertising event.
    AdvNonconnInd {
        advertiser_address: DeviceAddress,
        /// Advertising data (may be empty). Up to 31 octets / 15 AD structures.
        advertiser_data: &'a [AdStructure<'a>],
    },

    /// Scannable undirected advertising event.
    ///
    /// May also be referred to as `ADV_DISCOVER_IND`.
    AdvScanInd {
        advertiser_address: DeviceAddress,
        /// Advertising data (may be empty). Up to 31 octets / 15 AD structures.
        advertiser_data: &'a [AdStructure<'a>],
    },

    /// Scan request.
    ///
    /// Sent by a scanning device, received by an advertising device.
    ScanReq {
        scanner_address: DeviceAddress,
        advertiser_address: DeviceAddress,
    },

    /// Response to a scan request.
    ///
    /// Sent by an advertising device to a scanning device.
    ScanRsp {
        advertiser_address: DeviceAddress,
        /// Response data (may be empty). Up to 31 octets / 15 AD structures.
        scan_response_data: &'a mut [AdStructure<'a>],
    },

    #[doc(hidden)]
    __Nonexhaustive,
}

impl<'a> StructuredPdu<'a> {
    /// Lowers this PDU into a payload buffer and a `Header`, preparing it for transmission.
    ///
    /// The number of Bytes stored in `payload` can be retrieved from the header using
    /// `Header::payload_length`.
    pub fn lower(&self, payload: &mut [u8]) -> Header {
        let ty = match *self {
            StructuredPdu::AdvInd { .. } => PduType::AdvInd,
            StructuredPdu::AdvDirectInd { .. } => PduType::AdvDirectInd,
            StructuredPdu::AdvNonconnInd { .. } => PduType::AdvNonconnInd,
            StructuredPdu::AdvScanInd { .. } => PduType::AdvScanInd,
            StructuredPdu::ScanReq { .. } => PduType::ScanReq,
            StructuredPdu::ScanRsp { .. } => PduType::ScanRsp,
            StructuredPdu::__Nonexhaustive => unreachable!(),
        };

        let mut header = Header::new(ty);

        match *self {
            StructuredPdu::AdvInd { ref advertiser_address, advertiser_data } |
            StructuredPdu::AdvNonconnInd { ref advertiser_address, advertiser_data } |
            StructuredPdu::AdvScanInd { ref advertiser_address, advertiser_data } => {
                payload[0..6].copy_from_slice(advertiser_address.raw());
                let data_buf = &mut payload[6..];
                let mut ad_size = 0;
                for ad in advertiser_data {
                    let bytes = ad.lower(&mut data_buf[ad_size..]);
                    ad_size += bytes;
                }

                assert!(data_buf.len() <= 31);
                assert!(ad_size < 50);  // 50 or something, not very important
                header.set_payload_length(6 + ad_size as u8);
                header.set_tx_add(advertiser_address.is_random());
                header.set_rx_add(false);
            },
            StructuredPdu::AdvDirectInd { ref advertiser_address, ref initiator_address } => {
                header.set_payload_length(6 + 6);
                header.set_tx_add(advertiser_address.is_random());
                header.set_rx_add(initiator_address.is_random());
                payload[0..6].copy_from_slice(advertiser_address.raw());
                payload[6..12].copy_from_slice(initiator_address.raw());
            },
            StructuredPdu::ScanReq { .. } => unimplemented!(),
            StructuredPdu::ScanRsp { .. } => unimplemented!(),
            StructuredPdu::__Nonexhaustive => unreachable!(),
        }

        header
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
/// Length may be in range 6 to 36 (inclusive).
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

enum_with_unknown! {
    /// 4-bit PDU type in `PduHeader`.
    ///
    /// `Adv*` type PDUs are sent while in Advertising state.
    #[derive(Debug)]
    pub enum PduType(u8) {
        /// Connectable undirected advertising event.
        AdvInd = 0b0000,
        /// Connectable directed advertising event.
        AdvDirectInd = 0b0001,
        /// Non-connectable undirected advertising event.
        AdvNonconnInd = 0b0010,
        ScanReq = 0b0011,
        ScanRsp = 0b0100,
        ConnectReq = 0b0101,
        /// Scannable undirected advertising event.
        AdvScanInd = 0b0110,
    }
}
