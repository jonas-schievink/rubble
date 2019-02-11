#![no_std]
#![no_main]

#[macro_use]
extern crate nb;
extern crate byteorder;
extern crate cortex_m;
extern crate cortex_m_rt;
extern crate cortex_m_semihosting;
extern crate embedded_hal;
extern crate fpa;
extern crate nrf51;
extern crate nrf51_hal;
extern crate panic_halt;
extern crate rtfm;
#[macro_use]
extern crate bitflags;

pub mod ble;
mod radio;
mod temp;

use ble::link::ad_structure::{AdStructure, Flags};
pub use ble::link::MAX_PDU_SIZE;
use ble::link::{AddressKind, DeviceAddress, LinkLayer};

use radio::{Baseband, BleRadio};
use temp::Temp;

use byteorder::{ByteOrder, LittleEndian};
use nrf51::UART0;
use nrf51_hal::prelude::*;
use nrf51_hal::serial::{self, Serial, BAUDRATEW};
use rtfm::app;

use core::fmt::Write;
use core::time::Duration;
use core::u32;

type Logger = serial::Tx<UART0>;

#[app(device = nrf51)]
const APP: () = {
    static mut BLE_TX_BUF: ::radio::PacketBuffer = [0; ::MAX_PDU_SIZE + 1];
    static mut BLE_RX_BUF: ::radio::PacketBuffer = [0; ::MAX_PDU_SIZE + 1];
    static mut BASEBAND: Baseband<Logger> = ();
    static BLE_TIMER: nrf51::TIMER0 = ();

    #[init(resources = [BLE_TX_BUF, BLE_RX_BUF])]
    fn init() {
        // On reset, internal 16MHz RC oscillator is active. Switch to ext. 16MHz crystal. This is
        // needed for Bluetooth to work (but is apparently done on radio activation, too?).

        // Ext. clock freq. defaults to 32 MHz for some reason
        device.CLOCK.xtalfreq.write(|w| w.xtalfreq()._16mhz());
        device
            .CLOCK
            .tasks_hfclkstart
            .write(|w| unsafe { w.bits(1) });
        while device.CLOCK.events_hfclkstarted.read().bits() == 0 {}

        // TIMER0 cfg, 32 bit @ 1 MHz
        // Mostly copied from the `nrf51-hal` crate.
        device.TIMER0.bitmode.write(|w| w.bitmode()._32bit());
        device
            .TIMER0
            .prescaler
            .write(|w| unsafe { w.prescaler().bits(4) });
        device.TIMER0.intenset.write(|w| w.compare0().set());
        device
            .TIMER0
            .shorts
            .write(|w| w.compare0_clear().enabled().compare0_stop().enabled());

        let mut temp = Temp::new(device.TEMP);
        temp.start_measurement();
        let temp = block!(temp.read()).unwrap();

        let mut serial = {
            let pins = device.GPIO.split();
            let rx = pins.pin1.downgrade();
            let tx = pins.pin2.into_push_pull_output().downgrade();
            Serial::uart0(device.UART0, tx, rx, BAUDRATEW::BAUD921600)
                .split()
                .0
        };
        writeln!(serial, "\n--- INIT ({}°C) ---", temp).unwrap();

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
            &[
                AdStructure::Flags(Flags::discoverable()),
                AdStructure::CompleteLocalName("CONCVRRENS CERTA CELERIS"),
            ],
        );

        // Queue first baseband update
        cfg_timer(&device.TIMER0, Some(Duration::from_millis(1)));

        BASEBAND = Baseband::new(
            BleRadio::new(device.RADIO, &device.FICR, resources.BLE_TX_BUF),
            resources.BLE_RX_BUF,
            ll,
        );
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
        t.events_compare[0].reset(); // acknowledge last compare event (FIXME unnecessary?)
        t.tasks_clear.write(|w| unsafe { w.bits(1) });
        t.tasks_start.write(|w| unsafe { w.bits(1) });
    } else {
        t.tasks_stop.write(|w| unsafe { w.bits(1) });
        t.tasks_clear.write(|w| unsafe { w.bits(1) });
        t.events_compare[0].reset(); // acknowledge last compare event (FIXME unnecessary?)
    }
}
