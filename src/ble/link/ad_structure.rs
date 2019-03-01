//! Advertising Data / Extended Inquiry Response (EIR) data.
//!
//! Part of GAP (Generic Access Profile).
//!
//! Also see the [assigned numbers document][gap] hosted by the SIG.
//!
//! [gap]: https://www.bluetooth.com/specifications/assigned-numbers/generic-access-profile

use {
    crate::ble::{
        bytes::*,
        utils::{MutSliceExt, SliceExt},
        uuid::{IsUuid, UuidKind},
        Error,
    },
    bitflags::bitflags,
};

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
    /// Lowers this AD structure into a Byte buffer.
    ///
    /// Returns the number of Bytes of `buf` that are used by this AD structure.
    pub fn lower(&self, buf: &'a mut [u8]) -> Result<usize, Error> {
        // First Byte = Length of record. Start encoding at offset 1, write length later.
        let (first, mut buf) = match buf.split_first_mut() {
            None => return Err(Error::Eof),
            Some(s) => s,
        };

        // Write the type tag and data, returning length of the data written (w/o the type byte)
        let len = match *self {
            AdStructure::Flags(ref flags) => {
                buf.write_byte(Type::FLAGS)?;
                buf.write_byte(flags.to_u8())?;
                1
            }
            AdStructure::ServiceUuids16 { incomplete, uuids } => {
                eof_unless!(uuids.len() < 127);
                buf.write_byte(if incomplete {
                    Type::INCOMPLETE_LIST_OF_16BIT_SERVICE_UUIDS
                } else {
                    Type::COMPLETE_LIST_OF_16BIT_SERVICE_UUIDS
                })?;

                eof_unless!(buf.len() >= uuids.len() * 2);
                for (dst, &src) in buf.chunks_mut(2).zip(uuids) {
                    dst[0] = src as u8;
                    dst[1] = (src >> 8) as u8;
                }

                uuids.len() as u8 * 2
            }
            AdStructure::ServiceData16 { uuid, data } => {
                assert!(data.len() < 255);
                buf.write_byte(Type::SERVICE_DATA_16BIT_UUID)?;
                buf.write_byte(uuid as u8)?;
                buf.write_byte((uuid >> 8) as u8)?;
                buf.write_slice(data)?;

                data.len() as u8 + 2
            }
            AdStructure::CompleteLocalName(name) => {
                assert!(name.len() < 255);
                buf.write_byte(Type::COMPLETE_LOCAL_NAME)?;
                buf.write_slice(name.as_bytes())?;

                name.len() as u8
            }
            AdStructure::ShortenedLocalName(name) => {
                assert!(name.len() < 255);
                buf.write_byte(Type::SHORTENED_LOCAL_NAME)?;
                buf.write_slice(name.as_bytes())?;

                name.len() as u8
            }
            AdStructure::__Nonexhaustive => unreachable!(),
        };

        *first = len + 1; // + 1 for the Type field
        Ok(len as usize + 2) // + Type field and prefix length byte
    }
}

pub struct ServiceUuids<'a, T: IsUuid> {
    complete: bool,
    data: BytesOr<'a, [T]>,
}

impl<'a, T: IsUuid> ServiceUuids<'a, T> {
    pub fn from_uuids(complete: bool, uuids: &'a [T]) -> Self {
        Self {
            complete,
            data: BytesOr::from_ref(uuids),
        }
    }

    /// Returns a boolean indicating whether this list is complete.
    ///
    /// If this returns `false`, the device offers more services not contained
    /// in this list.
    // FIXME figure out if/how GATT services are related to this
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Returns an iterator over the UUIDs stored in `self`.
    pub fn iter(&self) -> impl Iterator<Item = T> + 'a {
        self.data.iter()
    }

    fn type_(&self) -> u8 {
        match (T::KIND, self.complete) {
            (UuidKind::Uuid16, true) => Type::COMPLETE_LIST_OF_16BIT_SERVICE_UUIDS,
            (UuidKind::Uuid16, false) => Type::INCOMPLETE_LIST_OF_16BIT_SERVICE_UUIDS,
            (UuidKind::Uuid32, true) => Type::COMPLETE_LIST_OF_32BIT_SERVICE_UUIDS,
            (UuidKind::Uuid32, false) => Type::INCOMPLETE_LIST_OF_32BIT_SERVICE_UUIDS,
            (UuidKind::Uuid128, true) => Type::COMPLETE_LIST_OF_128BIT_SERVICE_UUIDS,
            (UuidKind::Uuid128, false) => Type::INCOMPLETE_LIST_OF_128BIT_SERVICE_UUIDS,
        }
    }
}

/// Decodes `ServiceUuids` from a byte sequence containing:
///
/// * **`TYPE`**: The right "(In)complete List of N-bit Service Class UUIDs"
///   type. Both the complete and incomplete type are accepted.
/// * **`UUID`**...: n*2/4/16 Bytes of UUID data, in *little* endian.
impl<'a, T: IsUuid> FromBytes<'a> for ServiceUuids<'a, T> {
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error> {
        let (t_complete, t_incomplete) = match T::KIND {
            UuidKind::Uuid16 => (
                Type::COMPLETE_LIST_OF_16BIT_SERVICE_UUIDS,
                Type::INCOMPLETE_LIST_OF_16BIT_SERVICE_UUIDS,
            ),
            UuidKind::Uuid32 => (
                Type::COMPLETE_LIST_OF_32BIT_SERVICE_UUIDS,
                Type::INCOMPLETE_LIST_OF_32BIT_SERVICE_UUIDS,
            ),
            UuidKind::Uuid128 => (
                Type::COMPLETE_LIST_OF_128BIT_SERVICE_UUIDS,
                Type::INCOMPLETE_LIST_OF_128BIT_SERVICE_UUIDS,
            ),
        };

        let ty = bytes.read_first().ok_or(Error::Eof)?;
        let complete = if ty == t_complete {
            true
        } else if ty == t_incomplete {
            false
        } else {
            return Err(Error::InvalidValue);
        };

        Ok(Self {
            complete,
            data: BytesOr::from_bytes(bytes)?,
        })
    }
}

impl<'a, T: IsUuid> ToBytes for ServiceUuids<'a, T> {
    fn space_needed(&self) -> usize {
        1 + self.data.space_needed()
    }

    fn to_bytes(&self, buffer: &mut &mut [u8]) -> Result<(), Error> {
        buffer.write_byte(self.type_())?;
        self.data.to_bytes(buffer)
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

/// Data Type constants.
///
/// https://www.bluetooth.com/specifications/assigned-numbers/generic-access-profile
enum Type {}

#[allow(unused)]
impl Type {
    const FLAGS: u8 = 0x01;
    const INCOMPLETE_LIST_OF_16BIT_SERVICE_UUIDS: u8 = 0x02;
    const COMPLETE_LIST_OF_16BIT_SERVICE_UUIDS: u8 = 0x03;
    const INCOMPLETE_LIST_OF_32BIT_SERVICE_UUIDS: u8 = 0x04;
    const COMPLETE_LIST_OF_32BIT_SERVICE_UUIDS: u8 = 0x05;
    const INCOMPLETE_LIST_OF_128BIT_SERVICE_UUIDS: u8 = 0x06;
    const COMPLETE_LIST_OF_128BIT_SERVICE_UUIDS: u8 = 0x07;
    const SHORTENED_LOCAL_NAME: u8 = 0x08;
    const COMPLETE_LOCAL_NAME: u8 = 0x09;
    const TX_POWER_LEVEL: u8 = 0x0A;
    const CLASS_OF_DEVICE: u8 = 0x0D;
    const SIMPLE_PAIRING_HASH_C: u8 = 0x0E;
    const SIMPLE_PAIRING_HASH_C192: u8 = 0x0E;
    const SIMPLE_PAIRING_RANDOMIZER_R: u8 = 0x0F;
    const SIMPLE_PAIRING_RANDOMIZER_R192: u8 = 0x0F;
    const DEVICE_ID: u8 = 0x10;
    const SECURITY_MANAGER_TK_VALUE: u8 = 0x10;
    const SECURITY_MANAGER_OUT_OF_BAND_FLAGS: u8 = 0x11;
    const SLAVE_CONNECTION_INTERVAL_RANGE: u8 = 0x12;
    const LIST_OF_16BIT_SERVICE_SOLICITATION_UUIDS: u8 = 0x14;
    const LIST_OF_128BIT_SERVICE_SOLICITATION_UUIDS: u8 = 0x15;
    const SERVICE_DATA: u8 = 0x16;
    const SERVICE_DATA_16BIT_UUID: u8 = 0x16;
    const PUBLIC_TARGET_ADDRESS: u8 = 0x17;
    const RANDOM_TARGET_ADDRESS: u8 = 0x18;
    const APPEARANCE: u8 = 0x19;
    const ADVERTISING_INTERVAL: u8 = 0x1A;
    const LE_BLUETOOTH_DEVICE_ADDRESS: u8 = 0x1B;
    const LE_ROLE: u8 = 0x1C;
    const SIMPLE_PAIRING_HASH_C256: u8 = 0x1D;
    const SIMPLE_PAIRING_RANDOMIZER_R256: u8 = 0x1E;
    const LIST_OF_32BIT_SERVICE_SOLICITATION_UUIDS: u8 = 0x1F;
    const SERVICE_DATA_32BIT_UUID: u8 = 0x20;
    const SERVICE_DATA_128BIT_UUID: u8 = 0x21;
    const LE_SECURE_CONNECTIONS_CONFIRMATION_VALUE: u8 = 0x22;
    const LE_SECURE_CONNECTIONS_RANDOM_VALUE: u8 = 0x23;
    const URI: u8 = 0x24;
    const INDOOR_POSITIONING: u8 = 0x25;
    const TRANSPORT_DISCOVERY_DATA: u8 = 0x26;
    const LE_SUPPORTED_FEATURES: u8 = 0x27;
    const CHANNEL_MAP_UPDATE_INDICATION: u8 = 0x28;
    const PB_ADV: u8 = 0x29;
    const MESH_MESSAGE: u8 = 0x2A;
    const MESH_BEACON: u8 = 0x2B;
    const THREE_D_INFORMATION_DATA: u8 = 0x3D;
    const _3D_INFORMATION_DATA: u8 = 0x3D;
    const MANUFACTURER_SPECIFIC_DATA: u8 = 0xFF;
}
