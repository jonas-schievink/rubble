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
pub struct SeqNum(bool);

impl SeqNum {
    /// A sequence number of 0 (default value).
    pub const ZERO: Self = SeqNum(false);

    /// A sequence number of 1.
    pub const ONE: Self = SeqNum(true);
}

impl fmt::Display for SeqNum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char(if self.0 { '1' } else { '0' })
    }
}

impl fmt::Debug for SeqNum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as fmt::Display>::fmt(self, f)
    }
}

impl Add for SeqNum {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        SeqNum(self.0 ^ rhs.0)
    }
}

impl Add<&'_ SeqNum> for SeqNum {
    type Output = Self;

    fn add(self, rhs: &'_ SeqNum) -> Self {
        self + *rhs
    }
}

impl AddAssign for SeqNum {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl AddAssign<&'_ SeqNum> for SeqNum {
    fn add_assign(&mut self, rhs: &'_ SeqNum) {
        *self = *self + *rhs;
    }
}
