use crate::{bytes::*, uuid::*, Error};
use core::{cmp::PartialEq, fmt};

/// ATT protocol UUID (either a 16 or a 128-bit UUID).
///
/// 32-bit UUIDs are not supported by ATT are must be converted to 128-bit UUIDs.
#[derive(Copy, Clone, Eq)]
pub enum AttUuid {
    Uuid16(Uuid16),
    Uuid128(Uuid),
}

impl FromBytes<'_> for AttUuid {
    fn from_bytes(bytes: &mut ByteReader<'_>) -> Result<Self, Error> {
        Ok(match bytes.bytes_left() {
            2 => AttUuid::Uuid16(Uuid16::from_bytes(bytes)?),
            16 => AttUuid::Uuid128(<Uuid as FromBytes>::from_bytes(bytes)?),
            _ => return Err(Error::InvalidLength),
        })
    }
}

impl ToBytes for AttUuid {
    fn to_bytes(&self, writer: &mut ByteWriter<'_>) -> Result<(), Error> {
        match self {
            AttUuid::Uuid16(uuid) => uuid.to_bytes(writer),
            AttUuid::Uuid128(uuid) => uuid.to_bytes(writer),
        }
    }
}

impl PartialEq for AttUuid {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // 16-bit UUIDs can be compared directly
            (AttUuid::Uuid16(a), AttUuid::Uuid16(b)) => a == b,

            // All other combinations need to convert to 128-bit UUIDs
            (AttUuid::Uuid128(a), b) | (b, AttUuid::Uuid128(a)) => {
                let b: Uuid = (*b).into();
                *a == b
            }
        }
    }
}

impl PartialEq<Uuid16> for AttUuid {
    fn eq(&self, other: &Uuid16) -> bool {
        self == &Self::from(*other)
    }
}

impl PartialEq<Uuid> for AttUuid {
    fn eq(&self, other: &Uuid) -> bool {
        self == &Self::from(*other)
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

impl Into<Uuid> for AttUuid {
    fn into(self) -> Uuid {
        match self {
            AttUuid::Uuid16(u) => u.into(),
            AttUuid::Uuid128(u) => u,
        }
    }
}

impl fmt::Debug for AttUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttUuid::Uuid16(u) => u.fmt(f),
            AttUuid::Uuid128(u) => u.fmt(f),
        }
    }
}
