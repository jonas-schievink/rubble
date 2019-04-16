//! BLE beacon support, without dealing with Link-Layer stuff.

use {
    super::{
        link::{ad_structure::AdStructure, advertising::PduBuf, DeviceAddress, Transmitter},
        phy::AdvertisingChannel,
    },
    crate::Error,
};

/// A BLE beacon.
///
/// FIXME: This has to randomly offset the broadcast interval
pub struct Beacon {
    pdu: PduBuf,
}

impl Beacon {
    /// Creates a new beacon that will broadcast a packet on all advertisement
    /// channels.
    ///
    /// # Parameters
    ///
    /// * **`addr`**: Address of the beacon device.
    /// * **`data`**: Data to broadcast. This must fit within a single PDU.
    ///
    /// # Errors
    ///
    /// If `data` doesn't fit in a single PDU, an error will be returned.
    pub fn new(addr: DeviceAddress, data: &[AdStructure]) -> Result<Self, Error> {
        let pdu = PduBuf::beacon(addr, data)?;
        Ok(Self { pdu })
    }

    /// Broadcasts the beacon data using `tx`.
    ///
    /// This will broadcast once on every advertising channel.
    pub fn broadcast<T: Transmitter>(&self, tx: &mut T) {
        // The spec says that we have to broadcast on all 3 channels in sequence, so that the total
        // time of this broadcast ("advertising event") is <10ms.

        // FIXME: Do we have to write the payload again every time we send (ie.
        // is the transmitter allowed to overwrite it)?

        let payload = self.pdu.payload();
        let buf = tx.tx_payload_buf();
        buf[..payload.len()].copy_from_slice(payload);

        for channel in AdvertisingChannel::iter_all() {
            tx.transmit_advertising(self.pdu.header(), channel);
        }
    }
}
