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
        let this = mem::replace(self, &mut []);
        *self = &mut this[other.len()..];
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
