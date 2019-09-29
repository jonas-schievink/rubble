//! An SPSC queue for data channel PDUs.
//!
//! Data channel PDUs are received and transmitted in time-critical code, so they're sent through
//! this queue to be processed at a later time (perhaps in the application's idle loop).
//!
//! The queue contains Link-Layer data channel packets, which consist of a 2-Byte header and a
//! dynamically-sized payload.

use {
    crate::{
        bytes::*,
        link::data::{self, Llid},
        Error,
    },
    bbqueue::{self, BBQueue},
    byteorder::{ByteOrder, LittleEndian},
};

/// A splittable SPSC queue for Link-Layer PDUs.
///
/// Must fit at least one packet with `MIN_PDU_BUF` bytes.
pub trait PacketQueue {
    /// Producing half of the queue.
    type Producer: Producer;

    /// Consuming half of the queue.
    type Consumer: Consumer;

    /// Splits the queue into its producing and consuming ends.
    fn split(self) -> (Self::Producer, Self::Consumer);
}

/// The producing (writing) half of a packet queue.
pub trait Producer {
    /// Returns the largest payload size that can be successfully enqueued in the current state.
    ///
    /// This is necessarily a conservative estimate, since the consumer half of the queue might
    /// remove a packet from the queue immediately after this function returns, creating more free
    /// space.
    fn free_space(&mut self) -> u8; // FIXME &self

    /// Enqueues a PDU with known size using a closure.
    ///
    /// This is an object-safe method complemented by its generic counterpart `produce_with`. Only
    /// this method need to be implemented.
    fn produce_dyn(
        &mut self,
        payload_bytes: u8,
        f: &mut dyn FnMut(&mut ByteWriter<'_>) -> Result<Llid, Error>,
    ) -> Result<(), Error>;

    /// Enqueues a PDU with known size using a closure.
    ///
    /// This will check if `payload_bytes` are available in the queue, and bail with `Error::Eof` if
    /// not. If sufficient space is available, a `ByteWriter` with access to that space is
    /// constructed and `f` is called. If `f` returns a successful result, the data is committed to
    /// the queue. If not, the queue is left unchanged.
    fn produce_with<E>(
        &mut self,
        payload_bytes: u8,
        f: impl FnOnce(&mut ByteWriter<'_>) -> Result<Llid, E>,
    ) -> Result<(), E>
    where
        E: From<Error>,
        Self: Sized,
    {
        let mut f = Some(f);
        let mut r = None;
        self.produce_dyn(payload_bytes, &mut |bytes| {
            let f = f.take().unwrap();
            let result = f(bytes);
            if let Ok(llid) = result {
                r = Some(Ok(()));
                Ok(llid)
            } else {
                r = Some(result.map(|_| ()));
                Err(Error::InvalidValue)
            }
        })
        .ok();

        r.unwrap()
    }
}

/// The consuming (reading) half of a packet queue.
pub trait Consumer {
    /// Returns whether there is a packet to dequeue.
    fn has_data(&mut self) -> bool; // FIXME &self

    /// Passes the next raw packet in the queue to a closure.
    ///
    /// The closure returns a `Consume` value to indicate whether the packet should remain in the
    /// queue or be removed.
    fn consume_raw_with<R>(
        &mut self,
        f: impl FnOnce(data::Header, &[u8]) -> Consume<R>,
    ) -> Result<R, Error>;

    /// Passes the next packet in the queue to a closure.
    ///
    /// The closure returns a `Consume` value to indicate whether the packet should remain in the
    /// queue or be removed.
    fn consume_pdu_with<R>(
        &mut self,
        f: impl FnOnce(data::Header, data::Pdu<'_, &[u8]>) -> Consume<R>,
    ) -> Result<R, Error> {
        self.consume_raw_with(|header, raw| {
            let pdu = match data::Pdu::parse(header, raw) {
                Ok(pdu) => pdu,
                Err(e) => return Consume::always(Err(e)),
            };

            f(header, pdu)
        })
    }
}

/// Bundles a `T` along with information telling a queue whether to consume a packet.
#[derive(Debug)]
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

/// `PacketQueue` is only implemented for `&'static` references to a `BBQueue` to avoid soundness
/// bugs in `bbqueue`.
impl PacketQueue for &'static BBQueue {
    type Producer = BbqProducer;
    type Consumer = BbqConsumer;

    fn split(self) -> (Self::Producer, Self::Consumer) {
        let (p, c) = BBQueue::split(self);
        (BbqProducer { inner: p }, BbqConsumer { inner: c })
    }
}

pub struct BbqProducer {
    inner: bbqueue::Producer,
}

impl Producer for BbqProducer {
    fn free_space(&mut self) -> u8 {
        let cap = self.inner.capacity();
        match self.inner.grant_max(cap) {
            Ok(grant) => {
                let space = grant.len();
                self.inner.commit(0, grant);
                space as u8
            }
            Err(_) => 0,
        }
    }

    fn produce_dyn(
        &mut self,
        payload_bytes: u8,
        f: &mut dyn FnMut(&mut ByteWriter<'_>) -> Result<Llid, Error>,
    ) -> Result<(), Error> {
        // 2 additional bytes for the header
        let mut grant = match self.inner.grant(usize::from(payload_bytes + 2)) {
            Ok(grant) => grant,
            Err(bbqueue::Error::GrantInProgress) => unreachable!("grant in progress"),
            Err(bbqueue::Error::InsufficientSize) => return Err(Error::Eof.into()),
        };

        let mut writer = ByteWriter::new(&mut grant[2..]);
        let free = writer.space_left();
        let result = f(&mut writer);
        let used = free - writer.space_left();
        assert!(used <= 255);

        let llid = match result {
            Ok(llid) => llid,
            Err(e) => {
                self.inner.commit(0, grant);
                return Err(e);
            }
        };

        let mut header = data::Header::new(llid);
        header.set_payload_length(used as u8);
        LittleEndian::write_u16(&mut grant, header.to_u16());

        self.inner.commit(used + 2, grant);
        Ok(())
    }
}

pub struct BbqConsumer {
    inner: bbqueue::Consumer,
}

impl Consumer for BbqConsumer {
    fn has_data(&mut self) -> bool {
        // We only commit whole packets at a time, so if we can read *any* data, we can read an
        // entire packet
        if let Ok(grant) = self.inner.read() {
            self.inner.release(0, grant);
            true
        } else {
            false
        }
    }

    fn consume_raw_with<R>(
        &mut self,
        f: impl FnOnce(data::Header, &[u8]) -> Consume<R>,
    ) -> Result<R, Error> {
        // We only ever commit whole PDUs at a time, so reading can also read one PDU at a time
        let grant = match self.inner.read() {
            Ok(grant) => grant,
            Err(bbqueue::Error::GrantInProgress) => unreachable!("grant in progress"),
            Err(bbqueue::Error::InsufficientSize) => return Err(Error::Eof),
        };

        let mut bytes = ByteReader::new(&grant);
        let raw_header: [u8; 2] = bytes.read_array().unwrap();
        let header = data::Header::parse(&raw_header);
        let pl_len = usize::from(header.payload_length());
        let raw_payload = bytes.read_slice(pl_len)?;

        let res = f(header, raw_payload);
        if res.consume {
            self.inner.release(pl_len + 2, grant);
        } else {
            self.inner.release(0, grant);
        }
        res.result
    }
}
