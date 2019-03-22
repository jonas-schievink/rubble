//! Data Channel operations.

use {
    crate::ble::link::SequenceNumber,
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
/// The `MD` field specifies that the device sending the packet has more data to send. When both
/// slave and master send a packet with the `MD` bit set to 0, the connection is closed.
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
pub struct Header(u16);

impl Header {
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
    pub fn nesn(&self) -> SequenceNumber {
        let bit = self.0 & 0b0100;
        if bit == 0 {
            SequenceNumber::zero()
        } else {
            SequenceNumber::one()
        }
    }

    /// Returns the value of the `SN` field (Sequence Number).
    pub fn sn(&self) -> SequenceNumber {
        let bit = self.0 & 0b1000;
        if bit == 0 {
            SequenceNumber::zero()
        } else {
            SequenceNumber::one()
        }
    }

    /// Returns whether the `MD` field is set (More Data).
    pub fn md(&self) -> bool {
        let bit = self.0 & 0b1_0000;
        bit != 0
    }
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
