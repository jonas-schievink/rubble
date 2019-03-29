use {
    crate::ble::{
        link::{
            data::{ControlPdu, Pdu},
            queue::{Consume, Consumer, Producer},
            FeatureSet,
        },
        Error,
    },
    log::info,
};

/// Data channel packet processor.
///
/// This hooks up to the Real-Time part of the LE Link Layer via a packet queue. This part can run
/// at a lower priority (eg. being driven in the apps idle loop) and receives and transmits packets
/// using the packet queue.
///
/// Data channel PDUs can either contain L2CAP data or an LL Control PDU. This responder handles
/// both, which is why it's neither placed in the `link` nor `l2cap` modules.
pub struct Responder {
    tx: Producer,
    rx: Option<Consumer>,
}

impl Responder {
    pub fn new(tx: Producer, rx: Consumer) -> Self {
        Self { tx, rx: Some(rx) }
    }

    /// Returns `true` when this responder has work to do.
    ///
    /// If this returns `true`, `process` may be called to process incoming packets and send
    /// outgoing ones.
    pub fn has_work(&mut self) -> bool {
        self.with_rx(|rx, _| rx.has_data())
    }

    /// Processes a single incoming packets in the packet queue.
    ///
    /// Returns `Error::Eof` if there are no incoming packets in the RX queue.
    pub fn process_one(&mut self) -> Result<(), Error> {
        self.with_rx(|rx, this| {
            rx.consume_pdu_with(|_, pdu| match pdu {
                Pdu::Control { data } => {
                    // The only LL Control PDU we have to support is `LL_FEATURE_REQ` (at least
                    // Android doesn't like if it's unsupported; I haven't found anything in the
                    // spec that says it has to be supported).

                    // We don't support any other LL Control PDU right now. Also see:
                    // https://github.com/jonas-schievink/rubble/issues/26

                    let pdu = data.read();
                    info!("LL Control PDU: {:?}", pdu);
                    let response = match pdu {
                        ControlPdu::FeatureReq { .. } => ControlPdu::FeatureRsp {
                            slave_features: FeatureSet::supported(),
                        },
                        _ => ControlPdu::UnknownRsp {
                            unknown_type: pdu.opcode(),
                        },
                    };

                    // Consume the LL Control PDU iff we can fit the response in the TX buffer:
                    Consume::on_success(this.tx.produce_pdu(Pdu::from(&response)))
                }
                _ => unimplemented!(),
            })
        })
    }

    /// A helper method that splits `self` into the `rx` and the remaining `Self`.
    ///
    /// This can possibly be removed after *RFC 2229 (Closures Capture Disjoint Fields)* is
    /// implemented in stable Rust.
    fn with_rx<R>(&mut self, f: impl FnOnce(&mut Consumer, &mut Self) -> R) -> R {
        let mut rx = self.rx.take().unwrap();
        let result = f(&mut rx, self);
        self.rx = Some(rx);
        result
    }
}
