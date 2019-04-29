//! Attribute handles.

use {
    crate::{
        att::{AttError, ErrorCode},
        bytes::*,
        Error,
    },
    core::{fmt, ops::RangeInclusive},
};

/// A 16-bit handle uniquely identifying an attribute on an ATT server.
///
/// The `0x0000` handle (`NULL`) is invalid and must not be used.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AttHandle(u16);

impl AttHandle {
    /// The `0x0000` handle is not used for actual attributes, but as a special placeholder when no
    /// attribute handle is valid (eg. in error responses).
    pub const NULL: Self = AttHandle(0x0000);

    /// Returns the raw 16-bit integer representing this handle.
    pub fn as_u16(&self) -> u16 {
        self.0
    }

    /// Create an attribute handle from a raw u16
    pub fn from_raw(raw: u16) -> Self {
        AttHandle(raw)
    }
}

impl fmt::Debug for AttHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#06X}", self.0)
    }
}

impl FromBytes<'_> for AttHandle {
    fn from_bytes(bytes: &mut ByteReader) -> Result<Self, Error> {
        Ok(AttHandle(bytes.read_u16_le()?))
    }
}

/// A (de)serializable handle range that isn't checked for validity.
#[derive(Debug, Copy, Clone)]
pub struct RawHandleRange {
    start: AttHandle,
    end: AttHandle,
}

impl RawHandleRange {
    /// Checks that this handle range is valid according to the Bluetooth spec.
    ///
    /// Returns an `AttError` that should be sent as a response if the range is invalid.
    pub fn check(&self) -> Result<HandleRange, AttError> {
        if self.start.0 > self.end.0 || self.start.0 == 0 {
            Err(AttError {
                code: ErrorCode::InvalidHandle,
                handle: self.start,
            })
        } else {
            Ok(HandleRange(self.start..=self.end))
        }
    }
}

impl FromBytes<'_> for RawHandleRange {
    fn from_bytes(bytes: &mut ByteReader) -> Result<Self, Error> {
        Ok(Self {
            start: AttHandle::from_bytes(bytes)?,
            end: AttHandle::from_bytes(bytes)?,
        })
    }
}

impl ToBytes for RawHandleRange {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u16_le(self.start.as_u16())?;
        writer.write_u16_le(self.end.as_u16())?;
        Ok(())
    }
}

/// A (de)serializable handle range that has been checked for validity.
#[derive(Debug)]
pub struct HandleRange(RangeInclusive<AttHandle>);

impl HandleRange {
    /// Checks if an AttHandle is in a HandleRange
    pub fn contains(&self, handle: AttHandle) -> bool {
        self.0.start().0 <= handle.as_u16() && self.0.end().0 >= handle.as_u16()
    }

    pub fn start(&self) -> AttHandle {
        *self.0.start()
    }
}
