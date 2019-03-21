//! Link-Layer.
//!
//! Note that a hardware BLE radio will already implement a few aspects of the link layer (such as
//! CRC calculation, preamble generation, etc.). Consider this module to be a construction kit for
//! BLE Link-Layers: Take whatever your hardware can do, supplement it with a few condiments from
//! this module, and you get a (hopefully) working Link-Layer.
//!
//! Refer to the Link Layer Specification for details and more graphics and tables.
//!
//! All following graphics are based on the Bluetooth specification. If a field is marked with `-`,
//! it should be set to 0 when sending such a packet, and ignored when receiving it (the spec calls
//! these "RFU" = Reserved for Future Use).
//!
//! # Packet Format
//!
//! All values are transmitted in little-endian bit order unless otherwise noted. All fields in
//! graphics are ordered starting with the field transmitted first (LSB).
//!
//! The following graphic illustrates the raw in-air packet format. The packet transfers a PDU whose
//! format depends on whether it is sent on an *advertising channel* or a *data channel*.
//!
//! ```notrust
//! LSB                                                     MSB
//! +-----------+----------------+---------------+------------+
//! | Preamble  | Access Address |     PDU       |  CRC       |
//! | (1 octet) | (4 octets)     | (2-39 octets) | (3 octets) |
//! +-----------+----------------+---------------+------------+
//!                              \---------------/      ^
//!                                      |              |
//!                                      +--------------+
//!                                     CRC is calculated
//!                                       over the PDU
//!
//!                              \----------------------------/
//!                                    Data Whitening is
//!                                 applied to PDU and CRC
//! ```
//!
//! The 24-bit CRC value is transmitted MSb first. Length of the PDU depends on the kind of PDU
//! (advertising or data channel).
//!
//! ## Advertising Channel PDU
//!
//! Each advertising channel PDU consists of a 16-bit header and a variably-sized payload, the
//! length of which is stored in a header field.
//!
//! ```notrust
//! LSB                                           MSB
//! +-------------+---------------------------------+
//! |  Header     |             Payload             |
//! |  (16 bits)  |    (length stored in header)    |
//! +-------------+---------------------------------+
//! ```
//!
//! The header looks like this:
//!
//! ```notrust
//! LSB                                                                     MSB
//! +------------+------------+---------+---------+--------------+------------+
//! |  PDU Type  |     -      |  TxAdd  |  RxAdd  |    Length    |     -      |
//! |  (4 bits)  |  (2 bits)  | (1 bit) | (1 bit) |   (6 bits)   |  (2 bits)  |
//! +------------+------------+---------+---------+--------------+------------+
//! ```
//!
//! The `TxAdd` and `RxAdd` field are only used for some payloads, for all others, they should be
//! set to 0.
//!
//! Length may be in range 6 to 36 (inclusive).
//!
//! The data in `Payload` depends on the PDU Type. Refer to the spec or [`advertising::PduType`] for
//! details.
//!
//! [`advertising::PduType`]: advertising/enum.PduType.html
//!
//! ## Data Channel PDU
//!
//! A data channel PDU also contains a 16-bit header (but with a different layout) and a
//! variably-sized payload.
//!
//! If the connection is encrypted and the payload contains at least 1 octet, a Message Integrity
//! Check (MIC) is appended at the end.
//!
//! ```notrust
//! LSB                                          MSB
//! +-----------+----------------------+ - - - - - +
//! |  Header   |        Payload       |    MIC    |
//! | (16 bits) |    (0..=27 octets)   | (32 bits) |
//! +-----------+----------------------+ - - - - - +
//! ```
//!
//! Header layout:
//!
//! ```notrust
//! LSB                                                                           MSB
//! +----------+---------+---------+---------+------------+--------------+----------+
//! |   LLID   |  NESN   |   SN    |   MD    |     -      |    Length    |    -     |
//! | (2 bits) | (1 bit) | (1 bit) | (1 bit) |  (3 bits)  |   (5 bits)   | (3 bits) |
//! +----------+---------+---------+---------+------------+--------------+----------+
//! ```
//!
//! Payload format depends on the value of the 2-bit `LLID` field:
//!
//! * `0b00`: Reserved value.
//! * `0b01`: LL Data PDU Continuation fragment or empty PDU.
//! * `0b10`: LL Data PDU Start of L2CAP message (or complete message if no fragmentation
//!   necessary).
//! * `0b11`: LL Control PDU.
//!
//! The `NESN` field specifies the **N**ext **E**xpected **S**equence **N**umber. The `SN` field
//! specifies the **S**equence **N**umber of this PDU.
//!
//! The `MD` field specifies that the device sending the packet has more data to send. When both
//! slave and master send a packet with the `MD` bit set to 0, the connection is closed.
//!
//! The `Length` field specifies the length of payload **and `MIC`**. Its maximum value is 31,
//! resulting in a 27 octet Payload (the maximum) and a 32-bit `MIC`.
//!
//! Note that the `Length` field is 1 bit shorter than for Advertising Channel PDUs.

pub mod ad_structure;
pub mod advertising;
pub mod data;
mod device_address;

pub use self::device_address::*;

use {
    self::{
        ad_structure::AdStructure,
        advertising::{Pdu, PduBuf},
    },
    super::{
        crc::ble_crc24,
        log::{Logger, NoopLogger},
        phy::{AdvertisingChannel, DataChannel, Radio},
        utils::HexSlice,
        Error,
    },
    byteorder::{ByteOrder, LittleEndian},
    core::{ops::Range, time::Duration},
    ux::u24,
};

/// The CRC polynomial to use for CRC24 generation.
///
/// If your radio has hardware support for CRC generation, you may use (parts of) this value to
/// configure it (if necessary). The CRC should be computed only over the PDU. Also note that the
/// CRC, unlike every other field, is transmitted MSb first.
///
/// Counting from the least-significant bit (bit 0), bit `k` in this value is set if the term `x^k`
/// occurs in the CRC polynomial. This includes bit 24, which is usually not explicitly specified.
///
/// Written out, the polynomial is:
///
/// `x^24 + x^10 + x^9 + x^6 + x^4 + x^3 + x + 1`
pub const CRC_POLY: u32 = 0b00000001_00000000_00000110_01011011;

/// Max. PDU payload size in Bytes (for both advertising and data channels).
pub const MAX_PAYLOAD_SIZE: usize = 255;

/// Max. PDU size in octets (header + payload).
pub const MAX_PDU_SIZE: usize = MAX_PAYLOAD_SIZE + 2; // data & adv. have a 16-bit header

/// Max. total Link-Layer packet size in octets.
pub const MAX_PACKET_SIZE: usize = 1 /* preamble */ + 4 /* access addr */ + MAX_PDU_SIZE + 3 /* crc */;

/// Link-Layer state machine, according to the Bluetooth spec.
enum State {
    /// Radio silence: Not listening, not transmitting anything.
    Standby,

    /// Device is an advertiser. Periodic transmission of advertising packets and (optionally)
    /// listening for responses.
    ///
    /// Note that this does not imply that the device wants to establish a *connection* with another
    /// device: The device could also be a beacon that's broadcasting advertising packets with data.
    Advertising {
        /// Advertising interval.
        // TODO: check spec for allowed/recommended values and check for them
        interval: Duration,

        /// Precomputed PDU payload to copy into the transmitter's buffer.
        pdu: advertising::PduBuf,

        /// Next advertising channel to use for a message.
        // FIXME: spec check; no idea what order or change delay
        channel: AdvertisingChannel,
    },

    /// Connected with other device.
    #[allow(unused)]
    Connection {
        access_address: u32,
        crc: u24,
        // ...
    },
}

/// Implementation of the BLE Link-Layer logic.
///
/// Users of this struct must provide a way to send and receive Link-Layer PDUs via a `Transmitter`.
pub struct LinkLayer<L: Logger> {
    dev_addr: DeviceAddress,
    state: State,
    logger: L,
}

impl LinkLayer<NoopLogger> {
    /// Creates a new Link-Layer in standby state (ie. not transmitting or listening for data).
    pub fn new(dev_addr: DeviceAddress) -> Self {
        Self::with_logger(dev_addr, NoopLogger)
    }
}

impl<L: Logger> LinkLayer<L> {
    /// Creates a new Link-Layer with a custom logger.
    pub fn with_logger(dev_addr: DeviceAddress, mut logger: L) -> Self {
        trace!(logger, "new LinkLayer, dev={:?}", dev_addr);
        Self {
            dev_addr,
            logger,
            state: State::Standby,
        }
    }

    pub fn logger(&mut self) -> &mut L {
        &mut self.logger
    }

    /// Starts advertising this device, optionally sending data along with the advertising PDU.
    ///
    /// Note that advertisements will only be sent by calling `update`.
    pub fn start_advertise(
        &mut self,
        interval: Duration,
        data: &[AdStructure],
    ) -> Result<(), Error> {
        // TODO tear down existing connection?

        let pdu = PduBuf::discoverable(self.dev_addr, data)?;
        debug!(self.logger, "start_advertise: adv_data = {:?}", data);
        debug!(self.logger, "start_advertise: PDU = {:?}", pdu);
        self.state = State::Advertising {
            interval,
            pdu,
            channel: AdvertisingChannel::first(),
        };
        Ok(())
    }

    /// Process an incoming packet from an advertising channel.
    ///
    /// The access address of the packet must be `ADVERTISING_ADDRESS`.
    pub fn process_adv_packet<T: Transmitter>(
        &mut self,
        tx: &mut T,
        header: advertising::Header,
        mut payload: &[u8],
        crc_ok: bool,
    ) -> Cmd {
        let pdu = advertising::Pdu::from_header_and_payload(header, &mut payload);

        if let Ok(pdu) = pdu {
            if let State::Advertising { channel, .. } = &self.state {
                if crc_ok && pdu.receiver() == Some(&self.dev_addr) {
                    // Got a packet addressed at us, can be a scan or connect request
                    match pdu {
                        Pdu::ScanRequest { .. } => {
                            let scan_data = &[]; // TODO make this configurable
                            let response = PduBuf::scan_response(self.dev_addr, scan_data).unwrap();
                            tx.transmit_advertising(response.header(), *channel);

                            // Log after responding to meet timing
                            debug!(self.logger, "<- SCAN REQUEST {:?}", pdu);
                            debug!(self.logger, "-> {:?}", response);
                        }
                        Pdu::ConnectRequest { lldata, .. } => {
                            debug!(self.logger, "<- CONNECT REQUEST {:?}", pdu);

                            self.state = State::Connection {
                                access_address: lldata.access_address(),
                                crc: lldata.crc_init(),
                            };
                        }
                        _ => {}
                    }
                }
            }
        }

        trace!(
            self.logger,
            "ADV<- {}{:?}, {:?}",
            if crc_ok { "" } else { "BADCRC " },
            header,
            HexSlice(payload),
        );
        trace!(self.logger, "{:?}\n\n", pdu);

        match self.state {
            State::Standby => unreachable!("standby, can't receive packets"),
            State::Connection { .. } => unimplemented!(),
            State::Advertising { channel, .. } => {
                Cmd {
                    radio: RadioCmd::ListenAdvertising { channel },
                    // no change
                    next_update: None,
                }
            }
        }
    }

    /// Process an incoming data channel packet.
    pub fn process_data_packet<T: Transmitter>(
        &mut self,
        tx: &mut T,
        header: data::Header,
        payload: &[u8],
    ) -> Cmd {
        let _ = (tx, header, payload);
        unimplemented!();
    }

    /// Update the Link-Layer state.
    ///
    /// This should be called in regular intervals, independent of whether packets were received and
    /// processed.
    ///
    /// # Parameters
    ///
    /// * `tx`: A `Transmitter` for sending packets.
    /// * `elapsed`: Time since the last `update` call or creation of this `LinkLayer`.
    pub fn update<T: Transmitter>(&mut self, tx: &mut T) -> Cmd {
        match self.state {
            State::Advertising {
                interval,
                ref pdu,
                ref mut channel,
            } => {
                *channel = channel.cycle();
                let payload = pdu.payload();
                let buf = tx.tx_payload_buf();
                buf[..payload.len()].copy_from_slice(payload);

                // FIXME According to the spec, this has to broadcast on all advertising channels

                //trace!(self.logger, "->[ADV] {} MHz", channel.freq());
                tx.transmit_advertising(pdu.header(), *channel);

                Cmd {
                    radio: RadioCmd::ListenAdvertising { channel: *channel },
                    next_update: Some(interval),
                }
            }
            _ => unimplemented!(),
        }
    }

    pub fn is_advertising(&self) -> bool {
        if let State::Advertising { .. } = self.state {
            true
        } else {
            false
        }
    }
}

/// Command returned by the Link-Layer to the user.
///
/// Specifies how the radio should be configured and when/if to call `LinkLayer::update` again.
#[must_use]
pub struct Cmd {
    /// Radio configuration request.
    pub radio: RadioCmd,

    /// Time until `LinkLayer::update` should be called.
    ///
    /// If this is `None`, `update` doesn't need to be called because the Link-Layer is in Standby
    /// state.
    pub next_update: Option<Duration>,
    // FIXME This is used both as "if this is None, don't change the next update time" and "if this
    // is None, don't call `update`"
}

/// Specifies whether the radio should listen for transmissions.
///
/// Returned by the Link-Layer update and processing methods to reconfigure the radio as needed.
pub enum RadioCmd {
    /// Turn the radio off and don't call `LinkLayer::process_*` methods.
    ///
    /// `LinkLayer::update` must still be called according to `Cmd`'s `next_update` field.
    Off,

    /// Listen on an advertising channel. If a packet is received, pass it to
    /// `LinkLayer::process_adv_packet`.
    ListenAdvertising {
        /// The advertising channel to listen on.
        channel: AdvertisingChannel,
    },

    /// Listen on a data channel. If a matching packet is received, pass it to
    /// `LinkLayer::process_data_packet`.
    ListenData {
        /// The data channel to listen on.
        channel: DataChannel,

        /// The Access Address to listen for.
        ///
        /// Packets with a different Access Address must not be passed to the Link-Layer. You may be
        /// able to use your Radio's hardware address matching for this.
        access_address: u32,
    },
}

/// Trait for Link Layer packet transmission.
///
/// The specifics of sending a Link-Layer packet depend on the underlying hardware. The `link`
/// module provides building blocks that enable implementations without any BLE hardware support,
/// just a compatible radio is needed.
pub trait Transmitter {
    /// Get a reference to the Transmitter's PDU payload buffer.
    // FIXME This really wants to be [u8; MAX_PAYLOAD] instead, but conversion from &[u8] isn't convenient
    fn tx_payload_buf(&mut self) -> &mut [u8];

    /// Transmit an Advertising Channel PDU.
    ///
    /// For Advertising Channel PDUs, the CRC initialization value is always `CRC_PRESET`, and the
    /// Access Address is always `ADVERTISING_ADDRESS`.
    ///
    /// The implementor is expected to send the preamble and access address, and assemble the rest
    /// of the packet, and must apply data whitening and do the CRC calculation. The inter-frame
    /// spacing also has to be implemented by the implementor (`T_IFS`).
    ///
    /// # Parameters
    ///
    /// * `header`: Advertising Channel PDU Header to prepend to the Payload in `payload_buf()`.
    /// * `channel`: Advertising Channel Index to transmit on.
    fn transmit_advertising(&mut self, header: advertising::Header, channel: AdvertisingChannel);

    /// Transmit a Data Channel PDU.
    ///
    /// The implementor is expected to send the preamble and assemble the rest of the packet, and
    /// must apply data whitening and do the CRC calculation.
    ///
    /// # Parameters
    ///
    /// * `access_address`: The Access Address of the Link-Layer packet.
    /// * `crc_iv`: CRC calculation initial value (`CRC_PRESET` for advertising channel).
    /// * `header`: Data Channel PDU Header to be prepended to the Payload in `payload_buf()`.
    /// * `channel`: Data Channel Index to transmit on.
    fn transmit_data(
        &mut self,
        access_address: u32,
        crc_iv: u32,
        header: data::Header,
        channel: DataChannel,
    );
}

/// A `Transmitter` that lowers Link-Layer packets to raw byte arrays that can be directly
/// transmitted over the air, given a suitable radio.
///
/// This implements preamble generation, CRC calculation and whitening in software.
pub struct RawTransmitter<R: Radio> {
    tx_buf: [u8; MAX_PACKET_SIZE],
    radio: R,
}

// First 5 octets are Preamble and Access Address
const PDU_START: usize = 5;
const HEADER_RANGE: Range<usize> = PDU_START..PDU_START + 2;
const PAYLOAD_RANGE: Range<usize> = PDU_START + 2..PDU_START + MAX_PDU_SIZE;

impl<R: Radio> RawTransmitter<R> {
    pub fn new(radio: R) -> Self {
        Self {
            tx_buf: [0; MAX_PACKET_SIZE as usize],
            radio,
        }
    }

    fn transmit(&mut self, access_address: u32, payload_length: u8, crc_iv: u32, freq: u16) {
        let preamble = if access_address & 1 == 1 {
            0b01010101
        } else {
            0b10101010
        };
        self.tx_buf[0] = preamble;

        LittleEndian::write_u32(&mut self.tx_buf[1..5], access_address);

        let crc = ble_crc24(
            &self.tx_buf[PDU_START..PDU_START + 2 + payload_length as usize],
            crc_iv,
        );
        LittleEndian::write_u24(&mut self.tx_buf[MAX_PACKET_SIZE - 3..], crc);

        // TODO whitening
        if true {
            unimplemented!();
        }

        self.radio.transmit(&mut self.tx_buf, freq);
    }
}

impl<R: Radio> Transmitter for RawTransmitter<R> {
    fn tx_payload_buf(&mut self) -> &mut [u8] {
        &mut self.tx_buf[PAYLOAD_RANGE]
    }

    fn transmit_advertising(&mut self, header: advertising::Header, channel: AdvertisingChannel) {
        LittleEndian::write_u16(&mut self.tx_buf[HEADER_RANGE], header.to_u16());
        self.transmit(
            advertising::ACCESS_ADDRESS,
            header.payload_length(),
            advertising::CRC_PRESET,
            channel.freq(),
        );
    }

    fn transmit_data(
        &mut self,
        access_address: u32,
        crc_iv: u32,
        header: data::Header,
        channel: DataChannel,
    ) {
        LittleEndian::write_u16(&mut self.tx_buf[HEADER_RANGE], header.to_u16());
        self.transmit(
            access_address,
            header.payload_length(),
            crc_iv,
            channel.freq(),
        );
    }
}

/*

Stack API

When connected, user code is expected to...

* Listen for transmissions (filtered by Access Address, probably)
* Forward PDUs of matched transmissions to the stack
* While the stack does its thing, give it the ability to transmit PDUs

Actually, it seems that the same applies when in any other Link-Layer state except `Standby`.

TODO: what about inter frame spacing?

TODO: make AccessAddress type with support for validation and generation

*/
