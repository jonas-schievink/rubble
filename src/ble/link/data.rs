//! Data Channel operations.

pub struct Header(u16);

impl Header {
    /// Returns the raw representation of the header.
    ///
    /// The returned `u16` must be transmitted LSB ans LSb first as the first 2 octets of the PDU.
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
