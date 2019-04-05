use {
    crate::ble::{
        l2cap::{ChannelMapper, L2CAPState},
        link::{
            data::{ControlPdu, Pdu},
            queue::{Consume, Consumer, Producer},
            FeatureSet,
        },
        utils::{Hex, HexSlice},
        Error, BLUETOOTH_VERSION,
    },
    log::info,
};

/// Data channel packet processor.
///
/// This hooks up to the Real-Time part of the LE Link Layer via a packet queue. This part can run
/// at a lower priority (eg. being driven in the apps idle loop) and receives and transmits packets
/// using the packet queue.
///
/// LL Control PDUs are answered by the responder directly, and all L2CAP data is forwarded to an
/// `L2CAPState<M>`.
pub struct Responder<M: ChannelMapper> {
    tx: Producer,
    rx: Option<Consumer>,
    l2cap: L2CAPState<M>,
}

impl<M: ChannelMapper> Responder<M> {
    pub fn new(tx: Producer, rx: Consumer, l2cap: L2CAPState<M>) -> Self {
        Self {
            tx,
            rx: Some(rx),
            l2cap,
        }
    }

    /// Returns `true` when this responder has work to do.
    ///
    /// If this returns `true`, `process` may be called to process incoming packets and send
    /// outgoing ones.
    pub fn has_work(&mut self) -> bool {
        self.with_rx(|rx, _| rx.has_data())
    }

    /// Processes a single incoming packet in the packet queue.
    ///
    /// Returns `Error::Eof` if there are no incoming packets in the RX queue.
    pub fn process_one(&mut self) -> Result<(), Error> {
        self.with_rx(|rx, this| {
            rx.consume_pdu_with(|_, pdu| match pdu {
                Pdu::Control { data } => {
                    // We don't support any other LL Control PDU right now. Also see:
                    // https://github.com/jonas-schievink/rubble/issues/26

                    let pdu = data.read();
                    info!("LL Control PDU: {:?}", pdu);
                    let response = match pdu {
                        ControlPdu::FeatureReq { .. } => ControlPdu::FeatureRsp {
                            slave_features: FeatureSet::supported(),
                        },
                        ControlPdu::VersionInd { .. } => {
                            // FIXME this should be something real, and defined somewhere else
                            let comp_id = 0xFFFF;
                            // FIXME this should correlate with the Cargo package version
                            let sub_vers_nr = 0x0000;

                            ControlPdu::VersionInd {
                                vers_nr: BLUETOOTH_VERSION,
                                comp_id: Hex(comp_id),
                                sub_vers_nr: Hex(sub_vers_nr),
                            }
                        }
                        _ => ControlPdu::UnknownRsp {
                            unknown_type: pdu.opcode(),
                        },
                    };

                    // Consume the LL Control PDU iff we can fit the response in the TX buffer:
                    Consume::on_success(this.tx.produce_pdu(Pdu::from(&response)))
                }
                Pdu::DataStart { message } => {
                    info!("L2start: {:?}", HexSlice(message));
                    this.l2cap.process_start(message, &mut this.tx)
                }
                Pdu::DataCont { message } => {
                    info!("L2cont {:?}", HexSlice(message));
                    this.l2cap.process_cont(message, &mut this.tx)
                }
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
