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
            ( $p0:ident, $uart:ident ) => {{
                let rxd = $p0.$rx_pin.into_floating_input().degrade();
                let txd = $p0.$tx_pin.into_push_pull_output(Level::Low).degrade();

                let pins = hal::uarte::Pins {
                    rxd,
                    txd,
                    cts: None,
                    rts: None,
                };

                $uart.constrain(pins, Parity::EXCLUDED, Baudrate::$baudrate)
            }};
        }
    };
}

#[macro_use]
mod config;
mod logger;

// Import the right HAL/PAC crate, depending on the target chip
#[cfg(feature = "52810")]
use nrf52810_hal as hal;
#[cfg(feature = "52832")]
use nrf52832_hal as hal;
#[cfg(feature = "52840")]
use nrf52840_hal as hal;

use {
    bbqueue::Consumer,
    byteorder::{ByteOrder, LittleEndian},
    core::fmt::Write,
    hal::{
        gpio::Level,
        prelude::*,
        target::UARTE0,
        uarte::{Baudrate, Parity, Uarte},
    },
    rubble::{
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
    rubble_nrf5x::{
        radio::{BleRadio, PacketBuffer},
        timer::BleTimer,
    },
};

pub enum AppConfig {}

impl Config for AppConfig {
    type Timer = BleTimer<hal::target::TIMER0>;
    type Transmitter = BleRadio;
    type ChannelMapper = BleChannelMap<BatteryServiceAttrs, NoSecurity>;

    type PacketQueue = &'static mut SimpleQueue;
    type PacketProducer = SimpleProducer<'static>;
    type PacketConsumer = SimpleConsumer<'static>;
}

#[rtfm::app(device = crate::hal::target, peripherals = true)]
const APP: () = {
    struct Resources {
        #[init([0; MIN_PDU_BUF])]
        ble_tx_buf: PacketBuffer,
        #[init([0; MIN_PDU_BUF])]
        ble_rx_buf: PacketBuffer,
        #[init(SimpleQueue::new())]
        tx_queue: SimpleQueue,
        #[init(SimpleQueue::new())]
        rx_queue: SimpleQueue,
        ble_ll: LinkLayer<AppConfig>,
        ble_r: Responder<AppConfig>,
        radio: BleRadio,
        serial: Uarte<UARTE0>,
        log_sink: Consumer,
    }

    #[init(resources = [ble_tx_buf, ble_rx_buf, tx_queue, rx_queue])]
    fn init(ctx: init::Context) -> init::LateResources {
        {
            // On reset the internal high frequency clock is used, but starting the HFCLK task
            // switches to the external crystal; this is needed for Bluetooth to work.
            ctx.device
                .CLOCK
                .tasks_hfclkstart
                .write(|w| unsafe { w.bits(1) });
            while ctx.device.CLOCK.events_hfclkstarted.read().bits() == 0 {}
        }

        let ble_timer = BleTimer::init(ctx.device.TIMER0);

        let p0 = ctx.device.P0.split();

        let uart = ctx.device.UARTE0;
        let mut serial = apply_config!(p0, uart);
        writeln!(serial, "\n--- INIT ---").unwrap();

        let mut devaddr = [0u8; 6];
        let devaddr_lo = ctx.device.FICR.deviceaddr[0].read().bits();
        let devaddr_hi = ctx.device.FICR.deviceaddr[1].read().bits() as u16;
        LittleEndian::write_u32(&mut devaddr, devaddr_lo);
        LittleEndian::write_u16(&mut devaddr[4..], devaddr_hi);

        let devaddr_type = if ctx
            .device
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
        let mut radio = BleRadio::new(
            ctx.device.RADIO,
            &ctx.device.FICR,
            ctx.resources.ble_tx_buf,
            ctx.resources.ble_rx_buf,
        );

        let log_sink = logger::init(ble_timer.create_stamp_source());

        // Create TX/RX queues
        let (tx, tx_cons) = ctx.resources.tx_queue.split();
        let (rx_prod, rx) = ctx.resources.rx_queue.split();

        // Create the actual BLE stack objects
        let mut ble_ll = LinkLayer::<AppConfig>::new(device_address, ble_timer);

        let ble_r = Responder::new(
            tx,
            rx,
            L2CAPState::new(BleChannelMap::with_attributes(BatteryServiceAttrs::new())),
        );

        // Send advertisement and set up regular interrupt
        let next_update = ble_ll
            .start_advertise(
                Duration::from_millis(200),
                &[AdStructure::CompleteLocalName("CONCVRRENS CERTA CELERIS")],
                &mut radio,
                tx_cons,
                rx_prod,
            )
            .unwrap();

        ble_ll.timer().configure_interrupt(next_update);

        init::LateResources {
            radio,
            ble_ll,
            ble_r,
            serial,
            log_sink,
        }
    }

    #[task(binds = RADIO, resources = [radio, ble_ll], spawn = [ble_worker])]
    fn radio(ctx: radio::Context) {
        let ble_ll: &mut LinkLayer<AppConfig> = ctx.resources.ble_ll;
        if let Some(cmd) = ctx
            .resources
            .radio
            .recv_interrupt(ble_ll.timer().now(), ble_ll)
        {
            ctx.resources.radio.configure_receiver(cmd.radio);
            ble_ll.timer().configure_interrupt(cmd.next_update);

            if cmd.queued_work {
                // If there's any lower-priority work to be done, ensure that happens.
                // If we fail to spawn the task, it's already scheduled.
                ctx.spawn.ble_worker().ok();
            }
        }
    }

    #[task(binds = TIMER0, resources = [radio, ble_ll], spawn = [ble_worker])]
    fn timer0(ctx: timer0::Context) {
        let timer = ctx.resources.ble_ll.timer();
        if !timer.is_interrupt_pending() {
            return;
        }
        timer.clear_interrupt();

        let cmd = ctx.resources.ble_ll.update_timer(&mut *ctx.resources.radio);
        ctx.resources.radio.configure_receiver(cmd.radio);

        ctx.resources
            .ble_ll
            .timer()
            .configure_interrupt(cmd.next_update);

        if cmd.queued_work {
            // If there's any lower-priority work to be done, ensure that happens.
            // If we fail to spawn the task, it's already scheduled.
            ctx.spawn.ble_worker().ok();
        }
    }

    #[idle(resources = [log_sink, serial])]
    fn idle(ctx: idle::Context) -> ! {
        // Drain the logging buffer through the serial connection
        loop {
            if cfg!(feature = "log") {
                while let Ok(grant) = ctx.resources.log_sink.read() {
                    for chunk in grant.buf().chunks(255) {
                        ctx.resources.serial.write(chunk).unwrap();
                    }

                    ctx.resources.log_sink.release(grant.buf().len(), grant);
                }
            }
        }
    }

    #[task(resources = [ble_r])]
    fn ble_worker(ctx: ble_worker::Context) {
        // Fully drain the packet queue
        while ctx.resources.ble_r.has_work() {
            ctx.resources.ble_r.process_one().unwrap();
        }
    }

    extern "C" {
        fn WDT();
    }
};
