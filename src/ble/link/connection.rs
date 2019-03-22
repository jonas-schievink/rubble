//! Link-Layer management when in Connection state.

use crate::ble::{
    link::{advertising::ConnectRequestData, SequenceNumber},
    phy::{ChannelMap, DataChannel},
};

/// Connection state.
#[allow(unused)] // FIXME implement connections and remove this
pub struct Connection {
    access_address: u32,
    crc_init: u32,
    channel_map: ChannelMap,
    hop: u8,

    /// Data channel on which the last connection event took place.
    ///
    /// Also known as `previous_event_channel` (yes, the spec uses both).
    ///
    /// Starts out as 0 when the connection is first established.
    last_unmapped_channel: DataChannel,

    // Acknowledgement / Flow Control state
    transmit_seq_num: SequenceNumber,
    next_expected_seq_num: SequenceNumber,
}

impl Connection {
    /// Initializes a connection state according to the `LLData` contained in the `CONNECT_REQ` PDU.
    pub fn new(lldata: &ConnectRequestData) -> Self {
        Self {
            access_address: lldata.access_address(),
            crc_init: lldata.crc_init().into(),
            channel_map: *lldata.channel_map(),
            hop: lldata.hop(),

            last_unmapped_channel: DataChannel::new(0),
            transmit_seq_num: SequenceNumber::zero(),
            next_expected_seq_num: SequenceNumber::zero(),
        }
    }

    /// Calculates the data channel on which the next connection event will take place, and hops to
    /// the next channel.
    ///
    /// According to: `4.5.8.2 Channel Selection`.
    #[allow(unused)] // FIXME implement connections and remove this
    fn hop_channel(&mut self) -> DataChannel {
        let unmapped_channel =
            DataChannel::new((self.last_unmapped_channel.index() + self.hop) % 37);

        // Advance channels. This is only supposed to be done once a connection event "closes", but
        // shouldn't matter if we do it here, since we *always* hop channels.
        self.last_unmapped_channel = unmapped_channel;

        if self.channel_map.is_used(unmapped_channel) {
            unmapped_channel
        } else {
            // This channel isn't used, remap channel according to map
            let remapping_index = unmapped_channel.index() % self.channel_map.num_used_channels();
            self.channel_map.by_index(remapping_index)
        }
    }
}
