//! Electrical and code checks.
//!
//! These are the tests the app can honestly run without a SPICE engine: they
//! walk the real netlist (wires + breadboard strips + part internals), and they
//! cross-check the generated sketch against the drawing. A wrong pin in the
//! code, a missing flyback diode, or an LED without a resistor are all caught
//! here rather than on the bench.

use crate::model::*;
use crate::templates::{Built, Tid};
use crate::text::{tr, Lang, L};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sev {
    Pass,
    Warn,
    Fail,
}

#[derive(Clone)]
pub struct Check {
    pub name: L,
    pub sev: Sev,
    pub detail: String,
}

impl Check {
    fn pass(name: L, detail: String) -> Self {
        Self { name, sev: Sev::Pass, detail }
    }
    fn warn(name: L, detail: String) -> Self {
        Self { name, sev: Sev::Warn, detail }
    }
    fn fail(name: L, detail: String) -> Self {
        Self { name, sev: Sev::Fail, detail }
    }
}

#[derive(Clone)]
pub struct Sim {
    /// Drain voltage of the switching element over time, 0..255.
    pub points: Vec<f32>,
    /// Milliseconds between samples.
    pub step_ms: f32,
}

#[derive(Clone)]
pub struct Report {
    pub checks: Vec<Check>,
    pub sim: Option<Sim>,
}

impl Report {
    pub fn passed(&self) -> usize {
        self.checks.iter().filter(|c| c.sev == Sev::Pass).count()
    }
    pub fn failed(&self) -> usize {
        self.checks.iter().filter(|c| c.sev == Sev::Fail).count()
    }
    pub fn worst(&self) -> Sev {
        if self.checks.iter().any(|c| c.sev == Sev::Fail) {
            Sev::Fail
        } else if self.checks.iter().any(|c| c.sev == Sev::Warn) {
            Sev::Warn
        } else {
            Sev::Pass
        }
    }
}

fn pin_label(c: &Circuit, pin: usize) -> String {
    format!("{}.{}", c.parts[c.pins[pin].part].label, c.pins[pin].name)
}

fn pins_with_kind(c: &Circuit, nets_roots: &[usize], root: usize, kind: PinKind) -> Vec<usize> {
    c.pins
        .iter()
        .enumerate()
        .filter(|(i, p)| nets_roots[*i] == root && p.kind == kind)
        .map(|(i, _)| i)
        .collect()
}

/// Signal pins of the board itself, not of a module that happens to have an
/// input pin (a relay module's IN is meant to sit on a pin).
fn uno_signal_on(c: &Circuit, roots: &[usize], root: usize) -> bool {
    let Some(uno) = c.arduino else { return false };
    c.pins.iter().enumerate().any(|(i, p)| {
        roots[i] == root && p.part == uno && p.kind == PinKind::Signal
    })
}

fn parts_with<'a>(c: &'a Circuit, kind: Kind) -> impl Iterator<Item = &'a Part> {
    c.parts.iter().filter(move |p| p.kind == kind)
}

fn ohms(value: &str) -> Option<f32> {
    let lower = value.to_lowercase();
    let mult = if lower.contains('к') || lower.contains('k') { 1000.0 } else { 1.0 };
    let digits: String = value
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits.parse::<f32>().ok().map(|n| n * mult)
}

fn forward_volts(value: &str) -> f32 {
    let v = value.to_lowercase();
    if v.contains("син") || v.contains("blue") || v.contains("rgb") {
        3.1
    } else if v.contains("зел") || v.contains("green") || v.contains("бел") || v.contains("white") {
        2.6
    } else {
        1.9
    }
}

/// `const uint8_t BTN = 2;` and `#define BTN 2` style declarations. The value
/// is kept as written, because analog pins are named `A0` and not numbered.
fn declared(code: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in code.lines() {
        let line = line.trim();
        let line = line.split("//").next().unwrap_or("").trim();
        if !(line.starts_with("const ") || line.starts_with("#define ")) {
            continue;
        }
        if line.starts_with("#define ") {
            let mut it = line[8..].split_whitespace();
            if let (Some(name), Some(value)) = (it.next(), it.next()) {
                out.push((name.to_string(), value.to_string()));
            }
            continue;
        }
        let Some(eq) = line.find('=') else { continue };
        let Some(semi) = line[eq..].find(';') else { continue };
        let name = line[..eq].split_whitespace().last().unwrap_or("").to_string();
        let value = line[eq + 1..eq + semi].trim();
        if value.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            out.push((name, value.to_string()));
        }
    }
    out
}

fn pin_mode_pins(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in code.lines().filter(|l| l.contains("pinMode(")) {
        if let Some(start) = line.find("pinMode(") {
            let rest = &line[start + 8..];
            if let Some(comma) = rest.find(',') {
                out.push(rest[..comma].trim().to_string());
            }
        }
    }
    out
}

/// Pin numbers written into `analogWrite(...)` calls.
fn analog_write_targets(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in code.lines() {
        if let Some(start) = line.find("analogWrite(") {
            let rest = &line[start + 12..];
            if let Some(comma) = rest.find(',') {
                out.push(rest[..comma].trim().to_string());
            }
        }
    }
    out
}

fn analog_read_targets(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in code.lines() {
        if let Some(start) = line.find("analogRead(") {
            let rest = &line[start + 11..];
            if let Some(paren) = rest.find(')') {
                out.push(rest[..paren].trim().to_string());
            }
        }
    }
    out
}

pub fn run(built: &Built, lang: Lang) -> Report {
    Report { sim: simulate(built), ..run_circuit(&built.circuit, lang, Some(&built.code)) }
}

/// The same checks on a bare netlist, for circuits drawn by hand. Without a
/// sketch the code-side checks are skipped instead of failing.
pub fn run_circuit(c: &Circuit, lang: Lang, code: Option<&str>) -> Report {
    let code = code.unwrap_or("");
    let nets = c.nets();
    let roots: Vec<usize> = (0..c.pins.len()).map(|i| nets.root(i)).collect();
    let Some(uno) = c.arduino else {
        return Report { checks: Vec::new(), sim: None };
    };
    let uno_5v = roots[c.pin(uno, "5V")];
    let uno_gnd = roots[c.pin(uno, "GND1")];
    let uno_3v3 = roots[c.pin(uno, "3V3")];
    let mut checks = Vec::new();

    // A driver board's outputs are a real supply for the load behind them:
    // a motor on L298N OUT1 is powered even though it never touches 5 V.
    let module_out_roots: Vec<usize> = c
        .parts
        .iter()
        .filter(|p| p.kind == Kind::Module)
        .filter(|p| {
            p.pins.iter().any(|&i| {
                matches!(c.pins[i].name, "VCC" | "+5V" | "+12V" | "3V3")
                    && {
                        let r = roots[i];
                        r == uno_5v || r == uno_3v3 || r == roots[c.pin(uno, "VIN")]
                    }
            })
        })
        .flat_map(|p| {
            p.pins
                .iter()
                .filter(|&&i| {
                    c.pins[i].name.starts_with("OUT")
                        || matches!(c.pins[i].name, "COM" | "+")
                })
                .map(|&i| roots[i])
        })
        .collect();

    // A module fed from 12 V makes its own 5 V, so that pin is an output and
    // may legitimately stay free.
    let regulator_outputs: Vec<usize> = c
        .parts
        .iter()
        .filter(|p| p.kind == Kind::Module)
        .filter(|p| {
            p.pins.iter().any(|&i| {
                c.pins[i].name == "+12V" && {
                    let r = roots[i];
                    r == uno_5v || r == uno_3v3 || r == roots[c.pin(uno, "VIN")]
                }
            })
        })
        .flat_map(|p| p.pins.iter().copied())
        .filter(|&i| c.pins[i].name == "+5V")
        .collect();

    // 1. Every part ground reaches the board ground.
    let flying: Vec<String> = c
        .pins
        .iter()
        .enumerate()
        .filter(|(_, p)| p.kind == PinKind::Ground && p.part != uno)
        .filter(|(i, _)| roots[*i] != uno_gnd)
        .map(|(i, _)| pin_label(c, i))
        .collect();
    checks.push(if flying.is_empty() {
        Check::pass(
            L::new("Земля", "Ground"),
            tr(lang, "Все выводы земли на одном узле с GND платы.", "Every ground pin shares the board's GND node.").to_string(),
        )
    } else {
        Check::fail(
            L::new("Земля", "Ground"),
            format!(
                "{}: {}",
                tr(lang, "не доходит до GND", "does not reach GND"),
                flying.join(", ")
            ),
        )
    });

    // 1b. Nothing may hang in the air, and an empty board is not a circuit.
    let loose: Vec<String> = c
        .pins
        .iter()
        .enumerate()
        .filter(|(_, p)| p.part != uno && p.hole == Hole::OffBoard && p.kind == PinKind::Passive)
        .filter(|(i, _)| nets.pins_in(roots[*i]).len() < 2)
        .map(|(i, _)| pin_label(c, i))
        .collect();
    let parts_total = c.parts.iter().filter(|p| p.kind != Kind::Arduino).count();
    checks.push(if parts_total == 0 {
        Check::warn(
            L::new("Монтаж", "Assembly"),
            tr(lang, "На макетке пока нет деталей.", "There are no parts on the breadboard yet.").to_string(),
        )
    } else if !loose.is_empty() {
        Check::warn(
            L::new("Монтаж", "Assembly"),
            format!("{}: {}", tr(lang, "выводы без подключения", "unconnected terminals"), loose.join(", ")),
        )
    } else {
        Check::pass(
            L::new("Монтаж", "Assembly"),
            tr(lang, "Каждый вывод детали к чему-то подключён.", "Every part terminal is connected to something.").to_string(),
        )
    });

    // 2. Every power pin reaches 5 V, 3V3 or VIN.
    let orphan_power: Vec<String> = c
        .pins
        .iter()
        .enumerate()
        .filter(|(i, p)| {
            p.kind == PinKind::Power && p.part != uno && !regulator_outputs.contains(i)
        })
        .filter(|(i, _)| {
            let r = roots[*i];
            r != uno_5v
                && r != uno_3v3
                && r != roots[c.pin(uno, "VIN")]
                && !module_out_roots.contains(&r)
        })
        .map(|(i, _)| pin_label(c, i))
        .collect();
    checks.push(if orphan_power.is_empty() {
        Check::pass(
            L::new("Питание", "Power"),
            tr(lang, "Питание приходит с платы, висящих выводов нет.", "Power comes from the board, nothing floats.").to_string(),
        )
    } else {
        Check::fail(
            L::new("Питание", "Power"),
            format!("{}: {}", tr(lang, "без питания", "unpowered"), orphan_power.join(", ")),
        )
    });

    // 3. Rails are not shorted.
    checks.push(if uno_5v != uno_gnd && uno_5v != uno_3v3 {
        Check::pass(
            L::new("Короткое замыкание", "Short circuit"),
            tr(lang, "5 В, 3.3 В и земля — разные узлы.", "5 V, 3.3 V and ground are separate nodes.").to_string(),
        )
    } else {
        Check::fail(
            L::new("Короткое замыкание", "Short circuit"),
            tr(lang, "Питание соединено с землёй — плата сгорит.", "Power is wired to ground — the board would die.").to_string(),
        )
    });

    // 4. LEDs have a series resistor on everyAnode.
    let mut led_problems = Vec::new();
    for led in parts_with(c, Kind::Led) {
        let pin_count = led.pins.len();
        let channels = if pin_count >= 4 { 3 } else { 1 };
        for ch in 0..channels {
            let pin = led.pins[ch];
            let r = roots[pin];
            let has_r = c
                .pins
                .iter()
                .enumerate()
                .any(|(i, p)| roots[i] == r && c.parts[p.part].kind == Kind::Resistor);
            if !has_r {
                led_problems.push(pin_label(c, pin));
            }
        }
    }
    checks.push(if led_problems.is_empty() {
        Check::pass(
            L::new("Резисторы светодиодов", "LED resistors"),
            tr(lang, "У каждого канала свой токоограничивающий резистор.", "Every channel has its own current-limiting resistor.").to_string(),
        )
    } else {
        Check::fail(
            L::new("Резисторы светодиодов", "LED resistors"),
            format!("{}: {}", tr(lang, "без резистора", "no resistor"), led_problems.join(", ")),
        )
    });

    // 5. LED current stays inside the pin's 20 mA.
    let mut over = Vec::new();
    for led in parts_with(c, Kind::Led) {
        let vf = forward_volts(&led.value);
        let pin = led.pins[0];
        let net_r: Vec<f32> = c
            .pins
            .iter()
            .enumerate()
            .filter(|(i, p)| roots[*i] == roots[pin] && c.parts[p.part].kind == Kind::Resistor)
            .filter_map(|(_, p)| ohms(&c.parts[p.part].value))
            .collect();
        for r in net_r {
            if r <= 0.0 {
                continue;
            }
            let i = (5.0 - vf) / r;
            if i > 0.02 {
                over.push(format!("{} — {:.0} мА", led.label, i * 1000.0));
            }
        }
    }
    checks.push(if over.is_empty() {
        Check::pass(
            L::new("Ток пина", "Pin current"),
            tr(lang, "Ток каждого канала ниже 20 мА.", "Every channel stays under 20 mA.").to_string(),
        )
    } else {
        Check::fail(
            L::new("Ток пина", "Pin current"),
            over.join("; "),
        )
    });

    // 6. Inductive loads get a flyback diode across them.
    let mut fly = Vec::new();
    for load in c.parts.iter().filter(|p| p.kind.inductive() && p.kind != Kind::Relay) {
        // A driver board already carries the flyback diodes for its outputs.
        if load.pins.iter().any(|&i| module_out_roots.contains(&roots[i])) {
            continue;
        }
        let a = roots[load.pins[0]];
        let b = roots[load.pins[1]];
        let diode = c.parts.iter().find(|d| {
            d.kind == Kind::Diode
                && ((roots[d.pins[0]] == a && roots[d.pins[1]] == b)
                    || (roots[d.pins[0]] == b && roots[d.pins[1]] == a))
        });
        match diode {
            Some(d) => {
                // Cathode must sit on the positive side.
                let k = roots[c.pin(c.parts.iter().position(|p| std::ptr::eq(p, d)).unwrap(), "K")];
                let positive_side = if pins_with_kind(c, &roots, a, PinKind::Power).is_empty()
                    && pins_with_kind(c, &roots, b, PinKind::Power).is_empty()
                {
                    // Neither side is a power pin: compare with the rail that
                    // carries 5 V.
                    if a == uno_5v { a } else { b }
                } else if !pins_with_kind(c, &roots, a, PinKind::Power).is_empty() {
                    a
                } else {
                    b
                };
                if k != positive_side {
                    fly.push(format!("{}: {}", load.label, tr(lang, "диод в обратную сторону", "diode is reversed")));
                }
            }
            None => fly.push(format!("{}: {}", load.label, tr(lang, "нет обратного диода", "no flyback diode"))),
        }
    }
    checks.push(if fly.is_empty() {
        Check::pass(
            L::new("Обратный диод", "Flyback diode"),
            tr(lang, "Индуктивная нагрузка защищена диодом в правильной полярности.", "The inductive load has a correctly oriented diode.").to_string(),
        )
    } else {
        Check::fail(L::new("Обратный диод", "Flyback diode"), fly.join("; "))
    });

    // 7. Loads are not hung straight on a pin.
    let mut direct = Vec::new();
    for load in c.parts.iter().filter(|p| matches!(p.kind, Kind::Motor | Kind::Speaker)) {
        for &pin in &load.pins {
            if uno_signal_on(c, &roots, roots[pin]) {
                direct.push(format!("{} → {}", load.label, pin_label(c, pin)));
                break;
            }
        }
    }
    checks.push(if direct.is_empty() {
        Check::pass(
            L::new("Нагрузка", "Load"),
            tr(lang, "Силовая нагрузка не висит на пине напрямую.", "The power load is not wired straight to a pin.").to_string(),
        )
    } else {
        Check::fail(
            L::new("Нагрузка", "Load"),
            format!("{}: {}", tr(lang, "прямо на пин", "straight to a pin"), direct.join(", ")),
        )
    });

    // 8. Transistor gates are driven and not left floating.
    let mut gates = Vec::new();
    for q in c.parts.iter().filter(|p| p.kind.is_transistor()) {
        let gate = q.pins[0];
        let r = roots[gate];
        let resistors = c
            .pins
            .iter()
            .enumerate()
            .filter(|(i, p)| roots[*i] == r && c.parts[p.part].kind == Kind::Resistor)
            .count();
        if resistors == 0 {
            gates.push(format!("{}: {}", q.label, tr(lang, "затвор/база без резистора", "gate/base without a resistor")));
        }
        // The source/emitter has to reach ground.
        let tail = q
            .pins
            .iter()
            .copied()
            .find(|&p| matches!(c.pins[p].name, "S" | "E"))
            .unwrap_or(q.pins[q.pins.len() - 1]);
        if roots[tail] != uno_gnd {
            gates.push(format!("{}: {}", q.label, tr(lang, "исток/эмиттер не на земле", "source/emitter is not grounded")));
        }
    }
    checks.push(if gates.is_empty() {
        Check::pass(
            L::new("Ключ", "Switch"),
            tr(lang, "Ключ управляется через резистор и надёжно привязан к земле.", "The switch is driven through a resistor and tied to ground.").to_string(),
        )
    } else {
        Check::fail(L::new("Ключ", "Switch"), gates.join("; "))
    });

    // 9. Buttons bridge a signal to a supply, and the sketch enables a pull-up.
    let mut btns = Vec::new();
    let pullup = code.contains("INPUT_PULLUP");
    for b in parts_with(c, Kind::Button) {
        let side_a = roots[b.pins[0]];
        let side_b = roots[b.pins[2]];
        let a_signal = uno_signal_on(c, &roots, side_a);
        let b_signal = uno_signal_on(c, &roots, side_b);
        let a_gnd = side_a == uno_gnd;
        let b_gnd = side_b == uno_gnd;
        if !((a_signal && b_gnd) || (b_signal && a_gnd)) {
            btns.push(format!("{}: {}", b.label, tr(lang, "не замыкает сигнал на землю", "does not bridge signal to ground")));
        } else if !pullup && !code.is_empty() {
            btns.push(format!("{}: {}", b.label, tr(lang, "нужен INPUT_PULLUP", "needs INPUT_PULLUP")));
        }
    }
    checks.push(if btns.is_empty() {
        Check::pass(
            L::new("Кнопка", "Button"),
            tr(lang, "Кнопка замыкает сигнал на землю, подтяжка включена в коде.", "The button pulls the signal to ground and the sketch enables the pull-up.").to_string(),
        )
    } else {
        Check::fail(L::new("Кнопка", "Button"), btns.join("; "))
    });

    // Checks 10-12 read the sketch, so a hand-drawn circuit skips them.
    if !code.is_empty() {
        // 10. Sketch pins exist on the drawing and are wired.
        let declared = declared(code);
        let modes = pin_mode_pins(code);
        let mut mismatched = Vec::new();
        let mut used = Vec::new();
        let wired = |pin_name: &str| -> bool {
            (0..c.parts[uno].pins.len())
                .map(|i| c.parts[uno].pins[i])
                .find(|&p| c.pins[p].name == pin_name)
                .map(|p| nets.pins_in(roots[p]).len() >= 2)
                .unwrap_or(false)
        };
        for (name, value) in &declared {
            if !modes.contains(name) {
                continue;
            }
            used.push(name.clone());
            let pin_name = if value.starts_with('A') { value.clone() } else { format!("D{value}") };
            if !wired(&pin_name) {
                mismatched.push(format!("{name} → {pin_name}"));
            }
        }
        for name in analog_read_targets(code) {
            used.push(name.clone());
            let value = declared
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
                .unwrap_or(name.clone());
            let pin_name = if value.starts_with('A') { value } else { format!("A{value}") };
            if !wired(&pin_name) {
                mismatched.push(format!("{name} → {pin_name}"));
            }
        }
        checks.push(if mismatched.is_empty() {
            Check::pass(
                L::new("Код ↔ схема", "Code vs drawing"),
                format!(
                    "{} {}",
                    tr(lang, "Каждый пин из скетча есть в схеме и подключён:", "Every pin in the sketch exists on the drawing and is wired:"),
                    if used.is_empty() { tr(lang, "нет объявленных пинов", "no declared pins").to_string() } else { used.join(", ") }
                ),
            )
        } else {
            Check::fail(
                L::new("Код ↔ схема", "Code vs drawing"),
                format!("{}: {}", tr(lang, "в схеме не подключено", "not wired on the drawing"), mismatched.join(", ")),
            )
        });

        // 11. PWM only on timer pins.
        let pwm_ok = [3u32, 5, 6, 9, 10, 11];
        let mut bad_pwm = Vec::new();
        for target in analog_write_targets(code) {
            let value = declared
                .iter()
                .find(|(n, _)| *n == target)
                .map(|(_, v)| v.clone())
                .unwrap_or(target);
            if let Ok(n) = value.trim_start_matches('D').parse::<u32>() {
                if !pwm_ok.contains(&n) {
                    bad_pwm.push(format!("D{n}"));
                }
            }
        }
        checks.push(if bad_pwm.is_empty() {
            Check::pass(
                L::new("ШИМ", "PWM"),
                tr(lang, "ШИМ только на пинах таймеров: 3, 5, 6, 9, 10, 11.", "PWM only on timer pins: 3, 5, 6, 9, 10, 11.").to_string(),
            )
        } else {
            Check::fail(
                L::new("ШИМ", "PWM"),
                format!("{}: {}", tr(lang, "аппаратного ШИМ нет на", "no hardware PWM on"), bad_pwm.join(", ")),
            )
        });

        // 12. delay() in loop blocks every other task.
        let loop_body = code.split("void loop()").nth(1).unwrap_or("");
        checks.push(if loop_body.contains("delay(") {
            Check::warn(
                L::new("Цикл без блокировок", "Non-blocking loop"),
                tr(lang, "В loop() есть delay(): кнопка и ШИМ будут отзываться рывками.", "loop() calls delay(): the button and PWM will stutter.").to_string(),
            )
        } else {
            Check::pass(
                L::new("Цикл без блокировок", "Non-blocking loop"),
                tr(lang, "loop() крутится на millis(), без delay().", "loop() runs on millis(), with no delay().").to_string(),
            )
        });
    }

    // 13. Power budget of the 5 V regulator.
    let draw: f32 = c
        .parts
        .iter()
        .map(|p| match p.kind {
            Kind::Motor => 250.0,
            Kind::Servo => 250.0,
            Kind::Relay => 70.0,
            Kind::Speaker => 400.0,
            Kind::Buzzer => 30.0,
            Kind::Peltier => 2000.0,
            Kind::Led => 20.0,
            Kind::Module => crate::modules::find(&p.value)
                .map(|m| m.current_ma as f32)
                .unwrap_or(0.0),
            _ => 0.0,
        })
        .sum();
    checks.push(if draw > 400.0 {
        Check::warn(
            L::new("Бюджет питания", "Power budget"),
            format!(
                "{:.0} {} — {}",
                draw,
                tr(lang, "мА суммарно", "mA in total"),
                tr(lang, "USB-порт даёт около 500 мА, лучше внешний блок питания", "USB gives about 500 mA, so an external supply is safer")
            ),
        )
    } else {
        Check::pass(
            L::new("Бюджет питания", "Power budget"),
            format!("{:.0} {} — {}", draw, tr(lang, "мА суммарно", "mA in total"), tr(lang, "запас есть", "there is headroom")),
        )
    });

    // 13b. Modules from the book: the hungry ones need their own supply, and
    // the 3.3 V ones must not be tied to 5 V.
    for part in c.parts.iter().filter(|p| p.kind == Kind::Module) {
        let Some(m) = crate::modules::find(&part.value) else { continue };
        if m.current_ma >= 400 {
            checks.push(Check::warn(
                L::new("Питание модуля", "Module supply"),
                format!(
                    "{} {} {} {}",
                    m.name,
                    tr(lang, "берёт до", "draws up to"),
                    m.current_ma,
                    tr(lang, "мА в импульсе: нужен отдельный блок питания.", "mA in bursts: use its own supply.")
                ),
            ));
        }
        if m.three_three {
            let wired_5v = m
                .pins
                .iter()
                .filter(|(name, _, _)| matches!(*name, "3V3" | "VCC"))
                .filter_map(|(name, _, _)| {
                    part.pins.iter().copied().find(|&i| c.pins[i].name == *name)
                })
                .any(|i| roots[i] == uno_5v);
            checks.push(if wired_5v {
                Check::fail(
                    L::new("3.3 В", "3.3 V"),
                    format!(
                        "{} {}",
                        m.name,
                        tr(lang, "питается от 5 В, а рассчитан на 3.3 В.", "is wired to 5 V but expects 3.3 V.")
                    ),
                )
            } else {
                Check::pass(
                    L::new("3.3 В", "3.3 V"),
                    format!(
                        "{} {}",
                        m.name,
                        tr(lang, "подключён к 3.3 В, как и положено.", "is on the 3.3 V rail, as it should be.")
                    ),
                )
            });
        }
    }

    // 14. The rails used by the build are bridged across the middle gap.
    if c.bare {
        checks.push(Check::pass(
            L::new("Шины питания", "Power rails"),
            tr(lang, "Схема собрана без макетки — шины не нужны.", "Built without a breadboard, so there are no rails to bridge.").to_string(),
        ));
    } else {
    let bridged = c.wires.iter().any(|w| {
        matches!(
            (w.a, w.b),
            (End::Hole(Hole::Rail(_, RailHalf::Left, _)), End::Hole(Hole::Rail(_, RailHalf::Right, _)))
                | (End::Hole(Hole::Rail(_, RailHalf::Right, _)), End::Hole(Hole::Rail(_, RailHalf::Left, _)))
        )
    });
    let uses_right = c.pins.iter().any(|p| matches!(p.hole, Hole::Rail(_, RailHalf::Right, _)));
    let uses_left = c.pins.iter().any(|p| matches!(p.hole, Hole::Rail(_, RailHalf::Left, _)));
    checks.push(if uses_left && uses_right && !bridged {
        Check::fail(
            L::new("Шины питания", "Power rails"),
            tr(lang, "Обе половины шины используются, но перемычки между ними нет.", "Both halves of a rail are used without a bridge between them.").to_string(),
        )
    } else {
        Check::pass(
            L::new("Шины питания", "Power rails"),
            tr(lang, "Шины питания связаны между половинами макетки.", "Power rails are joined across the board's halves.").to_string(),
        )
    });
    }

    Report {
        checks,
        sim: None,
    }
}

/// Discrete-time model of the generated behaviour, so the UI can show what the
/// sketch actually does with repeated presses.
fn simulate(built: &Built) -> Option<Sim> {
    match built.tid {
        Tid::MotorSteps => {
            let (steps, step_ms) = (6.0f32, 20.0f32);
            let mut duty = 0.0f32;
            let mut points = Vec::new();
            for press in 0..6 {
                let target = (press as f32 * 255.0 / (steps - 1.0)).round();
                for _ in 0..25 {
                    // 180 ms full ramp at one count per 0.7 ms ≈ 25 samples of 20 ms.
                    if duty < target {
                        duty += (target - duty) / 25.0;
                    } else if duty > target {
                        duty -= (duty - target) / 25.0;
                    }
                    points.push(duty);
                }
            }
            Some(Sim { points, step_ms })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{Intent, Load, Input};

    fn build_all() -> Vec<Built> {
        let cases = [
            (Load::Motor, Some(Input::Button)),
            (Load::Led, Some(Input::Button)),
            (Load::Led, None),
            (Load::Led, Some(Input::Ldr)),
            (Load::Servo, None),
            (Load::Buzzer, None),
        ];
        let mut out = Vec::new();
        for (load, input) in cases {
            for choice in 0..3 {
                let intent = Intent {
                    load,
                    input,
                    transistor: false,
                    presses: false,
                    lesson: None,
                    action: None,
                    raw: String::new(),
                };
                out.push(crate::templates::build(&intent, choice, Lang::Ru).unwrap());
            }
        }
        out
    }

    /// A circuit drawn by hand has no sketch, but the supply and rail checks
    /// are about the drawing alone: they must still appear in the report.
    #[test]
    fn a_hand_drawn_circuit_still_gets_the_supply_checks() {
        let intent = Intent {
            load: Load::Motors,
            input: None,
            transistor: false,
            presses: false,
            lesson: None,
            action: None,
            raw: String::new(),
        };
        let built = crate::templates::build(&intent, 0, Lang::Ru).unwrap();
        let report = run_circuit(&built.circuit, Lang::Ru, None);
        let names: Vec<&str> = report.checks.iter().map(|c| c.name.ru).collect();
        for want in ["Бюджет питания", "Шины питания"] {
            assert!(names.contains(&want), "{want} missing from {names:?}");
        }
        // The sketch checks stay out of a hand-drawn report.
        assert!(!names.contains(&"Код ↔ схема"), "{names:?}");
        assert!(!names.contains(&"Цикл без блокировок"), "{names:?}");

        // A hungry module reports its own supply even without a sketch.
        let gsm = Intent {
            load: Load::Gsm,
            input: None,
            transistor: false,
            presses: false,
            lesson: None,
            action: None,
            raw: String::new(),
        };
        let built = crate::templates::build(&gsm, 0, Lang::Ru).unwrap();
        let report = run_circuit(&built.circuit, Lang::Ru, None);
        let names: Vec<&str> = report.checks.iter().map(|c| c.name.ru).collect();
        assert!(names.contains(&"Питание модуля"), "{names:?}");
    }

    #[test]
    fn every_generated_circuit_passes_its_checks() {
        for built in build_all() {
            let report = run(&built, Lang::Ru);
            let failed: Vec<&str> = report
                .checks
                .iter()
                .filter(|c| c.sev == Sev::Fail)
                .map(|c| c.name.ru)
                .collect();
            assert!(
                failed.is_empty(),
                "{} choice {}: {:?}\n{}",
                built.title,
                built.choice,
                failed,
                report
                    .checks
                    .iter()
                    .map(|c| format!("{} {:?} {}", c.name.ru, c.sev, c.detail))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }

    #[test]
    fn ohms_parses_both_units() {
        assert_eq!(ohms("220 Ω"), Some(220.0));
        assert_eq!(ohms("10 кОм"), Some(10_000.0));
        assert_eq!(ohms("4.7 кОм"), Some(4700.0));
        assert_eq!(ohms("1 кОм"), Some(1000.0));
    }

    #[test]
    fn declared_reads_constants() {
        let pins = declared("const uint8_t BTN = 2;\nconst int LED = 9;\nconst uint8_t SENSE = A0;\n// const x = 4;\n");
        assert_eq!(
            pins,
            vec![
                ("BTN".to_string(), "2".to_string()),
                ("LED".to_string(), "9".to_string()),
                ("SENSE".to_string(), "A0".to_string())
            ]
        );
    }
}
