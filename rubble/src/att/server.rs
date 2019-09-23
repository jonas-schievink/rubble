//! ATT server implementation.

use {
    super::{
        pdus::{AttPdu, ByGroupAttData, ByTypeAttData, ErrorCode, Opcode},
        AttError, AttributeProvider, Handle, HandleRange,
    },
    crate::{
        bytes::{ByteReader, FromBytes, ToBytes},
        l2cap::{Protocol, ProtocolObj, Sender},
        Error,
    },
};

/// An Attribute Protocol server providing read and write access to stored attributes.
pub struct AttributeServer<A: AttributeProvider> {
    attrs: A,
}

impl<A: AttributeProvider> AttributeServer<A> {
    /// Creates an AttributeServer with Attributes
    pub fn new(attrs: A) -> Self {
        Self { attrs }
    }

    /// Returns the `ATT_MTU` value, the maximum size of an ATT PDU that can be processed and sent
    /// out by the server.
    fn att_mtu(&self) -> u8 {
        Self::RSP_PDU_SIZE
    }
}

impl<A: AttributeProvider> AttributeServer<A> {
    /// Process an incoming request (or command) PDU and return a response.
    ///
    /// This may return an `AttError`, which the caller will then send as a response. In the success
    /// case, this method will send the response (if any).
    fn process_request(
        &mut self,
        msg: &AttPdu<'_>,
        responder: &mut Sender<'_>,
    ) -> Result<(), AttError> {
        /// Error returned when an ATT error should be sent back.
        ///
        /// Returning this from inside `responder.send_with` will not send the response and
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

        match msg {
            AttPdu::ExchangeMtuReq { mtu: _mtu } => {
                responder
                    .send(AttPdu::ExchangeMtuRsp {
                        mtu: u16::from(Self::RSP_PDU_SIZE),
                    })
                    .unwrap();
                Ok(())
            }

            AttPdu::ReadByTypeReq {
                handle_range,
                attribute_type,
            } => {
                let range = handle_range.check()?;

                let result = responder.send_with(|writer| {
                    // If no attributes match request, return `AttributeNotFound` error, else send
                    // `ReadByTypeResponse` with at least one entry

                    writer.write_u8(Opcode::ReadByTypeRsp.into())?;
                    let length = writer.split_next_mut().ok_or(Error::Eof)?;

                    let mut size = None;
                    let att_mtu = self.att_mtu();
                    self.attrs
                        .for_attrs_in_range(range, |_provider, attr| {
                            if attr.att_type == *attribute_type {
                                let data =
                                    ByTypeAttData::new(att_mtu, attr.handle, attr.value.as_ref());
                                if size == Some(data.encoded_size()) || size.is_none() {
                                    // Can try to encode `data`. If we run out of space, end the list.
                                    data.to_bytes(writer)?;
                                    size = Some(data.encoded_size());
                                }
                            }

                            Ok(())
                        })
                        .ok();

                    if let Some(size) = size {
                        // At least one attr
                        *length = size;
                        Ok(())
                    } else {
                        Err(AttError::attribute_not_found().into())
                    }
                });

                match result {
                    Ok(()) => Ok(()),
                    Err(RspError(e)) => Err(e),
                }
            }

            AttPdu::ReadByGroupReq {
                handle_range,
                group_type,
            } => {
                let range = handle_range.check()?;

                // Reject if `group_type` is not a grouping attribute
                if !self.attrs.is_grouping_attr(*group_type) {
                    return Err(AttError::new(
                        ErrorCode::UnsupportedGroupType,
                        range.start(),
                    ));
                }

                let result = responder.send_with(|writer| {
                    // If no attributes match request, return `AttributeNotFound` error, else send
                    // response with at least one entry.

                    writer.write_u8(Opcode::ReadByGroupRsp.into())?;
                    let length = writer.split_next_mut().ok_or(Error::Eof)?;

                    let mut size = None;
                    let att_mtu = self.att_mtu();
                    self.attrs
                        .for_attrs_in_range(range, |provider, attr| {
                            if attr.att_type == *group_type {
                                let data = ByGroupAttData::new(
                                    att_mtu,
                                    attr.handle,
                                    provider.group_end(attr.handle).unwrap().handle,
                                    attr.value.as_ref(),
                                );
                                if size == Some(data.encoded_size()) || size.is_none() {
                                    // Can try to encode `data`. If we run out of space, end the list.
                                    data.to_bytes(writer)?;
                                    size = Some(data.encoded_size());
                                }
                            }

                            Ok(())
                        })
                        .ok();

                    if let Some(size) = size {
                        // At least one attr
                        *length = size;
                        debug!(
                            "ATT->ReadByGroupRsp (size={}, left={})",
                            size,
                            writer.space_left()
                        );
                        Ok(())
                    } else {
                        Err(AttError::attribute_not_found().into())
                    }
                });

                match result {
                    Ok(()) => Ok(()),
                    Err(RspError(e)) => Err(e),
                }
            }

            AttPdu::ReadReq { handle } => {
                responder
                    .send_with(|writer| -> Result<(), Error> {
                        writer.write_u8(Opcode::ReadRsp.into())?;

                        self.attrs.for_attrs_in_range(
                            HandleRange::new(*handle, *handle),
                            |_provider, attr| {
                                let value = if writer.space_left() < attr.value.as_ref().len() {
                                    &attr.value.as_ref()[..writer.space_left()]
                                } else {
                                    attr.value.as_ref()
                                };
                                writer.write_slice(value)
                            },
                        )?;

                        Ok(())
                    })
                    .unwrap();

                Ok(())
            }

            // Responses are always invalid here
            AttPdu::ErrorRsp { .. }
            | AttPdu::ExchangeMtuRsp { .. }
            | AttPdu::FindInformationRsp { .. }
            | AttPdu::FindByTypeValueRsp { .. }
            | AttPdu::ReadByTypeRsp { .. }
            | AttPdu::ReadRsp { .. }
            | AttPdu::ReadBlobRsp { .. }
            | AttPdu::ReadMultipleRsp { .. }
            | AttPdu::ReadByGroupRsp { .. }
            | AttPdu::WriteRsp { .. }
            | AttPdu::PrepareWriteRsp { .. }
            | AttPdu::ExecuteWriteRsp { .. }
            | AttPdu::HandleValueNotification { .. }
            | AttPdu::HandleValueIndication { .. } => {
                Err(AttError::new(ErrorCode::InvalidPdu, Handle::NULL))
            }

            // Unknown (undecoded) or unimplemented requests and commands
            AttPdu::Unknown { .. }
            | AttPdu::FindInformationReq { .. }
            | AttPdu::FindByTypeValueReq { .. }
            | AttPdu::ReadBlobReq { .. }
            | AttPdu::ReadMultipleReq { .. }
            | AttPdu::WriteReq { .. }
            | AttPdu::WriteCommand { .. }
            | AttPdu::SignedWriteCommand { .. }
            | AttPdu::PrepareWriteReq { .. }
            | AttPdu::ExecuteWriteReq { .. }
            | AttPdu::HandleValueConfirmation { .. } => {
                if msg.opcode().is_command() {
                    // According to the spec, unknown Command PDUs should be ignored
                    Ok(())
                } else {
                    // Unknown requests are rejected with a `RequestNotSupported` error
                    Err(AttError::new(ErrorCode::RequestNotSupported, Handle::NULL))
                }
            }
        }
    }
}

impl<A: AttributeProvider> ProtocolObj for AttributeServer<A> {
    fn process_message(&mut self, message: &[u8], mut responder: Sender<'_>) -> Result<(), Error> {
        let pdu = &AttPdu::from_bytes(&mut ByteReader::new(message))?;
        let opcode = pdu.opcode();
        debug!("ATT<- {:?}", pdu);

        match self.process_request(pdu, &mut responder) {
            Ok(()) => Ok(()),
            Err(att_error) => {
                debug!("ATT-> {:?}", att_error);

                responder.send(AttPdu::ErrorRsp {
                    opcode: opcode,
                    handle: att_error.handle(),
                    error_code: att_error.error_code(),
                })
            }
        }
    }
}

impl<A: AttributeProvider> Protocol for AttributeServer<A> {
    // FIXME: Would it be useful to have this as a runtime parameter instead?
    const RSP_PDU_SIZE: u8 = 23;
}
