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
