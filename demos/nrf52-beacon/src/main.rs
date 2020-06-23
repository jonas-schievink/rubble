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

use rubble::beacon::Beacon;
use rubble::link::{ad_structure::AdStructure, MIN_PDU_BUF};
use rubble_nrf5x::radio::{BleRadio, PacketBuffer};
use rubble_nrf5x::utils::get_device_address;

#[rtic::app(device = crate::hal::target, peripherals = true)]
const APP: () = {
    struct Resources {
        #[init([0; MIN_PDU_BUF])]
        ble_tx_buf: PacketBuffer,
        #[init([0; MIN_PDU_BUF])]
        ble_rx_buf: PacketBuffer,
        radio: BleRadio,
        beacon: Beacon,
        beacon_timer: hal::target::TIMER1,
    }

    #[init(resources = [ble_tx_buf, ble_rx_buf])]
    fn init(ctx: init::Context) -> init::LateResources {
        // On reset, the internal high frequency clock is already used, but we
        // also need to switch to the external HF oscillator. This is needed
        // for Bluetooth to work.
        let _clocks = hal::clocks::Clocks::new(ctx.device.CLOCK).enable_ext_hfosc();

        // Configure TIMER1 as the beacon timer. It's only used as a 16-bit timer.
        ctx.device.TIMER1.bitmode.write(|w| w.bitmode()._16bit());
        // prescaler = 2^9    = 512
        // 16 MHz / prescaler = 31_250 Hz
        ctx.device
            .TIMER1
            .prescaler
            .write(|w| unsafe { w.prescaler().bits(9) }); // 0-9
        ctx.device.TIMER1.intenset.write(|w| w.compare0().set());
        ctx.device
            .TIMER1
            .shorts
            .write(|w| w.compare0_clear().enabled());
        ctx.device.TIMER1.cc[0].write(|w| unsafe { w.bits(31_250 / 3) }); // ~3x per second
        ctx.device
            .TIMER1
            .tasks_clear
            .write(|w| unsafe { w.bits(1) });

        ctx.device
            .TIMER1
            .tasks_start
            .write(|w| unsafe { w.bits(1) });

        // Determine device address
        let device_address = get_device_address();

        // Rubble currently requires an RX buffer even though the radio is only used as a TX-only
        // beacon.
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

        init::LateResources {
            radio,
            beacon,
            beacon_timer: ctx.device.TIMER1,
        }
    }

    /// Fire the beacon.
    #[task(binds = TIMER1, resources = [beacon_timer, beacon, radio])]
    fn timer1(ctx: timer1::Context) {
        // Acknowledge event so that the interrupt doesn't keep firing
        ctx.resources.beacon_timer.events_compare[0].reset();

        ctx.resources.beacon.broadcast(ctx.resources.radio);
    }
};
