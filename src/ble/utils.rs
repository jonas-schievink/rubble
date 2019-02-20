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
    fn write_byte<'b>(&'b mut self, byte: u8) -> Result<(), Error>
    where
        'a: 'b;

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

/// Extensions on `&'a [u8]`.
pub trait SliceExt {
    /// Read the first byte in the slice `self` and advance `self` to point
    /// past the byte.
    fn read_byte(&mut self) -> Result<u8, Error>;
}

impl<'a> SliceExt for &'a [u8] {
    fn read_byte(&mut self) -> Result<u8, Error> {
        match self.split_first() {
            Some((first, rest)) => {
                *self = rest;
                Ok(*first)
            }
            None => Err(Error::Eof),
        }
    }
}
