//! Link-Layer connection management.

use {
    crate::ble::{
        bytes::*,
        link::{
            advertising::ConnectRequestData,
            data::{self, Header, Llid, Pdu},
            Cmd, HwInterface, Logger, NextUpdate, RadioCmd, SeqNum, Transmitter,
        },
        phy::{ChannelMap, DataChannel},
        time::{Duration, Instant, Timer},
        utils::HexSlice,
    },
    core::marker::PhantomData,
};

/// Connection state.
pub struct Connection<L: Logger, T: Timer> {
    access_address: u32,
    crc_init: u32,
    channel_map: ChannelMap,

    /// Number of (unmapped) channels to hop between each connection event.
    hop: u8,

    /// Connection event interval in µs.
    conn_interval: u32,

    /// Unmapped data channel on which the next connection event will take place.
    ///
    /// Also known as `lastUnmappedChannel` or `previous_event_channel` (yes, the spec uses both).
    unmapped_channel: DataChannel,

    /// Actual data channel on which the next data packets will be exchanged.
    channel: DataChannel,

    // Acknowledgement / Flow Control state
    /// `SN` bit to be used
    transmit_seq_num: SeqNum,
    next_expected_seq_num: SeqNum,

    /// Header of the last transmitted packet, used for retransmission.
    last_header: data::Header,

    /// Whether we have ever received a data packet in this connection.
    received_packet: bool,

    _p: PhantomData<(L, T)>,
}

impl<L: Logger, T: Timer> Connection<L, T> {
    /// Initializes a connection state according to the `LLData` contained in the `CONNECT_REQ`
    /// advertising PDU.
    ///
    /// Returns the connection state and a `Cmd` to apply to the radio/timer.
    ///
    /// # Parameters
    ///
    /// * **`lldata`**: Data contained in the `CONNECT_REQ` advertising PDU.
    /// * **`rx_end`**: Instant at which the `CONNECT_REQ` PDU was fully received.
    pub fn create(lldata: &ConnectRequestData, rx_end: Instant) -> (Self, Cmd) {
        assert_eq!(
            lldata.slave_latency(),
            0,
            "slave latency is not implemented"
        );

        let mut this = Self {
            access_address: lldata.access_address(),
            crc_init: lldata.crc_init().into(),
            channel_map: *lldata.channel_map(),
            hop: lldata.hop(),
            conn_interval: lldata.interval(),

            unmapped_channel: DataChannel::new(0),
            channel: DataChannel::new(0),

            transmit_seq_num: SeqNum::ZERO,
            next_expected_seq_num: SeqNum::ZERO,
            last_header: Header::new(Llid::DataCont),
            received_packet: false,

            _p: PhantomData,
        };

        // Calculate the first channel to use
        this.hop_channel();

        let cmd = Cmd {
            next_update: NextUpdate::At(
                rx_end + lldata.end_of_tx_window() + Duration::from_micros(500),
            ),
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
    pub fn process_data_packet<R: Transmitter>(
        &mut self,
        rx_end: Instant,
        tx: &mut R,
        hw: &mut HwInterface<L, T>,
        header: data::Header,
        payload: &[u8],
        crc_ok: bool,
    ) -> Result<Cmd, ()> {
        let _needs_processing = if header.sn() == self.next_expected_seq_num && crc_ok {
            // New (non-resent) PDU, acknowledge it
            self.next_expected_seq_num += SeqNum::ONE;
            true
        } else {
            false
        };

        if header.nesn() == self.transmit_seq_num || !crc_ok {
            // Last packet not acknowledged, resend.
            // If CRC is bad, this bit could be flipped, so we always retransmit in that case.
            if self.received_packet {
                self.last_header.set_nesn(self.next_expected_seq_num);
                let d = hw.timer.now().duration_since(rx_end);
                tx.transmit_data(
                    self.access_address,
                    self.crc_init,
                    self.last_header,
                    self.channel,
                );
                let before_log = hw.timer.now();
                trace!(hw.logger, "<<RESEND {} after RX>>", d);
                trace!(
                    hw.logger,
                    "<<That LOG took {}>>",
                    hw.timer.now().duration_since(before_log)
                );
            } else {
                // We've never received (and thus sent) a data packet before, so we can't
                // *re*transmit anything. Send empty PDU instead.
                self.received_packet = true;
                self.send(Pdu::empty(), tx, &mut hw.logger);
            }
        } else {
            self.received_packet = true;
            // Here we'll always send a new packet (which might be empty if we don't have anything
            // to say). If `needs_processing` is set, we'll also process the received PDU before
            // sending.

            self.transmit_seq_num += SeqNum::ONE;

            // Send a new packet
            self.send(Pdu::empty(), tx, &mut hw.logger);
        }

        let last_channel = self.channel;

        // If both devices set MD to `false`, the connection event closes and we hop to the next
        // channel.
        // If the CRC is bad, we must hop anyways.
        if !crc_ok || (!header.md() && !self.has_more_data()) {
            self.hop_channel();
        }

        trace!(
            hw.logger,
            "DATA({}->{})<- {}{:?}, {:?}",
            last_channel.index(),
            self.channel.index(),
            if crc_ok { "" } else { "BADCRC, " },
            header,
            HexSlice(payload)
        );

        Ok(Cmd {
            next_update: NextUpdate::At(hw.timer.now() + self.conn_event_timeout()),
            radio: RadioCmd::ListenData {
                channel: self.channel,
                access_address: self.access_address,
                crc_init: self.crc_init,
            },
        })
    }

    pub fn timer_update(&mut self, hw: &mut HwInterface<L, T>) -> Result<Cmd, ()> {
        if self.received_packet {
            // No packet from master, skip this connection event and listen on the next channel

            let last_channel = self.channel;
            self.hop_channel();
            trace!(
                hw.logger,
                "DATA({}->{}): missed conn event",
                last_channel.index(),
                self.channel.index()
            );

            Ok(Cmd {
                next_update: NextUpdate::At(hw.timer.now() + self.conn_event_timeout()),
                radio: RadioCmd::ListenData {
                    channel: self.channel,
                    access_address: self.access_address,
                    crc_init: self.crc_init,
                },
            })
        } else {
            // Master did not transmit the first packet during this transmit window.

            // TODO: Move the transmit window forward by the `connInterval`.

            trace!(hw.logger, "missed transmit window");
            Err(())
        }
    }

    fn conn_event_timeout(&self) -> Duration {
        // Time out ~500µs after the anchor point of the next conn event.
        Duration::from_micros(self.conn_interval + 500)
    }

    /// Whether we want to send more data during this connection event.
    ///
    /// Note that this *has to* change to `false` eventually, even if there's more data to be sent,
    /// because the connection event must close at least `T_IFS` before the next one occurs.
    fn has_more_data(&self) -> bool {
        false
    }

    /// Advances the `unmapped_channel` and `channel` fields to the next data channel on which a
    /// connection event will take place.
    ///
    /// According to: `4.5.8.2 Channel Selection`.
    fn hop_channel(&mut self) {
        let unmapped_channel = DataChannel::new((self.unmapped_channel.index() + self.hop) % 37);

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
    fn send<R: Transmitter>(&mut self, pdu: Pdu<'_>, tx: &mut R, logger: &mut L) {
        let mut payload_writer = ByteWriter::new(tx.tx_payload_buf());
        // Serialize PDU. This should never fail, because the upper layers are supposed to fragment
        // packets so they always fit.
        pdu.to_bytes(&mut payload_writer)
            .expect("EOF when serializing data PDU");

        let mut header = Header::new(pdu.llid());
        header.set_md(self.has_more_data());
        header.set_nesn(self.next_expected_seq_num);
        header.set_sn(self.transmit_seq_num);
        self.last_header = header;

        tx.transmit_data(self.access_address, self.crc_init, header, self.channel);

        trace!(logger, "DATA->{:?}, {:?}", header, pdu);
    }
}
