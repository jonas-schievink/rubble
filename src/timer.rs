//! Generic `Timer` implementation that works with all 3 timers on the chip.

use {
    crate::ble::{
        link::NextUpdate,
        time::{Duration, Instant, Timer},
    },
    nrf52810_hal::nrf52810_pac::{TIMER0, TIMER1, TIMER2},
};

/// Implements rubble's `Timer` trait for a timer on the nRF chip.
pub struct BleTimer<T: NrfTimer> {
    inner: T,
    next: Instant,
    interrupt_enabled: bool,
}

impl<T: NrfTimer> BleTimer<T> {
    /// Initializes the timer.
    pub fn init(mut peripheral: T) -> Self {
        peripheral.init();
        Self {
            inner: peripheral,
            next: Instant::from_raw_micros(0),
            interrupt_enabled: false,
        }
    }

    pub fn configure_interrupt(&mut self, next: NextUpdate) {
        match next {
            NextUpdate::Keep => {
                // Don't call `set_interrupt` when the interrupt is already configured
                if !self.interrupt_enabled {
                    self.inner.set_interrupt(self.next);
                    self.interrupt_enabled = true;
                }
            }
            NextUpdate::Disable => {
                self.inner.clear_interrupt();
                self.interrupt_enabled = false;
            }
            NextUpdate::At(instant) => {
                self.inner.set_interrupt(instant);
                self.interrupt_enabled = true;
            }
            NextUpdate::In(duration) => {
                // FIXME: temporary conversion from core's duration to ours
                let micros = duration.as_micros();
                assert!(micros <= u32::max_value() as u128);
                let duration2 = Duration::from_micros(micros as u32);
                let instant = self.now() + duration2;
                self.inner.set_interrupt(instant);
                self.interrupt_enabled = true;
            }
        }
    }

    pub fn is_interrupt_pending(&self) -> bool {
        self.inner.is_pending()
    }

    pub fn clear_interrupt(&mut self) {
        self.inner.clear_interrupt();
    }

    /// Provides access to the raw peripheral. Use with caution.
    pub fn inner(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: NrfTimer> Timer for BleTimer<T> {
    fn now(&self) -> Instant {
        self.inner.now()
    }
}

/// Extension trait implemented for the nRF timer peripherals.
///
/// We use `CC[0]` to read the counter value, and `CC[1]` to set timer interrupts.
pub trait NrfTimer: Timer {
    /// Initialize the timer so that it counts at a rate of 1 MHz.
    fn init(&mut self);

    /// Configures the timer's interrupt to fire at the given `Instant`.
    fn set_interrupt(&mut self, at: Instant);

    /// Disables or acknowledges this timer's interrupt.
    fn clear_interrupt(&mut self);

    /// Returns whether a timer interrupt is currently pending.
    ///
    /// This must be called by the interrupt handler to avoid spurious timer events.
    fn is_pending(&self) -> bool;
}

macro_rules! impl_timer {
    ($ty:ident) => {
        impl NrfTimer for $ty {
            fn init(&mut self) {
                self.bitmode.write(|w| w.bitmode()._32bit());
                // 2^4 = 16
                // 16 MHz / 16 = 1 MHz = µs resolution
                self.prescaler.write(|w| unsafe { w.prescaler().bits(4) });
                self.tasks_clear.write(|w| w.tasks_clear().trigger());
                self.tasks_start.write(|w| w.tasks_start().trigger());
            }

            fn set_interrupt(&mut self, at: Instant) {
                // Not sure if an assertion is the right thing here, we might want to trigger the
                // interrupt immediately instead.
                at.duration_since(self.now());

                self.cc[1].write(|w| unsafe { w.bits(at.raw_micros()) });
                self.events_compare[1].reset();
                self.intenset.write(|w| w.compare1().set());
            }

            fn clear_interrupt(&mut self) {
                self.intenclr.write(|w| w.compare1().clear());
                self.events_compare[1].reset();
            }

            fn is_pending(&self) -> bool {
                self.events_compare[1]
                    .read()
                    .events_compare()
                    .is_generated()
            }
        }

        impl Timer for $ty {
            fn now(&self) -> Instant {
                self.tasks_capture[0].write(|w| w.tasks_capture().trigger());
                let micros = self.cc[0].read().bits();
                Instant::from_raw_micros(micros)
            }
        }
    };
}

impl_timer!(TIMER0);
impl_timer!(TIMER1);
impl_timer!(TIMER2);
