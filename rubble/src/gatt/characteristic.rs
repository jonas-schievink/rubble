use {
    crate::{att::AttUuid, uuid::Uuid16},
    bitflags::bitflags,
};

bitflags! {
    pub struct Properties: u8 {
        const BROADCAST    = 0x01;
        const READ         = 0x02;
        const WRITE_NO_RSP = 0x04;
        const WRITE        = 0x08;
        const NOTIFY       = 0x10;
        const INDICATE     = 0x20;
        const AUTH_WRITES  = 0x40;
        const EXTENDED     = 0x80;
    }
}

pub trait Characteristic {
    const PROPS: Properties;

    /// The UUID assigned to the characteristic type.
    const UUID: AttUuid;
}

pub struct BatteryLevel {
    /// Battery level in percent (0-100).
    percentage: u8,
}

impl BatteryLevel {
    pub fn new(percentage: u8) -> Self {
        assert!(percentage <= 100);
        Self { percentage }
    }

    pub fn percentage(&self) -> u8 {
        self.percentage
    }
}

impl Characteristic for BatteryLevel {
    const PROPS: Properties = Properties::READ;
    const UUID: AttUuid = AttUuid::Uuid16(Uuid16(0x2A19));
}
