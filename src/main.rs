#![no_std]
#![no_main]

// We need to import this crate explicitly so we have a panic handler
extern crate panic_semihosting;

pub mod ble;
mod radio;

use {
    crate::{
        ble::{
            beacon::Beacon,
            link::{
                ad_structure::AdStructure, AddressKind, DeviceAddress, LinkLayer, MAX_PDU_SIZE,
            },
        },
        radio::{Baseband, BleRadio, PacketBuffer},
    },
    byteorder::{ByteOrder, LittleEndian},
    core::{fmt::Write, time::Duration, u32},
    nrf51::UART0,
    nrf51_hal::{
        prelude::*,
        serial::{self, Serial, BAUDRATEW},
    },
    rtfm::app,
};

type Logger = serial::Tx<UART0>;

/// Whether to broadcast a beacon or to establish a proper connection.
///
/// This is just used to test different code paths. Note that you can't do both
/// at the same time unless you also generate separate device addresses.
const TEST_BEACON: bool = true;

#[app(device = nrf51)]
const APP: () = {
    static mut BLE_TX_BUF: PacketBuffer = [0; MAX_PDU_SIZE + 1];
    static mut BLE_RX_BUF: PacketBuffer = [0; MAX_PDU_SIZE + 1];
    static mut BASEBAND: Baseband<Logger> = ();
    static mut BEACON: Beacon = ();
    static mut BEACON_TIMER: nrf51::TIMER1 = ();
    static BLE_TIMER: nrf51::TIMER0 = ();

    #[init(resources = [BLE_TX_BUF, BLE_RX_BUF])]
    fn init() {
        {
            // On reset, internal 16MHz RC oscillator is active. Switch to ext. 16MHz crystal.
            // This is needed for Bluetooth to work (but is apparently done automatically on radio
            // activation, too?).

            // Ext. clock freq. defaults to 32 MHz for some reason
            device.CLOCK.xtalfreq.write(|w| w.xtalfreq()._16mhz());
            device
                .CLOCK
                .tasks_hfclkstart
                .write(|w| unsafe { w.bits(1) });
            while device.CLOCK.events_hfclkstarted.read().bits() == 0 {}
        }

        {
            // TIMER0 cfg, 32 bit @ 1 MHz
            // Mostly copied from the `nrf51-hal` crate.
            device.TIMER0.bitmode.write(|w| w.bitmode()._32bit());
            device
                .TIMER0
                .prescaler
                .write(|w| unsafe { w.prescaler().bits(4) }); // 2^4 = µs resolution
            device.TIMER0.intenset.write(|w| w.compare0().set());
            device
                .TIMER0
                .shorts
                .write(|w| w.compare0_clear().enabled().compare0_stop().enabled());
        }

        {
            // Configure TIMER1 as the beacon timer. It's only a 16-bit timer.
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

        let mut serial = {
            let pins = device.GPIO.split();
            let rx = pins.pin1.downgrade();
            let tx = pins.pin2.into_push_pull_output().downgrade();
            Serial::uart0(device.UART0, tx, rx, BAUDRATEW::BAUD921600)
                .split()
                .0
        };
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

        let mut ll = LinkLayer::with_logger(device_address, serial);
        ll.start_advertise(
            Duration::from_millis(100),
            &[AdStructure::CompleteLocalName("CONCVRRENS CERTA CELERIS")],
        )
        .unwrap();

        let baseband = Baseband::new(
            BleRadio::new(device.RADIO, &device.FICR, resources.BLE_TX_BUF),
            resources.BLE_RX_BUF,
            ll,
        );

        let beacon = Beacon::new(
            device_address,
            &[AdStructure::CompleteLocalName("Rusty Beacon")],
        )
        .unwrap();

        if !TEST_BEACON {
            // Queue first baseband update
            cfg_timer(&device.TIMER0, Some(Duration::from_millis(1)));
        }

        BEACON = beacon;
        BEACON_TIMER = device.TIMER1;
        BASEBAND = baseband;
        BLE_TIMER = device.TIMER0;
    }

    #[interrupt(resources = [BLE_TIMER, BASEBAND])]
    fn RADIO() {
        if let Some(new_timeout) = resources.BASEBAND.interrupt() {
            cfg_timer(&resources.BLE_TIMER, Some(new_timeout));
        }
    }

    #[interrupt(resources = [BLE_TIMER, BASEBAND])]
    fn TIMER0() {
        let maybe_next_update = resources.BASEBAND.update();
        cfg_timer(&resources.BLE_TIMER, maybe_next_update);
    }

    /// Fire the beacon.
    #[interrupt(resources = [BEACON_TIMER, BEACON, BASEBAND])]
    fn TIMER1() {
        // acknowledge event
        resources.BEACON_TIMER.events_compare[0].reset();

        let log = resources.BASEBAND.logger();
        writeln!(log, "-> beacon").unwrap();
        resources.BEACON.broadcast(resources.BASEBAND.transmitter());
    }
};

/// Reconfigures TIMER0 to raise an interrupt after `duration` has elapsed.
///
/// TIMER0 is stopped if `duration` is `None`.
///
/// Note that if the timer has already queued an interrupt, the task will still be run after the
/// timer is stopped by this function.
fn cfg_timer(t: &nrf51::TIMER0, duration: Option<Duration>) {
    // Timer activation code is also copied from the `nrf51-hal` crate.
    if let Some(duration) = duration {
        assert!(duration.as_secs() < ((u32::MAX - duration.subsec_micros()) / 1_000_000) as u64);
        let us = (duration.as_secs() as u32) * 1_000_000 + duration.subsec_micros();
        t.cc[0].write(|w| unsafe { w.bits(us) });
        // acknowledge last compare event (FIXME unnecessary?)
        t.events_compare[0].reset();
        t.tasks_clear.write(|w| unsafe { w.bits(1) });
        t.tasks_start.write(|w| unsafe { w.bits(1) });
    } else {
        t.tasks_stop.write(|w| unsafe { w.bits(1) });
        t.tasks_clear.write(|w| unsafe { w.bits(1) });
        // acknowledge last compare event (FIXME unnecessary?)
        t.events_compare[0].reset();
    }
}
