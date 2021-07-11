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

mod handle;
mod pdus;
mod server;
mod uuid;

use self::{handle::*, pdus::*};
use crate::{l2cap::Sender, Error};

pub use self::handle::{Handle, HandleRange};
pub use self::server::{AttributeServer, AttributeServerTx};
pub use self::uuid::AttUuid;

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

impl<T: AsRef<[u8]>> Attribute<T> {
    /// Creates a new attribute.
    pub fn new(att_type: AttUuid, handle: Handle, value: T) -> Self {
        assert_ne!(handle, Handle::NULL);
        Attribute {
            att_type,
            handle,
            value,
        }
    }

    /// Retrieves the attribute's value as a slice.
    pub fn value(&self) -> &[u8] {
        self.value.as_ref()
    }

    /// Overrides the previously set attribute's value.
    pub fn set_value(&mut self, value: T) {
        self.value = value;
    }
}

pub enum AttributeAccessPermissions {
    Readable,
    Writeable,
    ReadableAndWriteable,
}

impl AttributeAccessPermissions {
    fn is_readable(&self) -> bool {
        match self {
            AttributeAccessPermissions::Readable
            | AttributeAccessPermissions::ReadableAndWriteable => true,
            AttributeAccessPermissions::Writeable => false,
        }
    }
    fn is_writeable(&self) -> bool {
        match self {
            AttributeAccessPermissions::Writeable
            | AttributeAccessPermissions::ReadableAndWriteable => true,
            AttributeAccessPermissions::Readable => false,
        }
    }
}

impl Default for AttributeAccessPermissions {
    fn default() -> Self {
        AttributeAccessPermissions::Readable
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
        f: impl FnMut(&Self, &Attribute<dyn AsRef<[u8]>>) -> Result<(), Error>,
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
    fn group_end(&self, handle: Handle) -> Option<&Attribute<dyn AsRef<[u8]>>>;

    /// Retrieves the permissions for the given attribute.
    ///
    /// These are used purely for access control within rubble, and won't be
    /// communicated with clients. They should be coordinated beforehand as part
    /// of a larger protocol.
    ///
    /// Defaults to read-only. If this is overwritten and some attributes are made writeable,
    /// `write_attribute` must be implemented as well.
    fn attr_access_permissions(&self, _handle: Handle) -> AttributeAccessPermissions {
        AttributeAccessPermissions::Readable
    }

    /// Attempts to write data to the given attribute.
    ///
    /// This will only be called on handles for which
    /// `attribute_access_permissions` returns
    /// [`AttributeAccessPermissions::Writeable`]
    /// or [`AttributeAccessPermissions::ReadableAndWriteable`].
    ///
    /// By default, panics on all writes. This must be overwritten if
    /// `attribute_access_permissions` is.
    fn write_attr(&mut self, _handle: Handle, _data: &[u8]) -> Result<(), Error> {
        unimplemented!("by default, no attributes should have write access permissions, and this should never be called");
    }

    /// If this read is from dynamic data fill the buffer and return the length of the data.
    /// If not return None.
    ///
    /// Currently the buffer is 256 bytes.
    ///
    /// By default returns `None`.
    fn read_attr_dynamic(&mut self, _handle: Handle, _buffer: &mut [u8]) -> Option<usize> {
        None
    }

    /// In order to write data longer than what would fit one write request the procedure is explained in
    /// BLUETOOTH CORE SPECIFICATION Version 5.2 | Vol 3, Part F section 3.4.6.
    fn prepare_write_attr(
        &mut self,
        _handle: Handle,
        _offset: u16,
        _data: &[u8],
    ) -> Result<(), Error> {
        unimplemented!("you need to implement prepare_write_attr to make queued writes work")
    }

    /// In order to write data longer than what would fit one write request the procedure is explained in
    /// BLUETOOTH CORE SPECIFICATION Version 5.2 | Vol 3, Part F section 3.4.6.
    fn execute_write_attr(&mut self, _flags: u8) -> Result<(), Error> {
        unimplemented!("you need to implement execute_write_attr to make queued writes work")
    }

    /// See BLUETOOTH CORE SPECIFICATION Version 5.2 | Vol 3, Part F section 3.4.3.1 on what to implement here.
    fn find_information(
        &mut self,
        _range: HandleRange,
        _responder: &mut Sender<'_>,
    ) -> Result<(), Error> {
        unimplemented!("you need to implement find_information to make things like Client Characteristic Configuration work")
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
        _f: impl FnMut(&Self, &Attribute<dyn AsRef<[u8]>>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn is_grouping_attr(&self, _uuid: AttUuid) -> bool {
        false
    }

    fn group_end(&self, _handle: Handle) -> Option<&Attribute<dyn AsRef<[u8]>>> {
        None
    }
}
