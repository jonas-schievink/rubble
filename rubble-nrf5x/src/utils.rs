//! Useful utilities related to Rubble on the nRF52.

use crate::pac;
use crate::pac::ficr::deviceaddrtype::DEVICEADDRTYPE_A;
use rubble::link::{AddressKind, DeviceAddress};

/// Return the `DeviceAddress`, which is pre-programmed in the device FICR
/// (Factory information configuration registers).
pub fn get_device_address() -> DeviceAddress {
    // FICR is read-only, so accessing it directly should be safe
    let ficr = unsafe { &*pac::FICR::ptr() };

    // Address bytes
    let mut devaddr = [0u8; 6];
    let devaddr_lo = ficr.deviceaddr[0].read().bits();
    let devaddr_hi = ficr.deviceaddr[1].read().bits() as u16;
    devaddr[..4].copy_from_slice(&devaddr_lo.to_le_bytes());
    devaddr[4..].copy_from_slice(&devaddr_hi.to_le_bytes());

    // Address type
    let devaddr_type = match ficr.deviceaddrtype.read().deviceaddrtype().variant() {
        DEVICEADDRTYPE_A::PUBLIC => AddressKind::Public,
        DEVICEADDRTYPE_A::RANDOM => AddressKind::Random,
    };

    DeviceAddress::new(devaddr, devaddr_type)
}
