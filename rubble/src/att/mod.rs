//! Implementation of the Attribute Protocol (ATT).
//!
//! ATT always runs over L2CAP channel `0x0004`, which is connected by default as soon as the
//! Link-Layer connection is established.
//!
//! ATT is used by GATT, the *Generic Attribute Profile*, which introduces the concept of *Services*
//! and *Characteristics* which can all be accessed and discovered over the Attribute Protocol.
//!
//! # Attributes
//!
//! The ATT server hosts a list of *Attributes*, which consist of the following:
//!
//! * A 16-bit *Attribute Handle* ([`AttHandle`]) uniquely identifying the attribute.
//! * A 16- or 128-bit UUID identifying the attribute type. This provides information about how to
//!   interpret the attribute's value (eg. as a little-endian 32-bit integer).
//! * The attribute's *value*, consisting of a dynamically-sized byte array of up to 512 Bytes.
//! * A set of *permissions*, restricting the operations that can be performed on the attribute.
//!
//! ## Attribute Grouping
//!
//! TODO: Figure out how the hell this works and write it down in human-readable form.
//!
//! [`AttHandle`]: struct.AttHandle.html

mod handle;
mod uuid;

use {
    self::handle::*,
    crate::{
        bytes::*,
        l2cap::{L2CAPResponder, Protocol, ProtocolObj},
        utils::HexSlice,
        Error,
    },
    log::{debug, trace},
};

pub use self::handle::AttHandle;
pub use self::uuid::AttUuid;

enum_with_unknown! {
    /// Specifies an ATT operation to perform.
    ///
    /// The byte values assigned to opcodes are chosen so that the most significant 2 bits indicate
    /// additional information that can be useful in some cases:
    ///
    /// ```notrust
    /// MSb                            LSb
    /// +-----------+---------+----------+
    /// | Signature | Command |  Method  |
    /// |   1 bit   |  1 bit  |  6 bits  |
    /// +-----------+---------+----------+
    /// ```
    ///
    /// * **`Signature`** is set to 1 to indicate that the Attribute Opcode and Parameters are
    ///   followed by an Authentication Signature. This is only allowed for the *Write Command*,
    ///   resulting in the `SignedWriteCommand`.
    /// * **`Command`** is set to 1 when the PDU is a command. This is done purely so that the server
    ///   can ignore unknown commands. Unlike *Requests*, Commands are not followed by a server
    ///   response.
    /// * **`Method`** defines which operation to perform.
    #[derive(Debug, Copy, Clone)]
    enum Opcode(u8) {
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

impl Opcode {
    /// Returns the raw byte corresponding to the opcode `self`.
    fn raw(&self) -> u8 {
        u8::from(*self)
    }

    /// Returns whether the `Signature` bit in this opcode is set.
    ///
    /// If the bit is set, this is an authenticated operation. The opcode parameters are followed by
    /// a 12-Byte signature.
    fn is_authenticated(&self) -> bool {
        self.raw() & 0x80 != 0
    }

    /// Returns whether the `Command` bit in this opcode is set.
    ///
    /// Commands sent to the server are not followed by a server response (ie. it is not indicated
    /// whether they succeed). Unimplemented commands should be ignored, according to the spec.
    fn is_command(&self) -> bool {
        self.raw() & 0x40 != 0
    }
}

/// Structured representation of an ATT message (request or response).
///
/// Note that many responses will need their own type that wraps an iterator.
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
    Unknown {
        opcode: Opcode,
        params: HexSlice<&'a [u8]>,
    },
}

/// *Read By Group Type* response PDU holding an iterator.
struct ReadByGroupRsp<'a, I: Iterator<Item = ByGroupAttData<'a>>> {
    items: I,
}

impl<'a, I: Iterator<Item = ByGroupAttData<'a>>> ReadByGroupRsp<'a, I> {
    fn encode(self, writer: &mut ByteWriter) -> Result<(), Error> {
        // This is pretty complicated to encode: The length depends on the attributes we fetch from
        // the iterator, and has to be written last, but is located at the start.
        // All the attributes we encode must have the same length. If they don't, we simply stop
        // when reaching the first one with a different size.

        writer.write_u8(Opcode::ReadByGroupRsp.into())?;
        let mut length = writer.split_off(1)?;

        let mut size = None;
        let left = writer.space_left();

        // Encode attribute data until we run out of space or the encoded size differs from the
        // first entry. This might write partial data, but we set the preceding length correctly, so
        // it shouldn't matter.
        for att in self.items {
            trace!("read by group rsp: {:?}", att);
            if let Err(_) = att.to_bytes(writer) {
                break;
            }

            let used = left - writer.space_left();
            if let Some(expected_size) = size {
                if used != expected_size {
                    break;
                }
            } else {
                size = Some(used);
            }
        }

        let size = size.expect("empty response");
        assert!(size <= usize::from(u8::max_value()));
        length.write_u8(size as u8).unwrap();
        Ok(())
    }
}

impl AttMsg<'_> {
    fn opcode(&self) -> Opcode {
        match self {
            AttMsg::ErrorRsp { .. } => Opcode::ErrorRsp,
            AttMsg::ExchangeMtuReq { .. } => Opcode::ExchangeMtuReq,
            AttMsg::ExchangeMtuRsp { .. } => Opcode::ExchangeMtuRsp,
            AttMsg::ReadByGroupReq { .. } => Opcode::ReadByGroupReq,
            AttMsg::Unknown { opcode, .. } => *opcode,
        }
    }
}

/// A PDU sent from server to client (over L2CAP).
#[derive(Debug)]
struct OutgoingPdu<'a>(AttMsg<'a>);

impl<'a> From<AttMsg<'a>> for OutgoingPdu<'a> {
    fn from(msg: AttMsg<'a>) -> Self {
        OutgoingPdu(msg)
    }
}

impl<'a> FromBytes<'a> for OutgoingPdu<'a> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        let opcode = Opcode::from(bytes.read_u8()?);
        let auth = opcode.is_authenticated();

        let msg = match opcode {
            Opcode::ErrorRsp => AttMsg::ErrorRsp {
                opcode: Opcode::from(bytes.read_u8()?),
                handle: AttHandle::from_bytes(bytes)?,
                error_code: ErrorCode::from(bytes.read_u8()?),
            },
            Opcode::ExchangeMtuReq => AttMsg::ExchangeMtuReq {
                mtu: bytes.read_u16_le()?,
            },
            Opcode::ExchangeMtuRsp => AttMsg::ExchangeMtuRsp {
                mtu: bytes.read_u16_le()?,
            },
            Opcode::ReadByGroupReq => AttMsg::ReadByGroupReq {
                handle_range: RawHandleRange::from_bytes(bytes)?,
                group_type: AttUuid::from_bytes(bytes)?,
            },
            _ => AttMsg::Unknown {
                opcode,
                params: HexSlice(bytes.read_slice(bytes.bytes_left() - if auth { 12 } else { 0 })?),
            },
        };

        if auth {
            // Ignore signature
            bytes.skip(12)?;
        }
        Ok(OutgoingPdu(msg))
    }
}

impl ToBytes for OutgoingPdu<'_> {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u8(self.0.opcode().into())?;
        match self.0 {
            AttMsg::ErrorRsp {
                opcode,
                handle,
                error_code,
            } => {
                writer.write_u8(opcode.into())?;
                writer.write_u16_le(handle.as_u16())?;
                writer.write_u8(error_code.into())?;
            }
            AttMsg::ExchangeMtuReq { mtu } => {
                writer.write_u16_le(mtu)?;
            }
            AttMsg::ExchangeMtuRsp { mtu } => {
                writer.write_u16_le(mtu)?;
            }
            AttMsg::ReadByGroupReq {
                handle_range,
                group_type,
            } => {
                handle_range.to_bytes(writer)?;
                group_type.to_bytes(writer)?;
            }
            AttMsg::Unknown { opcode: _, params } => {
                writer.write_slice(params.0)?;
            }
        }
        if self.0.opcode().is_authenticated() {
            // Write a dummy signature. This should never really be reached since the server never
            // sends authenticated messages.
            writer.write_slice(&[0; 12])?;
        }
        Ok(())
    }
}

/// An ATT PDU transferred from client to server as the L2CAP protocol payload.
///
/// Outgoing PDUs are just `AttMsg`s.
#[derive(Debug)]
struct IncomingPdu<'a> {
    /// The 1-Byte opcode value. It is kept around since it needs to be returned in error responses.
    opcode: Opcode,
    /// Decoded message (request or command) including parameters.
    params: AttMsg<'a>,
    /// `Some` if `opcode.is_authenticated()` is `true`, `None` if not.
    ///
    /// If present, contains 12 Bytes.
    signature: Option<HexSlice<&'a [u8]>>,
}

impl<'a> FromBytes<'a> for IncomingPdu<'a> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        let opcode = Opcode::from(bytes.read_u8()?);
        let auth = opcode.is_authenticated();

        Ok(Self {
            opcode,
            params: match opcode {
                Opcode::ErrorRsp => AttMsg::ErrorRsp {
                    opcode: Opcode::from(bytes.read_u8()?),
                    handle: AttHandle::from_bytes(bytes)?,
                    error_code: ErrorCode::from(bytes.read_u8()?),
                },
                Opcode::ExchangeMtuReq => AttMsg::ExchangeMtuReq {
                    mtu: bytes.read_u16_le()?,
                },
                Opcode::ExchangeMtuRsp => AttMsg::ExchangeMtuRsp {
                    mtu: bytes.read_u16_le()?,
                },
                Opcode::ReadByGroupReq => AttMsg::ReadByGroupReq {
                    handle_range: RawHandleRange::from_bytes(bytes)?,
                    group_type: AttUuid::from_bytes(bytes)?,
                },
                _ => AttMsg::Unknown {
                    opcode,
                    params: HexSlice(
                        bytes.read_slice(bytes.bytes_left() - if auth { 12 } else { 0 })?,
                    ),
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

impl ToBytes for IncomingPdu<'_> {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u8(self.opcode.into())?;
        match self.params {
            AttMsg::ErrorRsp {
                opcode,
                handle,
                error_code,
            } => {
                writer.write_u8(opcode.into())?;
                writer.write_u16_le(handle.as_u16())?;
                writer.write_u8(error_code.into())?;
            }
            AttMsg::ExchangeMtuReq { mtu } => {
                writer.write_u16_le(mtu)?;
            }
            AttMsg::ExchangeMtuRsp { mtu } => {
                writer.write_u16_le(mtu)?;
            }
            AttMsg::ReadByGroupReq {
                handle_range,
                group_type,
            } => {
                handle_range.to_bytes(writer)?;
                group_type.to_bytes(writer)?;
            }
            AttMsg::Unknown { opcode: _, params } => {
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
    /// The type of the attribute as a UUID16, EG "Primary Service" or "Anaerobic Heart Rate Lower Limit"
    pub att_type: AttUuid,
    /// Unique server-side identifer for attribute
    pub handle: AttHandle,
    /// Attribute values can be any fixed length or variable length octet array, which if too large
    /// can be sent across multiple PDUs
    pub value: HexSlice<&'a [u8]>,
    /// Permissions associated with the attribute
    pub permission: AttPermission,
}

/// Permissions associated with an attribute
pub struct AttPermission {
    _access: AccessPermission,
    _encryption: EncryptionPermission,
    _authentication: AuthenticationPermission,
    _authorization: AuthorizationPermission,
}

pub enum AccessPermission {
    Readable,
    Writeable,
    ReadableWritable,
}

pub enum EncryptionPermission {
    EncryptionRequired,
    EncryptionNotRequired,
}

pub enum AuthenticationPermission {
    AuthenticationRequired,
    AuthenticationNotRequired,
}

pub enum AuthorizationPermission {
    AuthorizationRequired,
    AuthorizationNotRequired,
}

impl Default for AttPermission {
    fn default() -> Self {
        Self {
            _access: AccessPermission::Readable,
            _encryption: EncryptionPermission::EncryptionNotRequired,
            _authentication: AuthenticationPermission::AuthenticationNotRequired,
            _authorization: AuthorizationPermission::AuthorizationNotRequired,
        }
    }
}

/// Trait for attribute sets that can be hosted by an `AttributeServer`.
pub trait Attributes {
    fn attributes(&mut self) -> &[Attribute];
}

/// An empty attribute set.
pub struct NoAttributes;

impl Attributes for NoAttributes {
    fn attributes(&mut self) -> &[Attribute] {
        &[]
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
    /// Process an incoming request (or command) PDU and return a response.
    ///
    /// This may return an `AttError`, which the caller will then send as a response. In the success
    /// case, this method will send the response (if any).
    fn process_request<'a>(
        &mut self,
        pdu: IncomingPdu,
        responder: &mut L2CAPResponder,
    ) -> Result<(), AttError> {
        /// Error returned when an ATT error should be sent back.
        ///
        /// Returning this from inside `responder.respond_with` will not send the response and
        /// instead bail out of the closure.
        struct RspError(AttError);

        impl From<Error> for RspError {
            fn from(e: Error) -> Self {
                panic!("unexpected error: {}", e);
            }
        }

        impl From<AttError> for RspError {
            fn from(att: AttError) -> Self {
                RspError(att)
            }
        }

        match pdu.params {
            AttMsg::ReadByGroupReq {
                handle_range,
                group_type,
            } => {
                let range = handle_range.check()?;

                // TODO: Ask GATT whether `group_type` is a grouping attribute, reject if not

                let requested_attrs = self
                    .attrs
                    .attributes()
                    .iter()
                    .filter(|att| att.att_type == group_type && range.contains(att.handle))
                    .map(|att| {
                        ByGroupAttData {
                            handle: att.handle,
                            end_group_handle: att.handle, // TODO: Ask GATT where the group ends
                            value: att.value,
                        }
                    });

                let result = responder.respond_with(|writer| {
                    // If no attributes match request, return `AttributeNotFound` error, else send
                    // `ReadByGroupResponse` with at least one entry
                    if requested_attrs.clone().next().is_none() {
                        Err(AttError {
                            code: ErrorCode::AttributeNotFound,
                            handle: AttHandle::NULL,
                        }
                        .into())
                    } else {
                        ReadByGroupRsp {
                            items: requested_attrs,
                        }
                        .encode(writer)?;
                        Ok(())
                    }
                });

                match result {
                    Ok(()) => Ok(()),
                    Err(RspError(e)) => Err(e),
                }
            }
            AttMsg::ExchangeMtuReq { mtu: _mtu } => {
                responder
                    .respond(OutgoingPdu(AttMsg::ExchangeMtuRsp {
                        mtu: u16::from(Self::RSP_PDU_SIZE),
                    }))
                    .unwrap();
                Ok(())
            }

            AttMsg::Unknown { .. } => {
                if pdu.opcode.is_command() {
                    // According to the spec, unknown Command PDUs should be ignored
                    Ok(())
                } else {
                    // Unknown requests are rejected with a `RequestNotSupported` error
                    Err(AttError {
                        code: ErrorCode::RequestNotSupported,
                        handle: AttHandle::NULL,
                    })
                }
            }

            // Responses are always invalid here
            AttMsg::ErrorRsp { .. } | AttMsg::ExchangeMtuRsp { .. } => Err(AttError {
                code: ErrorCode::InvalidPdu,
                handle: AttHandle::NULL,
            }),
        }
    }
}

impl<A: Attributes> ProtocolObj for AttributeServer<A> {
    fn process_message(
        &mut self,
        message: &[u8],
        mut responder: L2CAPResponder,
    ) -> Result<(), Error> {
        let pdu = IncomingPdu::from_bytes(&mut ByteReader::new(message))?;
        let opcode = pdu.opcode;
        debug!("ATT msg received: {:?}", pdu);

        match self.process_request(pdu, &mut responder) {
            Ok(()) => Ok(()),
            Err(att_error) => {
                debug!("ATT error: {:?}", att_error);

                responder.respond(OutgoingPdu(AttMsg::ErrorRsp {
                    opcode: opcode,
                    handle: att_error.handle,
                    error_code: att_error.code,
                }))
            }
        }
    }
}

impl<A: Attributes> Protocol for AttributeServer<A> {
    // FIXME: Would it be useful to have this as a runtime parameter instead?
    const RSP_PDU_SIZE: u8 = 23;
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

/// Attribute Data returned in *Read By Type* response.
#[derive(Debug)]
pub struct ByTypeAttData<'a> {
    handle: AttHandle,
    value: HexSlice<&'a [u8]>,
}

impl<'a> FromBytes<'a> for ByTypeAttData<'a> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        Ok(ByTypeAttData {
            handle: AttHandle::from_bytes(bytes)?,
            value: HexSlice(bytes.read_rest()),
        })
    }
}

impl<'a> ToBytes for ByTypeAttData<'a> {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u16_le(self.handle.as_u16())?;
        writer.write_slice(self.value.0)?;
        Ok(())
    }
}

/// Attribute Data returned in *Read By Group Type* response.
#[derive(Debug, Copy, Clone)]
pub struct ByGroupAttData<'a> {
    /// The handle of this attribute.
    handle: AttHandle,
    end_group_handle: AttHandle,
    value: HexSlice<&'a [u8]>,
}

impl<'a> FromBytes<'a> for ByGroupAttData<'a> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        Ok(ByGroupAttData {
            handle: AttHandle::from_bytes(bytes)?,
            end_group_handle: AttHandle::from_bytes(bytes)?,
            value: HexSlice(bytes.read_rest()),
        })
    }
}

/// The `ToBytes` impl will truncate the value if it doesn't fit.
impl<'a> ToBytes for ByGroupAttData<'a> {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u16_le(self.handle.as_u16())?;
        writer.write_u16_le(self.end_group_handle.as_u16())?;
        if writer.space_left() >= self.value.0.len() {
            writer.write_slice(self.value.0)?;
        } else {
            writer
                .write_slice(&self.value.0[..writer.space_left()])
                .unwrap();
        }
        Ok(())
    }
}
