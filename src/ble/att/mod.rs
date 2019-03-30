//! Implementation of the Attribute Protocol (ATT).

use crate::ble::{
    l2cap::{L2CAPResponder, Protocol},
    link::queue::Consume,
};

pub struct AttributeServer {}

impl AttributeServer {
    pub fn empty() -> Self {
        Self {}
    }
}

impl Protocol for AttributeServer {
    fn process_message(&mut self, _message: &[u8], _responder: L2CAPResponder) -> Consume<()> {
        unimplemented!()
    }
}
