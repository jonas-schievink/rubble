//! The LE Security Manager protocol.
//!
//! The Security Manager is a mandatory part of BLE and is connected to L2CAP channel `0x0006` when
//! the Link-Layer connection is established.

use {
    crate::ble::{
        bytes::*,
        l2cap::{L2CAPResponder, Protocol, ProtocolObj},
        Error,
    },
    log::warn,
};

/// Supported security levels.
pub trait SecurityLevel {
    const MTU: u8;
}

/// LE Secure Connections are not supported and will not be established.
pub struct NoSecurity;
impl SecurityLevel for NoSecurity {
    // 23 Bytes when LE Secure Connections are unsupported
    const MTU: u8 = 23;
}

/// Indicates support for LE Secure Connections.
pub struct SecureConnections;
impl SecurityLevel for SecureConnections {
    // 65 Bytes when LE Secure Connections are supported
    const MTU: u8 = 65;
}

/// The LE Security Manager.
///
/// Manages pairing and key generation and exchange.
pub struct SecurityManager<S: SecurityLevel> {
    _security: S,
}

impl SecurityManager<NoSecurity> {
    pub fn no_security() -> Self {
        Self {
            _security: NoSecurity,
        }
    }
}

impl<S: SecurityLevel> ProtocolObj for SecurityManager<S> {
    fn process_message(
        &mut self,
        mut message: &[u8],
        _responder: L2CAPResponder,
    ) -> Result<(), Error> {
        let cmd = Command::from_bytes(&mut message)?;
        warn!("[NYI] security manager; cmd = {:?}", cmd);
        Ok(())
    }
}

impl<S: SecurityLevel> Protocol for SecurityManager<S> {
    const RSP_PDU_SIZE: u8 = S::MTU;
}

#[derive(Debug, Copy, Clone)]
enum Command<'a> {
    Unknown { code: CommandCode, data: &'a [u8] },
}

impl<'a> FromBytes<'a> for Command<'a> {
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error> {
        let code = CommandCode::from(bytes.read_u8()?);
        Ok(match code {
            _ => Command::Unknown {
                code,
                data: bytes.read_slice(bytes.len())?,
            },
        })
    }
}

enum_with_unknown! {
    #[derive(Debug, Copy, Clone)]
    enum CommandCode(u8) {
        PairingRequest = 0x01,
        PairingResponse = 0x02,
        PairingConfirm = 0x03,
        PairingRandom = 0x04,
        PairingFailed = 0x05,
        EncryptionInformation = 0x06,
        MasterIdentification = 0x07,
        IdentityInformation = 0x08,
        IdentityAddressInformation = 0x09,
        SigningInformation = 0x0A,
        SecurityRequest = 0x0B,
        PairingPublicKey = 0x0C,
        PairingDhKeyCheck = 0x0D,
        PairingKeypressNotification = 0x0E,
    }
}
