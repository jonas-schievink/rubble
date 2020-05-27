//! An SPSC queue for data channel PDUs.
//!
//! Data channel PDUs are received and transmitted in time-critical code, so they're sent through
//! a queue to be processed at a later time (eg. in the application's idle loop).
//!
//! This module defines the following important items:
//!
//! * The [`PacketQueue`] trait, which is implemented by all types providing a packet queue. A type
//!   implementing this trait is needed to use the Rubble stack.
//! * The [`Producer`] and [`Consumer`] traits, which define the queue functionality used after
//!   splitting a [`PacketQueue`].
//! * The [`SimpleQueue`], [`SimpleProducer`] and [`SimpleConsumer`] types, a minimal implementation
//!   of the queue interface defined by [`PacketQueue`], [`Producer`] and [`Consumer`].
//!
//! [`PacketQueue`]: trait.PacketQueue.html
//! [`Producer`]: trait.Producer.html
//! [`Consumer`]: trait.Consumer.html
//! [`SimpleQueue`]: struct.SimpleQueue.html
//! [`SimpleProducer`]: struct.SimpleProducer.html
//! [`SimpleConsumer`]: struct.SimpleConsumer.html

use {
    crate::{
        bytes::*,
        link::{
            data::{self, Llid},
            MIN_DATA_PAYLOAD_BUF, MIN_DATA_PDU_BUF,
        },
        Error,
    },
    byteorder::{ByteOrder, LittleEndian},
    heapless::{
        consts::U1,
        spsc::{self, MultiCore},
    },
};

/// A splittable SPSC queue for data channel PDUs.
///
/// Implementations of this trait must fit at least one data channel packet with a total size of
/// `MIN_DATA_PDU_BUF` bytes (header and payload).
pub trait PacketQueue {
    /// Producing (writing) half of the queue.
    type Producer: Producer;

    /// Consuming (reading) half of the queue.
    type Consumer: Consumer;

    /// Splits the queue into its producing and consuming ends.
    ///
    /// Note that this takes `self` by value, while many queue implementations provide a `split`
    /// method that takes `&'a mut self` and returns producer and consumer types with lifetimes in
    /// them. These implementations still work with this interface, however: `PacketQueue` needs to
    /// be implemented generically over a lifetime `'a`, for a self type `&'a mut OtherQueue`, then
    /// the associated `Producer` and `Consumer` types can make use of the lifetime `'a`.
    ///
    /// For an example of that, see the provided impl for `&'a mut SimpleQueue` in this file.
    fn split(self) -> (Self::Producer, Self::Consumer);
}

/// The producing (writing) half of a packet queue.
pub trait Producer {
    /// Returns the largest payload size that can be successfully enqueued in the current state.
    ///
    /// This is necessarily a conservative estimate, since the consumer half of the queue might
    /// remove a packet from the queue immediately after this function returns, creating more free
    /// space.
    ///
    /// After a call to this method, the next call to any of the `produce_*` methods must not fail
    /// when a `payload_bytes` value less than or equal to the returned free payload space is
    /// passed.
    fn free_space(&self) -> u8;

    /// Enqueues a PDU with known size using a closure.
    ///
    /// *This is an object-safe method complemented by its generic counterpart `produce_with`. Only
    /// this method need to be implemented.*
    ///
    /// This will check if `payload_bytes` are available in the queue, and bail with `Error::Eof` if
    /// not. If sufficient space is available, a `ByteWriter` with access to that space is
    /// constructed and `f` is called. If `f` returns a successful result, the data is committed to
    /// the queue. If not, the queue is left unchanged.
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
        // This forwards to `produce_dyn`, but the call should be trivial to devirtualize (only
        // simple constant propagation is needed). The `Option`s should then be trivial to optimize
        // out as well.

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
    fn has_data(&self) -> bool;

    /// Passes the next raw packet in the queue to a closure.
    ///
    /// The closure returns a `Consume` value to indicate whether the packet should remain in the
    /// queue or be removed.
    ///
    /// If the queue is empty, `Error::Eof` is returned.
    fn consume_raw_with<R>(
        &mut self,
        f: impl FnOnce(data::Header, &[u8]) -> Consume<R>,
    ) -> Result<R, Error>;

    /// Passes the next packet in the queue to a closure.
    ///
    /// The closure returns a `Consume` value to indicate whether the packet should remain in the
    /// queue or be removed.
    ///
    /// If the queue is empty, `Error::Eof` is returned.
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
/// for other queue implementations.
///
/// This queue also minimizes RAM usage: In addition to the raw buffer space, only minimal space is
/// needed for housekeeping.
pub struct SimpleQueue {
    inner: spsc::Queue<[u8; MIN_DATA_PDU_BUF], U1, u8, MultiCore>,
}

impl SimpleQueue {
    /// Creates a new, empty queue.
    pub const fn new() -> Self {
        Self {
            inner: spsc::Queue(heapless::i::Queue::u8()),
        }
    }
}

impl<'a> PacketQueue for &'a mut SimpleQueue {
    type Producer = SimpleProducer<'a>;

    type Consumer = SimpleConsumer<'a>;

    fn split(self) -> (Self::Producer, Self::Consumer) {
        let (p, c) = self.inner.split();
        (SimpleProducer { inner: p }, SimpleConsumer { inner: c })
    }
}

/// Producer (writer) half returned by `SimpleQueue::split`.
pub struct SimpleProducer<'a> {
    inner: spsc::Producer<'a, [u8; MIN_DATA_PDU_BUF], U1, u8, MultiCore>,
}

impl<'a> Producer for SimpleProducer<'a> {
    fn free_space(&self) -> u8 {
        // We can only have space for either 0 or 1 packets with min. payload size
        if self.inner.ready() {
            MIN_DATA_PAYLOAD_BUF as u8
        } else {
            0
        }
    }

    fn produce_dyn(
        &mut self,
        payload_bytes: u8,
        f: &mut dyn FnMut(&mut ByteWriter<'_>) -> Result<Llid, Error>,
    ) -> Result<(), Error> {
        assert!(usize::from(payload_bytes) <= MIN_DATA_PAYLOAD_BUF);

        if !self.inner.ready() {
            return Err(Error::Eof);
        }

        let mut buf = [0; MIN_DATA_PDU_BUF];
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

/// Consumer (reader) half returned by `SimpleQueue::split`.
pub struct SimpleConsumer<'a> {
    inner: spsc::Consumer<'a, [u8; MIN_DATA_PDU_BUF], U1, u8, MultiCore>,
}

impl<'a> Consumer for SimpleConsumer<'a> {
    fn has_data(&self) -> bool {
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

/// Runs Rubble's packet queue testsuite against the given `PacketQueue`.
///
/// This can be used when implementing your own packet queue. Simply create a `#[test]` function as
/// usual and call `run_tests` from there. The function will panic when any test fails.
///
/// The passed `queue` must be an empty queue with bounded space for at least one packet. "Bounded"
/// here means that the queue must have a finite, practical size as the test suite may fill it up
/// completely.
///
/// Simultaneously, this function ensures that `PacketQueue` implementors can be used by a generic
/// function, something that sometimes doesn't work when invariant lifetimes are involved.
pub fn run_tests(queue: impl PacketQueue) {
    fn assert_empty(c: &mut impl Consumer) {
        assert!(!c.has_data(), "empty queue `has_data()` returned true");

        let err = c
            .consume_raw_with(|_, _| -> Consume<()> {
                unreachable!("`consume_raw_with` on empty queue invoked the callback");
            })
            .unwrap_err();

        assert_eq!(
            err,
            Error::Eof,
            "empty queues `consume_raw_with` didn't return expected `Error::Eof"
        );

        let err = c
            .consume_pdu_with(|_, _| -> Consume<()> {
                unreachable!("`consume_pdu_with` on empty queue invoked the callback");
            })
            .unwrap_err();

        assert_eq!(
            err,
            Error::Eof,
            "empty queues `consume_pdu_with` didn't return expected `Error::Eof"
        );
    }

    let (mut p, mut c) = queue.split();
    assert_empty(&mut c);

    let free_space = p.free_space();
    assert!(
        free_space >= MIN_DATA_PAYLOAD_BUF as u8,
        "empty queue has space for {} byte payload, need at least {}",
        free_space,
        MIN_DATA_PAYLOAD_BUF
    );

    // Enqueue the largest packet
    p.produce_with(MIN_DATA_PAYLOAD_BUF as u8, |writer| -> Result<_, Error> {
        assert_eq!(
            writer.space_left(),
            MIN_DATA_PAYLOAD_BUF,
            "produce_with didn't pass ByteWriter with correct buffer"
        );
        writer.write_slice(&[0; MIN_DATA_PAYLOAD_BUF]).unwrap();
        Ok(Llid::DataStart)
    })
    .expect("enqueuing packet failed");

    assert!(
        c.has_data(),
        "consumer's `has_data()` still false after enqueuing packet"
    );

    // Peek at the packet
    c.consume_raw_with(|header, data| -> Consume<()> {
        assert_eq!(usize::from(header.payload_length()), MIN_DATA_PAYLOAD_BUF);
        assert_eq!(
            data,
            &[0; MIN_DATA_PAYLOAD_BUF][..],
            "consume_raw_with didn't yield correct payload"
        );
        Consume::never(Ok(()))
    })
    .expect("consume_raw_with failed when data is available");

    assert!(
        c.has_data(),
        "`has_data()` returned false after using `Consume::never`"
    );

    // Now consume it
    c.consume_pdu_with(|header, _| -> Consume<()> {
        assert_eq!(usize::from(header.payload_length()), MIN_DATA_PAYLOAD_BUF);
        Consume::always(Ok(()))
    })
    .expect("consume_pdu_with failed when data is available");

    // Queue should be emptied out
    assert_empty(&mut c);

    // Enqueue the smallest packet (0 payload bytes)
    p.produce_with(0, |writer| -> Result<_, Error> {
        assert_eq!(
            writer.space_left(),
            MIN_DATA_PAYLOAD_BUF,
            "produce_with didn't pass ByteWriter with correct buffer"
        );
        Ok(Llid::DataStart)
    })
    .expect("enqueuing packet failed");

    assert!(
        c.has_data(),
        "consumer's `has_data()` still false after enqueuing empty packet"
    );

    // Peek at the packet
    c.consume_raw_with(|header, data| -> Consume<()> {
        assert_eq!(usize::from(header.payload_length()), 0);
        assert_eq!(
            data,
            &[][..],
            "consume_raw_with didn't yield correct empty payload"
        );
        Consume::never(Ok(()))
    })
    .expect("consume_raw_with failed when empty packet is available");

    assert!(
        c.has_data(),
        "`has_data()` returned false after using `Consume::never`"
    );

    // Now consume it
    c.consume_pdu_with(|header, _| -> Consume<()> {
        assert_eq!(usize::from(header.payload_length()), 0);
        Consume::always(Ok(()))
    })
    .expect("consume_pdu_with failed when empty packet is available");

    // Queue should be emptied out
    assert_empty(&mut c);

    // FIXME: This test could do a lot more
}

#[test]
fn simple_queue() {
    run_tests(&mut SimpleQueue::new());
}
