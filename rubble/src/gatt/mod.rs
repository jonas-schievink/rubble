//! Implementation of the Generic Attribute Profile (GATT).
//!
//! GATT describes a service framework that uses the Attribute Protocol for discovery and
//! interaction

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
    attributes: [Attribute<'a>; 1],
}

impl<'a> GattServer<'a> {
    pub fn new() -> Self {
        Self {
            _services: &[],
            attributes: [Attribute {
                att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                handle: AttHandle::from_raw(0x0001),
                value: HexSlice(&[0xCD, 0xAB]),
                permission: AttPermission::default(),
            }],
        }
    }
}

impl<'a> AttributeProvider for GattServer<'a> {
    fn for_each_attr(
        &mut self,
        f: &mut dyn FnMut(&mut Attribute) -> Result<(), Error>,
    ) -> Result<(), Error> {
        for att in &mut self.attributes {
            f(att)?;
        }
        Ok(())
    }

    fn is_grouping_attr(&self, uuid: AttUuid) -> bool {
        uuid == Uuid16(0x2800)
    }

    fn group_end(&self, handle: AttHandle) -> Option<&Attribute> {
        for att in &self.attributes {
            if att.handle == handle && att.att_type == Uuid16(0x2800) {
                return Some(att);
            }
        }

        None
    }
}
