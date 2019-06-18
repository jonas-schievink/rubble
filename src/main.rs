#![no_std]
#![no_main]

mod ble;

use core::fmt::{self, Write};
use core::panic::PanicInfo;

struct Logger;

impl Write for Logger {
    fn write_str(&mut self, _s: &str) -> fmt::Result {
        Ok(())
    }
}

#[export_name = "main"]
fn main() {
    let _: nrf52810_hal::nrf52810_pac::TIMER0;
    let _ = write!(Logger, "{:?}", None::<crate::ble::Duration>);
    crate::ble::link::process_data_packet();
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
