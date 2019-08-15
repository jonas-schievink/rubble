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
mod pdus;
mod server;
mod uuid;

use {
    self::{handle::*, pdus::*},
    crate::{bytes::*, utils::HexSlice, Error},
};

pub use self::handle::AttHandle;
pub use self::server::AttributeServer;
pub use self::uuid::AttUuid;

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

        let msg = AttMsg::from_reader(bytes, opcode)?;

        if auth {
            // Ignore signature
            bytes.skip(12)?;
        }
        Ok(OutgoingPdu(msg))
    }
}

impl ToBytes for OutgoingPdu<'_> {
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        writer.write_u8(self.0.opcode().into())?;

        self.0.to_writer(writer)?;

        if self.0.opcode().is_authenticated() {
            // Write a dummy signature. This should never really be reached since the server never
            // sends authenticated messages.
            writer.write_slice(&[0; 12])?;
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
}

/// Trait for attribute sets that can be hosted by an `AttributeServer`.
pub trait AttributeProvider {
    /// Calls a closure `f` with every attribute stored in `self`.
    ///
    /// All attributes will have ascending, consecutive handle values starting at `0x0001`.
    ///
    /// If `f` returns an error, this function will stop calling `f` and propagate the error
    /// upwards. If `f` returns `Ok(())`, iteration will continue.
    fn for_each_attr(
        &mut self,
        f: &mut dyn FnMut(&mut Attribute<'_>) -> Result<(), Error>,
    ) -> Result<(), Error>;

    /// Returns whether the `filter` closure matches any attribute in `self`.
    fn any(&mut self, filter: &mut dyn FnMut(&mut Attribute<'_>) -> bool) -> bool {
        match self.for_each_attr(&mut |att| {
            if filter(att) {
                Err(Error::Eof)
            } else {
                Ok(())
            }
        }) {
            Err(Error::Eof) => true,
            _ => false,
        }
    }

    /// Returns whether `uuid` is a valid grouping attribute that can be used in *Read By Group
    /// Type* requests.
    fn is_grouping_attr(&self, uuid: AttUuid) -> bool;

    /// Queries the last attribute that is part of the attribute group denoted by the grouping
    /// attribute at `handle`.
    ///
    /// If `handle` does not refer to a grouping attribute, returns `None`.
    ///
    /// TODO: Human-readable docs that explain what grouping is
    fn group_end(&self, handle: AttHandle) -> Option<&Attribute<'_>>;
}

/// An empty attribute set.
///
/// FIXME: Is this even legal according to the spec?
pub struct NoAttributes;

impl AttributeProvider for NoAttributes {
    fn for_each_attr(
        &mut self,
        _: &mut dyn FnMut(&mut Attribute<'_>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn is_grouping_attr(&self, _uuid: AttUuid) -> bool {
        false
    }

    fn group_end(&self, _handle: AttHandle) -> Option<&Attribute<'_>> {
        None
    }
}
