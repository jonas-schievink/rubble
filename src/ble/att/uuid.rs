use {
    crate::ble::{bytes::*, uuid::*, Error},
    core::fmt,
};

/// ATT protocol UUID (either a 16 or a 128-bit UUID).
///
/// 32-bit UUIDs are not supported by ATT are must be converted to 128-bit UUIDs.
#[derive(Copy, Clone, PartialEq)]
pub enum AttUuid {
    Uuid16(Uuid16),
    Uuid128(Uuid),
}

impl FromBytes<'_> for AttUuid {
    fn from_bytes(bytes: &mut ByteReader) -> Result<Self, Error> {
        Ok(match bytes.bytes_left() {
            2 => AttUuid::Uuid16(Uuid16::from_bytes(bytes)?),
            16 => AttUuid::Uuid128(<Uuid as FromBytes>::from_bytes(bytes)?),
            _ => return Err(Error::InvalidLength),
        })
    }
}

impl ToBytes for AttUuid {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        match self {
            AttUuid::Uuid16(uuid) => uuid.to_bytes(writer),
            AttUuid::Uuid128(uuid) => uuid.to_bytes(writer),
        }
    }
}

impl From<Uuid16> for AttUuid {
    fn from(uu: Uuid16) -> Self {
        AttUuid::Uuid16(uu)
    }
}

impl From<Uuid32> for AttUuid {
    fn from(uu: Uuid32) -> Self {
        AttUuid::Uuid128(uu.into())
    }
}

impl From<Uuid> for AttUuid {
    fn from(uu: Uuid) -> Self {
        AttUuid::Uuid128(uu)
    }
}

impl fmt::Debug for AttUuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AttUuid::Uuid16(u) => u.fmt(f),
            AttUuid::Uuid128(u) => u.fmt(f),
        }
    }
}
