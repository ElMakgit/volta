//! Resource ledger used before a circuit is drawn.
//!
//! Two displays on A4/A5 are fine, two relays on D7 are not. The ledger hands
//! out pins, I2C addresses and current from one place so a plan cannot claim
//! the same resource twice.

use std::collections::BTreeSet;

use crate::model::{Circuit, End, PinKind};

#[derive(Debug)]
pub struct Resources {
    used_pins: BTreeSet<String>,
    i2c: Vec<u8>,
    current_ma: u16,
}

impl Resources {
    /// The Uno can comfortably supply 400 mA from its 5 V rail.
    pub const BUDGET_MA: u16 = 400;

    pub fn new(c: &Circuit) -> Self {
        let Some(uno) = c.arduino else {
            return Self { used_pins: BTreeSet::new(), i2c: Vec::new(), current_ma: 0 };
        };
        let used_pins = c
            .wires()
            .flat_map(|w| [w.a, w.b])
            .filter_map(|e| match e {
                End::Pin(p) if c.pins[p].part == uno => Some(c.pins[p].name.to_string()),
                _ => None,
            })
            .collect();
        Self { used_pins, i2c: Vec::new(), current_ma: 0 }
    }

    /// Reserve the first free pin from the preferred list.
    pub fn claim_pin(&mut self, candidates: &[&str]) -> Option<String> {
        for name in candidates {
            if !self.used_pins.contains(*name) {
                self.used_pins.insert((*name).to_string());
                return Some((*name).to_string());
            }
        }
        None
    }

    /// Reserve an I2C address; 0x3C and 0x3D for OLEDs, 0x27 and 0x26 for LCDs.
    pub fn claim_i2c(&mut self, preferred: &[u8]) -> Option<u8> {
        for addr in preferred {
            if !self.i2c.contains(addr) {
                self.i2c.push(*addr);
                return Some(*addr);
            }
        }
        None
    }

    /// True when the rail still has room for another device.
    pub fn claim_current(&mut self, ma: u16) -> bool {
        if self.current_ma.saturating_add(ma) > Self::BUDGET_MA {
            return false;
        }
        self.current_ma = self.current_ma.saturating_add(ma);
        true
    }

    pub fn current_ma(&self) -> u16 {
        self.current_ma
    }

    /// A pin is usable when it actually exists and is not a supply rail.
    pub fn usable(c: &Circuit, uno: usize, name: &str) -> bool {
        c.parts
            .get(uno)
            .and_then(|p| p.pins.iter().find(|&&i| c.pins[i].name == name))
            .is_some_and(|&i| {
                c.pins[i].active && !matches!(c.pins[i].kind, PinKind::Power | PinKind::Ground)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, PinSpec, Hole, Pt};

    #[test]
    fn resources_do_not_hand_out_the_same_pin_twice() {
        let mut c = Circuit::new();
        let uno = c.add_arduino(Pt::new(60.0, 20.0));
        let mut r = Resources::new(&c);
        assert_eq!(r.claim_pin(&["D8", "D7"]).as_deref(), Some("D8"));
        assert_eq!(r.claim_pin(&["D8", "D7"]).as_deref(), Some("D7"));
        assert!(r.claim_pin(&["D8", "D7"]).is_none());
        assert!(Resources::usable(&c, uno, "A4"));
        assert!(!Resources::usable(&c, uno, "5V"));

        assert_eq!(r.claim_i2c(&[0x3C, 0x3D]).unwrap(), 0x3C);
        assert_eq!(r.claim_i2c(&[0x3C, 0x3D]).unwrap(), 0x3D);
        assert!(r.claim_current(100));
        assert!(!r.claim_current(400));
        let _ = (Kind::Led, Hole::OffBoard, PinSpec { name: "x", pos: Pt::new(0.0, 0.0), hole: Hole::OffBoard, kind: PinKind::Passive });
    }
}
