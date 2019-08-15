//! ATT server implementation.

use {
    super::{
        pdus::{AttPdu, ByGroupAttData, ByTypeAttData, ErrorCode, ReadByGroupRsp, ReadByTypeRsp},
        AttError, Attribute, AttributeProvider, Handle,
    },
    crate::{
        bytes::{ByteReader, FromBytes},
        l2cap::{L2CAPResponder, Protocol, ProtocolObj},
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
}

impl<A: AttributeProvider> AttributeServer<A> {
    /// Process an incoming request (or command) PDU and return a response.
    ///
    /// This may return an `AttError`, which the caller will then send as a response. In the success
    /// case, this method will send the response (if any).
    fn process_request<'a>(
        &mut self,
        msg: &AttPdu<'_>,
        responder: &mut L2CAPResponder<'_>,
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

        match msg {
            AttPdu::ExchangeMtuReq { mtu: _mtu } => {
                responder
                    .respond(AttPdu::ExchangeMtuRsp {
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

                let mut filter = |att: &mut Attribute<'_>| {
                    att.att_type == *attribute_type && range.contains(att.handle)
                };

                let result = responder.respond_with(|writer| {
                    // If no attributes match request, return `AttributeNotFound` error, else send
                    // `ReadByTypeResponse` with at least one entry
                    if self.attrs.any(&mut filter) {
                        ReadByTypeRsp {
                            item_fn: |cb: &mut dyn FnMut(
                                ByTypeAttData<'_>,
                            )
                                -> Result<(), Error>| {
                                // Build the `ByTypeAttData`s for all matching attributes and call
                                // `cb` with them.
                                self.attrs.for_each_attr(&mut |att: &mut Attribute<'_>| {
                                    if att.att_type == *attribute_type && range.contains(att.handle)
                                    {
                                        cb(ByTypeAttData::new(att.handle, att.value.as_ref()))?;
                                    }

                                    Ok(())
                                })
                            },
                        }
                        .encode(writer)?;
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

                let mut filter = |att: &mut Attribute<'_>| {
                    att.att_type == *group_type && range.contains(att.handle)
                };

                let result = responder.respond_with(|writer| {
                    // If no attributes match request, return `AttributeNotFound` error, else send
                    // `ReadByGroupResponse` with at least one entry
                    if self.attrs.any(&mut filter) {
                        ReadByGroupRsp {
                            // FIXME very poor formatting on rustfmt's part here :/
                            item_fn: |cb: &mut dyn FnMut(
                                ByGroupAttData<'_>,
                            )
                                -> Result<(), Error>| {
                                // Build the `ByGroupAttData`s for all matching attributes and call
                                // `cb` with them.
                                self.attrs.for_each_attr(&mut |att: &mut Attribute<'_>| {
                                    if att.att_type == *group_type && range.contains(att.handle) {
                                        cb(ByGroupAttData::new(
                                            att.handle,
                                            Handle::from_raw(0x003), // TODO: Ask GATT where the group ends
                                            att.value.as_ref(),
                                        ))?;
                                    }

                                    Ok(())
                                })
                            },
                        }
                        .encode(writer)?;
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
                self.attrs
                    .for_each_attr(&mut |att: &mut Attribute<'_>| {
                        // Handles are unique so this can only occur once (no bail-out required)
                        if att.handle == *handle {
                            responder
                                .respond(AttPdu::ReadRsp { value: att.value })
                                .unwrap();
                        }

                        Ok(())
                    })
                    .unwrap();

                Err(AttError::attribute_not_found())
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
    fn process_message(
        &mut self,
        message: &[u8],
        mut responder: L2CAPResponder<'_>,
    ) -> Result<(), Error> {
        let pdu = &AttPdu::from_bytes(&mut ByteReader::new(message))?;
        let opcode = pdu.opcode();
        debug!("ATT msg received: {:?}", pdu);

        match self.process_request(pdu, &mut responder) {
            Ok(()) => Ok(()),
            Err(att_error) => {
                debug!("ATT error: {:?}", att_error);

                responder.respond(AttPdu::ErrorRsp {
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
