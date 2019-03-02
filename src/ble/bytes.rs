//! Utilities for parsing from and encoding into bytes.

use {
    crate::ble::{utils::SliceExt, Error},
    core::{fmt, iter, mem},
};

/// Reference to a `T`, or to a byte slice that can be decoded as a `T`.
pub struct BytesOr<'a, T: ?Sized>(Inner<'a, T>);

enum Inner<'a, T: ?Sized> {
    Bytes(&'a [u8]),
    Or(&'a T),
}

impl<'a, T: ?Sized> Clone for Inner<'a, T> {
    fn clone(&self) -> Self {
        match self {
            Inner::Bytes(b) => Inner::Bytes(b),
            Inner::Or(t) => Inner::Or(t),
        }
    }
}

impl<'a, T: ?Sized> Clone for BytesOr<'a, T> {
    fn clone(&self) -> Self {
        BytesOr(self.0.clone())
    }
}

impl<'a, T: ?Sized> Copy for BytesOr<'a, T> {}
impl<'a, T: ?Sized> Copy for Inner<'a, T> {}

impl<'a, T: fmt::Debug + FromBytes<'a> + Copy> fmt::Debug for BytesOr<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.read().fmt(f)
    }
}

impl<'a, T: fmt::Debug + FromBytes<'a> + Copy> fmt::Debug for BytesOr<'a, [T]> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<'a, T: ?Sized> BytesOr<'a, T> {
    /// Creates a `BytesOr` that holds on to a `T` via reference.
    pub fn from_ref(value: &'a T) -> Self {
        BytesOr(Inner::Or(value))
    }
}

/// Creates a `BytesOr` that stores bytes that can be decoded to a `T`.
///
/// This will check that `bytes` can indeed be decoded as a `T`, and returns
/// an error if not.
impl<'a, T: FromBytes<'a>> FromBytes<'a> for BytesOr<'a, T> {
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error> {
        {
            let mut bytes = &mut *bytes;
            T::from_bytes(&mut bytes)?;
            if !bytes.is_empty() {
                return Err(Error::IncompleteParse);
            }
        }
        Ok(BytesOr(Inner::Bytes(bytes)))
    }
}

/// Creates a `BytesOr` that stores bytes that can be decoded to a sequence
/// of `T`s.
///
/// This will check that `bytes` can indeed be decoded as a sequence of
/// `T`s, and returns an error if not.
impl<'a, T: FromBytes<'a>> FromBytes<'a> for BytesOr<'a, [T]> {
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error> {
        {
            let mut bytes = &mut *bytes;
            T::from_bytes(&mut bytes)?;
            if !bytes.is_empty() {
                return Err(Error::IncompleteParse);
            }
        }
        Ok(BytesOr(Inner::Bytes(bytes)))
    }
}

impl<'a, T: ToBytes> ToBytes for BytesOr<'a, T> {
    fn to_bytes(&self, buffer: &mut ByteWriter) -> Result<(), Error> {
        match self.0 {
            Inner::Bytes(b) => buffer.write_slice(b),
            Inner::Or(t) => t.to_bytes(buffer),
        }
    }
}

impl<'a, T: ToBytes> ToBytes for BytesOr<'a, [T]> {
    fn to_bytes(&self, buffer: &mut ByteWriter) -> Result<(), Error> {
        match self.0 {
            Inner::Bytes(b) => buffer.write_slice(b),
            Inner::Or(ts) => {
                for t in ts {
                    t.to_bytes(buffer)?;
                }
                Ok(())
            }
        }
    }
}

impl<'a, T: Copy + FromBytes<'a>> BytesOr<'a, T> {
    /// Reads the `T`, possibly by parsing the stored bytes.
    pub fn read(&self) -> T {
        match self.0 {
            Inner::Bytes(mut b) => {
                let t = T::from_bytes(&mut b).unwrap();
                assert!(b.is_empty());
                t
            }
            Inner::Or(t) => *t,
        }
    }
}

impl<'a, T: Copy + FromBytes<'a>> BytesOr<'a, T> {
    #[allow(dead_code)] // FIXME: USE ME!
    pub fn iter(&self) -> impl Iterator<Item = T> + 'a {
        iter::once(self.read())
    }
}

impl<'a, T: Copy + FromBytes<'a>> BytesOr<'a, [T]> {
    /// Returns an iterator over all `T`s stored in `self`.
    ///
    /// The iterator will copy or decode `T`s out of `self`.
    pub fn iter(&self) -> impl Iterator<Item = T> + 'a {
        IterBytesOr { inner: *self }
    }
}

/// An iterator over values stored in a `BytesOr`.
struct IterBytesOr<'a, T> {
    inner: BytesOr<'a, [T]>,
}

impl<'a, T: Copy + FromBytes<'a>> Iterator for IterBytesOr<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner.0 {
            Inner::Bytes(b) => {
                if b.is_empty() {
                    None
                } else {
                    Some(T::from_bytes(b).unwrap())
                }
            }
            Inner::Or(slice) => slice.read_first(),
        }
    }
}

/// Wrapper around a byte slice that can be used to encode data into bytes.
pub struct ByteWriter<'a>(&'a mut [u8]);

impl<'a> ByteWriter<'a> {
    /// Creates a writer that will write to `buf`.
    pub fn new(buf: &'a mut [u8]) -> Self {
        ByteWriter(buf)
    }

    pub fn into_inner(self) -> &'a mut [u8] {
        self.0
    }

    /// Returns the number of bytes that can be written to `self` until it is full.
    pub fn space_left(&self) -> usize {
        self.0.len()
    }

    pub fn write_byte<'b>(&'b mut self, byte: u8) -> Result<(), Error>
    where
        'a: 'b,
    {
        let first = self.split_next_mut().ok_or(Error::Eof)?;
        *first = byte;
        Ok(())
    }

    pub fn write_slice<'b>(&'b mut self, other: &[u8]) -> Result<(), Error>
    where
        'a: 'b,
    {
        eof_unless!(self.space_left() >= other.len());
        self.0[..other.len()].copy_from_slice(other);
        let this = mem::replace(&mut self.0, &mut []);
        self.0 = &mut this[other.len()..];
        Ok(())
    }

    /// Splits off the next byte in the buffer.
    ///
    /// The writer will be advanced to point to the rest of the underlying
    /// buffer.
    ///
    /// This allows filling in the value of the byte later, after writing more
    /// data.
    pub fn split_next_mut<'b>(&'b mut self) -> Option<&'a mut u8>
    where
        'a: 'b,
    {
        let this = mem::replace(&mut self.0, &mut []);
        // Slight contortion to please the borrow checker:
        if this.is_empty() {
            self.0 = this;
            None
        } else {
            let (first, rest) = this.split_first_mut().unwrap();
            self.0 = rest;
            Some(first)
        }
    }
}

/// Trait for encoding a value into a byte buffer.
pub trait ToBytes {
    /// Converts `self` to bytes and writes them into `buffer`, advancing
    /// `buffer` to point past the encoded value.
    ///
    /// If `buffer` does not contain enough space, an error will be returned and
    /// the state of the buffer is unspecified (eg. `self` may be partially
    /// written into `buffer`).
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error>;
}

/// Trait for decoding values from a slice.
pub trait FromBytes<'a>: Sized {
    /// Decode a `Self` from a byte slice, advancing `bytes` to point past the
    /// data that was read.
    ///
    /// If `bytes` contains data not valid for the target type, or contains an
    /// insufficient number of bytes, an error will be returned and the state of
    /// `bytes` is unspecified (it can point to arbitrary data).
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error>;
}
