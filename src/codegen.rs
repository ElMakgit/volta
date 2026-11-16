//! Code fragments derived from the circuit graph.
//!
//! When a task names extra devices, their code is emitted from the parts and
//! wires that were actually placed, not from a separate sketch template. The
//! pin in the sketch is therefore the pin in the drawing by construction.

use crate::model::{Circuit, End, Kind};
use crate::text::Lang;

fn uno_pin_for(c: &Circuit, part: usize, pin_name: &str) -> Option<String> {
    let local = c.parts.get(part)?.pins.iter().copied().find(|&p| c.pins[p].name == pin_name)?;
    for w in c.wires() {
        let other = if w.a == End::Pin(local) {
            Some(w.b)
        } else if w.b == End::Pin(local) {
            Some(w.a)
        } else {
            None
        };
        if let Some(End::Pin(p)) = other {
            if c.pins[p].part == c.arduino? {
                return Some(c.pins[p].name.to_string());
            }
        }
    }
    None
}

fn pin_number(name: &str) -> u8 {
    name.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(0)
}

/// Add setup and loop lines for the devices on the bench. A missing wire is
/// skipped instead of guessing a pin, so code and schematic stay in step.
pub fn augment_from_circuit(c: &Circuit, code: &mut String, hourly: bool, lang: Lang) {
    let Some(uno) = c.arduino else { return };
    let buzzers: Vec<(String, u8)> = c
        .parts_id()
        .filter(|(_, p)| p.kind == Kind::Buzzer && p.active)
        .filter_map(|(id, _)| uno_pin_for(c, id, "+").map(|name| { let pin = pin_number(&name); (name, pin) }))
        .collect();
    let relays: Vec<(String, u8)> = c
        .parts_id()
        .filter(|(_, p)| p.kind == Kind::Relay && p.active)
        .filter_map(|(id, _)| uno_pin_for(c, id, "IN").map(|name| { let pin = pin_number(&name); (name, pin) }))
        .collect();
    if (buzzers.is_empty() || code.contains("BUZZ_PIN")) && relays.is_empty() {
        return;
    }

    let mut decl = String::new();
    let mut setup = String::new();
    let mut tick = String::new();
    for (i, (_, pin)) in buzzers.iter().enumerate() {
        let name = if i == 0 { "BUZZ_PIN" } else { "BUZZ_PIN2" };
        decl.push_str(&format!("const uint8_t {name} = {pin};\n"));
        setup.push_str(&format!("  pinMode({name}, OUTPUT);\n"));
        if hourly {
            tick.push_str(&format!(
                "  if (now.hour() != lastHour) {{ lastHour = now.hour(); tone({name}, 1800, 80); }}\n"
            ));
        } else {
            let unit = if lang == Lang::Ru { "мс" } else { "ms" };
            tick.push_str(&format!(
                "  if (millis() - lastBeep >= 3600000UL) {{ lastBeep = millis(); tone({name}, 1800, 80); }} // 1 {unit}\n"
            ));
        }
    }
    for (i, (_, pin)) in relays.iter().enumerate() {
        let name = if i == 0 { "RELAY_PIN" } else { "RELAY_PIN2" };
        decl.push_str(&format!("const uint8_t {name} = {pin};\n"));
        setup.push_str(&format!("  pinMode({name}, OUTPUT);\n"));
        tick.push_str(&format!("  digitalWrite({name}, millis() < 1000 ? HIGH : LOW);\n"));
    }
    decl.push_str("uint32_t lastBeep = 0;\nuint8_t lastHour = 255;\n");

    if let Some(at) = code.find("void setup()") {
        code.insert_str(at, &decl);
    }
    if let Some(at) = code.find("void setup() {") {
        code.insert_str(at + "void setup() {".len(), &format!("\n{setup}"));
    }
    if let Some(at) = code.rfind('}') {
        code.insert_str(at, &tick);
    }
    let _ = uno;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Hole, PinKind, PinSpec, Pt};

    #[test]
    fn a_buzzer_code_uses_the_pin_from_the_wire() {
        let mut c = Circuit::new();
        let uno = c.add_arduino(Pt::new(60.0, 20.0));
        let b = c.add_part(
            Kind::Buzzer,
            "BZ1",
            "пьезо",
            Pt::new(4.0, 90.0),
            vec![
                PinSpec { name: "-", pos: Pt::new(4.0, 90.0), hole: Hole::OffBoard, kind: PinKind::Passive },
                PinSpec { name: "+", pos: Pt::new(10.0, 90.0), hole: Hole::OffBoard, kind: PinKind::Passive },
            ],
            vec![],
        );
        let plus = c.pin(b, "+");
        let d8 = c.pin(uno, "D8");
        c.wire(End::Pin(plus), End::Pin(d8), crate::model::NetClass::Signal);
        let mut code = String::from("void setup() {\n}\n\nvoid loop() {\n}\n");
        augment_from_circuit(&c, &mut code, false, Lang::Ru);
        assert!(code.contains("BUZZ_PIN = 8"), "{code}");
        assert!(code.contains("tone(BUZZ_PIN"));
    }
}
