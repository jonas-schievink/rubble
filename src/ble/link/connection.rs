//! Link-Layer connection management.

use crate::ble::{
    link::{
        advertising::ConnectRequestData, data, Cmd, Logger, RadioCmd, SequenceNumber, Transmitter,
    },
    phy::{ChannelMap, DataChannel},
    utils::HexSlice,
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
    ///
    /// Returns the connection state and a `Cmd` to apply to the radio.
    pub fn create(lldata: &ConnectRequestData) -> (Self, Cmd) {
        let mut this = Self {
            access_address: lldata.access_address(),
            crc_init: lldata.crc_init().into(),
            channel_map: *lldata.channel_map(),
            hop: lldata.hop(),

            last_unmapped_channel: DataChannel::new(0),
            transmit_seq_num: SequenceNumber::zero(),
            next_expected_seq_num: SequenceNumber::zero(),
        };

        let cmd = Cmd {
            next_update: None, // FIXME connection timeout
            radio: RadioCmd::ListenData {
                channel: this.hop_channel(),
                access_address: this.access_address,
                crc_init: this.crc_init,
            },
        };

        (this, cmd)
    }

    /// Called when a data channel packet is received.
    pub fn process_data_packet<T: Transmitter, L: Logger>(
        &mut self,
        _tx: &mut T,
        logger: &mut L,
        header: data::Header,
        payload: &[u8],
        crc_ok: bool,
    ) -> Cmd {
        trace!(
            logger,
            "DATA<- {}{:?}, {:?}",
            if crc_ok { "" } else { "BADCRC" },
            header,
            HexSlice(payload)
        );
        unimplemented!()
    }

    pub fn timer_update<L: Logger>(&mut self, _logger: &mut L) -> Cmd {
        unimplemented!()
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
