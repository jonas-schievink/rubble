//! The LE Security Manager protocol.
//!
//! The Security Manager is a mandatory part of BLE and is connected to L2CAP channel `0x0006` when
//! the Link-Layer connection is established.

use {
    crate::ble::{
        l2cap::{L2CAPResponder, Protocol, ProtocolObj},
        utils::HexSlice,
        Error,
    },
    log::warn,
};

pub struct SecurityManager {}

impl SecurityManager {
    pub fn new() -> Self {
        Self {}
    }
}

impl ProtocolObj for SecurityManager {
    fn process_message(&mut self, message: &[u8], _responder: L2CAPResponder) -> Result<(), Error> {
        warn!("[NYI] security manager; message = {:?}", HexSlice(message));
        Ok(())
    }
}

impl Protocol for SecurityManager {
    const RSP_PDU_SIZE: u8 = 40;
}
