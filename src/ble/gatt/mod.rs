//! Implementation of the Generic Attribute Profile (GATT).
//!
//! GATT describes a service framework that uses the Attribute Protocol for discovery and
//! interaction

use {
    crate::ble::{
        att::{AttHandle, AttPermission, AttUuid, Attribute, Attributes},
        utils::HexSlice,
        uuid::Uuid16,
    },
    core::default::Default,
};

/// A collection of data and associated behaviors to accomplish a particular function or feature
///
/// There are two types of services:
/// * Primary services expose the primary usable functionality of a device
/// * Secondary services are only intended to be referenced from another service or higher level
///   specification
pub trait Service {
    fn get_type(&self) -> ServiceType;
}

pub enum ServiceType {
    Primary,
    Secondary,
}

/// A characteristic is a value used in a service along with properties and configuration
/// information about how the value is accessed and information about how the value is displayed
/// or represented
pub trait Characteristic {}

/// A GATT server to run on top of an ATT server
pub struct GattServer<'a, S: Service> {
    _services: &'a [S],
    temp: [Attribute<'static>; 1],
}

impl<'a, S: Service> GattServer<'a, S> {
    pub fn new() -> Self {
        Self {
            _services: &[],
            temp: [Attribute {
                att_type: AttUuid::Uuid16(Uuid16(0x2800)),
                handle: AttHandle::from_raw(0x1234),
                value: HexSlice(&[0xCD, 0xAB]),
                permission: AttPermission::default(),
            }],
        }
    }
}

impl<S: Service> Attributes for GattServer<'static, S> {
    fn attributes(&self) -> &[Attribute] {
        &self.temp
    }
}

pub struct PrimaryService {
    _uuid: AttUuid,
}

impl Service for PrimaryService {
    fn get_type(&self) -> ServiceType {
        ServiceType::Primary
    }
}
