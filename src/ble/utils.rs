use {crate::ble::Error, core::mem};

/// Creates an enum that can be converted from and to a primitive type, with invalid values becoming
/// a catch-all `Unknown` variant.
///
/// This is copied almost verbatim from [smoltcp].
///
/// [smoltcp]: https://github.com/m-labs/smoltcp/blob/cd893e6ab60f094d684b37be7bc013bf79f0459d/src/macros.rs
macro_rules! enum_with_unknown {
    (
        $( #[$enum_attr:meta] )*
        pub enum $name:ident($ty:ty) {
            $(
              $( #[$variant_attr:meta] )*
              $variant:ident = $value:expr $(,)*
            ),+
        }
    ) => {
        $( #[$enum_attr] )*
        pub enum $name {
            $(
              $( #[$variant_attr] )*
              $variant
            ),*,
            Unknown($ty)
        }

        impl ::core::convert::From<$ty> for $name {
            fn from(value: $ty) -> Self {
                match value {
                    $( $value => $name::$variant ),*,
                    other => $name::Unknown(other)
                }
            }
        }

        impl ::core::convert::From<$name> for $ty {
            fn from(value: $name) -> Self {
                match value {
                    $( $name::$variant => $value ),*,
                    $name::Unknown(other) => other
                }
            }
        }
    }
}

/// Early-return `Error::Eof` if the given expression evaluates to `false`.
macro_rules! eof_unless {
    ( $e:expr ) => {
        if !$e {
            return Err(Error::Eof);
        }
    };
}

/// Reference to a `T`, or to a byte slice that can be decoded as a `T`.
pub enum BytesOr<'a, T: ?Sized> {
    Bytes(&'a [u8]),
    Or(&'a T),
}

impl<'a, T: ?Sized> Clone for BytesOr<'a, T> {
    fn clone(&self) -> Self {
        match self {
            BytesOr::Bytes(b) => BytesOr::Bytes(b),
            BytesOr::Or(t) => BytesOr::Or(t),
        }
    }
}

impl<'a, T: ?Sized> Copy for BytesOr<'a, T> {}

impl<'a, T: ?Sized> BytesOr<'a, T> {
    pub fn from_bytes(bytes: &'a [u8]) -> Self {
        BytesOr::Bytes(bytes)
    }

    pub fn from_ref(value: &'a T) -> Self {
        BytesOr::Or(value)
    }
}

impl<'a, T: Copy + FromBytes> BytesOr<'a, T> {
    /// Reads the `T`, possibly by parsing the stored bytes.
    #[allow(dead_code)] // FIXME: USE ME!
    pub fn read(&self) -> Result<T, Error> {
        match self {
            BytesOr::Bytes(mut b) => {
                let t = T::from_bytes(&mut b)?;
                if b.is_empty() {
                    Ok(t)
                } else {
                    Err(Error::IncompleteParse)
                }
            }
            BytesOr::Or(t) => Ok(**t),
        }
    }
}

impl<'a, T: Copy + FromBytes> BytesOr<'a, [T]> {
    /// Returns an iterator over all `T`s stored in `self`.
    ///
    /// If `self` stored a `[T]` directly, the iterator will simply copy the
    /// elements out of the slice. If `self` stores bytes, the iterator will
    /// try to decode `T`s successively until all bytes have been read. If
    /// decoding any value fails, an error will be yielded. After an error is
    /// yielded, the state of the iterator is undefined and it must no longer be
    /// used.
    pub fn iter(&self) -> IterBytesOr<'a, T> {
        IterBytesOr { inner: *self }
    }
}

/// An iterator over values stored in a `BytesOr`.
pub struct IterBytesOr<'a, T> {
    inner: BytesOr<'a, [T]>,
}

impl<'a, T: Copy + FromBytes> Iterator for IterBytesOr<'a, T> {
    type Item = Result<T, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            BytesOr::Bytes(b) => {
                if b.is_empty() {
                    None
                } else {
                    Some(T::from_bytes(b))
                }
            }
            BytesOr::Or(slice) => slice.read_first().map(Ok),
        }
    }
}

/// Trait for encoding a value into a byte buffer.
pub trait ToBytes {
    /// Returns the number of bytes needed to encode `self`.
    ///
    /// If `to_bytes` is called with a buffer of at least this size, `to_bytes`
    /// *must not* return an error, and it *must not* write more than this
    /// number of bytes. Violating these rules isn't unsafe, but still always a
    /// bug.
    fn space_needed(&self) -> usize;

    /// Converts `self` to bytes and writes them into `buffer`, advancing
    /// `buffer` to point past the encoded value.
    ///
    /// If `buffer` does not contain enough space, an error will be returned and
    /// the state of the buffer is unspecified (eg. `self` may be partially
    /// written into `buffer`).
    fn to_bytes(&self, buffer: &mut &mut [u8]) -> Result<(), Error>;
}

/// Trait for decoding values from a slice.
pub trait FromBytes: Sized {
    /// Decode a `Self` from a byte slice, advancing `bytes` to point past the
    /// data that was read.
    ///
    /// If `bytes` contains data not valid for the target type, or contains an
    /// insufficient number of bytes, an error will be returned and the state of
    /// `bytes` is unspecified (it can point to arbitrary data).
    fn from_bytes(bytes: &mut &[u8]) -> Result<Self, Error>;
}

/// Extensions on `&'a mut [u8]`.
pub trait MutSliceExt<'a> {
    /// Writes a byte to the beginning of `self` and updates `self` to point
    /// behind the written byte.
    ///
    /// If `self` is empty, returns an error.
    fn write_byte<'b>(&'b mut self, byte: u8) -> Result<(), Error>
    where
        'a: 'b;

    /// Copies all elements from `other` into `self` and advances `self` to
    /// point behind the copied elements.
    ///
    /// If `self` is empty, returns an error.
    fn write_slice<'b>(&'b mut self, other: &[u8]) -> Result<(), Error>
    where
        'a: 'b;
}

impl<'a> MutSliceExt<'a> for &'a mut [u8] {
    fn write_byte<'b>(&'b mut self, byte: u8) -> Result<(), Error>
    where
        'a: 'b,
    {
        // The `mem::replace` is needed to work around a complex borrowing restriction:
        // If we had `'b: 'a` instead of `'a: 'b`, a call to `write_byte` would result in the
        // "infinite self-borrow" problem, which makes the method useless. The `'a: 'b` means that
        // we have a `&'b mut &'a mut [u8]`, and we could only get a shortened `&'b mut [u8]` out of
        // that (for soundness reasons - the same thing that makes invariance necessary).
        // By using `mem::replace` we can safely get a `&'a mut [u8]` out instead (replacing what's
        // behind the reference with a `&'static mut []`).
        match mem::replace(self, &mut []).split_first_mut() {
            Some((first, rest)) => {
                *first = byte;
                *self = rest;
                Ok(())
            }
            None => Err(Error::Eof),
        }
    }

    fn write_slice<'b>(&'b mut self, other: &[u8]) -> Result<(), Error>
    where
        'a: 'b,
    {
        eof_unless!(self.len() >= other.len());
        self[..other.len()].copy_from_slice(other);
        Ok(())
    }
}

/// Extensions on `&'a [T]`.
pub trait SliceExt<T: Copy> {
    /// Returns a copy of the first element in the slice `self` and advances
    /// `self` to point past the element.
    fn read_first(&mut self) -> Option<T>;

    /// Reads a slice-like or array-like type `S` out of `self`.
    ///
    /// `self` will be updated to point past the read data.
    ///
    /// If `self` doesn't contain enough elements to fill an `S`, returns `None`
    /// without changing `self`.
    fn read_array<S>(&mut self) -> Option<S>
    where
        S: Default + AsMut<[T]>;
}

impl<'a, T: Copy> SliceExt<T> for &'a [T] {
    fn read_first(&mut self) -> Option<T> {
        let (first, rest) = self.split_first()?;
        *self = rest;
        Some(*first)
    }

    fn read_array<S>(&mut self) -> Option<S>
    where
        S: Default + AsMut<[T]>,
    {
        let mut buf = S::default();
        let slice = buf.as_mut();
        if self.len() < slice.len() {
            return None;
        }

        slice.copy_from_slice(&self[..slice.len()]);
        *self = &self[slice.len()..];
        Some(buf)
    }
}
