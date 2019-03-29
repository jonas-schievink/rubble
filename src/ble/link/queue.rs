//! A splittable queue for data channel PDUs.
//!
//! Data channel PDUs are received and transmitted in time-critical code, so they're sent through
//! this queue to be processed at a later time (perhaps in the application's idle loop).

use {
    crate::ble::{bytes::*, link::data, Error},
    bbqueue::{self, BBQueue},
    byteorder::{ByteOrder, LittleEndian},
};

/// Writing end of a packet queue.
pub struct Producer {
    inner: bbqueue::Producer,
}

impl Producer {
    /// Queries whether there is a free contiguous chunk of memory of at least `space` bytes.
    pub fn has_space(&mut self, space: usize) -> bool {
        self.inner.grant(space).is_ok()
    }

    /// Enqueues a data channel PDU.
    ///
    /// Returns `Error::Eof` when the queue does not have enough free space for both header and
    /// payload.
    pub fn produce_pdu(&mut self, payload: data::Pdu) -> Result<(), Error> {
        // Problem: We don't know the PDU size before encoding it, but it needs to go into the
        // header.

        // Get the largest contiguous buffer segment and write the PDU there, skipping the header
        // FIXME: This is probably an inefficient use of bbqueue...
        let mut grant = match self.inner.grant_max(usize::max_value()) {
            Ok(grant) => grant,
            Err(bbqueue::Error::GrantInProgress) => unreachable!("grant in progress"),
            Err(bbqueue::Error::InsufficientSize) => return Err(Error::Eof),
        };

        if grant.len() < 2 {
            return Err(Error::Eof);
        }

        let mut writer = ByteWriter::new(&mut grant[2..]);
        let free = writer.space_left();
        payload.to_bytes(&mut writer)?;
        let used = free - writer.space_left();
        assert!(used <= 255);

        let mut header = data::Header::new(payload.llid());
        header.set_payload_length(used as u8);
        LittleEndian::write_u16(&mut grant, header.to_u16());

        self.inner.commit(used + 2, grant);
        Ok(())
    }

    /// Enqueues a data channel PDU, where the payload is given as raw bytes.
    ///
    /// The payload will not be checked for validity.
    pub fn produce_raw(&mut self, header: data::Header, payload: &[u8]) -> Result<(), Error> {
        if usize::from(header.payload_length()) != payload.len() {
            return Err(Error::InvalidLength);
        }

        let len = usize::from(header.payload_length()) + 2;
        let mut grant = match self.inner.grant(len) {
            Ok(grant) => grant,
            Err(bbqueue::Error::GrantInProgress) => unreachable!("grant in progress"),
            Err(bbqueue::Error::InsufficientSize) => return Err(Error::Eof),
        };

        LittleEndian::write_u16(&mut grant, header.to_u16());
        grant[2..].copy_from_slice(payload);
        self.inner.commit(len, grant);
        Ok(())
    }
}

/// Reading end of a packet queue.
pub struct Consumer {
    inner: bbqueue::Consumer,
}

impl Consumer {
    /// Queries whether there is at least 1 packet that can be consumed.
    pub fn has_data(&mut self) -> bool {
        // We only commit whole packets at a time, so if we can read *any* data, we can read an
        // entire packet
        if let Ok(grant) = self.inner.read() {
            self.inner.release(0, grant);
            true
        } else {
            false
        }
    }

    /// Tries to read a packet from the queue and passes it to `f`.
    ///
    /// Returns `Error::Eof` if the queue is empty. Other errors can also be returned (eg. if
    /// parsing the data fails).
    pub fn consume_pdu_with<R>(
        &mut self,
        f: impl FnOnce(data::Header, data::Pdu) -> Consume<R>,
    ) -> Result<R, Error> {
        self.consume_raw_with(|header, raw| {
            let pdu = match data::Pdu::parse(header, raw) {
                Ok(pdu) => pdu,
                Err(e) => return Consume::always(Err(e)),
            };

            f(header, pdu)
        })
    }

    pub fn consume_raw_with<R>(
        &mut self,
        f: impl FnOnce(data::Header, &[u8]) -> Consume<R>,
    ) -> Result<R, Error> {
        // We only ever commit whole PDUs at a time, so reading can also read one PDU at a time
        let grant = match self.inner.read() {
            Ok(grant) => grant,
            Err(bbqueue::Error::GrantInProgress) => unreachable!("grant in progress"),
            Err(bbqueue::Error::InsufficientSize) => return Err(Error::Eof),
        };

        let mut bytes = &*grant;
        let raw_header: [u8; 2] = bytes.read_array().unwrap();
        let header = data::Header::parse(&raw_header);
        let pl_len = usize::from(header.payload_length());
        let raw_payload = &bytes[..pl_len];

        let res = f(header, raw_payload);
        if res.consume {
            self.inner.release(pl_len + 2, grant);
        } else {
            self.inner.release(0, grant);
        }
        res.result
    }
}

/// Bundles a `T` along with information telling a queue whether to consume a packet.
pub struct Consume<T> {
    consume: bool,
    result: Result<T, Error>,
}

impl<T> Consume<T> {
    /// Consume the currently processed packet iff `consume` is `true`, then return `result`.
    pub fn new(consume: bool, result: Result<T, Error>) -> Self {
        Self { consume, result }
    }

    /// Consume the currently processed packet, then return `result`.
    pub fn always(result: Result<T, Error>) -> Self {
        Self {
            consume: true,
            result,
        }
    }

    /// Do not consume the currently processed packet, then return `result`.
    ///
    /// The next call to the `Consumer::consume_*` methods will yield the same packet again.
    pub fn never(result: Result<T, Error>) -> Self {
        Self {
            consume: false,
            result,
        }
    }

    /// Consume the currently processed packet if `result` indicates success, then return the
    /// result.
    pub fn on_success(result: Result<T, Error>) -> Self {
        Self {
            consume: result.is_ok(),
            result,
        }
    }
}

/// Converts a `BBQueue` to a pair of packet queue endpoints.
pub fn create(bb: &'static mut BBQueue) -> (Producer, Consumer) {
    let (p, c) = bb.split();
    (Producer { inner: p }, Consumer { inner: c })
}
