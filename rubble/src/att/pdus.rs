//! Packets and types used in the ATT protocol.

use super::{AttUuid, Handle, RawHandleRange};
use crate::{bytes::*, utils::HexSlice, Error};
use core::convert::TryInto;

enum_with_unknown! {
    /// Error codes that can be sent from the ATT server to the client in response to a request.
    ///
    /// Used as the payload of `ErrorRsp` PDUs.
    #[derive(Copy, Clone, Debug, defmt::Format)]
    pub enum ErrorCode(u8) {
        /// Attempted to use an `Handle` that isn't valid on this server.
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
    handle: Handle,
}

impl AttError {
    pub fn new(code: ErrorCode, handle: Handle) -> Self {
        Self { code, handle }
    }

    pub fn attribute_not_found() -> Self {
        Self::new(ErrorCode::AttributeNotFound, Handle::NULL)
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
    pub fn handle(&self) -> Handle {
        self.handle
    }
}

/// Attribute Data returned in *Read By Type* response.
#[derive(Debug)]
pub struct ByTypeAttData<'a> {
    handle: Handle,
    value: HexSlice<&'a [u8]>,
}

impl<'a> ByTypeAttData<'a> {
    /// Creates a *Read By Type Response* attribute data structure from the attribute's handle and
    /// value.
    pub fn new(att_mtu: u8, handle: Handle, mut value: &'a [u8]) -> Self {
        let max_val_len = usize::from(att_mtu - 2);
        if value.len() > max_val_len {
            value = &value[..max_val_len];
        }

        Self {
            handle,
            value: HexSlice(value),
        }
    }

    /// Returns the encoded size of this `ByTypeAttData` structure.
    pub fn encoded_size(&self) -> u8 {
        // 2 for the handle, whatever's left for the value
        2 + self.value.as_ref().len() as u8
    }
}

impl<'a> FromBytes<'a> for ByTypeAttData<'a> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        Ok(ByTypeAttData {
            handle: Handle::from_bytes(bytes)?,
            value: HexSlice(bytes.read_rest()),
        })
    }
}

impl<'a> ToBytes for ByTypeAttData<'a> {
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        writer.write_u16_le(self.handle.as_u16())?;

        // If the writer doesn't have enough space, truncate the value
        writer.write_slice_truncate(self.value.as_ref());

        Ok(())
    }
}

/// Attribute Data returned in *Read By Group Type* response.
#[derive(Debug, Copy, Clone)]
pub struct ByGroupAttData<'a> {
    /// The handle of this attribute.
    handle: Handle,
    group_end_handle: Handle,
    value: HexSlice<&'a [u8]>,
}

impl<'a> ByGroupAttData<'a> {
    pub fn new(att_mtu: u8, handle: Handle, group_end_handle: Handle, mut value: &'a [u8]) -> Self {
        // 2 Bytes for `handle`, 2 Bytes for `group_end_handle`
        let max_val_len = usize::from(att_mtu - 2 - 2);
        if value.len() > max_val_len {
            value = &value[..max_val_len];
        }

        Self {
            handle,
            group_end_handle,
            value: HexSlice(value),
        }
    }

    pub fn encoded_size(&self) -> u8 {
        // 2 Bytes for `handle`, 2 Bytes for `group_end_handle`
        2 + 2 + self.value.as_ref().len() as u8
    }
}

impl<'a> FromBytes<'a> for ByGroupAttData<'a> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        Ok(ByGroupAttData {
            handle: Handle::from_bytes(bytes)?,
            group_end_handle: Handle::from_bytes(bytes)?,
            value: HexSlice(bytes.read_rest()),
        })
    }
}

/// The `ToBytes` impl will truncate the value if it doesn't fit.
impl<'a> ToBytes for ByGroupAttData<'a> {
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        writer.write_u16_le(self.handle.as_u16())?;
        writer.write_u16_le(self.group_end_handle.as_u16())?;

        // If the writer doesn't have enough space, truncate the value
        writer.write_slice_truncate(self.value.as_ref());

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
pub enum AttPdu<'a> {
    /// Request could not be completed due to an error.
    ErrorRsp {
        /// The opcode that caused the error.
        opcode: Opcode,
        /// The attribute handle on which the operation failed.
        handle: Handle,
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
        handle: Handle,
    },
    ReadRsp {
        value: HexSlice<&'a [u8]>,
    },
    ReadBlobReq {
        handle: Handle,
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
        handle: Handle,
        value: HexSlice<&'a [u8]>,
    },
    WriteRsp,
    WriteCommand {
        handle: Handle,
        value: HexSlice<&'a [u8]>,
    },
    SignedWriteCommand {
        handle: Handle,
        value: HexSlice<&'a [u8]>,
        signature: HexSlice<&'a [u8; 12]>,
    },
    PrepareWriteReq {
        handle: Handle,
        offset: u16,
        value: HexSlice<&'a [u8]>,
    },
    PrepareWriteRsp {
        handle: Handle,
        offset: u16,
        value: HexSlice<&'a [u8]>,
    },
    ExecuteWriteReq {
        /// 0x00 – Cancel all prepared writes
        /// 0x01 – Immediately write all pending prepared values
        flags: u8,
    },
    ExecuteWriteRsp,

    /// Attribute value change notification sent from server to client.
    ///
    /// Not acknowledged by client.
    HandleValueNotification {
        handle: Handle,
        value: HexSlice<&'a [u8]>,
    },

    /// Attribute value change indication sent by server, acknowledged by client.
    HandleValueIndication {
        handle: Handle,
        value: HexSlice<&'a [u8]>,
    },

    /// Confirmation returned by client in response to a `HandleValueIndication`.
    HandleValueConfirmation,
    Unknown {
        opcode: Opcode,
        params: HexSlice<&'a [u8]>,
    },
}

impl<'a> FromBytes<'a> for AttPdu<'a> {
    fn from_bytes(bytes: &mut ByteReader<'a>) -> Result<Self, Error> {
        let opcode = Opcode::from(bytes.read_u8()?);
        Ok(match opcode {
            Opcode::ErrorRsp => AttPdu::ErrorRsp {
                opcode: Opcode::from(bytes.read_u8()?),
                handle: Handle::from_bytes(bytes)?,
                error_code: ErrorCode::from(bytes.read_u8()?),
            },
            Opcode::ExchangeMtuReq => AttPdu::ExchangeMtuReq {
                mtu: bytes.read_u16_le()?,
            },
            Opcode::ExchangeMtuRsp => AttPdu::ExchangeMtuRsp {
                mtu: bytes.read_u16_le()?,
            },
            Opcode::FindInformationReq => AttPdu::FindInformationReq {
                handle_range: RawHandleRange::from_bytes(bytes)?,
            },
            Opcode::FindInformationRsp => AttPdu::FindInformationRsp {
                format: bytes.read_u8()?,
                data: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::FindByTypeValueReq => AttPdu::FindByTypeValueReq {
                handle_range: RawHandleRange::from_bytes(bytes)?,
                attribute_type: bytes.read_u16_le()?,
                attribute_value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::FindByTypeValueRsp => AttPdu::FindByTypeValueRsp {
                handles_information_list: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadByTypeReq => AttPdu::ReadByTypeReq {
                handle_range: RawHandleRange::from_bytes(bytes)?,
                attribute_type: AttUuid::from_bytes(bytes)?,
            },
            Opcode::ReadByTypeRsp => AttPdu::ReadByTypeRsp {
                length: bytes.read_u8()?,
                data_list: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadReq => AttPdu::ReadReq {
                handle: Handle::from_bytes(bytes)?,
            },
            Opcode::ReadRsp => AttPdu::ReadRsp {
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadBlobReq => AttPdu::ReadBlobReq {
                handle: Handle::from_bytes(bytes)?,
                offset: bytes.read_u16_le()?,
            },
            Opcode::ReadBlobRsp => AttPdu::ReadBlobRsp {
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadMultipleReq => AttPdu::ReadMultipleReq {
                handles: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadMultipleRsp => AttPdu::ReadMultipleRsp {
                values: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ReadByGroupReq => AttPdu::ReadByGroupReq {
                handle_range: RawHandleRange::from_bytes(bytes)?,
                group_type: AttUuid::from_bytes(bytes)?,
            },
            Opcode::ReadByGroupRsp => AttPdu::ReadByGroupRsp {
                length: bytes.read_u8()?,
                data_list: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::WriteReq => AttPdu::WriteReq {
                handle: Handle::from_bytes(bytes)?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::WriteRsp => AttPdu::WriteRsp {},
            Opcode::WriteCommand => AttPdu::WriteCommand {
                handle: Handle::from_bytes(bytes)?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::SignedWriteCommand => AttPdu::SignedWriteCommand {
                handle: Handle::from_bytes(bytes)?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left() - 12)?),
                signature: HexSlice(bytes.read_slice(12)?.try_into().unwrap()),
            },
            Opcode::PrepareWriteReq => AttPdu::PrepareWriteReq {
                handle: Handle::from_bytes(bytes)?,
                offset: bytes.read_u16_le()?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::PrepareWriteRsp => AttPdu::PrepareWriteRsp {
                handle: Handle::from_bytes(bytes)?,
                offset: bytes.read_u16_le()?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::ExecuteWriteReq => AttPdu::ExecuteWriteReq {
                flags: bytes.read_u8()?,
            },
            Opcode::ExecuteWriteRsp => AttPdu::ExecuteWriteRsp {},
            Opcode::HandleValueNotification => AttPdu::HandleValueNotification {
                handle: Handle::from_bytes(bytes)?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::HandleValueIndication => AttPdu::HandleValueIndication {
                handle: Handle::from_bytes(bytes)?,
                value: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
            Opcode::HandleValueConfirmation => AttPdu::HandleValueConfirmation {},
            Opcode::Unknown(_) => AttPdu::Unknown {
                opcode,
                params: HexSlice(bytes.read_slice(bytes.bytes_left())?),
            },
        })
    }
}

impl<'a> ToBytes for AttPdu<'a> {
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        writer.write_u8(self.opcode().into())?;

        match *self {
            AttPdu::ErrorRsp {
                opcode,
                handle,
                error_code,
            } => {
                writer.write_u8(opcode.into())?;
                writer.write_u16_le(handle.as_u16())?;
                writer.write_u8(error_code.into())?;
            }
            AttPdu::ExchangeMtuReq { mtu } => {
                writer.write_u16_le(mtu)?;
            }
            AttPdu::ExchangeMtuRsp { mtu } => {
                writer.write_u16_le(mtu)?;
            }
            AttPdu::FindInformationReq { handle_range } => {
                handle_range.to_bytes(writer)?;
            }
            AttPdu::FindInformationRsp { format, data } => {
                writer.write_u8(format)?;
                writer.write_slice(data.as_ref())?;
            }
            AttPdu::FindByTypeValueReq {
                handle_range,
                attribute_type,
                attribute_value,
            } => {
                handle_range.to_bytes(writer)?;
                writer.write_u16_le(attribute_type)?;
                writer.write_slice(attribute_value.as_ref())?;
            }
            AttPdu::FindByTypeValueRsp {
                handles_information_list,
            } => {
                writer.write_slice(handles_information_list.as_ref())?;
            }
            AttPdu::ReadByTypeReq {
                handle_range,
                attribute_type,
            } => {
                handle_range.to_bytes(writer)?;
                attribute_type.to_bytes(writer)?;
            }
            AttPdu::ReadByTypeRsp { length, data_list } => {
                writer.write_u8(length)?;
                writer.write_slice(data_list.as_ref())?;
            }
            AttPdu::ReadReq { handle } => {
                handle.to_bytes(writer)?;
            }
            AttPdu::ReadRsp { value } => {
                writer.write_slice(value.as_ref())?;
            }
            AttPdu::ReadBlobReq { handle, offset } => {
                handle.to_bytes(writer)?;
                writer.write_u16_le(offset)?;
            }
            AttPdu::ReadBlobRsp { value } => {
                writer.write_slice(value.as_ref())?;
            }
            AttPdu::ReadMultipleReq { handles } => {
                writer.write_slice(handles.as_ref())?;
            }
            AttPdu::ReadMultipleRsp { values } => {
                writer.write_slice(values.as_ref())?;
            }
            AttPdu::ReadByGroupReq {
                handle_range,
                group_type,
            } => {
                handle_range.to_bytes(writer)?;
                group_type.to_bytes(writer)?;
            }
            AttPdu::ReadByGroupRsp { length, data_list } => {
                writer.write_u8(length)?;
                writer.write_slice(data_list.as_ref())?;
            }
            AttPdu::WriteReq { handle, value } => {
                handle.to_bytes(writer)?;
                writer.write_slice(value.as_ref())?;
            }
            AttPdu::WriteRsp => {}
            AttPdu::WriteCommand { handle, value } => {
                handle.to_bytes(writer)?;
                writer.write_slice(value.as_ref())?;
            }
            AttPdu::SignedWriteCommand {
                handle,
                value,
                signature,
            } => {
                handle.to_bytes(writer)?;
                writer.write_slice(value.as_ref())?;
                writer.write_slice(*signature.as_ref())?;
            }
            AttPdu::PrepareWriteReq {
                handle,
                offset,
                value,
            } => {
                handle.to_bytes(writer)?;
                writer.write_u16_le(offset)?;
                writer.write_slice(value.as_ref())?;
            }
            AttPdu::PrepareWriteRsp {
                handle,
                offset,
                value,
            } => {
                handle.to_bytes(writer)?;
                writer.write_u16_le(offset)?;
                writer.write_slice(value.as_ref())?;
            }
            AttPdu::ExecuteWriteReq { flags } => {
                writer.write_u8(flags)?;
            }
            AttPdu::ExecuteWriteRsp => {}
            AttPdu::HandleValueNotification { handle, value } => {
                handle.to_bytes(writer)?;
                writer.write_slice_truncate(value.as_ref());
            }
            AttPdu::HandleValueIndication { handle, value } => {
                handle.to_bytes(writer)?;
                writer.write_slice_truncate(value.as_ref());
            }
            AttPdu::HandleValueConfirmation => {}
            AttPdu::Unknown { opcode: _, params } => {
                writer.write_slice(params.as_ref())?;
            }
        }

        Ok(())
    }
}

impl AttPdu<'_> {
    pub fn opcode(&self) -> Opcode {
        match self {
            AttPdu::ErrorRsp { .. } => Opcode::ErrorRsp,
            AttPdu::ExchangeMtuReq { .. } => Opcode::ExchangeMtuReq,
            AttPdu::ExchangeMtuRsp { .. } => Opcode::ExchangeMtuRsp,
            AttPdu::ReadByTypeReq { .. } => Opcode::ReadByTypeReq,
            AttPdu::ReadByTypeRsp { .. } => Opcode::ReadByTypeRsp,
            AttPdu::FindInformationReq { .. } => Opcode::FindInformationReq,
            AttPdu::FindInformationRsp { .. } => Opcode::FindInformationRsp,
            AttPdu::FindByTypeValueReq { .. } => Opcode::FindByTypeValueReq,
            AttPdu::FindByTypeValueRsp { .. } => Opcode::FindByTypeValueRsp,
            AttPdu::ReadReq { .. } => Opcode::ReadReq,
            AttPdu::ReadRsp { .. } => Opcode::ReadRsp,
            AttPdu::ReadBlobReq { .. } => Opcode::ReadBlobReq,
            AttPdu::ReadBlobRsp { .. } => Opcode::ReadBlobRsp,
            AttPdu::ReadMultipleReq { .. } => Opcode::ReadMultipleReq,
            AttPdu::ReadMultipleRsp { .. } => Opcode::ReadMultipleRsp,
            AttPdu::ReadByGroupReq { .. } => Opcode::ReadByGroupReq,
            AttPdu::ReadByGroupRsp { .. } => Opcode::ReadBlobRsp,
            AttPdu::WriteReq { .. } => Opcode::WriteReq,
            AttPdu::WriteRsp { .. } => Opcode::WriteRsp,
            AttPdu::WriteCommand { .. } => Opcode::WriteCommand,
            AttPdu::SignedWriteCommand { .. } => Opcode::SignedWriteCommand,
            AttPdu::PrepareWriteReq { .. } => Opcode::PrepareWriteReq,
            AttPdu::PrepareWriteRsp { .. } => Opcode::PrepareWriteRsp,
            AttPdu::ExecuteWriteReq { .. } => Opcode::ExecuteWriteReq,
            AttPdu::ExecuteWriteRsp { .. } => Opcode::ExecuteWriteRsp,
            AttPdu::HandleValueNotification { .. } => Opcode::HandleValueNotification,
            AttPdu::HandleValueIndication { .. } => Opcode::HandleValueIndication,
            AttPdu::HandleValueConfirmation { .. } => Opcode::HandleValueConfirmation,
            AttPdu::Unknown { opcode, .. } => *opcode,
        }
    }
}
