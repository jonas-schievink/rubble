use core::fmt;

/// Specifies whether a device address is randomly generated or a LAN MAC address.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum AddressKind {
    /// Publicly registered IEEE 802-2001 LAN MAC address.
    Public,
    /// Randomly generated address.
    Random,
}

/// A Bluetooth device address.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct DeviceAddress {
    bytes: [u8; 6],
    kind: AddressKind,
}

impl DeviceAddress {
    /// Create a new device address from 6 raw Bytes and an address kind specifier.
    ///
    /// The `raw` array contains the address Bytes as they are sent over the air (LSB first).
    pub fn new(bytes: [u8; 6], kind: AddressKind) -> Self {
        DeviceAddress { bytes, kind }
    }

    /// Returns the address kind.
    pub fn kind(&self) -> AddressKind {
        self.kind
    }

    /// Returns whether this address is randomly generated.
    pub fn is_random(&self) -> bool {
        self.kind == AddressKind::Random
    }

    /// Returns the raw bytes making up this address.
    pub fn raw(&self) -> &[u8; 6] {
        &self.bytes
    }
}

impl fmt::Debug for DeviceAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, b) in self.bytes.iter().enumerate() {
            if i != 0 {
                f.write_str(":")?;
            }
            write!(f, "{:02x}", b)?;
        }

        write!(f, "({:?})", self.kind)?;

        Ok(())
    }
}
