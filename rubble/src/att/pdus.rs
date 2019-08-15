//! Packets and types used in the ATT protocol.

use {
    super::{AttHandle, AttUuid, RawHandleRange},
    crate::{bytes::*, utils::HexSlice, Error},
    core::convert::TryInto,
};

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

impl AttError {
    pub fn new(code: ErrorCode, handle: AttHandle) -> Self {
        Self { code, handle }
    }

    pub fn attribute_not_found() -> Self {
        Self::new(ErrorCode::AttributeNotFound, AttHandle::NULL)
    }

    /// The error code describing this error.
    ///
    /// These are all defined by the spec.
    pub fn error_code(&self) -> ErrorCode {
        self.code
    }

    /// The handle of the attribute causing the error.
    ///
    /// This can be the `NULL` handle if there's no attribute to blame.
    pub fn handle(&self) -> AttHandle {
        self.handle
    }
}

/// Attribute Data returned in *Read By Type* response.
#[derive(Debug)]
pub struct ByTypeAttData<'a> {
    handle: AttHandle,
    value: HexSlice<&'a [u8]>,
}

impl<'a> ByTypeAttData<'a> {
    pub fn new(handle: AttHandle, value: &'a [u8]) -> Self {
        Self {
            handle,
            value: HexSlice(value),
        }
    }
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
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        writer.write_u16_le(self.handle.as_u16())?;
        writer.write_slice(self.value.as_ref())?;
        // FIXME: The value should be truncated if it doesn't fit.
        Ok(())
    }
}

/// Attribute Data returned in *Read By Group Type* response.
#[derive(Debug, Copy, Clone)]
pub struct ByGroupAttData<'a> {
    /// The handle of this attribute.
    handle: AttHandle,
    group_end_handle: AttHandle,
    value: HexSlice<&'a [u8]>,
}

impl<'a> ByGroupAttData<'a> {
    pub fn new(handle: AttHandle, group_end_handle: AttHandle, value: &'a [u8]) -> Self {
        Self {
            handle,
            group_end_handle,
            value: HexSlice(value),
        }
    }
}

impl<'a> FromBytes<'a> for ByGroupAttData<'a> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        Ok(ByGroupAttData {
            handle: AttHandle::from_bytes(bytes)?,
            group_end_handle: AttHandle::from_bytes(bytes)?,
            value: HexSlice(bytes.read_rest()),
        })
    }
}

/// The `ToBytes` impl will truncate the value if it doesn't fit.
impl<'a> ToBytes for ByGroupAttData<'a> {
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        writer.write_u16_le(self.handle.as_u16())?;
        writer.write_u16_le(self.group_end_handle.as_u16())?;
        if writer.space_left() >= self.value.as_ref().len() {
            writer.write_slice(self.value.as_ref())?;
        } else {
            writer
                .write_slice(&self.value.as_ref()[..writer.space_left()])
                .unwrap();
        }
        Ok(())
    }
}

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
    pub enum Opcode(u8) {
        ErrorRsp = 0x01,
        ExchangeMtuReq = 0x02,
        ExchangeMtuRsp = 0x03,
        FindInformationReq = 0x04,
        FindInformationRsp = 0x05,
        FindByTypeValueReq = 0x06,
        FindByTypeValueRsp = 0x07,
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
    pub fn raw(&self) -> u8 {
        u8::from(*self)
    }

    /// Returns whether the `Signature` bit in this opcode is set.
    ///
    /// If the bit is set, this is an authenticated operation. The opcode parameters are followed by
    /// a 12-Byte signature.
    pub fn is_authenticated(&self) -> bool {
        self.raw() & 0x80 != 0
    }

    /// Returns whether the `Command` bit in this opcode is set.
    ///
    /// Commands sent to the server are not followed by a server response (ie. it is not indicated
    /// whether they succeed). Unimplemented commands should be ignored, according to the spec.
    pub fn is_command(&self) -> bool {
        self.raw() & 0x40 != 0
    }
}

/// Structured representation of an ATT message (request or response).
///
/// Note that many responses will need their own type that wraps an iterator.
#[derive(Debug)]
pub enum AttMsg<'a> {
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
    /// Used to obtain mapping of attribute handles and their types
    FindInformationReq {
        handle_range: RawHandleRange,
    },
    FindInformationRsp {
        /// 0x01 - data is a list of handles and 16-bit UUIDs
        /// 0x02 - data is a list of handles and 128-bit UUIDs
        format: u8,
        /// minimum size of 4 bytes
        data: HexSlice<&'a [u8]>,
    },
    /// Used to obtain the handles of attributes with a given type and value.
    FindByTypeValueReq {
        handle_range: RawHandleRange,
        attribute_type: u16,
        attribute_value: HexSlice<&'a [u8]>,
    },
    FindByTypeValueRsp {
        /// A single "Handles Information" is 2 octets found handle, 2 octets
        /// group end handle
        handles_information_list: HexSlice<&'a [u8]>,
    },
    ReadByTypeReq {
        handle_range: RawHandleRange,
        /// 16 or 128 bit UUID allowed
        attribute_type: AttUuid,
    },
    ReadByTypeRsp {
        /// The length of each attribute handle-value pair in the list
        length: u8,
        data_list: HexSlice<&'a [u8]>,
    },
    ReadReq {
        handle: AttHandle,
    },
    ReadRsp {
        value: HexSlice<&'a [u8]>,
    },
    ReadBlobReq {
        handle: AttHandle,
        offset: u16,
    },
    ReadBlobRsp {
        value: HexSlice<&'a [u8]>,
    },
    ReadMultipleReq {
        /// Minimum length of two handles
        handles: HexSlice<&'a [u8]>,
    },
    ReadMultipleRsp {
        values: HexSlice<&'a [u8]>,
    },
    ReadByGroupReq {
        handle_range: RawHandleRange,
        group_type: AttUuid,
    },
    ReadByGroupRsp {
        length: u8,
        data_list: HexSlice<&'a [u8]>,
    },
    WriteReq {
        handle: AttHandle,
        value: HexSlice<&'a [u8]>,
    },
    WriteRsp,
    WriteCommand {
        handle: AttHandle,
        value: HexSlice<&'a [u8]>,
    },
    SignedWriteCommand {
        handle: AttHandle,
        value: HexSlice<&'a [u8]>,
        signature: HexSlice<&'a [u8; 12]>,
    },
    PrepareWriteReq {
        handle: AttHandle,
        offset: u16,
        value: HexSlice<&'a [u8]>,
    },
    PrepareWriteRsp {
        handle: AttHandle,
        offset: u16,
        value: HexSlice<&'a [u8]>,
    },
    ExecuteWriteReq {
        /// 0x00 – Cancel all prepared writes
        /// 0x01 – Immediately write all pending prepared values
        flags: u8,
    },
    ExecuteWriteRsp,
    HandleValueNotification {
        handle: AttHandle,
        value: HexSlice<&'a [u8]>,
    },
    HandleValueIndication {
        handle: AttHandle,
        value: HexSlice<&'a [u8]>,
    },
    HandleValueConfirmation,
    Unknown {
        opcode: Opcode,
        params: HexSlice<&'a [u8]>,
    },
}

impl<'a> AttMsg<'a> {
    /// Reads the parameters of an ATT message from `bytes`.
    pub fn from_reader(bytes: &mut ByteReader<'a>, opcode: Opcode) -> Result<Self, Error> {
        Ok(match opcode {
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
            Opcode::FindInformationReq => AttMsg::FindInformationReq {
                handle_range: RawHandleRange::from_bytes(bytes)?,
            },
            Opcode::FindInformationRsp => AttMsg::FindInformationRsp {
                format: bytes.read_u8()?,
                data: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::FindByTypeValueReq => AttMsg::FindByTypeValueReq {
                handle_range: RawHandleRange::from_bytes(bytes)?,
                attribute_type: bytes.read_u16_le()?,
                attribute_value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::FindByTypeValueRsp => AttMsg::FindByTypeValueRsp {
                handles_information_list: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadByTypeReq => AttMsg::ReadByTypeReq {
                handle_range: RawHandleRange::from_bytes(bytes)?,
                attribute_type: AttUuid::from_bytes(bytes)?,
            },
            Opcode::ReadByTypeRsp => AttMsg::ReadByTypeRsp {
                length: bytes.read_u8()?,
                data_list: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadReq => AttMsg::ReadReq {
                handle: AttHandle::from_bytes(bytes)?,
            },
            Opcode::ReadRsp => AttMsg::ReadRsp {
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadBlobReq => AttMsg::ReadBlobReq {
                handle: AttHandle::from_bytes(bytes)?,
                offset: bytes.read_u16_le()?,
            },
            Opcode::ReadBlobRsp => AttMsg::ReadBlobRsp {
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadMultipleReq => AttMsg::ReadMultipleReq {
                handles: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadMultipleRsp => AttMsg::ReadMultipleRsp {
                values: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadByGroupReq => AttMsg::ReadByGroupReq {
                handle_range: RawHandleRange::from_bytes(bytes)?,
                group_type: AttUuid::from_bytes(bytes)?,
            },
            Opcode::ReadByGroupRsp => AttMsg::ReadByGroupRsp {
                length: bytes.read_u8()?,
                data_list: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::WriteReq => AttMsg::WriteReq {
                handle: AttHandle::from_bytes(bytes)?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::WriteRsp => AttMsg::WriteRsp {},
            Opcode::WriteCommand => AttMsg::WriteCommand {
                handle: AttHandle::from_bytes(bytes)?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::SignedWriteCommand => AttMsg::SignedWriteCommand {
                handle: AttHandle::from_bytes(bytes)?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left() - 12)?),
                signature: HexSlice(bytes.read_slice(12)?.try_into().unwrap()),
            },
            Opcode::PrepareWriteReq => AttMsg::PrepareWriteReq {
                handle: AttHandle::from_bytes(bytes)?,
                offset: bytes.read_u16_le()?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::PrepareWriteRsp => AttMsg::PrepareWriteRsp {
                handle: AttHandle::from_bytes(bytes)?,
                offset: bytes.read_u16_le()?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ExecuteWriteReq => AttMsg::ExecuteWriteReq {
                flags: bytes.read_u8()?,
            },
            Opcode::ExecuteWriteRsp => AttMsg::ExecuteWriteRsp {},
            Opcode::HandleValueNotification => AttMsg::HandleValueNotification {
                handle: AttHandle::from_bytes(bytes)?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::HandleValueIndication => AttMsg::HandleValueIndication {
                handle: AttHandle::from_bytes(bytes)?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::HandleValueConfirmation => AttMsg::HandleValueConfirmation {},
            Opcode::Unknown(_) => AttMsg::Unknown {
                opcode,
                params: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
        })
    }

    pub fn to_writer(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        match *self {
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
            AttMsg::FindInformationReq { handle_range } => {
                handle_range.to_bytes(writer)?;
            }
            AttMsg::FindInformationRsp { format, data } => {
                writer.write_u8(format)?;
                writer.write_slice(data.as_ref())?;
            }
            AttMsg::FindByTypeValueReq {
                handle_range,
                attribute_type,
                attribute_value,
            } => {
                handle_range.to_bytes(writer)?;
                writer.write_u16_le(attribute_type)?;
                writer.write_slice(attribute_value.as_ref())?;
            }
            AttMsg::FindByTypeValueRsp {
                handles_information_list,
            } => {
                writer.write_slice(handles_information_list.as_ref())?;
            }
            AttMsg::ReadByTypeReq {
                handle_range,
                attribute_type,
            } => {
                handle_range.to_bytes(writer)?;
                attribute_type.to_bytes(writer)?;
            }
            AttMsg::ReadByTypeRsp { length, data_list } => {
                writer.write_u8(length)?;
                writer.write_slice(data_list.as_ref())?;
            }
            AttMsg::ReadReq { handle } => {
                handle.to_bytes(writer)?;
            }
            AttMsg::ReadRsp { value } => {
                writer.write_slice(value.as_ref())?;
            }
            AttMsg::ReadBlobReq { handle, offset } => {
                handle.to_bytes(writer)?;
                writer.write_u16_le(offset)?;
            }
            AttMsg::ReadBlobRsp { value } => {
                writer.write_slice(value.as_ref())?;
            }
            AttMsg::ReadMultipleReq { handles } => {
                writer.write_slice(handles.as_ref())?;
            }
            AttMsg::ReadMultipleRsp { values } => {
                writer.write_slice(values.as_ref())?;
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
                writer.write_slice(data_list.as_ref())?;
            }
            AttMsg::WriteReq { handle, value } => {
                handle.to_bytes(writer)?;
                writer.write_slice(value.as_ref())?;
            }
            AttMsg::WriteRsp => {}
            AttMsg::WriteCommand { handle, value } => {
                handle.to_bytes(writer)?;
                writer.write_slice(value.as_ref())?;
            }
            AttMsg::SignedWriteCommand {
                handle,
                value,
                signature,
            } => {
                handle.to_bytes(writer)?;
                writer.write_slice(value.as_ref())?;
                writer.write_slice(*signature.as_ref())?;
            }
            AttMsg::PrepareWriteReq {
                handle,
                offset,
                value,
            } => {
                handle.to_bytes(writer)?;
                writer.write_u16_le(offset)?;
                writer.write_slice(value.as_ref())?;
            }
            AttMsg::PrepareWriteRsp {
                handle,
                offset,
                value,
            } => {
                handle.to_bytes(writer)?;
                writer.write_u16_le(offset)?;
                writer.write_slice(value.as_ref())?;
            }
            AttMsg::ExecuteWriteReq { flags } => {
                writer.write_u8(flags)?;
            }
            AttMsg::ExecuteWriteRsp => {}
            AttMsg::HandleValueNotification { handle, value } => {
                handle.to_bytes(writer)?;
                writer.write_slice(value.as_ref())?;
            }
            AttMsg::HandleValueIndication { handle, value } => {
                handle.to_bytes(writer)?;
                writer.write_slice(value.as_ref())?;
            }
            AttMsg::HandleValueConfirmation => {}
            AttMsg::Unknown { opcode: _, params } => {
                writer.write_slice(params.as_ref())?;
            }
        }

        Ok(())
    }

    pub fn opcode(&self) -> Opcode {
        match self {
            AttMsg::ErrorRsp { .. } => Opcode::ErrorRsp,
            AttMsg::ExchangeMtuReq { .. } => Opcode::ExchangeMtuReq,
            AttMsg::ExchangeMtuRsp { .. } => Opcode::ExchangeMtuRsp,
            AttMsg::ReadByTypeReq { .. } => Opcode::ReadByTypeReq,
            AttMsg::ReadByTypeRsp { .. } => Opcode::ReadByTypeRsp,
            AttMsg::FindInformationReq { .. } => Opcode::FindInformationReq,
            AttMsg::FindInformationRsp { .. } => Opcode::FindInformationRsp,
            AttMsg::FindByTypeValueReq { .. } => Opcode::FindByTypeValueReq,
            AttMsg::FindByTypeValueRsp { .. } => Opcode::FindByTypeValueRsp,
            AttMsg::ReadReq { .. } => Opcode::ReadReq,
            AttMsg::ReadRsp { .. } => Opcode::ReadRsp,
            AttMsg::ReadBlobReq { .. } => Opcode::ReadBlobReq,
            AttMsg::ReadBlobRsp { .. } => Opcode::ReadBlobRsp,
            AttMsg::ReadMultipleReq { .. } => Opcode::ReadMultipleReq,
            AttMsg::ReadMultipleRsp { .. } => Opcode::ReadMultipleRsp,
            AttMsg::ReadByGroupReq { .. } => Opcode::ReadByGroupReq,
            AttMsg::ReadByGroupRsp { .. } => Opcode::ReadBlobRsp,
            AttMsg::WriteReq { .. } => Opcode::WriteReq,
            AttMsg::WriteRsp { .. } => Opcode::WriteRsp,
            AttMsg::WriteCommand { .. } => Opcode::WriteCommand,
            AttMsg::SignedWriteCommand { .. } => Opcode::SignedWriteCommand,
            AttMsg::PrepareWriteReq { .. } => Opcode::PrepareWriteReq,
            AttMsg::PrepareWriteRsp { .. } => Opcode::PrepareWriteRsp,
            AttMsg::ExecuteWriteReq { .. } => Opcode::ExecuteWriteReq,
            AttMsg::ExecuteWriteRsp { .. } => Opcode::ExecuteWriteRsp,
            AttMsg::HandleValueNotification { .. } => Opcode::HandleValueNotification,
            AttMsg::HandleValueIndication { .. } => Opcode::HandleValueIndication,
            AttMsg::HandleValueConfirmation { .. } => Opcode::HandleValueConfirmation,
            AttMsg::Unknown { opcode, .. } => *opcode,
        }
    }
}

/// *Read By Group Type* response PDU holding an iterator.
pub struct ReadByGroupRsp<
    F: FnMut(&mut dyn FnMut(ByGroupAttData<'_>) -> Result<(), Error>) -> Result<(), Error>,
> {
    pub item_fn: F,
}

impl<
        'a,
        F: FnMut(&mut dyn FnMut(ByGroupAttData<'_>) -> Result<(), Error>) -> Result<(), Error>,
    > ReadByGroupRsp<F>
{
    pub fn encode(mut self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
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
        (self.item_fn)(&mut |att: ByGroupAttData<'_>| {
            trace!("read by group rsp: {:?}", att);
            att.to_bytes(writer)?;

            let used = left - writer.space_left();
            if let Some(expected_size) = size {
                if used != expected_size {
                    return Err(Error::InvalidLength);
                }
            } else {
                size = Some(used);
            }

            Ok(())
        })
        .ok();

        let size = size.expect("empty response");
        assert!(size <= usize::from(u8::max_value()));
        length.write_u8(size as u8).unwrap();
        Ok(())
    }
}

/// *Read By Type* response PDU holding an iterator.
pub struct ReadByTypeRsp<
    F: FnMut(&mut dyn FnMut(ByTypeAttData<'_>) -> Result<(), Error>) -> Result<(), Error>,
> {
    pub item_fn: F,
}

impl<'a, F: FnMut(&mut dyn FnMut(ByTypeAttData<'_>) -> Result<(), Error>) -> Result<(), Error>>
    ReadByTypeRsp<F>
{
    pub fn encode(mut self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        // This is pretty complicated to encode: The length depends on the attributes we fetch from
        // the iterator, and has to be written last, but is located at the start.
        // All the attributes we encode must have the same length. If they don't, we simply stop
        // when reaching the first one with a different size.

        writer.write_u8(Opcode::ReadByTypeRsp.into())?;
        let mut length = writer.split_off(1)?;

        let mut size = None;
        let left = writer.space_left();

        // Encode attribute data until we run out of space or the encoded size differs from the
        // first entry. This might write partial data, but we set the preceding length correctly, so
        // it shouldn't matter.
        (self.item_fn)(&mut |att: ByTypeAttData<'_>| {
            trace!("read by type rsp: {:?}", att);
            att.to_bytes(writer)?;

            let used = left - writer.space_left();
            if let Some(expected_size) = size {
                if used != expected_size {
                    return Err(Error::InvalidLength);
                }
            } else {
                size = Some(used);
            }

            Ok(())
        })
        .ok();

        let size = size.expect("empty response");
        assert!(size <= usize::from(u8::max_value()));
        length.write_u8(size as u8).unwrap();
        Ok(())
    }
}

/// An ATT PDU transferred from client to server as the L2CAP protocol payload.
///
/// Outgoing PDUs are just `AttMsg`s.
#[derive(Debug)]
pub struct IncomingPdu<'a> {
    /// The 1-Byte opcode value. It is kept around since it needs to be returned in error responses.
    opcode: Opcode,
    /// Decoded message (request or command) including parameters.
    params: AttMsg<'a>,
}

impl<'a> IncomingPdu<'a> {
    pub fn opcode(&self) -> Opcode {
        self.opcode
    }

    pub fn att_msg(&self) -> &AttMsg<'a> {
        &self.params
    }
}

impl<'a> FromBytes<'a> for IncomingPdu<'a> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        let opcode = Opcode::from(bytes.read_u8()?);

        Ok(Self {
            opcode,
            params: AttMsg::from_reader(bytes, opcode)?,
        })
    }
}

impl ToBytes for IncomingPdu<'_> {
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        writer.write_u8(self.opcode.into())?;

        self.params.to_writer(writer)?;

        Ok(())
    }
}
