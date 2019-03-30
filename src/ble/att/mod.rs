//! Implementation of the Attribute Protocol (ATT).

use {
    crate::ble::{
        bytes::*,
        l2cap::{L2CAPResponder, Protocol},
        link::queue::Consume,
        utils::HexSlice,
        Error,
    },
    log::debug,
};

#[derive(Debug)]
struct Pdu<'a> {
    command: bool,
    method: Method,
    params: HexSlice<&'a [u8]>,
    /// `Some` if the `authenticated` bit was set, `None` if not.
    signature: Option<HexSlice<&'a [u8]>>,
}

impl<'a> FromBytes<'a> for Pdu<'a> {
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error> {
        let opcode = bytes.read_first()?;
        let auth = opcode & 0x80 != 0;
        let command = opcode & 0x40 != 0;
        let method = Method::from(opcode & 0x3f);

        let params = bytes.read_slice(bytes.len() - if auth { 12 } else { 0 })?;
        let signature = if auth {
            Some(bytes.read_slice(12)?)
        } else {
            None
        };

        Ok(Self {
            command,
            method,
            params: HexSlice(params),
            signature: signature.map(HexSlice),
        })
    }
}

pub struct AttributeServer {}

impl AttributeServer {
    pub fn empty() -> Self {
        Self {}
    }
}

impl Protocol for AttributeServer {
    fn process_message(&mut self, mut message: &[u8], _responder: L2CAPResponder) -> Consume<()> {
        let message = match Pdu::from_bytes(&mut message) {
            Ok(m) => m,
            Err(e) => return Consume::always(Err(e)),
        };
        debug!("ATT msg: {:?}", message);

        Consume::always(Ok(()))
    }
}

enum_with_unknown! {
    #[derive(Debug)]
    enum Method(u8) {
        Error = 0x01,
        ExchangeMtuReq = 0x02,
        ExchangeMtuRsp = 0x03,
        FindInformationReq = 0x04,
        FindInformationRsp = 0x05,
        FindByTypeReq = 0x06,
        FindByTypeRsp = 0x07,
        ReadByTypeReq = 0x08,
        ReadByTypeRsp = 0x09,
        ReadReq = 0x0A,
        ReadRsp = 0x0B,
        ReadBlobReq = 0x0C,
        ReadBlobRsp = 0x0D,
        ReadMultipleReq = 0x0E,
        ReadMultipleRsp = 0x0F,
        ReadByGroupReq = 0x10,
        ReadByGroupRsp = 0x11,
        WriteReq = 0x12,
        WriteRsp = 0x13,
        WriteCommand = 0x52,
        SignedWriteCommand = 0xD2,
        PrepareWriteReq = 0x16,
        PrepareWriteRsp = 0x17,
        ExecuteWriteReq = 0x18,
        ExecuteWriteRsp = 0x19,
        HandleValueNotification = 0x1B,
        HandleValueIndication = 0x1D,
        HandleValueConfirmation = 0x1E,
    }
}
