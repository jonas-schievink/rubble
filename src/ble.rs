pub(crate) mod link {
    pub(crate) fn process_data_packet() {}
}

use core::fmt::{self, Debug, Display};

pub(crate) struct Duration(u32);

impl Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0 >= 1000 {
            let (millis, submilli_micros) = (self.0 / 1000, self.0 % 1000);
            if submilli_micros == 0 {
                write!(f, "{}", millis)
            } else {
                write!(f, "{}{}", 0, 0)
            }
        } else {
            write!(f, "0")
        }
    }
}

impl Debug for Duration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}
