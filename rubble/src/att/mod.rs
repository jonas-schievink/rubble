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
//! * A 16-bit *Attribute Handle* ([`Handle`]) uniquely identifying the attribute.
//! * A 16- or 128-bit UUID identifying the attribute type. This provides information about how to
//!   interpret the attribute's value (eg. as a little-endian 32-bit integer).
//! * The attribute's *value*, consisting of a dynamically-sized byte array of up to 512 Bytes.
//! * A set of *permissions*, restricting the operations that can be performed on the attribute.
//!
//! ## Attribute Grouping
//!
//! The higher-level protocol layer using ATT (ie. GATT) may define a set of attribute types as
//! *Grouping Attributes*. These attribute types are allowed for use in *Read By Group Type*
//! requests, and can also influence the behavior of other requests (such as *Find By Type Value*).
//!
//! All *Grouping Attributes* can be seen as the start of a group of attributes. Groups are formed
//! by indicating the *Group End Handle* to the client, which is the handle of the last attribute in
//! the group. The *Group End Handle* isn't known by the ATT server and must be provided by the
//! higher-level protocol (GATT).
//!
//! [`Handle`]: struct.Handle.html

mod handle;
mod pdus;
mod server;
mod uuid;

use {
    self::{handle::*, pdus::*},
    crate::{utils::HexSlice, Error},
};

pub use self::handle::{Handle, HandleRange};
pub use self::server::{AttributeServer, AttributeServerTx};
pub use self::uuid::AttUuid;

/// An ATT server attribute
pub struct Attribute<'a> {
    /// The type of the attribute as a UUID16, EG "Primary Service" or "Anaerobic Heart Rate Lower Limit"
    pub att_type: AttUuid,
    /// Unique server-side identifer for attribute
    pub handle: Handle,
    /// Attribute values can be any fixed length or variable length octet array, which if too large
    /// can be sent across multiple PDUs
    pub value: HexSlice<&'a [u8]>,
}

/// Trait for attribute sets that can be hosted by an `AttributeServer`.
pub trait AttributeProvider {
    /// Calls a closure `f` with every attribute whose handle is inside `range`, ascending.
    ///
    /// If `f` returns an error, this function will stop calling `f` and propagate the error
    /// upwards. If `f` returns `Ok(())`, iteration will continue.
    ///
    /// This function would ideally return an iterator instead of invoking a callback, but it's not
    /// currently possible to express the iterator type generically (it would need lifetime-generic
    /// associated types), and all workarounds seem to be severely limiting.
    fn for_attrs_in_range(
        &mut self,
        range: HandleRange,
        f: impl FnMut(&Self, Attribute<'_>) -> Result<(), Error>,
    ) -> Result<(), Error>;

    /// Returns whether `uuid` is a valid grouping attribute type that can be used in *Read By
    /// Group Type* requests.
    fn is_grouping_attr(&self, uuid: AttUuid) -> bool;

    /// Queries the last attribute that is part of the attribute group denoted by the grouping
    /// attribute at `handle`.
    ///
    /// If `handle` does not refer to a grouping attribute, returns `None`.
    ///
    /// TODO: Human-readable docs that explain what grouping is
    fn group_end(&self, handle: Handle) -> Option<&Attribute<'_>>;
}

/// An empty attribute set.
///
/// FIXME: Is this even legal according to the spec?
pub struct NoAttributes;

impl AttributeProvider for NoAttributes {
    fn for_attrs_in_range(
        &mut self,
        _range: HandleRange,
        _f: impl FnMut(&Self, Attribute<'_>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn is_grouping_attr(&self, _uuid: AttUuid) -> bool {
        false
    }

    fn group_end(&self, _handle: Handle) -> Option<&Attribute<'_>> {
        None
    }
}
