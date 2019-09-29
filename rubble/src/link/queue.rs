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
        link::{
            data::{self, Llid},
            MIN_PAYLOAD_BUF, MIN_PDU_BUF,
        },
        Error,
    },
    byteorder::{ByteOrder, LittleEndian},
    heapless::{
        consts::U1,
        spsc::{self, MultiCore},
    },
};

/// A splittable SPSC queue for Link-Layer PDUs.
///
/// Must fit at least one packet with `MIN_PDU_BUF` bytes.
pub trait PacketQueue<'a> {
    /// Producing half of the queue.
    type Producer: Producer;

    /// Consuming half of the queue.
    type Consumer: Consumer;

    /// Splits the queue into its producing and consuming ends.
    fn split(&'a mut self) -> (Self::Producer, Self::Consumer);
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

/// A simple packet queue that can hold a single packet.
///
/// This type is compatible with thumbv6 cores, which lack atomic operations that might be needed
/// for some queue implementations.
pub struct SimpleQueue {
    inner: spsc::Queue<[u8; MIN_PDU_BUF], U1, u8, MultiCore>,
}

impl SimpleQueue {
    /// Creates a new, empty queue.
    pub const fn new() -> Self {
        Self {
            inner: spsc::Queue(heapless::i::Queue::u8()),
        }
    }
}

impl<'a> PacketQueue<'a> for SimpleQueue {
    type Producer = SimpleProducer<'a>;

    type Consumer = SimpleConsumer<'a>;

    fn split(&'a mut self) -> (Self::Producer, Self::Consumer) {
        let (p, c) = self.inner.split();
        (SimpleProducer { inner: p }, SimpleConsumer { inner: c })
    }
}

pub struct SimpleProducer<'a> {
    inner: spsc::Producer<'a, [u8; MIN_PDU_BUF], U1, u8, MultiCore>,
}

impl<'a> Producer for SimpleProducer<'a> {
    fn free_space(&mut self) -> u8 {
        if self.inner.ready() {
            MIN_PAYLOAD_BUF as u8
        } else {
            0
        }
    }

    fn produce_dyn(
        &mut self,
        payload_bytes: u8,
        f: &mut dyn FnMut(&mut ByteWriter<'_>) -> Result<Llid, Error>,
    ) -> Result<(), Error> {
        assert!(usize::from(payload_bytes) < MIN_PAYLOAD_BUF);

        if !self.inner.ready() {
            return Err(Error::Eof);
        }

        let mut buf = [0; MIN_PDU_BUF];
        let mut writer = ByteWriter::new(&mut buf[2..]);
        let free = writer.space_left();
        let llid = f(&mut writer)?;
        let used = free - writer.space_left();

        let mut header = data::Header::new(llid);
        header.set_payload_length(used as u8);
        LittleEndian::write_u16(&mut buf, header.to_u16());

        self.inner.enqueue(buf).map_err(|_| ()).unwrap();
        Ok(())
    }
}

pub struct SimpleConsumer<'a> {
    inner: spsc::Consumer<'a, [u8; MIN_PDU_BUF], U1, u8, MultiCore>,
}

impl<'a> Consumer for SimpleConsumer<'a> {
    fn has_data(&mut self) -> bool {
        self.inner.ready()
    }

    fn consume_raw_with<R>(
        &mut self,
        f: impl FnOnce(data::Header, &[u8]) -> Consume<R>,
    ) -> Result<R, Error> {
        if let Some(packet) = self.inner.peek() {
            let mut bytes = ByteReader::new(packet);
            let raw_header: [u8; 2] = bytes.read_array().unwrap();
            let header = data::Header::parse(&raw_header);
            let pl_len = usize::from(header.payload_length());
            let raw_payload = bytes.read_slice(pl_len)?;

            let res = f(header, raw_payload);
            if res.consume {
                self.inner.dequeue().unwrap(); // can't fail
            }
            res.result
        } else {
            Err(Error::Eof)
        }
    }
}
