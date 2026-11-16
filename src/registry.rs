//! A capability registry built from the module table.
//!
//! Every ready-made board declares its pins and current in `modules.rs`; this
//! module adds the interface and a short wiring recipe, so the auto-assembler
//! can reason about devices instead of spelling out each board again.

use crate::modules::{self, Look};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interface {
    Power,
    Signal,
    I2c,
    Spi,
    Uart,
    OneWire,
}

impl Interface {
    pub fn label(self) -> &'static str {
        match self {
            Interface::Power => "power",
            Interface::Signal => "signal",
            Interface::I2c => "I2C",
            Interface::Spi => "SPI",
            Interface::Uart => "UART",
            Interface::OneWire => "1-Wire",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DeviceDef {
    pub name: &'static str,
    pub interface: Interface,
    pub pins: &'static [&'static str],
    pub current_ma: u16,
    pub recipe: &'static str,
}

fn interface_of(m: &modules::Module) -> Interface {
    let has = |name: &str| m.pins.iter().any(|p| p.0 == name);
    if has("SDA") && has("SCL") {
        Interface::I2c
    } else if has("CS") || has("MOSI") || has("SCK") {
        Interface::Spi
    } else if has("TX") || has("RX") || has("DIN") {
        Interface::Uart
    } else if has("DATA") && !has("SDA") {
        Interface::OneWire
    } else if matches!(m.look, Look::Lcd | Look::Display | Look::Segments | Look::Matrix | Look::Panel) {
        Interface::Signal
    } else {
        Interface::Power
    }
}

fn recipe_of(interface: Interface) -> &'static str {
    match interface {
        Interface::I2c => "VCC and GND, SDA to A4, SCL to A5",
        Interface::Spi => "VCC and GND, CS/DC/RST and the SPI bus",
        Interface::Uart => "VCC and GND, TX and RX crossed",
        Interface::OneWire => "VCC, GND, DATA with a pull-up",
        Interface::Signal => "VCC, GND and one signal pin",
        Interface::Power => "VCC, GND and the control pin",
    }
}

/// One entry per module in the library, derived from the same table the
/// painter uses. Adding a module to `modules.rs` adds it here automatically.
pub fn all() -> Vec<DeviceDef> {
    modules::ALL
        .iter()
        .map(|m| {
            let interface = interface_of(m);
            let pins: Vec<&str> = m.pins.iter().map(|p| p.0).collect();
            DeviceDef {
                name: m.name,
                interface,
                // The pin list is static in the module table; leaking this
                // small vector keeps the registry usable as a `const` lookup.
                pins: Box::leak(pins.into_boxed_slice()),
                current_ma: m.current_ma,
                recipe: recipe_of(interface),
            }
        })
        .collect()
}

pub fn device(name: &str) -> Option<DeviceDef> {
    all().into_iter().find(|d| d.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_module_has_a_capability_record() {
        let list = all();
        assert_eq!(list.len(), modules::ALL.len());
        for def in list {
            assert!(!def.pins.is_empty(), "{} has no pins", def.name);
            assert!(!def.recipe.is_empty(), "{} has no recipe", def.name);
            assert_eq!(device(def.name).map(|d| d.interface), Some(def.interface));
        }
        assert_eq!(device("OLED 0.96").map(|d| d.interface), Some(Interface::I2c));
    }
}
