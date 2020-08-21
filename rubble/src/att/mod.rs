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

use self::{handle::*, pdus::*};
use crate::Error;

pub use self::handle::{Handle, HandleRange};
pub use self::server::{AttributeServer, AttributeServerTx};
pub use self::uuid::AttUuid;

/// An attribute value that can be represented as a byte slice.
pub trait AttrValue {
    fn as_slice(&self) -> &[u8];
}

impl AttrValue for &[u8] {
    fn as_slice(&self) -> &[u8] {
        self
    }
}

impl AttrValue for () {
    fn as_slice(&self) -> &[u8] {
        &[]
    }
}

/// An ATT server attribute
pub struct Attribute<T>
where
    T: ?Sized,
{
    /// The type of the attribute as a UUID16, EG "Primary Service" or "Anaerobic Heart Rate Lower Limit"
    pub att_type: AttUuid,
    /// Unique server-side identifer for attribute
    pub handle: Handle,
    /// Attribute values can be any fixed length or variable length octet array, which if too large
    /// can be sent across multiple PDUs
    pub value: T,
}

impl<T: AttrValue> Attribute<T> {
    /// Creates a new attribute.
    pub fn new(att_type: AttUuid, handle: Handle, value: T) -> Self {
        assert_ne!(handle, Handle::NULL);
        Attribute {
            att_type,
            handle,
            value: value,
        }
    }

    /// Retrieves the attribute's value as a slice.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }

    /// Overrides the previously set attribute's value.
    pub fn set_value(&mut self, value: T) {
        self.value = value;
    }
}

pub enum AttributeAccessPermissions {
    Readable,
    Writeable,
    ReadableAndWritable,
}

impl AttributeAccessPermissions {
    fn can_read(&self) -> bool {
        match self {
            AttributeAccessPermissions::Readable
            | AttributeAccessPermissions::ReadableAndWritable => true,
            AttributeAccessPermissions::Writeable => false,
        }
    }
    fn can_write(&self) -> bool {
        match self {
            AttributeAccessPermissions::Writeable
            | AttributeAccessPermissions::ReadableAndWritable => true,
            AttributeAccessPermissions::Readable => false,
        }
    }
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
        f: impl FnMut(&Self, &Attribute<dyn AttrValue>) -> Result<(), Error>,
    ) -> Result<(), Error>;

    /// Returns whether `uuid` is a valid grouping attribute type that can be used in *Read By
    /// Group Type* requests.
    fn is_grouping_attr(&self, uuid: AttUuid) -> bool;

    /// Queries the last attribute that is part of the attribute group denoted by the grouping
    /// attribute at `handle`.
    ///
    /// If `handle` does not refer to a grouping attribute, returns `None`.
    ///
    /// Groups themselves are collections of attributes. An attribute is in
    /// exactly zero or one groups.
    ///
    /// For primary services and secondary services, the BLE specification gives
    /// exact definitions of what's in the group. The group begins with the
    /// "primary service" or "secondary service" attribute, is followed by
    /// the various attributes contained within that service, and ends with the
    /// last attribute contained within that service.
    ///
    /// TODO: document what the BLE spec has to say about grouping for characteristics.
    fn group_end(&self, handle: Handle) -> Option<&Attribute<dyn AttrValue>>;

    /// Retrieves the permissions for the given attribute.
    ///
    /// These are used purely for access control within rubble, and won't be
    /// communicated with clients. They should be coordinated beforehand as part
    /// of a larger protocol.
    ///
    /// Defaults to read-only. If this is overwritten, `write_attribute` should
    /// be overwritten.
    fn attribute_access_permissions(&self, handle: Handle) -> AttributeAccessPermissions {
        AttributeAccessPermissions::Readable
    }

    /// Attempts to write data to the given attribute.
    ///
    /// This will only be called on UUIDs for which
    /// `attribute_access_permissions` returns
    /// [`AttributeAccessPermissions::Writeable`] or [`AttributeAccessPermission::ReadableAndWriteable`].
    ///
    /// By default, panics on all writes. This should be overwritten if
    /// `attribute_access_permissions` is.
    fn write_attribute(&mut self, handle: Handle, data: &[u8]) -> Result<(), Error> {
        unimplemented!("by default, no attributes should have write access permissions, and this should never be called");
    }
}

/// An empty attribute set.
///
/// FIXME: Is this even legal according to the spec?
pub struct NoAttributes;

impl AttributeProvider for NoAttributes {
    fn for_attrs_in_range(
        &mut self,
        _range: HandleRange,
        _f: impl FnMut(&Self, &Attribute<dyn AttrValue>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn is_grouping_attr(&self, _uuid: AttUuid) -> bool {
        false
    }

    fn group_end(&self, _handle: Handle) -> Option<&Attribute<dyn AttrValue>> {
        None
    }
}
