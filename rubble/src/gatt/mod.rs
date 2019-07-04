//! Implementation of the Generic Attribute Profile (GATT).
//!
//! GATT describes a service framework that uses the Attribute Protocol for discovery and
//! interaction

pub mod characteristic;

use crate::{
    att::{AttHandle, AttUuid, Attribute, AttributeProvider},
    utils::HexSlice,
    uuid::Uuid16,
    Error,
};

/// A collection of data and associated behaviors to accomplish a particular function or feature
///
/// There are two types of services:
/// * Primary services expose the primary usable functionality of a device
/// * Secondary services are only intended to be referenced from another service or higher level
///   specification
pub struct Service<'a> {
    _uuid: AttUuid,
    _service_type: ServiceType,
    _includes: Option<&'a [Service<'a>]>,
    _characteristics: Option<&'a [Characteristic]>,
}

pub enum ServiceType {
    Primary,
    Secondary,
}

impl<'a> Service<'a> {
    pub fn as_attributes(&self) -> &[Attribute<'a>] {
        &[]
    }
}

/// A characteristic is a value used in a service along with properties and configuration
/// information about how the value is accessed and information about how the value is displayed
/// or represented
pub struct Characteristic {}

/// A GATT server to run on top of an ATT server
///
/// TODO: This is all temporary and need to offer a better interface for defining services
pub struct GattServerSimple<'a> {
    attributes: [Attribute<'a>; 3],
}

impl<'a> GattServerSimple<'a> {
    pub fn new() -> Self {
        Self {
            attributes: [
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                    handle: AttHandle::from_raw(0x0001),
                    value: HexSlice(&[0x0F, 0x18]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0002),
                    // 1 byte properties = 0x02
                    // 2 bytes handle = 0x0003
                    // 2 bytes UUID = 0x2A19
                    value: HexSlice(&[0x2A, 0x19, 0x00, 0x07, 0x02]),
                },
                // Characteristic declaration (Battery Level value)
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A19)),
                    handle: AttHandle::from_raw(0x0003),
                    // 48%
                    value: HexSlice(&[41u8]),
                },
                /*
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0002),
                    value: HexSlice(&[0x02, 0x03, 0x00, 0x00, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0003),
                    value: HexSlice(b"Device name here"),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0004),
                    value: HexSlice(&[0x02, 0x05, 0x00, 0x01, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A01)),
                    handle: AttHandle::from_raw(0x0005),
                    value: HexSlice(&[0x00, 0x00]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0006),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x01, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A04)),
                    handle: AttHandle::from_raw(0x0007),
                    value: HexSlice(&[0x50, 0x00, 0xA0, 0x00, 0x00, 0x00, 0xE8, 0x03]),
                },
                */
                /*
                // Characteristic definition (Battery Level)
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0006),
                    // 1 byte properties = 0x02
                    // 2 bytes handle = 0x0003
                    // 2 bytes UUID = 0x2A19
                    value: HexSlice(&[0x2A, 0x19, 0x00, 0x07, 0x02]),
                },
                // Characteristic declaration (Battery Level value)
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A19)),
                    handle: AttHandle::from_raw(0x0007),
                    // 48%
                    value: HexSlice(&[41u8]),
                },
                */
            ],
        }
    }
}

impl<'a> AttributeProvider for GattServerSimple<'a> {
    fn for_each_attr(
        &mut self,
        f: &mut dyn FnMut(&mut Attribute<'_>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        for att in &mut self.attributes {
            f(att)?;
        }
        Ok(())
    }

    fn is_grouping_attr(&self, uuid: AttUuid) -> bool {
        uuid == Uuid16(0x2800)
    }

    fn group_end(&self, handle: AttHandle) -> Option<&Attribute<'_>> {
        for att in &self.attributes {
            if att.handle == handle && att.att_type == Uuid16(0x2800) {
                return Some(att);
            }
        }

        None
    }
}

/// A GATT server to run on top of an ATT server
///
/// TODO: This is all temporary and need to offer a better interface for defining services
pub struct GattServerComplex<'a> {
    attributes: [Attribute<'a>; 44],
}

impl<'a> GattServerComplex<'a> {
    pub fn new() -> Self {
        Self {
            attributes: [
                // http://dev.ti.com/tirex/content/simplelink_cc2640r2_sdk_1_40_00_45/docs/blestack/ble_user_guide/html/ble-stack-3.x/gatt.html#gatt-characteristics-and-attributes

                // Profile declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                    handle: AttHandle::from_raw(0x0001),
                    value: HexSlice(&[0x00, 0x18]),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0002),
                    value: HexSlice(&[0x02, 0x03, 0x00, 0x00, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0003),
                    value: HexSlice(b"Rubble Peripheral"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0004),
                    value: HexSlice(&[0x00, 0x00]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A01)),
                    handle: AttHandle::from_raw(0x0005),
                    value: HexSlice(b"Simple BLE Peripheral"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0006),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A04)),
                    handle: AttHandle::from_raw(0x0007),
                    value: HexSlice(&[0x50, 0x00, 0xA0, 0x00, 0x00, 0x00, 0xE8, 0x03]),
                },
                // Profile declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                    handle: AttHandle::from_raw(0x0008),
                    value: HexSlice(&[0x01, 0x18]),
                },
                // Profile declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                    handle: AttHandle::from_raw(0x0009),
                    value: HexSlice(&[0x0A, 0x18]),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x000A),
                    value: HexSlice(&[0x02, 0x0B, 0x00, 0x23, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x000B),
                    value: HexSlice(&[0x88, 0xA9, 0x08, 0x00, 0x00, 0x0B, 0xC9, 0x68]),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x000C),
                    value: HexSlice(&[0x02, 0x0D, 0x00, 0x24, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x000D),
                    value: HexSlice(b"Model number"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x000E),
                    value: HexSlice(&[0x02, 0x0F, 0x00, 0x25, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x000F),
                    value: HexSlice(b"Serial number"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0010),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0011),
                    value: HexSlice(b"Firmware revision"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0012),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0013),
                    value: HexSlice(b"Hardware revision"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0014),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0015),
                    value: HexSlice(b"Software revision"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0016),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0017),
                    value: HexSlice(b"Manufacturer name"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0018),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0019),
                    value: HexSlice(b"regulatory_cert"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x001A),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x001B),
                    value: HexSlice(&[0x01, 0x0D, 0x00, 0x00, 0x00, 0x10, 0x01]),
                },
                // Profile declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                    handle: AttHandle::from_raw(0x01C),
                    value: HexSlice(&[0x5D, 0xFE]),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x001D),
                    value: HexSlice(&[0x0A, 0x1E, 0x00, 0xF1, 0xFF]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x001E),
                    value: HexSlice(b"1"),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x001F),
                    value: HexSlice(b"Characteristic 1"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0020),
                    value: HexSlice(&[0x02, 0x21, 0x00, 0xF2, 0xFF]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0021),
                    value: HexSlice(b"2"),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x0022),
                    value: HexSlice(b"Characteristic 2"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0022),
                    value: HexSlice(&[0x08, 0x24, 0x00, 0xF3, 0xFF]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0024),
                    value: HexSlice(b""),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x0025),
                    value: HexSlice(b"Characteristic 3"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0026),
                    value: HexSlice(&[0x10, 0x27, 0x00, 0xF4, 0xFF]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0027),
                    value: HexSlice(b"4"),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x0028),
                    value: HexSlice(&[0x00, 0x00]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x0029),
                    value: HexSlice(b"Characteristic 4"),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x002A),
                    value: HexSlice(&[0x0A, 0x1E, 0x00, 0xF1, 0xFF]),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x002B),
                    value: HexSlice(b"5"),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x002C),
                    value: HexSlice(b"Characteristic 5"),
                },
            ],
        }
    }
}

impl<'a> AttributeProvider for GattServerComplex<'a> {
    fn for_each_attr(
        &mut self,
        f: &mut dyn FnMut(&mut Attribute<'_>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        for att in &mut self.attributes[..] {
            f(att)?;
        }
        Ok(())
    }

    fn is_grouping_attr(&self, uuid: AttUuid) -> bool {
        uuid == Uuid16(0x2800)
    }

    fn group_end(&self, handle: AttHandle) -> Option<&Attribute<'_>> {
        for att in &self.attributes[..] {
            if att.handle == handle && att.att_type == Uuid16(0x2800) {
                return Some(att);
            }
        }

        None
    }
}
