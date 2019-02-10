//! Integrated 2.4 GHz radio with BLE support.
//!
//! The radio can be used with Nordic's own proprietary protocol, which I don't really care about,
//! so this will focus on Bluetooth Low-Energy (BLE).
//!
//! The radio can calculate the CRC, perform data whitening, automatically send the right preamble,
//! and match addresses.
//!
//! In order to be able to receive packet at all, the length of the currently received packet must
//! be known.
//!
//! # CRC
//!
//! To be able to correctly compute the CRC, the position and length of the transmitted PDU must be
//! known. The radio works with a flexible frame layout that looks like this on air:
//!
//! (B = Byte = Octet; b = bit = symbol)
//!
//! If field length is specified in `B`, only whole Bytes are allowed. If the length is specified
//! in `b`, any number of bits in the given range is allowed.
//!
//! ```notrust
//! +----------+---------+--------+---------+----------+----------+--------------+---------+
//! | Preamble |  Base   | Prefix |   S0    |  Length  |    S1    |   Payload    |   CRC   |
//! |  (1 B)   | (2-4 B) | (1 B)  | (0-1 B) | (0-15 b) | (0-15 b) | (`Length` B) | (0-3 B) |
//! +----------+---------+--------+---------+----------+----------+--------------+---------+
//!             \                / \                                            /
//!              \------+-------/   \---------------------+--------------------/
//!                     |                                 |
//!                  Address                             PDU
//! ```
//!
//! If S0, Length, and S1 are present (= length > 0 bits), their sizes are *always* rounded up to
//! whole Bytes in RAM. The least significant bits of the in-RAM Bytes will be sent/received over
//! the air. This poses a problem, since the packet isn't actually sent as it is in memory. The
//! stack works around this by only filling the `Payload` by itself and passing a `Header` struct to
//! the `Transmitter`, which can then do whatever is necessary to encode the header so that it's
//! sent correctly.
//!
//! In our case, this involves "splitting" the header into the `S0` field (everything preceding the
//! length), the `Length` field, and the `S1` field (which just contains 2 unused bits, but they
//! must still be sent, of course).

use ble::link::{MAX_PDU_SIZE, ADVERTISING_ADDRESS, CRC_PRESET, CRC_POLY, LinkLayer, Transmitter, RadioCmd, advertising, data};
use ble::link::log::Logger;
use ble::phy::{AdvertisingChannelIndex, DataChannelIndex};

use nrf51::{FICR, RADIO};
use nrf51::radio::state::STATER;

use core::time::Duration;

/// The buffer has an extra Byte because the 16-bit PDU header needs to be split in 3 Bytes for the
/// radio to understand it (S0 = pre-Length fields, Length, S1 = post-Length fields).
pub type PacketBuffer = [u8; MAX_PDU_SIZE + 1];

// BLE inter frame spacing in microseconds.
//const BLE_TIFS: u8 = 150;

pub struct BleRadio {
    radio: RADIO,
    tx_buf: &'static mut PacketBuffer,
}

impl BleRadio {
    // TODO: Use type-safe clock configuration to ensure that chip uses ext. crystal
    pub fn new(radio: RADIO, ficr: &FICR, tx_buf: &'static mut PacketBuffer) -> Self {
        assert!(radio.state.read().state().is_disabled());

        if ficr.overrideen.read().ble_1mbit().is_override_() {
            unsafe {
                radio.override0.write(|w| w.override0().bits(ficr.ble_1mbit[0].read().bits()));
                radio.override1.write(|w| w.override1().bits(ficr.ble_1mbit[1].read().bits()));
                radio.override2.write(|w| w.override2().bits(ficr.ble_1mbit[2].read().bits()));
                radio.override3.write(|w| w.override3().bits(ficr.ble_1mbit[3].read().bits()));
                radio.override4.write(|w| w.override4().bits(ficr.ble_1mbit[4].read().bits()).enable().set_bit());
            }
        }

        radio.mode.write(|w| w.mode().ble_1mbit());
        radio.txpower.write(|w| w.txpower().pos4d_bm());

        unsafe {
            radio.pcnf1.write(|w| w
                .maxlen().bits(37)   // no packet length limit
                .balen().bits(3)     // 3-Byte Base Address + 1-Byte Address Prefix
                .whiteen().set_bit() // Enable Data Whitening over PDU+CRC
            );
            radio.crccnf.write(|w| w
                .skipaddr().set_bit()   // skip address since only the S0, Length, S1 and Payload need CRC
                .len().three()          // 3 Bytes = CRC24
            );
            radio.crcpoly.write(|w| w.crcpoly().bits(CRC_POLY & 0xFFFFFF));

            // Configure logical address 0 as the canonical advertising address.
            // Base addresses are up to 32 bits in size. However, an 8 bit Address Prefix is
            // *always* appended, so we must use a 24 bit Base Address and the 8 bit Prefix.
            // BASE0 has, apparently, undocumented semantics: It is a proper 32-bit register, but
            // it ignores the *lowest* 8 bit and instead transmits the upper 24 as the low 24 bits
            // of the Access Address. Shift address up to fix this.
            radio.base0.write(|w| w.bits(ADVERTISING_ADDRESS << 8));
            radio.prefix0.write(|w| w.ap0().bits((ADVERTISING_ADDRESS >> 24) as u8));
        }

        // FIXME: No TIFS hardware support for now. Revisit when precise semantics are clear.
        // Activate END_DISABLE and DISABLED_TXEN shortcuts so TIFS is enforced. We might enable
        // more shortcuts later.
        /*radio.shorts.write(|w| w
            .end_disable().enabled()
            .disabled_txen().enabled()
        );*/

        /*unsafe {
            radio.tifs.write(|w| w.tifs().bits(BLE_TIFS));
        }*/

        // Configure shortcuts to simplify and speed up sending and receiving packets.
        radio.shorts.write(|w| w
            .ready_start().enabled()    // start transmission/recv immediately after ramp-up
            .end_disable().enabled()    // disable radio when transmission/recv is done
        );
        // We can now start the TXEN/RXEN tasks and the radio will do the rest and return to the
        // disabled state.

        Self {
            radio,
            tx_buf,
        }
    }

    /// Returns the current radio state.
    pub fn state(&self) -> STATER {
        self.radio.state.read().state()
    }

    /// Perform preparations to receive or send on an advertising channel.
    ///
    /// This will disable the radio, configure the packet layout, set initial values for CRC and
    /// whitening, and set the frequency to the given `channel`.
    ///
    /// To **transmit**, the `txaddress` must be set and the `packetptr` must be set to the TX
    /// buffer.
    ///
    /// To **receive**, the `rxaddresses` must be set to receive on logical address 0 and
    /// `packetptr` must be pointed to the RX buffer.
    ///
    /// Of course, other tasks may also be performed.
    fn prepare_txrx_advertising(&mut self, channel: AdvertisingChannelIndex) {
        unsafe {
            // Acknowledge left-over disable event
            self.radio.events_disabled.reset();

            if !self.state().is_disabled() {
                // In case we're currently receiving, stop that
                self.radio.tasks_disable.write(|w| w.bits(1));

                // Then wait until disable event is triggered
                while self.radio.events_disabled.read().bits() == 0 {}
            }
        }

        assert!(self.state().is_disabled());

        // Now we can freely configure all registers we need
        unsafe {
            self.radio.pcnf0.write(|w| w
                .s0len().bit(true)
                .lflen().bits(6)
                .s1len().bits(2)
            );

            self.radio.datawhiteiv.write(|w| w.datawhiteiv().bits(channel.whitening_iv()));
            self.radio.crcinit.write(|w| w.crcinit().bits(CRC_PRESET));
            self.radio.frequency.write(|w| w.frequency().bits((channel.freq() - 2400) as u8));
        }
    }

    /// Transmit a PDU from the internal buffer.
    ///
    /// This will block until the transmission has completed.
    ///
    /// Assumes that all registers are correct for this type of transmission.
    fn transmit(&mut self) {
        assert!(self.state().is_disabled());

        unsafe {
            // "The CPU should reconfigure this pointer every time before the RADIO is started via
            // the START task."
            self.radio.packetptr.write(|w| w.bits(self.tx_buf as *const _ as u32));

            // Acknowledge left-over disable event
            self.radio.events_disabled.reset();  // FIXME unnecessary, right?

            // ...and kick off the transmission
            self.radio.tasks_txen.write(|w| w.bits(1));

            // Then wait until disable event is triggered
            while self.radio.events_disabled.read().bits() == 0 {}

            // Now our `tx_buf` can be used again.
        }
    }
}

impl Transmitter for BleRadio {
    fn tx_payload_buf(&mut self) -> &mut [u8] {
        // Leave 3 Bytes for the data/advertising PDU header. The header is actually only 2 on-air
        // Bytes, but because of the way the radio works, `Length` must get its own Byte in RAM.
        &mut self.tx_buf[3..]
    }

    fn transmit_advertising(&mut self, header: advertising::Header, channel: AdvertisingChannelIndex) {
        let raw_header = header.to_u16();
        self.tx_buf[0] = raw_header as u8;         // S0     = 8 bits (LSB)
        self.tx_buf[1] = header.payload_length();  // Length = 6 bits
        self.tx_buf[2] = 0;                        // S1     = 2 unused bits = 0

        self.prepare_txrx_advertising(channel);

        // Set transmission address:
        // Logical addr. 0 uses BASE0 + PREFIX0, which is the canonical adv. Access Address
        self.radio.txaddress.write(|w| unsafe { w.txaddress().bits(0) });

        self.transmit();
    }

    fn transmit_data(&mut self, _access_address: u32, _crc_iv: u32, _header: data::Header, _channel: DataChannelIndex) {
        unimplemented!();
        //self.transmit(access_address, crc_iv, channel.whitening_iv(), channel.freq());
    }
}

pub struct Baseband<L: Logger> {
    radio: BleRadio,
    rx_buf: &'static mut PacketBuffer,
    ll: LinkLayer<L>,
}

impl<L: Logger> Baseband<L> {
    pub fn new(radio: BleRadio, rx_buf: &'static mut PacketBuffer, ll: LinkLayer<L>) -> Self {
        Self { radio, rx_buf, ll }
    }

    /// Call this when the `RADIO` interrupt fires.
    ///
    /// Returns a duration if the BLE stack requested that the next update time be changed. Returns
    /// `None` if the update time should stay as-is from the last `update` call.
    pub fn interrupt(&mut self) -> Option<Duration> {
        // Acknowledge END event:
        self.radio.radio.events_end.reset();

        if self.radio.radio.crcstatus.read().crcstatus().is_crcok() {
            let header = advertising::Header::parse(self.rx_buf);

            // TODO check that `payload_length` is sane

            let cmd = {
                let payload = &self.rx_buf[2..2 + header.payload_length() as usize];
                self.ll.process_adv_packet(&mut self.radio, header, payload)
            };
            self.configure_receiver(cmd.radio);
            cmd.next_update
        } else {
            // Wait until disable event is triggered
            while self.radio.radio.events_disabled.read().bits() == 0 {}

            // Re-enter recv mode
            self.radio.radio.tasks_rxen.write(|w| unsafe { w.bits(1) });

            None
        }
    }

    /// Configures the Radio for (not) receiving data according to `cmd`.
    fn configure_receiver(&mut self, cmd: RadioCmd) {
        match cmd {
            RadioCmd::Off => {
                // Disable `END` interrupt, effectively stopping reception
                self.radio.radio.intenclr.write(|w| w.end().clear());

                // Acknowledge left-over disable event
                self.radio.radio.events_disabled.reset();
                // Disable radio
                self.radio.radio.tasks_disable.write(|w| unsafe { w.bits(1) });
                // Then wait until disable event is triggered
                while self.radio.radio.events_disabled.read().bits() == 0 {}
                // And acknowledge it
                self.radio.radio.events_disabled.reset();
            }
            RadioCmd::ListenAdvertising { channel } => {
                self.radio.prepare_txrx_advertising(channel);

                let rx_buf = self.rx_buf as *mut _ as u32;
                self.radio.radio.packetptr.write(|w| unsafe { w.bits(rx_buf) });

                // Acknowledge left-over disable event
                self.radio.radio.events_disabled.write(|w| unsafe { w.bits(0) });

                // Enable `END` interrupt (packet fully received)
                self.radio.radio.intenset.write(|w| w.end().set());

                // Match on logical address 0 only
                self.radio.radio.rxaddresses.write(|w| w.addr0().enabled());

                // ...and enter RX mode
                self.radio.radio.tasks_rxen.write(|w| unsafe { w.bits(1) });
            }
            RadioCmd::ListenData { .. } => unimplemented!(),
        }
    }

    /// Updates the BLE state.
    ///
    /// Returns when and if to call `update` next time.
    // TODO docs
    pub fn update(&mut self) -> Option<Duration> {
        let cmd = self.ll.update(&mut self.radio);

        self.configure_receiver(cmd.radio);

        cmd.next_update
    }
}
