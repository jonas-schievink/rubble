#![no_std]
#![no_main]
#![warn(rust_2018_idioms)]

// We need to import this crate explicitly so we have a panic handler
use panic_semihosting as _;

/// Configuration macro to be called by the user configuration in `config.rs`.
///
/// Expands to yet another `apply_config!` macro that's called from `init` and performs some
/// hardware initialization based on the config values.
macro_rules! config {
    (
        baudrate = $baudrate:ident;
        tx_pin = $tx_pin:ident;
        rx_pin = $rx_pin:ident;
    ) => {
        macro_rules! apply_config {
            ( $p0:ident, $device:ident ) => {{
                let rxd = $p0.$rx_pin.into_floating_input().degrade();
                let txd = $p0.$tx_pin.into_push_pull_output(Level::Low).degrade();

                let pins = hal::uarte::Pins {
                    rxd,
                    txd,
                    cts: None,
                    rts: None,
                };

                $device
                    .UARTE0
                    .constrain(pins, Parity::EXCLUDED, Baudrate::$baudrate)
            }};
        }
    };
}

#[macro_use]
mod config;
mod logger;

// Import the right HAL/PAC crate, depending on the target chip
#[cfg(feature = "52810")]
use nrf52810_hal::{self as hal, nrf52810_pac as pac};
#[cfg(feature = "52832")]
use nrf52832_hal::{self as hal, nrf52832_pac as pac};
#[cfg(feature = "52840")]
use nrf52840_hal::{self as hal, nrf52840_pac as pac};

use {
    bbqueue::Consumer,
    byteorder::{ByteOrder, LittleEndian},
    core::fmt::Write,
    cortex_m_semihosting::hprintln,
    hal::{
        gpio::Level,
        prelude::*,
        uarte::{Baudrate, Parity, Uarte},
    },
    pac::UARTE0,
    rtfm::app,
    rubble::{
        beacon::Beacon,
        config::Config,
        gatt::BatteryServiceAttrs,
        l2cap::{BleChannelMap, L2CAPState},
        link::{
            ad_structure::AdStructure,
            queue::{PacketQueue, SimpleConsumer, SimpleProducer, SimpleQueue},
            AddressKind, DeviceAddress, LinkLayer, Responder, MIN_PDU_BUF,
        },
        security::NoSecurity,
        time::{Duration, Timer},
    },
    rubble_nrf52::{
        radio::{BleRadio, PacketBuffer},
        timer::BleTimer,
    },
};

pub enum AppConfig {}

impl Config for AppConfig {
    type Timer = BleTimer<pac::TIMER0>;
    type Transmitter = BleRadio;
    type ChannelMapper = BleChannelMap<BatteryServiceAttrs, NoSecurity>;

    type PacketQueue = &'static mut SimpleQueue;
    type PacketProducer = SimpleProducer<'static>;
    type PacketConsumer = SimpleConsumer<'static>;
}

/// Whether to broadcast a beacon or to establish a proper connection.
///
/// This is just used to test different code paths. Note that you can't do both
/// at the same time unless you also generate separate device addresses.
const TEST_BEACON: bool = false;

#[app(device = crate::pac)]
const APP: () = {
    static mut BLE_TX_BUF: PacketBuffer = [0; MIN_PDU_BUF];
    static mut BLE_RX_BUF: PacketBuffer = [0; MIN_PDU_BUF];
    static mut BLE_LL: LinkLayer<AppConfig> = ();
    static mut TX_QUEUE: SimpleQueue = SimpleQueue::new();
    static mut RX_QUEUE: SimpleQueue = SimpleQueue::new();
    static mut BLE_R: Responder<AppConfig> = ();
    static mut RADIO: BleRadio = ();
    static mut BEACON: Beacon = ();
    static mut BEACON_TIMER: pac::TIMER1 = ();
    static mut SERIAL: Uarte<UARTE0> = ();
    static mut LOG_SINK: Consumer = ();

    #[init(resources = [BLE_TX_BUF, BLE_RX_BUF, TX_QUEUE, RX_QUEUE])]
    fn init() {
        hprintln!("\n<< INIT >>\n").ok();

        {
            // On reset the internal high frequency clock is used, but starting the HFCLK task
            // switches to the external crystal; this is needed for Bluetooth to work.

            device
                .CLOCK
                .tasks_hfclkstart
                .write(|w| unsafe { w.bits(1) });
            while device.CLOCK.events_hfclkstarted.read().bits() == 0 {}
        }

        let ble_timer = BleTimer::init(device.TIMER0);

        {
            // Configure TIMER1 as the beacon timer. It's only used as a 16-bit timer.
            let timer = &mut device.TIMER1;
            timer.bitmode.write(|w| w.bitmode()._16bit());
            // prescaler = 2^9    = 512
            // 16 MHz / prescaler = 31_250 Hz
            timer.prescaler.write(|w| unsafe { w.prescaler().bits(9) }); // 0-9
            timer.intenset.write(|w| w.compare0().set());
            timer.shorts.write(|w| w.compare0_clear().enabled());
            timer.cc[0].write(|w| unsafe { w.bits(31_250 / 3) }); // ~3x per second
            timer.tasks_clear.write(|w| unsafe { w.bits(1) });

            if TEST_BEACON {
                timer.tasks_start.write(|w| unsafe { w.bits(1) });
            }
        }

        let p0 = device.P0.split();

        let mut serial = apply_config!(p0, device);
        writeln!(serial, "\n--- INIT ---").unwrap();

        let mut devaddr = [0u8; 6];
        let devaddr_lo = device.FICR.deviceaddr[0].read().bits();
        let devaddr_hi = device.FICR.deviceaddr[1].read().bits() as u16;
        LittleEndian::write_u32(&mut devaddr, devaddr_lo);
        LittleEndian::write_u16(&mut devaddr[4..], devaddr_hi);

        let devaddr_type = if device
            .FICR
            .deviceaddrtype
            .read()
            .deviceaddrtype()
            .is_public()
        {
            AddressKind::Public
        } else {
            AddressKind::Random
        };
        let device_address = DeviceAddress::new(devaddr, devaddr_type);

        let mut radio = BleRadio::new(device.RADIO, resources.BLE_TX_BUF, resources.BLE_RX_BUF);

        let beacon = Beacon::new(
            device_address,
            &[AdStructure::CompleteLocalName("Rusty Beacon (nRF52)")],
        )
        .unwrap();

        let log_sink = logger::init(ble_timer.create_stamp_source());

        // Create TX/RX queues
        let (tx, tx_cons) = resources.TX_QUEUE.split();
        let (rx_prod, rx) = resources.RX_QUEUE.split();

        // Create the actual BLE stack objects
        let mut ll = LinkLayer::<AppConfig>::new(device_address, ble_timer);

        let resp = Responder::new(
            tx,
            rx,
            L2CAPState::new(BleChannelMap::with_attributes(BatteryServiceAttrs::new())),
        );

        if !TEST_BEACON {
            // Send advertisement and set up regular interrupt
            let next_update = ll
                .start_advertise(
                    Duration::from_millis(200),
                    &[AdStructure::CompleteLocalName("CONCVRRENS CERTA CELERIS")],
                    &mut radio,
                    tx_cons,
                    rx_prod,
                )
                .unwrap();
            ll.timer().configure_interrupt(next_update);
        }

        RADIO = radio;
        BLE_LL = ll;
        BLE_R = resp;
        BEACON = beacon;
        BEACON_TIMER = device.TIMER1;
        SERIAL = serial;
        LOG_SINK = log_sink;
    }

    #[interrupt(resources = [RADIO, BLE_LL], spawn = [ble_worker])]
    fn RADIO() {
        if let Some(cmd) = resources
            .RADIO
            .recv_interrupt(resources.BLE_LL.timer().now(), &mut resources.BLE_LL)
        {
            resources.RADIO.configure_receiver(cmd.radio);
            resources
                .BLE_LL
                .timer()
                .configure_interrupt(cmd.next_update);

            if cmd.queued_work {
                // If there's any lower-priority work to be done, ensure that happens.
                // If we fail to spawn the task, it's already scheduled.
                spawn.ble_worker().ok();
            }
        }
    }

    #[interrupt(resources = [RADIO, BLE_LL], spawn = [ble_worker])]
    fn TIMER0() {
        let timer = resources.BLE_LL.timer();
        if !timer.is_interrupt_pending() {
            return;
        }
        timer.clear_interrupt();

        let cmd = resources.BLE_LL.update_timer(&mut *resources.RADIO);
        resources.RADIO.configure_receiver(cmd.radio);

        resources
            .BLE_LL
            .timer()
            .configure_interrupt(cmd.next_update);

        if cmd.queued_work {
            // If there's any lower-priority work to be done, ensure that happens.
            // If we fail to spawn the task, it's already scheduled.
            spawn.ble_worker().ok();
        }
    }

    /// Fire the beacon.
    #[interrupt(resources = [BEACON_TIMER, BEACON, RADIO])]
    fn TIMER1() {
        // acknowledge event
        resources.BEACON_TIMER.events_compare[0].reset();

        resources.BEACON.broadcast(&mut *resources.RADIO);
    }

    #[idle(resources = [LOG_SINK, SERIAL])]
    fn idle() -> ! {
        // Drain the logging buffer through the serial connection
        loop {
            if cfg!(feature = "log") {
                while let Ok(grant) = resources.LOG_SINK.read() {
                    for chunk in grant.buf().chunks(255) {
                        resources.SERIAL.write(chunk).unwrap();
                    }

                    resources.LOG_SINK.release(grant.buf().len(), grant);
                }
            }
        }
    }

    #[task(resources = [BLE_R])]
    fn ble_worker() {
        // Fully drain the packet queue
        while resources.BLE_R.has_work() {
            resources.BLE_R.process_one().unwrap();
        }
    }

    extern "C" {
        fn WDT();
    }
};
