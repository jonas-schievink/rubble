use core::{
    fmt::{self, Write},
    ops::{Add, AddAssign},
};

/// A 1-bit data packet sequence number.
///
/// This type implements wrapping arithmetic (although only `+` and `+=` operators are supported)
/// and also provides an `increment` method for use by the connection management code in the link
/// layer.
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct SequenceNumber(bool);

impl SequenceNumber {
    /// A sequence number of 0 (default value).
    pub fn zero() -> Self {
        SequenceNumber(false)
    }

    /// A sequence number of 1.
    pub fn one() -> Self {
        SequenceNumber(true)
    }

    /// Increments this number, wrapping around to zero.
    #[must_use]
    #[allow(unused)] // FIXME implement connections and remove this
    pub fn increment(self) -> Self {
        self + Self::one()
    }
}

impl fmt::Display for SequenceNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char(if self.0 { '1' } else { '0' })
    }
}

impl fmt::Debug for SequenceNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Display>::fmt(self, f)
    }
}

impl Add for SequenceNumber {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        SequenceNumber(self.0 ^ rhs.0)
    }
}

impl Add<&'_ SequenceNumber> for SequenceNumber {
    type Output = Self;

    fn add(self, rhs: &'_ SequenceNumber) -> Self {
        self + *rhs
    }
}

impl AddAssign for SequenceNumber {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl AddAssign<&'_ SequenceNumber> for SequenceNumber {
    fn add_assign(&mut self, rhs: &'_ SequenceNumber) {
        *self = *self + *rhs;
    }
}
