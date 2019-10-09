//! Defines PDUs, procedures and functionality used by the LE L2CAP signaling channel (`0x0001`).

use super::*;

enum_with_unknown! {
    /// Opcodes allowed on the LE Signaling Channel (CID `0x0005`).
    enum Code(u8) {
        CommandReject = 0x01,
        DisconnectionRequest = 0x06,
        DisconnectionResponse = 0x07,
        ConnectionParameterUpdateRequest = 0x12,
        ConnectionParameterUpdateResponse = 0x13,
        CreditBasedConnectionRequest = 0x14,
        CreditBasedConnectionResponse = 0x15,
        FlowControlCredit = 0x16,
    }
}

struct SignalingPacketHeader {
    code: u8,
    identifier: u8,
    length: u16,
}

/// Signaling channel state.
pub struct SignalingState {}

impl Protocol for SignalingState {
    const RSP_PDU_SIZE: u8 = 23;
}

impl ProtocolObj for SignalingState {
    fn process_message(&mut self, message: &[u8], responder: Sender<'_>) -> Result<(), Error> {
        unimplemented!()
    }
}
