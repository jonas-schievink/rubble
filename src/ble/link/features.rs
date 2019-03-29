use {
    crate::ble::{bytes::*, Error},
    bitflags::bitflags,
    byteorder::LittleEndian,
    log::warn,
};

bitflags! {
    /// A set of optional Link Layer features.
    pub struct FeatureSet: u64 {
        const LE_ENCRYPTION = (1 << 0);
        const CONN_PARAM_REQ = (1 << 1);
        const EXTENDED_REJECT_INDICATION = (1 << 2);
        const SLAVE_FEATURE_EXCHANGE = (1 << 3);
        const LE_PING = (1 << 4);
        const LE_PACKET_LENGTH_EXTENSION = (1 << 5);
        const LL_PRIVACY = (1 << 6);
        const EXT_SCANNER_FILTER_POLICIES = (1 << 7);
    }
}

impl FeatureSet {
    /// Returns the feature set supported by Rubble.
    pub fn supported() -> Self {
        FeatureSet::empty()
    }
}

impl ToBytes for FeatureSet {
    fn to_bytes(&self, writer: &mut ByteWriter) -> Result<(), Error> {
        writer.write_u64::<LittleEndian>(self.bits())
    }
}

impl<'a> FromBytes<'a> for FeatureSet {
    fn from_bytes(bytes: &mut &'a [u8]) -> Result<Self, Error> {
        let raw = bytes.read_u64::<LittleEndian>()?;
        let this = Self::from_bits_truncate(raw);
        if raw != this.bits() {
            warn!("unknown feature bits: {:b} (known: {:b})", raw, this.bits());
        }

        Ok(this)
    }
}
