use core::fmt;

/// Creates an enum that can be converted from and to a primitive type, with invalid values becoming
/// a catch-all `Unknown` variant.
///
/// This is copied almost verbatim from [smoltcp].
///
/// [smoltcp]: https://github.com/m-labs/smoltcp/blob/cd893e6ab60f094d684b37be7bc013bf79f0459d/src/macros.rs
macro_rules! enum_with_unknown {
    (
        $( #[$enum_attr:meta] )*
        $v:vis enum $name:ident($ty:ty) {
            $(
              $( #[$variant_attr:meta] )*
              $variant:ident = $value:expr $(,)*
            ),*
        }
    ) => {
        $( #[$enum_attr] )*
        $v enum $name {
            $(
              $( #[$variant_attr] )*
              $variant,
            )*
            Unknown($ty)
        }

        impl ::core::convert::From<$ty> for $name {
            fn from(value: $ty) -> Self {
                match value {
                    $( $value => $name::$variant, )*
                    other => $name::Unknown(other)
                }
            }
        }

        impl ::core::convert::From<$name> for $ty {
            fn from(value: $name) -> Self {
                match value {
                    $( $name::$variant => $value, )*
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

/// `Debug`-formats its contents as a hexadecimal byte slice.
#[derive(Copy, Clone)]
pub struct HexSlice<T>(pub T)
where
    T: AsRef<[u8]>;

impl<T: AsRef<[u8]>> fmt::Debug for HexSlice<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("[")?;
        for (i, byte) in self.0.as_ref().iter().enumerate() {
            if i != 0 {
                f.write_str(", ")?;
            }
            write!(f, "{:02x}", byte)?;
        }
        f.write_str("]")
    }
}

/// `Debug`-formats its contents in hexadecimal.
#[derive(Copy, Clone)]
pub struct Hex<T>(pub T)
where
    T: fmt::LowerHex;

impl<T: fmt::LowerHex> fmt::Debug for Hex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

/// Extension traits for primitive integers allowing bit reversal.
///
/// This exists only because the `reverse_bits` methods are still unstable. If they get stabilized,
/// this should get removed.
pub trait ReverseBits {
    /// Reverse all bits in `self`, returning the result in a new number.
    fn reverse_bits_ext(&self) -> Self;
}

impl ReverseBits for u8 {
    fn reverse_bits_ext(&self) -> Self {
        let mut input = *self;
        let mut result = 0;
        for _ in 0..8 {
            result <<= 1;
            result |= input & 1;
            input >>= 1;
        }
        result
    }
}
