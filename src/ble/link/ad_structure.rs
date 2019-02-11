//! Advertising Data / Extended Inquiry Response (EIR) data.
//!
//! Part of GAP (Generic Access Profile).

use bitflags::bitflags;

/// A list of AD structures can be sent along with an advertising packet or scan response.
///
/// This mechanism allows a scanner to, for example, receive the device's name without having to
/// establish a connection.
///
/// Unless otherwise noted, each variant of this enum should only be included at most once per
/// packet sent.
///
/// From a very unrepresentative scan, most devices seem to include Flags and Manufacturer Data, and
/// optionally a device name, of course.
#[derive(Debug)]
pub enum AdStructure<'a> {
    /// Device flags and baseband capabilities.
    ///
    /// This should be sent if any flags apply to the device. If not (ie. the value sent would be
    /// 0), this may be omitted.
    ///
    /// Must not be used in scan response data.
    Flags(Flags),

    /// List of 16-bit service UUIDs.
    ///
    /// Only one UUID size class is allowed in a single packet.
    ServiceUuids16 {
        /// Whether this is an incomplete (`true`) or complete (`false`) list of
        /// UUIDs.
        incomplete: bool,
        /// The list of service UUIDs to send.
        uuids: &'a [u16],
    },

    /// Service data with 16-bit service UUID.
    ServiceData16 {
        /// The 16-bit service UUID.
        uuid: u16,
        /// The associated service data. May be empty.
        data: &'a [u8],
    },

    /// Sets the full (unabbreviated) device name.
    ///
    /// This will be shown to the user when this device is found.
    CompleteLocalName(&'a str),

    /// Sets the shortened device name.
    ShortenedLocalName(&'a str),

    #[doc(hidden)]
    __Nonexhaustive,
}

impl<'a> AdStructure<'a> {
    // GAP Data Type Values.
    // https://www.bluetooth.com/specifications/assigned-numbers/generic-access-profile
    const TYPE_FLAGS: u8 = 0x01;
    const TYPE_INCOMPLETE_LIST_OF_16BIT_SERVICE_UUIDS: u8 = 0x02;
    const TYPE_COMPLETE_LIST_OF_16BIT_SERVICE_UUIDS: u8 = 0x03;
    const TYPE_SHORTENED_LOCAL_NAME: u8 = 0x08;
    const TYPE_COMPLETE_LOCAL_NAME: u8 = 0x09;
    const TYPE_SERVICE_DATA_16BIT_UUID: u8 = 0x16;

    /// Lowers this AD structure into a Byte buffer.
    ///
    /// Returns the number of Bytes of `buf` that are used by this AD structure.
    ///
    /// # Panics
    ///
    /// The buffer must have enough space, otherwise this method will panic. Since the buffer in
    /// advertising packets has a fixed size and AD structures are usually constant, a panic
    /// indicates that too much data was attempted to be put into the packet.
    pub fn lower(&self, buf: &'a mut [u8]) -> usize {
        // First Byte = Length of record. Start encoding at offset 1, write length later.
        let len = match *self {
            AdStructure::Flags(ref flags) => {
                buf[1] = Self::TYPE_FLAGS;
                buf[2] = flags.to_u8();
                1
            }
            AdStructure::ServiceUuids16 { incomplete, uuids } => {
                assert!(uuids.len() < 127);
                buf[1] = if incomplete {
                    Self::TYPE_INCOMPLETE_LIST_OF_16BIT_SERVICE_UUIDS
                } else {
                    Self::TYPE_COMPLETE_LIST_OF_16BIT_SERVICE_UUIDS
                };

                for (dst, &src) in buf[2..].chunks_mut(2).zip(uuids) {
                    dst[0] = src as u8;
                    dst[1] = (src >> 8) as u8;
                }

                uuids.len() as u8 * 2
            }
            AdStructure::ServiceData16 { uuid, data } => {
                assert!(data.len() < 255);
                buf[1] = Self::TYPE_SERVICE_DATA_16BIT_UUID;
                buf[2] = uuid as u8;
                buf[3] = (uuid >> 8) as u8;
                buf[4..4 + data.len()].copy_from_slice(data);

                data.len() as u8 + 2
            }
            AdStructure::CompleteLocalName(name) => {
                assert!(name.len() < 255);
                buf[1] = Self::TYPE_COMPLETE_LOCAL_NAME;
                buf[2..name.len() + 2].copy_from_slice(name.as_bytes());
                name.len() as u8
            }
            AdStructure::ShortenedLocalName(name) => {
                assert!(name.len() < 255);
                buf[1] = Self::TYPE_SHORTENED_LOCAL_NAME;
                buf[2..name.len() + 2].copy_from_slice(name.as_bytes());
                name.len() as u8
            }
            AdStructure::__Nonexhaustive => unreachable!(),
        };

        buf[0] = len + 1; // + Type length
        len as usize + 2 // + Type length and prefix length byte
    }
}

bitflags! {
    /// BR/EDR and LE compatibility flags.
    ///
    /// This is mandatory for most devices and can only be omitted if all flags are 0.
    pub struct Flags: u8 {
        const LE_LIMITED_DISCOVERABLE = 0b00000001;
        const LE_GENERAL_DISCOVERABLE = 0b00000010;
        const BR_EDR_NOT_SUPPORTED    = 0b00000100;
        const SIMUL_LE_BR_CONTROLLER  = 0b00001000;
        const SIMUL_LE_BR_HOST        = 0b00010000;
    }
}

impl Flags {
    /// Returns flags suitable for discoverable devices that want to establish a connection.
    ///
    /// The created `Flags` value specifies that this device is not BR/EDR (classic Bluetooth)
    /// capable and is in General Discoverable mode.
    pub fn discoverable() -> Flags {
        Self::BR_EDR_NOT_SUPPORTED | Self::LE_GENERAL_DISCOVERABLE
    }

    /// Returns flags suitable for non-connectable devices that just broadcast advertising packets.
    ///
    /// Creates a `Flags` value that specifies that BR/EDR (classic Bluetooth) is not supported and
    /// that this device is not discoverable.
    pub fn broadcast() -> Flags {
        Self::BR_EDR_NOT_SUPPORTED
    }

    /// Returns the raw representation of the flags.
    pub fn to_u8(&self) -> u8 {
        self.bits()
    }

    /// Returns a boolean indicating whether the device that sent this `Flags` value supports BR/EDR
    /// (aka "Classic Bluetooth").
    pub fn supports_classic_bluetooth(&self) -> bool {
        self.contains(Self::BR_EDR_NOT_SUPPORTED)
    }

    /// Device operating in LE Limited Discoverable mode.
    ///
    /// Either this or `le_general_discoverable()` must be set for the device to be discoverable.
    /// Note that "Broadcast Mode" still works with undiscoverable devices, since it doesn't need
    /// discovery or connections.
    pub fn le_limited_discoverable(&self) -> bool {
        self.contains(Self::LE_LIMITED_DISCOVERABLE)
    }

    /// Device operating in LE General Discoverable mode.
    ///
    /// Either this or `le_limited_discoverable()` must be set for the device to be discoverable.
    /// Note that "Broadcast Mode" still works with undiscoverable devices, since it doesn't need
    /// discovery or connections.
    pub fn le_general_discoverable(&self) -> bool {
        self.contains(Self::LE_GENERAL_DISCOVERABLE)
    }
}

impl<'a> From<Flags> for AdStructure<'a> {
    fn from(flags: Flags) -> Self {
        AdStructure::Flags(flags)
    }
}
