#![no_std]
#![no_main]
#![warn(rust_2018_idioms)]

// We need to import this crate explicitly so we have a panic handler
use panic_halt as _;

// Import the right HAL and PAC
#[cfg(feature = "52810")]
use nrf52810_hal::nrf52810_pac as pac;

#[cfg(feature = "52832")]
use nrf52832_hal::nrf52832_pac as pac;

#[cfg(feature = "52840")]
use nrf52840_hal::nrf52840_pac as pac;

use {
    byteorder::{ByteOrder, LittleEndian},
    rtfm::app,
    rubble::{
        beacon::Beacon,
        link::{ad_structure::AdStructure, AddressKind, DeviceAddress, MIN_PDU_BUF},
    },
    rubble_nrf52::radio::{BleRadio, PacketBuffer},
};

#[app(device = crate::pac)]
const APP: () = {
    static mut BLE_TX_BUF: PacketBuffer = [0; MIN_PDU_BUF];
    static mut BLE_RX_BUF: PacketBuffer = [0; MIN_PDU_BUF];
    static mut RADIO: BleRadio = ();
    static mut BEACON: Beacon = ();
    static mut BEACON_TIMER: pac::TIMER1 = ();

    #[init(resources = [BLE_TX_BUF, BLE_RX_BUF])]
    fn init() {
        {
            // On reset the internal high frequency clock is used, but starting the HFCLK task
            // switches to the external crystal; this is needed for Bluetooth to work.

            device
                .CLOCK
                .tasks_hfclkstart
                .write(|w| unsafe { w.bits(1) });
            while device.CLOCK.events_hfclkstarted.read().bits() == 0 {}
        }

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

            timer.tasks_start.write(|w| unsafe { w.bits(1) });
        }

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

        // Rubble currently requires an RX buffer even though the radio is only used as a TX-only
        // beacon.
        let radio = BleRadio::new(device.RADIO, resources.BLE_TX_BUF, resources.BLE_RX_BUF);

        let beacon = Beacon::new(
            device_address,
            &[AdStructure::CompleteLocalName("Rusty Beacon (nRF52)")],
        )
        .unwrap();

        RADIO = radio;
        BEACON = beacon;
        BEACON_TIMER = device.TIMER1;
    }

    /// Fire the beacon.
    #[interrupt(resources = [BEACON_TIMER, BEACON, RADIO])]
    fn TIMER1() {
        // Acknowledge event so that the interrupt doesn't keep firing
        resources.BEACON_TIMER.events_compare[0].reset();

        resources.BEACON.broadcast(&mut *resources.RADIO);
    }
};
