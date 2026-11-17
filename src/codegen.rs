//! Code fragments derived from the circuit graph.
//!
//! When a task names extra devices, their code is emitted from the parts and
//! wires that were actually placed, not from a separate sketch template. The
//! pin in the sketch is therefore the pin in the drawing by construction.

use crate::model::{Circuit, Kind};
use crate::text::Lang;

/// Add setup and loop lines for the parts the circuit augmenter added. Only
/// the new parts are listed, with the Uno pins that were wired for them: a
/// template that already drew and coded a part keeps its own sketch, so
/// nothing is declared twice.
pub fn augment_from_circuit(
    c: &Circuit,
    code: &mut String,
    hourly: bool,
    lang: Lang,
    added: &[crate::templates::AddedPart],
) {
    if added.is_empty() || c.arduino.is_none() {
        return;
    }
    let has_active = code.contains("bool active");
    let mut decl = String::new();
    let mut setup = String::new();
    let mut tick = String::new();
    let mut includes = String::new();
    let mut buzzers = 0usize;
    let mut relays = 0usize;
    let mut leds = 0usize;
    let mut rgbs = 0usize;
    let mut servos = 0usize;
    let mut motors = 0usize;
    let mut buttons = 0usize;

    for ap in added {
        let id = ap.part;
        let Some(p) = c.parts.get(id) else { continue };
        match p.kind {
            Kind::Buzzer | Kind::Speaker => {
                let Some(&pin) = ap.pins.first() else { continue };
                let name = if buzzers == 0 {
                    "BUZZ_PIN".to_string()
                } else {
                    format!("BUZZ_PIN{}", buzzers + 1)
                };
                buzzers += 1;
                decl.push_str(&format!("const uint8_t {name} = {pin};\n"));
                setup.push_str(&format!("  pinMode({name}, OUTPUT);\n"));
                if has_active {
                    tick.push_str(&format!(
                        "  if (active) tone({name}, 1800); else noTone({name});\n"
                    ));
                } else if hourly {
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
            Kind::Relay => {
                let Some(&pin) = ap.pins.first() else { continue };
                let name = if relays == 0 {
                    "RELAY_PIN".to_string()
                } else {
                    format!("RELAY_PIN{}", relays + 1)
                };
                relays += 1;
                decl.push_str(&format!("const uint8_t {name} = {pin};\n"));
                setup.push_str(&format!("  pinMode({name}, OUTPUT);\n"));
                if has_active {
                    tick.push_str(&format!("  digitalWrite({name}, active ? HIGH : LOW);\n"));
                } else {
                    tick.push_str(&format!(
                        "  digitalWrite({name}, (millis() / 1000) % 2 ? HIGH : LOW);\n"
                    ));
                }
            }
            Kind::Led if p.value.to_lowercase().contains("rgb") => {
                if ap.pins.len() < 3 {
                    continue;
                }
                let pins = &ap.pins;
                let n = rgbs;
                rgbs += 1;
                let names = if n == 0 {
                    ["RGB_R".to_string(), "RGB_G".to_string(), "RGB_B".to_string()]
                } else {
                    [
                        format!("RGB_R{}", n * 3 + 1),
                        format!("RGB_G{}", n * 3 + 1),
                        format!("RGB_B{}", n * 3 + 1),
                    ]
                };
                for (name, pin) in names.iter().zip(pins.iter()) {
                    decl.push_str(&format!("const uint8_t {name} = {pin};\n"));
                    setup.push_str(&format!("  pinMode({name}, OUTPUT);\n"));
                }
                if has_active {
                    for name in names.iter() {
                        tick.push_str(&format!(
                            "  digitalWrite({name}, active ? HIGH : LOW);\n"
                        ));
                    }
                } else {
                    tick.push_str("  uint32_t phase = (millis() / 700) % 3;\n");
                    for (i, name) in names.iter().enumerate() {
                        tick.push_str(&format!(
                            "  digitalWrite({name}, phase == {i} ? HIGH : LOW);\n"
                        ));
                    }
                }
            }
            Kind::Led => {
                let Some(&pin) = ap.pins.first() else { continue };
                let name = if leds == 0 {
                    "LED_PIN".to_string()
                } else {
                    format!("LED_PIN{}", leds + 1)
                };
                leds += 1;
                decl.push_str(&format!("const uint8_t {name} = {pin};\n"));
                setup.push_str(&format!("  pinMode({name}, OUTPUT);\n"));
                if has_active {
                    tick.push_str(&format!("  digitalWrite({name}, active ? HIGH : LOW);\n"));
                } else {
                    tick.push_str(&format!(
                        "  digitalWrite({name}, (millis() / 500) % 2 ? HIGH : LOW);\n"
                    ));
                }
            }
            Kind::Servo => {
                let Some(&pin) = ap.pins.first() else { continue };
                let obj = if servos == 0 {
                    "arm".to_string()
                } else {
                    format!("arm{}", servos + 1)
                };
                servos += 1;
                if !code.contains("#include <Servo.h>") {
                    includes.push_str("#include <Servo.h>\n");
                }
                decl.push_str(&format!("Servo {obj};\n"));
                setup.push_str(&format!("  {obj}.attach({pin});\n"));
                if has_active {
                    tick.push_str(&format!("  {obj}.write(active ? 90 : 0);\n"));
                } else {
                    tick.push_str(&format!("  {obj}.write((millis() / 15) % 180);\n"));
                }
            }
            Kind::Motor => {
                let Some(&pin) = ap.pins.first() else { continue };
                let name = if motors == 0 {
                    "MOTOR_PIN".to_string()
                } else {
                    format!("MOTOR_PIN{}", motors + 1)
                };
                motors += 1;
                decl.push_str(&format!("const uint8_t {name} = {pin};\n"));
                setup.push_str(&format!("  pinMode({name}, OUTPUT);\n"));
                if has_active {
                    tick.push_str(&format!(
                        "  analogWrite({name}, active ? 255 : 0);\n"
                    ));
                } else {
                    tick.push_str(&format!("  analogWrite({name}, 128);\n"));
                }
            }
            Kind::Button => {
                let Some(&pin) = ap.pins.first() else { continue };
                let name = if buttons == 0 {
                    "BUTTON_PIN".to_string()
                } else {
                    format!("BUTTON_PIN{}", buttons + 1)
                };
                buttons += 1;
                decl.push_str(&format!("const uint8_t {name} = {pin};\n"));
                setup.push_str(&format!("  pinMode({name}, INPUT_PULLUP);\n"));
            }
            _ => {}
        }
    }

    if buzzers > 0 {
        decl.push_str("uint32_t lastBeep = 0;\nuint8_t lastHour = 255;\n");
    }
    if decl.is_empty() && setup.is_empty() && tick.is_empty() {
        return;
    }
    if !includes.is_empty() {
        code.insert_str(0, &includes);
    }
    if let Some(at) = code.find("void setup()") {
        code.insert_str(at, &decl);
    }
    if let Some(at) = code.find("void setup() {") {
        code.insert_str(at + "void setup() {".len(), &format!("\n{setup}"));
    }
    if let Some(at) = code.rfind('}') {
        code.insert_str(at, &tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{End, Hole, PinKind, PinSpec, Pt};

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
        augment_from_circuit(
            &c,
            &mut code,
            false,
            Lang::Ru,
            &[crate::templates::AddedPart { part: b, pins: vec![8] }],
        );
        assert!(code.contains("BUZZ_PIN = 8"), "{code}");
        assert!(code.contains("tone(BUZZ_PIN"));
    }
}
