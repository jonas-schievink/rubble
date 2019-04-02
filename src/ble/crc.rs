use super::link::CRC_POLY;

/// Performs the BLE Link-Layer checksum computation.
///
/// The result has 24 bits stored in the lower 24 bits of a `u32`.
pub fn ble_crc24(data: &[u8], preset: u32) -> u32 {
    let polynomial = CRC_POLY & 0x00FFFFFF;
    let mut crc = preset;

    for byte in data {
        crc ^= u32::from(*byte) << 16;

        for _ in 0..8 {
            let msb = crc & 0x00800000 != 0;
            crc <<= 1;

            if msb {
                crc ^= polynomial;
            }
        }
    }

    crc & 0x00FFFFFF
}
