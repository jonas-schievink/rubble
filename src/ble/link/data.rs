//! Data Channel operations.

/// 16-bit data channel header preceding the payload.
///
/// Layout:
///
/// ```notrust
/// LSB                                                                           MSB
/// +----------+---------+---------+---------+------------+--------------+----------+
/// |   LLID   |  NESN   |   SN    |   MD    |     -      |    Length    |    -     |
/// | (2 bits) | (1 bit) | (1 bit) | (1 bit) |  (3 bits)  |   (5 bits)   | (3 bits) |
/// +----------+---------+---------+---------+------------+--------------+----------+
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
/// The `Length` field specifies the length of payload **and `MIC`**. Its maximum value is 31,
/// resulting in a 27 octet Payload (the maximum) and a 32-bit `MIC`.
///
/// Note that the `Length` field is 1 bit shorter than for Advertising Channel PDUs.
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
    /// Returns the raw representation of the header.
    ///
    /// The returned `u16` must be transmitted LSB and LSb first as the first 2 octets of the PDU.
    pub fn to_u16(&self) -> u16 {
        self.0
    }

    /// Returns the length of the payload in octets as specified in the `Length` field.
    ///
    /// According to the spec, the length must be in range 6...37, but this isn't checked by this
    /// function.
    pub fn payload_length(&self) -> u8 {
        // Subtle difference to advertising header: 5 bits, not 6!
        ((self.0 & 0b00011111_00000000) >> 8) as u8
    }
}
