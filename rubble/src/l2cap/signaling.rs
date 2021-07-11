//! L2CAP Signaling channel PDUs and functions (`0x0005`).

use super::{Protocol, ProtocolObj, Sender};
use crate::Error;

enum_with_unknown! {
    /// LE Signaling Channel opcodes.
    #[derive(Debug, Copy, Clone, defmt::Format)]
    enum Code(u8) {
        CommandReject = 0x01,
        DisconnectionReq = 0x06,
        DisconnectionRsp = 0x07,
        ConnectionParameterUpdateReq = 0x12,
        ConnectionParameterUpdateRsp = 0x13,
        CreditBasedConnectionReq = 0x14,
        CreditBasedConnectionRsp = 0x15,
        FlowControlCredit = 0x16,
    }
}

enum_with_unknown! {
    /// Reasons for a `CommandReject` response.
    #[derive(Debug, Copy, Clone, defmt::Format)]
    enum RejectReason(u16) {
        CommandNotUnderstood = 0x0000,
        SignalingMtuExceeded = 0x0001,
        InvalidCid = 0x0002,
    }
}

/// The `Protocol` implementor listening on the LE Signaling Channel `0x0005`.
pub struct SignalingState {}

impl SignalingState {
    pub fn new() -> Self {
        Self {}
    }
}

impl ProtocolObj for SignalingState {
    fn process_message(&mut self, message: &[u8], responder: Sender<'_>) -> Result<(), Error> {
        let _ = (message, responder);
        unimplemented!();
    }
}

impl Protocol for SignalingState {
    const RSP_PDU_SIZE: u8 = 23;
}
