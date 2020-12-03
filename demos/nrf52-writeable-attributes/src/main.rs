#![no_std]
#![no_main]

// #[cfg(feature = "52810")]
// use nrf52810_hal as hal;
// #[cfg(feature = "52832")]
use nrf52832_hal as hal;
// #[cfg(feature = "52840")]
// use nrf52840_hal as hal;

use core::cmp;
use hal::gpio::{Level, Output, Pin, PushPull};
use hal::prelude::OutputPin;
use rtt_target::{rprintln, rtt_init_print};
use rubble::{
    att::{AttUuid, Attribute, AttributeAccessPermissions, AttributeProvider, Handle, HandleRange},
    config::Config,
    l2cap::{BleChannelMap, L2CAPState},
    link::{
        ad_structure::AdStructure,
        queue::{PacketQueue, SimpleQueue},
        LinkLayer, Responder, MIN_PDU_BUF,
    },
    security::NoSecurity,
    time::{Duration, Timer},
    uuid::{Uuid128, Uuid16},
    Error,
};
use rubble_nrf5x::{
    radio::{BleRadio, PacketBuffer},
    timer::BleTimer,
    utils::get_device_address,
};

pub struct LedBlinkAttrs {
    // State and resources to be modified/queried when packets are received
    led_pin: Pin<Output<PushPull>>,
    // Attributes exposed to clients
    attributes: [Attribute<'static>; 3],
}

const PRIMARY_SERVICE_UUID16: Uuid16 = Uuid16(0x2800);
const CHARACTERISTIC_UUID16: Uuid16 = Uuid16(0x2803);

// TODO what UUID should this be? I took this from a course assignment :P
// 32e61089-2b22-4db5-a914-43ce41986c70
const LED_UUID128: [u8; 16] = [
    0x70, 0x6C, 0x98, 0x41, 0xCE, 0x43, 0x14, 0xA9, 0xB5, 0x4D, 0x22, 0x2B, 0x89, 0x10, 0xE6, 0x32,
];
// Replace bytes 12/13 (0x1089) of the 128-bit UUID with 0x108A
const LED_STATE_CHAR_UUID128: [u8; 16] = [
    0x70, 0x6C, 0x98, 0x41, 0xCE, 0x43, 0x14, 0xA9, 0xB5, 0x4D, 0x22, 0x2B, 0x8A, 0x10, 0xE6, 0x32,
];

const LED_CHAR_DECL_VALUE: [u8; 19] = [
    0x02 | 0x08, // 0x02 = read, 0x08 = write with response
    // 2 byte handle pointing to characteristic value
    0x03,
    0x00,
    // 128-bit UUID of characteristic value (copied from above constant)
    0x70,
    0x6C,
    0x98,
    0x41,
    0xCE,
    0x43,
    0x14,
    0xA9,
    0xB5,
    0x4D,
    0x22,
    0x2B,
    0x8A,
    0x10,
    0xE6,
    0x32,
];

impl LedBlinkAttrs {
    fn new(led_pin: Pin<Output<PushPull>>, led_state: &'static mut [u8; 1]) -> Self {
        Self {
            led_pin,
            attributes: [
                Attribute::new(
                    PRIMARY_SERVICE_UUID16.into(),
                    Handle::from_raw(0x0001),
                    &LED_UUID128,
                ),
                Attribute::new(
                    CHARACTERISTIC_UUID16.into(),
                    Handle::from_raw(0x0002),
                    &LED_CHAR_DECL_VALUE,
                ),
                // Characteristic value
                Attribute::new(
                    Uuid128::from_bytes(LED_STATE_CHAR_UUID128).into(),
                    Handle::from_raw(0x0003),
                    led_state,
                ),
            ],
        }
    }
}

impl AttributeProvider for LedBlinkAttrs {
    /// Retrieves the permissions for attribute with the given handle.
    fn attr_access_permissions(&self, handle: Handle) -> AttributeAccessPermissions {
        match handle.as_u16() {
            0x0003 => AttributeAccessPermissions::ReadableAndWritable,
            _ => AttributeAccessPermissions::Readable,
        }
    }

    /// Attempts to write data to the attribute with the given handle.
    /// If any of your attributes are writeable, this function must be implemented.
    fn write_attr(&mut self, handle: Handle, data: &[u8]) -> Result<(), Error> {
        match handle.as_u16() {
            0x0003 => {
                self.attributes[2].value.0[0..1].copy_from_slice(&data[0..1]);
                // If we receive a 1, activate the LED; otherwise deactivate it
                // Assumes LED is active low
                if data[0] == 1 {
                    self.led_pin.set_low().unwrap();
                } else {
                    self.led_pin.set_high().unwrap();
                }
                Ok(())
            }
            _ => panic!("Attempted to write an unwriteable attribute"),
        }
    }

    fn is_grouping_attr(&self, uuid: AttUuid) -> bool {
        uuid == PRIMARY_SERVICE_UUID16 || uuid == CHARACTERISTIC_UUID16
    }

    fn group_end(&self, handle: Handle) -> core::option::Option<&Attribute<'_>> {
        match handle.as_u16() {
            // Handles for the primary service and characteristic
            0x0001 | 0x0002 => Some(&self.attributes[2]),
            _ => None,
        }
    }

    // Boilerplate to apply a function to all attributes with handles within the specified range
    // This was copied from the implementation of gatt:BatteryServiceAttrs
    fn for_attrs_in_range(
        &mut self,
        range: HandleRange,
        mut f: impl FnMut(&Self, Attribute<'_>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let count = self.attributes.len();
        let start = usize::from(range.start().as_u16() - 1); // handles start at 1, not 0
        let end = usize::from(range.end().as_u16() - 1);

        let attrs = if start >= count {
            &[]
        } else {
            let end = cmp::min(count - 1, end);
            &self.attributes[start..=end]
        };

        for attr in attrs {
            f(
                self,
                Attribute {
                    att_type: attr.att_type,
                    handle: attr.handle,
                    value: attr.value,
                },
            )?;
        }
        Ok(())
    }
}

pub enum AppConfig {}

impl Config for AppConfig {
    type Timer = BleTimer<hal::pac::TIMER0>;
    type Transmitter = BleRadio;
    type ChannelMapper = BleChannelMap<LedBlinkAttrs, NoSecurity>;
    type PacketQueue = &'static mut SimpleQueue;
}

#[rtic::app(device = crate::hal::pac, peripherals = true)]
const APP: () = {
    struct Resources {
        // Program state, backed by RTIC
        #[init([0])]
        led_state: [u8; 1],
        // BLE boilerplate
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
    }

    #[init(resources = [led_state, ble_tx_buf, ble_rx_buf, tx_queue, rx_queue])]
    fn init(ctx: init::Context) -> init::LateResources {
        rtt_init_print!();
        // On reset, the internal high frequency clock is already used, but we
        // also need to switch to the external HF oscillator. This is needed
        // for Bluetooth to work.
        let _clocks = hal::clocks::Clocks::new(ctx.device.CLOCK).enable_ext_hfosc();

        let ble_timer = BleTimer::init(ctx.device.TIMER0);

        let p0 = hal::gpio::p0::Parts::new(ctx.device.P0);

        // Determine device address
        let device_address = get_device_address();

        let mut radio = BleRadio::new(
            ctx.device.RADIO,
            &ctx.device.FICR,
            ctx.resources.ble_tx_buf,
            ctx.resources.ble_rx_buf,
        );

        // Create TX/RX queues
        let (tx, tx_cons) = ctx.resources.tx_queue.split();
        let (rx_prod, rx) = ctx.resources.rx_queue.split();

        // Create the actual BLE stack objects
        let mut ble_ll = LinkLayer::<AppConfig>::new(device_address, ble_timer);

        let ble_r = Responder::new(
            tx,
            rx,
            L2CAPState::new(BleChannelMap::with_attributes(LedBlinkAttrs::new(
                p0.p0_23.into_push_pull_output(Level::High).degrade(),
                ctx.resources.led_state,
            ))),
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
        }
    }

    #[task(binds = RADIO, resources = [radio, ble_ll], spawn = [ble_worker], priority = 3)]
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

    #[task(binds = TIMER0, resources = [radio, ble_ll], spawn = [ble_worker], priority = 3)]
    fn timer0(ctx: timer0::Context) {
        let timer = ctx.resources.ble_ll.timer();
        if !timer.is_interrupt_pending() {
            return;
        }
        timer.clear_interrupt();

        let cmd = ctx.resources.ble_ll.update_timer(ctx.resources.radio);
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

    #[idle]
    fn idle(ctx: idle::Context) -> ! {
        unimplemented!()
    }

    #[task(resources = [ble_r], priority = 2)]
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

#[panic_handler]
fn panic(e: &core::panic::PanicInfo) -> ! {
    rprintln!("Unhandled panic; stopping");
    rprintln!("{}", e);
    loop {
        cortex_m::asm::bkpt();
    }
}
