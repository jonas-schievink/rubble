//! Physical layer interactions.
//!
//! Don't expect to see much code here: Most of this layer is hardware.
//!
//! BLE data is transmitted on 40 different RF channels numbered from 0 to 39 with ascending
//! frequencies. Channels 0, 12 and 39 are reserved for advertising, all others are used for data
//! transmission. BLE internally uses so-called "Channel Indices" which reorder the RF channels so
//! that indices 0..=36 refer to data channels and 37..=39 refer to the advertising channels
//! (presumably to simplify channel hopping). The Link-Layer is only interested in these channel
//! indices, so only those are implemented here.

use {crate::utils::ReverseBits, core::fmt};

/// Returns the center frequency in MHz corresponding to an RF channel.
fn rf_channel_freq(rf_channel: u8) -> u16 {
    2402 + u16::from(rf_channel) * 2
}

/// Returns the data whitening IV for a channel index (not RF channel).
fn whitening_iv(channel_idx: u8) -> u8 {
    debug_assert!(channel_idx <= 39);
    0b01000000 | channel_idx
}

/// One of the three advertising channels (channel indices 37, 38 or 39).
#[derive(Copy, Clone, Debug)]
pub struct AdvertisingChannel(u8);

impl AdvertisingChannel {
    /// Returns the first (lowest-numbered) advertising channel.
    pub fn first() -> Self {
        AdvertisingChannel(37)
    }

    /// Returns an iterator that yields all 3 advertising channels in ascending order.
    pub fn iter_all() -> impl Iterator<Item = Self> {
        [
            AdvertisingChannel(37),
            AdvertisingChannel(38),
            AdvertisingChannel(39),
        ]
        .iter()
        .cloned()
    }

    /// Returns the next advertising channel, or the first one if `self` is the last channel.
    pub fn cycle(&self) -> Self {
        if self.0 == 39 {
            AdvertisingChannel(37)
        } else {
            AdvertisingChannel(self.0 + 1)
        }
    }

    /// Returns the RF channel corresponding to this advertising channel index.
    ///
    /// RF channels 0, 12 and 39 are used for advertising.
    pub fn rf_channel(&self) -> u8 {
        match self.0 {
            37 => 0,
            38 => 12,
            39 => 39,
            _ => unreachable!(),
        }
    }

    /// Returns the center frequency of this channel in MHz.
    pub fn freq(&self) -> u16 {
        rf_channel_freq(self.rf_channel())
    }

    /// Calculates the initial value of the LFSR to use for data whitening.
    ///
    /// The value is a 7-bit value. The MSb will always be 0, and the 2nd MSb always 1 (Position 0).
    /// The LSb contains Position 6. Refer to the specification for details about the bit positions.
    ///
    /// The polynomial is always `x^7 + x^4 + 1`.
    ///
    /// Whitening is applied to PDU and CRC.
    pub fn whitening_iv(&self) -> u8 {
        whitening_iv(self.0)
    }
}

/// One of 37 data channels on which data channel PDUs are sent between connected devices.
///
/// (channel indices 0..=36)
#[derive(Copy, Clone, Debug)]
pub struct DataChannel(u8);

impl DataChannel {
    /// Creates a `DataChannelIndex` from a raw index.
    ///
    /// # Panics
    ///
    /// This will panic if `index` is not a valid data channel index. Valid indices are 0..=36.
    pub fn new(index: u8) -> Self {
        assert!(index <= 36);
        DataChannel(index)
    }

    /// Returns the data channel index.
    ///
    /// The returned value is always in range 0..=36.
    pub fn index(&self) -> u8 {
        self.0
    }

    /// Returns the RF channel corresponding to this data channel index.
    ///
    /// RF channels 1-11 and 13-38 are used for data transmission.
    pub fn rf_channel(&self) -> u8 {
        match self.0 {
            ch @ 0...10 => ch + 1,
            ch @ 11...36 => ch + 2,
            _ => unreachable!(),
        }
    }

    /// Returns the center frequency of this channel in MHz.
    pub fn freq(&self) -> u16 {
        rf_channel_freq(self.rf_channel())
    }

    /// Calculates the initial value of the LFSR to use for data whitening.
    ///
    /// The value is a 7-bit value. The MSb will always be 0, and the 2nd MSb always 1 (Position 0).
    /// The LSb contains Position 6. Refer to the specification for details about the bit positions.
    ///
    /// The polynomial is always `x^7 + x^4 + 1`.
    ///
    /// Whitening is applied to PDU and CRC.
    pub fn whitening_iv(&self) -> u8 {
        whitening_iv(self.0)
    }
}

/// A map marking data channels as used or unused.
///
/// A channel map must mark at least 2 channels as used.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ChannelMap {
    raw: [u8; 5],
    num_used_channels: u8,
}

impl ChannelMap {
    /// Create a new channel map from the raw format used in Connection Request PDUs (`ChM`).
    ///
    /// The first byte (LSB) contains flags for data channels 0 to 7, where the least significant
    /// bit is the flag for channel 0, and so on.
    ///
    /// Since there are only 37 data channels, but 40 bits in the 5 Bytes, the 3 most significant
    /// bits in the last Byte of `raw` are considered reserved for future use (RFU) and are ignored
    /// by this function.
    pub fn from_raw(mut raw: [u8; 5]) -> Self {
        raw[4] &= 0b11111; // clear RFU bits
        Self {
            raw,
            num_used_channels: raw.iter().map(|b| b.count_ones() as u8).sum(),
        }
    }

    /// Creates a new channel map that marks all data channels as used.
    pub fn with_all_channels() -> Self {
        Self {
            raw: [0xff, 0xff, 0xff, 0xff, 0b11111],
            num_used_channels: 37,
        }
    }

    /// Returns the number of data channels marked as used by this map.
    pub fn num_used_channels(&self) -> u8 {
        self.num_used_channels
    }

    /// Returns whether the given data channel is marked as used.
    pub fn is_used(&self, channel: DataChannel) -> bool {
        let byte = self.raw[channel.index() as usize / 8];
        let bitnum = channel.index() % 8;
        let mask = 1 << bitnum;

        byte & mask != 0
    }

    /// Returns an iterator over all data channels marked as used in this map.
    pub fn iter_used<'a>(&'a self) -> impl Iterator<Item = DataChannel> + 'a {
        self.raw
            .iter()
            .enumerate()
            .flat_map(move |(byteindex, byte)| {
                (0..8).filter_map(move |bitindex| {
                    if byte & (1 << bitindex) != 0 {
                        Some(DataChannel::new(byteindex as u8 * 8 + bitindex))
                    } else {
                        None
                    }
                })
            })
    }

    /// Returns the `n`th channel marked as used.
    ///
    /// # Panics
    ///
    /// This will panic when `n >= self.num_used_channels()`.
    pub fn by_index(&self, n: u8) -> DataChannel {
        self.iter_used()
            .nth(n.into())
            .expect("by_index: index out of bounds")
    }
}

impl fmt::Display for ChannelMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for b in &self.raw[..4] {
            write!(f, "{:08b}", b.reverse_bits_ext())?;
        }
        write!(f, "{:05b}", self.raw[4].reverse_bits_ext() >> 3)?;
        Ok(())
    }
}

impl fmt::Debug for ChannelMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({:?})", self, self.raw)
    }
}

/// Trait for raw 2.4 GHz non-BLE-specific radios.
///
/// You probably won't need to implement this trait, unless you're working with hardware that has
/// absolutely no special support for BLE. Usually, the Link-Layer `Transmitter` should be
/// implemented.
pub trait Radio {
    /// Transmit every Byte in `buf` over the air, LSb first, at `freq` MHz.
    ///
    /// TODO: Document all radio requirements
    fn transmit(&mut self, buf: &mut [u8], freq: u16);
}

// FIXME Add helpers for parsing adv/data PDU into their headers and payload so they can be passed to the LinkLayer
