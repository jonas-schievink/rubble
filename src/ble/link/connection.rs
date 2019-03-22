//! Link-Layer connection management.

use crate::ble::{
    link::{
        advertising::ConnectRequestData,
        data::{self, Llid, Pdu},
        Cmd, Logger, RadioCmd, SequenceNumber, Transmitter,
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

    /// Unmapped data channel on which the next connection event will take place.
    ///
    /// Also known as `lastUnmappedChannel` or `previous_event_channel` (yes, the spec uses both).
    unmapped_channel: DataChannel,

    /// Actual data channel on which the next data packets will be exchanged.
    channel: DataChannel,

    // Acknowledgement / Flow Control state
    transmit_seq_num: SequenceNumber,
    next_expected_seq_num: SequenceNumber,

    /// Header of the last transmitted packet, used for retransmission.
    last_header: data::Header,
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

            unmapped_channel: DataChannel::new(0),
            channel: DataChannel::new(0),

            transmit_seq_num: SequenceNumber::zero(),
            next_expected_seq_num: SequenceNumber::zero(),
            last_header: data::Header::new(0, Llid::DataCont),
        };
        this.hop_channel();

        let cmd = Cmd {
            next_update: None, // FIXME connection timeout
            radio: RadioCmd::ListenData {
                channel: this.channel,
                access_address: this.access_address,
                crc_init: this.crc_init,
            },
        };

        (this, cmd)
    }

    /// Called by the `LinkLayer` when a data channel packet is received.
    ///
    /// Returns `Err(())` when the connection is ended (not necessarily due to an error condition).
    pub fn process_data_packet<T: Transmitter, L: Logger>(
        &mut self,
        tx: &mut T,
        logger: &mut L,
        header: data::Header,
        payload: &[u8],
        crc_ok: bool,
    ) -> Result<Cmd, ()> {
        let _needs_processing = if header.sn() == self.next_expected_seq_num && crc_ok {
            // New (non-resent) PDU, acknowledge it
            self.next_expected_seq_num += SequenceNumber::one();
            true
        } else {
            false
        };

        if header.nesn() == self.transmit_seq_num {
            // Last packet not acknowledged, resend
            self.last_header.set_nesn(self.next_expected_seq_num);
            tx.transmit_data(
                self.access_address,
                self.crc_init,
                self.last_header,
                self.channel,
            );
            trace!(logger, "<<RESEND>>");
        } else {
            self.transmit_seq_num += SequenceNumber::one();

            // Send a new packet
            self.send(Pdu::empty(), tx);
        }

        let last_channel = self.channel;
        self.hop_channel();
        trace!(
            logger,
            "DATA({}->{})<- {}{:?}, {:?}",
            last_channel.index(),
            self.channel.index(),
            if crc_ok { "" } else { "BADCRC, " },
            header,
            HexSlice(payload)
        );

        Ok(Cmd {
            next_update: None, // FIXME wrong
            radio: RadioCmd::ListenData {
                channel: self.channel,
                access_address: self.access_address,
                crc_init: self.crc_init,
            },
        })
    }

    pub fn timer_update<L: Logger>(&mut self, _logger: &mut L) -> Cmd {
        unimplemented!()
    }

    /// Advances the `unmapped_channel` and `channel` fields to the next data channel on which a
    /// connection event will take place.
    ///
    /// According to: `4.5.8.2 Channel Selection`.
    fn hop_channel(&mut self) {
        let unmapped_channel = DataChannel::new((self.unmapped_channel.index() + self.hop) % 37);

        // Advance channels. This is only supposed to be done once a connection event "closes", but
        // shouldn't matter if we do it here, since we *always* hop channels.
        self.unmapped_channel = unmapped_channel;

        self.channel = if self.channel_map.is_used(unmapped_channel) {
            unmapped_channel
        } else {
            // This channel isn't used, remap channel according to map
            let remapping_index = unmapped_channel.index() % self.channel_map.num_used_channels();
            self.channel_map.by_index(remapping_index)
        };
    }

    /// Sends a new PDU to the connected device (ie. a non-retransmitted PDU).
    fn send<T: Transmitter>(&mut self, pdu: Pdu<'_>, tx: &mut T) {
        // TODO serialize payload

        let mut header = pdu.header();
        header.set_md(true);
        header.set_nesn(self.next_expected_seq_num);
        header.set_sn(self.transmit_seq_num);

        tx.transmit_data(self.access_address, self.crc_init, header, self.channel);
    }
}
