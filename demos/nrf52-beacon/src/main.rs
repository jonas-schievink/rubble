#![no_std]
#![no_main]
#![warn(rust_2018_idioms)]

// We need to import this crate explicitly so we have a panic handler
use panic_halt as _;

// Import the right HAL and PAC
#[cfg(feature = "52810")]
use nrf52810_hal as hal;

#[cfg(feature = "52832")]
use nrf52832_hal as hal;

#[cfg(feature = "52840")]
use nrf52840_hal as hal;

use rtic::cyccnt::U32Ext;
use rubble::beacon::Beacon;
use rubble::link::{ad_structure::AdStructure, MIN_PDU_BUF};
use rubble_nrf5x::radio::{BleRadio, PacketBuffer};
use rubble_nrf5x::utils::get_device_address;

#[rtic::app(device = crate::hal::pac, peripherals = true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
        #[init([0; MIN_PDU_BUF])]
        ble_tx_buf: PacketBuffer,
        #[init([0; MIN_PDU_BUF])]
        ble_rx_buf: PacketBuffer,
        radio: BleRadio,
        beacon: Beacon,
    }

    #[init(resources = [ble_tx_buf, ble_rx_buf], spawn = [update])]
    fn init(ctx: init::Context) -> init::LateResources {
        // On reset, the internal high frequency clock is already used, but we
        // also need to switch to the external HF oscillator. This is needed
        // for Bluetooth to work.
        let _clocks = hal::clocks::Clocks::new(ctx.device.CLOCK).enable_ext_hfosc();

        // Initialize (enable) the monotonic timer (CYCCNT)
        let mut core = ctx.core;
        core.DCB.enable_trace();
        core.DWT.enable_cycle_counter();

        // Determine device address
        let device_address = get_device_address();

        // Rubble currently requires an RX buffer even though the radio is only used as a TX-only beacon.
        let radio = BleRadio::new(
            ctx.device.RADIO,
            &ctx.device.FICR,
            ctx.resources.ble_tx_buf,
            ctx.resources.ble_rx_buf,
        );

        let beacon = Beacon::new(
            device_address,
            &[AdStructure::CompleteLocalName("Rusty Beacon (nRF52)")],
        )
        .unwrap();

        ctx.spawn.update().ok();

        init::LateResources { radio, beacon }
    }

    /// Fire the beacon.
    #[task(schedule = [update], resources = [beacon, radio])]
    fn update(ctx: update::Context) {
        ctx.resources.beacon.broadcast(ctx.resources.radio);

        ctx.schedule
            .update(ctx.scheduled + 10_666_666.cycles())
            .ok(); // about 3 times per second as the nrf52 runs with 32 MHz
    }

    // Here we list unused interrupt vectors that can be used to dispatch software tasks
    //
    // One needs one free interrupt per priority level used in software tasks.
    extern "C" {
        fn TIMER1();
    }
};
