//! Implementation of the Attribute Protocol (ATT).
//!
//! ATT always runs over L2CAP channel `0x0004`, which is connected by default as soon as the
//! Link-Layer connection is established.
//!
//! ATT is used by GATT, the *Generic Attribute Profile*, which introduces the concept of *Services*
//! and *Characteristics* which can all be accessed and discovered over the Attribute Protocol.

mod handle;

use {
    self::handle::*,
    crate::ble::{
        bytes::*,
        l2cap::{L2CAPResponder, Protocol, ProtocolObj},
        utils::HexSlice,
        uuid::{Uuid, Uuid16},
        Error,
    },
    core::fmt,
    log::debug,
};

pub use self::handle::AttHandle;

/// An ATT opcode containing the `Method` to execute as well as command and authentication bits.
#[derive(Copy, Clone)]
struct Opcode(u8);

impl Opcode {
    fn new(method: Method, auth: bool, command: bool) -> Self {
        let auth = if auth { 0x80 } else { 0x00 };
        let command = if command { 0x40 } else { 0x00 };
        let method = u8::from(method) & 0x3f;

        Opcode(auth | command | method)
    }

    fn is_authenticated(&self) -> bool {
        self.0 & 0x80 != 0
    }

    fn is_command(&self) -> bool {
        self.0 & 0x40 != 0
    }

    fn method(&self) -> Method {
        Method::from(self.0 & 0x3f)
    }
}

impl From<u8> for Opcode {
    fn from(u: u8) -> Self {
        Opcode(u)
    }
}

impl Into<u8> for Opcode {
    fn into(self) -> u8 {
        self.0
    }
}

impl fmt::Debug for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Opcode")
            .field("auth", &self.is_authenticated())
            .field("command", &self.is_command())
            .field("method", &self.method())
            .finish()
    }
}

/// ATT protocol UUID (either a 16 or a 128-bit UUID).
#[derive(Debug, Copy, Clone, PartialEq)]
enum AttUuid {
    Uuid16(Uuid16),
    Uuid128(Uuid),
}

impl FromBytes<'_> for AttUuid {
    fn from_bytes(bytes: &mut &'_ [u8]) -> Result<Self, Error> {
        Ok(match bytes.len() {
            2 => AttUuid::Uuid16(Uuid16::from_bytes(bytes)?),
            16 => AttUuid::Uuid128(<Uuid as FromBytes>::from_bytes(bytes)?),
            _ => return Err(Error::InvalidLength),
        })
    }
}

impl ToBytes for AttUuid {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        match self {
            AttUuid::Uuid16(uuid) => uuid.to_bytes(writer),
            AttUuid::Uuid128(uuid) => uuid.to_bytes(writer),
        }
    }
}

/// Structured representation of an ATT message (request or response).
#[derive(Debug)]
enum AttMsg<'a> {
    /// Request could not be completed due to an error.
    ErrorRsp {
        /// The opcode that caused the error.
        opcode: Opcode,
        /// The attribute handle on which the operation failed.
        handle: AttHandle,
        /// An error code describing the kind of error that occurred.
        error_code: ErrorCode,
    },
    ExchangeMtuReq {
        mtu: u16,
    },
    ExchangeMtuRsp {
        mtu: u16,
    },
    ReadByGroupReq {
        handle_range: RawHandleRange,
        group_type: AttUuid,
    },
    ReadByGroupRsp {
        length: u8,
        data_list: &'a [u8],
    },
    Unknown {
        params: HexSlice<&'a [u8]>,
    },
}

/// An ATT PDU transferred as the L2CAP protocol payload.
#[derive(Debug)]
struct Pdu<'a> {
    opcode: Opcode,
    params: AttMsg<'a>,
    /// `Some` if `opcode.is_authenticated()` is `true`, `None` if not.
    ///
    /// If present, contains 12 Bytes.
    signature: Option<HexSlice<&'a [u8]>>,
}

impl<'a> FromBytes<'a> for Pdu<'a> {
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error> {
        let opcode = Opcode::from(bytes.read_first()?);
        let auth = opcode.is_authenticated();

        Ok(Self {
            opcode,
            params: match opcode.method() {
                Method::ErrorRsp => AttMsg::ErrorRsp {
                    opcode: Opcode::from(bytes.read_first()?),
                    handle: AttHandle::from_bytes(bytes)?,
                    error_code: ErrorCode::from(bytes.read_first()?),
                },
                Method::ExchangeMtuReq => AttMsg::ExchangeMtuReq {
                    mtu: bytes.read_u16::<LittleEndian>()?,
                },
                Method::ExchangeMtuRsp => AttMsg::ExchangeMtuRsp {
                    mtu: bytes.read_u16::<LittleEndian>()?,
                },
                Method::ReadByGroupReq => AttMsg::ReadByGroupReq {
                    handle_range: RawHandleRange::from_bytes(bytes)?,
                    group_type: AttUuid::from_bytes(bytes)?,
                },
                _ => AttMsg::Unknown {
                    params: HexSlice(bytes.read_slice(bytes.len() - if auth { 12 } else { 0 })?),
                },
            },
            signature: if auth {
                Some(HexSlice(bytes.read_slice(12)?))
            } else {
                None
            },
        })
    }
}

impl ToBytes for Pdu<'_> {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u8(self.opcode.into())?;
        match self.params {
            AttMsg::ErrorRsp {
                opcode,
                handle,
                error_code,
            } => {
                writer.write_u8(opcode.into())?;
                writer.write_u16::<LittleEndian>(handle.as_u16())?;
                writer.write_u8(error_code.into())?;
            }
            AttMsg::ExchangeMtuReq { mtu } => {
                writer.write_u16::<LittleEndian>(mtu)?;
            }
            AttMsg::ExchangeMtuRsp { mtu } => {
                writer.write_u16::<LittleEndian>(mtu)?;
            }
            AttMsg::ReadByGroupReq {
                handle_range,
                group_type,
            } => {
                handle_range.to_bytes(writer)?;
                group_type.to_bytes(writer)?;
            }
            AttMsg::ReadByGroupRsp { length, data_list } => {
                writer.write_u8(length)?;
                writer.write_slice(data_list)?;
            }
            AttMsg::Unknown { params } => {
                writer.write_slice(params.0)?;
            }
        }
        if let Some(sig) = self.signature {
            writer.write_slice(sig.0)?;
        }
        Ok(())
    }
}

/// An ATT server attribute
pub struct Attribute<'a> {
    att_type: AttUuid,
    handle: AttHandle,
    value: HexSlice<&'a [u8]>,
    _permission: AttPermission,
}

/// Permissions associated with an attribute
pub struct AttPermission {}

/// Trait for attribute sets that can be hosted by an `AttributeServer`.
pub trait Attributes {
    fn attributes(&self) -> &[Attribute];
}

/// An empty attribute set.
pub struct NoAttributes;

impl Attributes for NoAttributes {
    fn attributes(&self) -> &[Attribute] {
        &[]
    }
}

/// Set with a single attribute.
pub struct OneAttribute {
    inner: [Attribute<'static>; 1],
}

impl OneAttribute {
    pub fn new() -> Self {
        Self {
            inner: [Attribute {
                att_type: AttUuid::Uuid16(Uuid16(0)),
                handle: AttHandle::from_raw(1),
                value: HexSlice(&[]),
                _permission: AttPermission {},
            }],
        }
    }
}

impl Attributes for OneAttribute {
    fn attributes(&self) -> &[Attribute] {
        &self.inner
    }
}

/// An Attribute Protocol server providing read and write access to stored attributes.
pub struct AttributeServer<A: Attributes> {
    attrs: A,
}

impl<A: Attributes> AttributeServer<A> {
    /// Creates an AttributeServer with Attributes
    pub fn new(attrs: A) -> Self {
        Self { attrs }
    }
}

impl<A: Attributes> AttributeServer<A> {
    /// Process an incoming request PDU and return a response.
    ///
    /// This may return an `AttError`, which the caller will then send as a response. In the success
    /// case, this method will send the response.
    fn process_request(
        &mut self,
        pdu: Pdu,
        responder: &mut L2CAPResponder,
    ) -> Result<(), AttError> {
        match pdu.params {
            AttMsg::ReadByGroupReq {
                handle_range,
                group_type,
            } => {
                let range = handle_range.check()?;

                let mut buf = [0u8; 16];
                let mut writer = ByteWriter::new(&mut buf);

                for att in self.attrs.attributes() {
                    if att.att_type == group_type && range.contains(att.handle) {
                        let data = ByGroupAttData {
                            handle: att.handle,
                            end_group_handle: AttHandle::from_raw(0),
                            value: att.value,
                        };

                        data.to_bytes(&mut writer).unwrap();
                    }
                }

                // If no attributes matched request, return AttributeNotFound error, else send ReadByGroupResponse
                if writer.space_left() == 16 {
                    let err = AttError {
                        code: ErrorCode::AttributeNotFound,
                        handle: AttHandle::NULL,
                    };

                    debug!("ATT Error: {:?}", err);

                    Err(err)
                } else {
                    let length = 2 + 2 + self.attrs.attributes()[0].value.0.len() as u8;

                    responder
                        .respond(Pdu {
                            opcode: Opcode::new(Method::ReadByGroupRsp, false, false),
                            params: AttMsg::ReadByGroupRsp {
                                length,
                                data_list: &buf,
                            },
                            signature: None,
                        })
                        .unwrap();

                    Ok(())
                }
            }
            AttMsg::ExchangeMtuReq { mtu: _mtu } => {
                responder
                    .respond(Pdu {
                        opcode: Opcode::new(Method::ExchangeMtuRsp, false, false),
                        params: AttMsg::ExchangeMtuRsp {
                            mtu: u16::from(Self::RSP_PDU_SIZE),
                        },
                        signature: None,
                    })
                    .unwrap();
                Ok(())
            }
            AttMsg::ErrorRsp { .. }
            | AttMsg::Unknown { .. }
            | AttMsg::ReadByGroupRsp { .. }
            | AttMsg::ExchangeMtuRsp { .. } => {
                return Err(AttError {
                    code: ErrorCode::InvalidPdu,
                    handle: AttHandle::NULL,
                });
            }
        }
    }
}

impl<A: Attributes> ProtocolObj for AttributeServer<A> {
    fn process_message(
        &mut self,
        mut message: &[u8],
        mut responder: L2CAPResponder,
    ) -> Result<(), Error> {
        let pdu = Pdu::from_bytes(&mut message)?;
        let opcode = pdu.opcode;
        debug!("ATT msg: {:?}", pdu);

        match self.process_request(pdu, &mut responder) {
            Ok(()) => Ok(()),
            Err(att_error) => responder.respond(Pdu {
                opcode: Opcode::new(Method::ErrorRsp, false, false),
                params: AttMsg::ErrorRsp {
                    opcode: opcode,
                    handle: att_error.handle,
                    error_code: att_error.code,
                },
                signature: None,
            }),
        }
    }
}

impl<A: Attributes> Protocol for AttributeServer<A> {
    // FIXME: Would it be useful to have this as a runtime parameter instead?
    const RSP_PDU_SIZE: u8 = 23;
}

enum_with_unknown! {
    #[derive(Debug)]
    enum Method(u8) {
        ErrorRsp = 0x01,
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

enum_with_unknown! {
    /// Error codes that can be sent from the ATT server to the client in response to a request.
    ///
    /// Used as the payload of `ErrorRsp` PDUs.
    #[derive(Copy, Clone, Debug)]
    pub enum ErrorCode(u8) {
        /// Attempted to use an `AttHandle` that isn't valid on this server.
        InvalidHandle = 0x01,
        /// Attribute isn't readable.
        ReadNotPermitted = 0x02,
        /// Attribute isn't writable.
        WriteNotPermitted = 0x03,
        /// Attribute PDU is invalid.
        InvalidPdu = 0x04,
        /// Authentication needed before attribute can be read/written.
        InsufficientAuthentication = 0x05,
        /// Server doesn't support this operation.
        RequestNotSupported = 0x06,
        /// Offset was past the end of the attribute.
        InvalidOffset = 0x07,
        /// Authorization needed before attribute can be read/written.
        InsufficientAuthorization = 0x08,
        /// Too many "prepare write" requests have been queued.
        PrepareQueueFull = 0x09,
        /// No attribute found within the specified attribute handle range.
        AttributeNotFound = 0x0A,
        /// Attribute can't be read/written using *Read Key Blob* request.
        AttributeNotLong = 0x0B,
        /// The encryption key in use is too weak to access an attribute.
        InsufficientEncryptionKeySize = 0x0C,
        /// Attribute value has an incorrect length for the operation.
        InvalidAttributeValueLength = 0x0D,
        /// Request has encountered an "unlikely" error and could not be completed.
        UnlikelyError = 0x0E,
        /// Attribute cannot be read/written without an encrypted connection.
        InsufficientEncryption = 0x0F,
        /// Attribute type is an invalid grouping attribute according to a higher-layer spec.
        UnsupportedGroupType = 0x10,
        /// Server didn't have enough resources to complete a request.
        InsufficientResources = 0x11,
    }
}

/// An error on the ATT protocol layer. Can be sent as a response.
#[derive(Debug)]
pub struct AttError {
    code: ErrorCode,
    handle: AttHandle,
}

/// Attribute Data returned in Read By Type response
#[derive(Debug)]
pub struct ByTypeAttData<'a> {
    handle: AttHandle,
    value: HexSlice<&'a [u8]>,
}

impl<'a, 'b> FromBytes<'b> for ByTypeAttData<'a>
where
    'b: 'a,
{
    fn from_bytes(bytes: &mut &'b [u8]) -> Result<Self, Error> {
        Ok(ByTypeAttData {
            handle: AttHandle::from_bytes(bytes)?,
            value: HexSlice(bytes.read_slice(bytes.len())?),
        })
    }
}

impl<'a> ToBytes for ByTypeAttData<'a> {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u16::<LittleEndian>(self.handle.as_u16())?;
        writer.write_slice(self.value.0)?;
        Ok(())
    }
}

/// Attribute Data returned in Read By Group Type response
#[derive(Debug)]
pub struct ByGroupAttData<'a> {
    handle: AttHandle,
    end_group_handle: AttHandle,
    value: HexSlice<&'a [u8]>,
}

impl<'a, 'b> FromBytes<'b> for ByGroupAttData<'a>
where
    'b: 'a,
{
    fn from_bytes(bytes: &mut &'b [u8]) -> Result<Self, Error> {
        Ok(ByGroupAttData {
            handle: AttHandle::from_bytes(bytes)?,
            end_group_handle: AttHandle::from_bytes(bytes)?,
            value: HexSlice(bytes.read_slice(bytes.len())?),
        })
    }
}

impl<'a> ToBytes for ByGroupAttData<'a> {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u16::<LittleEndian>(self.handle.as_u16())?;
        writer.write_u16::<LittleEndian>(self.end_group_handle.as_u16())?;
        writer.write_slice(self.value.0)?;
        Ok(())
    }
}
