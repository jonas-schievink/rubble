//! A splittable queue for data channel PDUs.
//!
//! Data channel PDUs are received and transmitted in time-critical code, so they're sent through
//! this queue to be processed at a later time (perhaps in the application's `idle` loop).

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
    /// Enqueues a data channel PDU.
    ///
    /// Returns `Error::Eof` when the queue does not have enough free space for both header and
    /// payload.
    pub fn produce_pdu(&mut self, header: data::Header, payload: data::Pdu) -> Result<(), Error> {
        let len = usize::from(header.payload_length()) + 2;
        let mut grant = match self.inner.grant(len) {
            Ok(grant) => grant,
            Err(bbqueue::Error::GrantInProgress) => unreachable!("grant in progress"),
            Err(bbqueue::Error::InsufficientSize) => return Err(Error::Eof),
        };

        LittleEndian::write_u16(&mut grant, header.to_u16());
        payload
            .to_bytes(&mut ByteWriter::new(&mut grant[2..]))
            .unwrap();
        self.inner.commit(len, grant);
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
    /// Tries to read a packet from the queue and passes it to `f`.
    ///
    /// Returns `Error::Eof` if the queue is empty. Other errors can also be returned (eg. if
    /// parsing the data fails).
    pub fn consume_pdu_with<R>(
        &mut self,
        f: impl FnOnce(data::Header, data::Pdu) -> R,
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
        let pdu = data::Pdu::parse(header, raw_payload)?;

        let result = f(header, pdu);

        self.inner.release(pl_len + 2, grant);
        Ok(result)
    }

    pub fn consume_raw_with<R>(
        &mut self,
        f: impl FnOnce(data::Header, &[u8]) -> R,
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

        let result = f(header, raw_payload);

        self.inner.release(pl_len + 2, grant);
        Ok(result)
    }
}

/// Converts a `BBQueue` to a pair of packet queue endpoints.
pub fn create(bb: &'static mut BBQueue) -> (Producer, Consumer) {
    let (p, c) = bb.split();
    (Producer { inner: p }, Consumer { inner: c })
}
