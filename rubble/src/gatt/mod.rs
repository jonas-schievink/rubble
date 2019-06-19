//! Implementation of the Generic Attribute Profile (GATT).
//!
//! GATT describes a service framework that uses the Attribute Protocol for discovery and
//! interaction

pub mod characteristic;

use crate::{
    att::{AttHandle, AttPermission, AttUuid, Attribute, AttributeProvider},
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
pub struct GattServer<'a> {
    _services: &'a [Service<'a>],
    attributes: [Attribute<'a>; 44],
}

impl<'a> GattServer<'a> {
    pub fn new() -> Self {
        Self {
            _services: &[],
            attributes: [
                // http://dev.ti.com/tirex/content/simplelink_cc2640r2_sdk_1_40_00_45/docs/blestack/ble_user_guide/html/ble-stack-3.x/gatt.html#gatt-characteristics-and-attributes

                // Profile declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                    handle: AttHandle::from_raw(0x0001),
                    value: HexSlice(&[0x00, 0x18]),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0002),
                    value: HexSlice(&[0x02, 0x03, 0x00, 0x00, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0003),
                    value: HexSlice(b"Rubble Peripheral"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0004),
                    value: HexSlice(&[0x00, 0x00]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A01)),
                    handle: AttHandle::from_raw(0x0005),
                    value: HexSlice(b"Simple BLE Peripheral"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0006),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A04)),
                    handle: AttHandle::from_raw(0x0007),
                    value: HexSlice(&[0x50, 0x00, 0xA0, 0x00, 0x00, 0x00, 0xE8, 0x03]),
                    permission: AttPermission::default(),
                },

                // Profile declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                    handle: AttHandle::from_raw(0x0008),
                    value: HexSlice(&[0x01, 0x18]),
                    permission: AttPermission::default(),
                },

                // Profile declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                    handle: AttHandle::from_raw(0x0009),
                    value: HexSlice(&[0x0A, 0x18]),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x000A),
                    value: HexSlice(&[0x02, 0x0B, 0x00, 0x23, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x000B),
                    value: HexSlice(&[0x88, 0xA9, 0x08, 0x00, 0x00, 0x0B, 0xC9, 0x68]),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x000C),
                    value: HexSlice(&[0x02, 0x0D, 0x00, 0x24, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x000D),
                    value: HexSlice(b"Model number"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x000E),
                    value: HexSlice(&[0x02, 0x0F, 0x00, 0x25, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x000F),
                    value: HexSlice(b"Serial number"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0010),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0011),
                    value: HexSlice(b"Firmware revision"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0012),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0013),
                    value: HexSlice(b"Hardware revision"),
                    permission: AttPermission::default(),
                },
                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0014),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0015),
                    value: HexSlice(b"Software revision"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0016),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0017),
                    value: HexSlice(b"Manufacturer name"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0018),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0019),
                    value: HexSlice(b"regulatory_cert"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x001A),
                    value: HexSlice(&[0x02, 0x07, 0x00, 0x04, 0x2A]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x001B),
                    value: HexSlice(&[0x01, 0x0D, 0x00, 0x00, 0x00, 0x10, 0x01]),
                    permission: AttPermission::default(),
                },

                // Profile declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                    handle: AttHandle::from_raw(0x01C),
                    value: HexSlice(&[0x5D, 0xFE]),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x001D),
                    value: HexSlice(&[0x0A, 0x1E, 0x00, 0xF1, 0xFF]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x001E),
                    value: HexSlice(b"1"),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x001F),
                    value: HexSlice(b"Characteristic 1"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0020),
                    value: HexSlice(&[0x02, 0x21, 0x00, 0xF2, 0xFF]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0021),
                    value: HexSlice(b"2"),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x0022),
                    value: HexSlice(b"Characteristic 2"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0022),
                    value: HexSlice(&[0x08, 0x24, 0x00, 0xF3, 0xFF]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0024),
                    value: HexSlice(b""),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x0025),
                    value: HexSlice(b"Characteristic 3"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x0026),
                    value: HexSlice(&[0x10, 0x27, 0x00, 0xF4, 0xFF]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x0027),
                    value: HexSlice(b"4"),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x0028),
                    value: HexSlice(&[0x00, 0x00]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x0029),
                    value: HexSlice(b"Characteristic 4"),
                    permission: AttPermission::default(),
                },

                // Characteristic declaration
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2803)),
                    handle: AttHandle::from_raw(0x002A),
                    value: HexSlice(&[0x0A, 0x1E, 0x00, 0xF1, 0xFF]),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A00)),
                    handle: AttHandle::from_raw(0x002B),
                    value: HexSlice(b"5"),
                    permission: AttPermission::default(),
                },
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2901)),
                    handle: AttHandle::from_raw(0x002C),
                    value: HexSlice(b"Characteristic 5"),
                    permission: AttPermission::default(),
                },
            ],
        }
    }
}

impl<'a> AttributeProvider for GattServer<'a> {
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
