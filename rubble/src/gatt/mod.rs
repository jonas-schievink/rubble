//! Implementation of the Generic Attribute Profile (GATT).
//!
//! GATT describes a service framework that uses the Attribute Protocol for discovery and
//! interaction

pub mod characteristic;

use {
    crate::{
        att::{AttUuid, Attribute, AttributeProvider, Handle, HandleRange},
        utils::HexSlice,
        uuid::Uuid16,
        Error,
    },
    core::{cmp, slice},
};

/// A demo `AttributeProvider` that will enumerate as a *Battery Service*.
pub struct BatteryServiceAttrs {
    attributes: [Attribute<'static>; 3],
}

impl BatteryServiceAttrs {
    pub fn new() -> Self {
        Self {
            attributes: [
                Attribute {
                    att_type: Uuid16(0x2800).into(), // "Primary Service"
                    handle: Handle::from_raw(0x0001),
                    value: HexSlice(&[0x0F, 0x18]), // "Battery Service" = 0x180F
                },
                Attribute {
                    att_type: Uuid16(0x2803).into(), // "Characteristic"
                    handle: Handle::from_raw(0x0002),
                    value: HexSlice(&[
                        0x02, // 1 byte properties: READ = 0x02
                        0x03, 0x00, // 2 bytes handle = 0x0003
                        0x19, 0x2A, // 2 bytes UUID = 0x2A19 (Battery Level)
                    ]),
                },
                // Characteristic value (Battery Level)
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x2A19)), // "Battery Level"
                    handle: Handle::from_raw(0x0003),
                    value: HexSlice(&[48u8]),
                },
            ],
        }
    }
}

impl AttributeProvider for BatteryServiceAttrs {
    fn for_attrs_in_range(
        &mut self,
        range: HandleRange,
        mut f: impl FnMut(&Self, Attribute<'_>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let count = self.attributes.len();
        let start = usize::from(range.start().as_u16() - 1); // handles start at 1, not 0
        let end = usize::from(range.end().as_u16() - 1);

        let attrs = if start >= count {
            &[]
        } else {
            let end = cmp::min(count - 1, end);
            &self.attributes[start..=end]
        };

        for attr in attrs {
            f(
                self,
                Attribute {
                    att_type: attr.att_type,
                    handle: attr.handle,
                    value: attr.value,
                },
            )?;
        }
        Ok(())
    }

    fn is_grouping_attr(&self, uuid: AttUuid) -> bool {
        uuid == Uuid16(0x2800) // FIXME not characteristics?
    }

    fn group_end(&self, handle: Handle) -> Option<&Attribute<'_>> {
        match handle.as_u16() {
            0x0001 => Some(&self.attributes[2]),
            0x0002 => Some(&self.attributes[2]),
            _ => return None,
        }
    }
}

pub struct Attributes<'a> {
    to_yield: slice::Iter<'a, Attribute<'a>>,
}

impl<'a> Iterator for Attributes<'a> {
    type Item = Attribute<'a>;

    fn next(&mut self) -> Option<Attribute<'a>> {
        self.to_yield.next().map(|attr| Attribute {
            att_type: attr.att_type,
            handle: attr.handle,
            value: attr.value,
        })
    }
}
