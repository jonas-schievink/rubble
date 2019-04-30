//! Generic `Timer` implementation that works with all 3 timers on the chip.

use {
    core::mem,
    nrf51_hal::nrf51::{TIMER0, TIMER1, TIMER2},
    rubble::{
        link::NextUpdate,
        time::{Instant, Timer},
    },
};

/// Implements Rubble's `Timer` trait for the timers on the nRF chip.
pub struct BleTimer<T: NrfTimerExt> {
    inner: T,
    next: Instant,
    interrupt_enabled: bool,
}

impl<T: NrfTimerExt> BleTimer<T> {
    /// Initializes the timer.
    pub fn init(mut peripheral: T) -> Self {
        peripheral.init();
        Self {
            inner: peripheral,
            next: Instant::from_raw_micros(0),
            interrupt_enabled: false,
        }
    }

    /// Configures the timer interrupt to fire according to `next`.
    pub fn configure_interrupt(&mut self, next: NextUpdate) {
        match next {
            NextUpdate::Keep => {
                // Don't call `set_interrupt` when the interrupt is already configured, since that
                // might result in races (it resets the event)
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
        }
    }

    /// Checks whether this timer's interrupt is pending.
    ///
    /// This will return `true` when interrupt handler should execute. To prevent spurious wakeups,
    /// the handler *must* check that this is `true` when it gets executed. The handler should
    /// acknowledge the interrupt by calling [`clear_interrupt`], otherwise the handler will be run
    /// immediately after returning.
    ///
    /// [`clear_interrupt`]: #method.clear_interrupt
    pub fn is_interrupt_pending(&self) -> bool {
        self.inner.is_pending()
    }

    /// Clears a pending interrupt and disables generation of further interrupts.
    pub fn clear_interrupt(&mut self) {
        self.inner.clear_interrupt();
    }

    /// Provides access to the raw peripheral. Use with caution.
    pub fn inner(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Creates a new `StampSource` using this timer.
    ///
    /// The `StampSource` can be used to obtain the current time, but can not do anything else. This
    /// restriction makes it safe to use even when the `BleTimer` it was created from is modified.
    pub fn create_stamp_source(&self) -> StampSource<T> {
        StampSource {
            inner: unsafe { self.inner.duplicate() },
        }
    }
}

impl<T: NrfTimerExt> Timer for BleTimer<T> {
    fn now(&self) -> Instant {
        self.inner.now()
    }
}

/// A timer interface that only allows reading the current time stamp.
pub struct StampSource<T: NrfTimerExt> {
    inner: T,
}

impl<T: NrfTimerExt> Timer for StampSource<T> {
    fn now(&self) -> Instant {
        self.inner.now()
    }
}

mod sealed {
    pub trait Sealed {}
}

/// Extension trait implemented for the nRF timer peripherals.
///
/// We use `CC[0]` to read the counter value, and `CC[1]` to set timer interrupts.
pub trait NrfTimerExt: sealed::Sealed {
    unsafe fn duplicate(&self) -> Self;

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

    /// Obtains the current time as an `Instant`.
    fn now(&self) -> Instant;
}

macro_rules! impl_timer {
    ($ty:ident) => {
        impl NrfTimerExt for $ty {
            unsafe fn duplicate(&self) -> Self {
                mem::transmute_copy(self)
            }

            fn init(&mut self) {
                self.bitmode.write(|w| w.bitmode()._32bit());
                // 2^4 = 16
                // 16 MHz / 16 = 1 MHz = µs resolution
                self.prescaler.write(|w| unsafe { w.prescaler().bits(4) });
                self.tasks_clear.write(|w| unsafe { w.bits(1) });
                self.tasks_start.write(|w| unsafe { w.bits(1) });
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
                self.events_compare[1].read().bits() == 1u32
            }

            fn now(&self) -> Instant {
                self.tasks_capture[0].write(|w| unsafe { w.bits(1) });
                let micros = self.cc[0].read().bits();
                Instant::from_raw_micros(micros)
            }
        }

        impl sealed::Sealed for $ty {}
    };
}

impl_timer!(TIMER0);
impl_timer!(TIMER1);
impl_timer!(TIMER2);
