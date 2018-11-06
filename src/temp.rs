use nrf51::TEMP;
use fpa::I30F2;
use nb;

/// Integrated temperature sensor.
pub struct Temp(TEMP);

impl Temp {
    /// Creates a new `Temp`, taking ownership of the temperature sensor's register block.
    pub fn new(raw: TEMP) -> Self {
        Temp(raw)
    }

    /// Kicks off a temperature measurement.
    ///
    /// The measurement can be retrieved by calling `read`.
    pub fn start_measurement(&mut self) {
        unsafe {
            self.0.tasks_start.write(|w| w.bits(1));
        }
    }

    /// Tries to read the last measurement.
    ///
    /// Before calling this, `start_measurement` must be called.
    ///
    /// Returns the measured temperature in °C.
    pub fn read(&mut self) -> nb::Result<I30F2, ()> {
        if self.0.events_datardy.read().bits() == 0 {
            return Err(nb::Error::WouldBlock);
        } else {
            self.0.events_datardy.reset();     // clear event
            let raw = self.0.temp.read().bits();
            Ok(I30F2::from_bits(raw as i32))
        }
    }
}
