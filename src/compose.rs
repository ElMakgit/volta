//! Turn a [`Composition`] into a plan, a circuit and a sketch.
//!
//! The builder does not know any particular sensor or output: it places the
//! source from its table entry, picks a free pin for the sink, and joins the
//! fragments. Adding a sensor is a data edit in `spec.rs`.

use crate::model::{Circuit, Kind, NetClass, Pt, Row};
use crate::parse::{Action, Load};
use crate::spec::*;
use crate::templates::{self, B, Built, Choice, Plan, Tid};
use crate::text::{tr, Lang, L};

/// A sentence with a sensor and a verb becomes a composition. The sink comes
/// from the sentence's load word, or from an explicit "реле"/"пищалка".
pub fn detect(
    raw: &str,
    load: Load,
    named_load: bool,
    lesson: Option<usize>,
    action: Option<Action>,
    condition: Option<Condition>,
) -> Option<Composition> {
    let text = crate::words::normalize(raw);
    // Only a sentence that asks the circuit to do something is composed:
    // "Измерь температуру" stays the lesson that teaches the sensor.
    // A screen named outright is a sink on its own: "температура на дисплее"
    // has no verb the action table catches, yet it asks for a readout. The
    // serial port is not a screen, so "вывести в порт" stays a lesson.
    let names_screen = !text.contains("порт")
        && (text.contains("диспле")
            || text.contains("экран")
            || text.contains("табло")
            || text.contains("lcd")
            || text.contains("oled")
            || text.contains("display"));
    // A named relay, buzzer or screen is itself a request to act:
    // "влажность выше 60, реле".
    let explicit_sink = text.contains("реле")
        || text.contains("relay")
        || text.contains("пищал")
        || text.contains("сирен")
        || text.contains("зуммер")
        || text.contains("buzzer")
        || text.contains("siren")
        || names_screen;
    let asks_action = matches!(
        action,
        Some(Action::Blink | Action::Alarm | Action::Toggle)
    );
    // A named output plus a spoken threshold is a request too: "цвет выше
    // 300, светодиод" has no verb, but it is plainly a lamp on a condition.
    let asks_condition = named_load && condition.is_some();
    if !asks_action && !explicit_sink && !asks_condition {
        return None;
    }
    // A sensor named with an everyday word ("температура", "давление") is
    // found through the alias table; a load word in the same sentence must
    // not hide the sensor that the sentence is about.
    // The book lesson is a strong hint, but it can be about the sink: in
    // "газ выше 400, реле" the relay lesson wins the vote. So fall back to
    // the source whose own keywords match the sentence best.
    let source = lesson
        .and_then(source_for_lesson)
        .or_else(|| source_in(&text))?;
    // A window belongs to one reading. If the sentence names a second
    // sensor, its threshold is not this sensor's: refuse rather than show a
    // range that mixes two boards.
    if matches!(condition, Some(Condition::Between { .. })) {
        let names_other = SOURCES.iter().any(|s| {
            s.id != source.id
                && s.keys.iter().any(|k| crate::words::mentions(&text, k))
        });
        if names_other {
            return None;
        }
    }
    let sink = if text.contains("реле") || text.contains("relay") {
        SinkId::Relay
    } else if text.contains("пищал")
        || text.contains("сирен")
        || text.contains("зуммер")
        || text.contains("buzzer")
        || text.contains("siren")
    {
        SinkId::Buzzer
    } else if text.contains("серво") || text.contains("servo") {
        SinkId::Servo
    } else if text.contains("мотор")
        || text.contains("motor")
        || text.contains("двигател")
        || text.contains("вентилятор")
        || text.contains("насос")
    {
        SinkId::Motor
    } else if text.contains("rgb")
        || text.contains("трехцвет")
        || text.contains("цветной светодиод")
    {
        SinkId::Rgb
    } else if names_screen {
        // A character display is the plainer readout: it is what "дисплей"
        // means by default, and the one the lessons put first. OLED has to be
        // asked for by name.
        if text.contains("2004") {
            SinkId::Lcd2004
        } else if text.contains("oled") || text.contains("графическ") {
            SinkId::Oled
        } else {
            SinkId::Lcd
        }
    } else {
        match load {
            Load::Led => SinkId::Led,
            Load::Buzzer => SinkId::Buzzer,
            _ => return None,
        }
    };
    let behavior = match sink {
        SinkId::Led => match action {
            Some(Action::Toggle) => Behavior::Solid,
            _ => Behavior::Blink,
        },
        SinkId::Buzzer => Behavior::Beep,
        SinkId::Relay => Behavior::Switch,
        SinkId::Lcd | SinkId::Lcd2004 | SinkId::Oled => Behavior::Show,
        SinkId::Servo => Behavior::Turn,
        SinkId::Motor => Behavior::Spin,
        SinkId::Rgb => Behavior::Color,
    };
    Some(Composition {
        source: source.id,
        sink,
        behavior,
        condition,
    })
}

/// The words "Понял так" shows for a composition: source · condition · action.
pub fn recipe_words(c: Composition) -> Vec<L> {
    let source = source(c.source);
    // A screen shows the reading itself, so a threshold would be a lie: the
    // recipe names the board instead of a condition.
    if c.behavior == Behavior::Show {
        return vec![source.short, sink(c.sink).title];
    }
    let condition = c.condition.unwrap_or_else(|| {
        source
            .variants
            .first()
            .map(|v| v.condition)
            .unwrap_or(Condition::Active)
    });
    vec![source.short, condition.label(), c.behavior.label()]
}

/// The declared source whose keywords best match the sentence.
fn source_in(text: &str) -> Option<&'static SourceSpec> {
    let mut best: Option<((usize, usize), &'static SourceSpec)> = None;
    for s in SOURCES {
        let mut hits = 0;
        let mut longest = 0;
        for key in s.keys {
            if crate::words::mentions(text, key) {
                hits += 1;
                longest = longest.max(key.len());
            }
        }
        if hits == 0 {
            continue;
        }
        if best.map(|(score, _)| (hits, longest) > score).unwrap_or(true) {
            best = Some(((hits, longest), s));
        }
    }
    best.map(|(_, s)| s)
}

struct Resolved<'a> {
    source: &'a SourceSpec,
    variant: &'a Variant,
    condition: Condition,
    sink: SinkId,
    /// The Uno pins the sink lands on. A single free pin for a part, or the
    /// two wires of the I2C bus, which a source may already be on.
    sink_pins: Vec<&'static str>,
    behavior: Behavior,
    /// The number the sentence gave, if it gave one: a comparator variant has
    /// to say that the number does not reach its sketch.
    spoken: Option<Condition>,
}

fn resolve(c: &Composition, variant: usize) -> Option<Resolved<'static>> {
    let source = source(c.source);
    let v = source.variants.get(variant)?;
    let condition = match v.logic {
        Logic::Flag => Condition::Active,
        Logic::Number => c.condition.unwrap_or(v.condition),
    };
    let sink = sink(c.sink);
    let sink_pins = match sink.wiring {
        // A part cannot land on a pin the source already uses. The first free
        // candidate wins, which keeps the drawing stable between runs.
        SinkWiring::Pin => {
            let free: Vec<&'static str> = sink
                .pins
                .iter()
                .copied()
                .filter(|p| !v.pins.contains(p))
                .take(sink.take)
                .collect();
            if free.len() < sink.take {
                return None;
            }
            free
        }
        // The bus is shared on purpose: an I2C source and an I2C screen are
        // two devices on the same two wires.
        SinkWiring::I2c => sink.pins.to_vec(),
    };
    Some(Resolved {
        source,
        variant: v,
        condition,
        sink: c.sink,
        sink_pins,
        behavior: c.behavior,
        spoken: c.condition,
    })
}

/// Whether any of the source's ways of being read leaves a free pin for the
/// sink. A false here is a real refusal, not something to paper over.
pub fn fits(c: Composition) -> bool {
    (0..source(c.source).variants.len()).any(|i| resolve(&c, i).is_some())
}

fn action_phrase(sink: SinkId, behavior: Behavior, lang: Lang) -> &'static str {
    match (sink, behavior) {
        (SinkId::Led, Behavior::Blink) => {
            tr(lang, "мигающий светодиод", "a blinking LED")
        }
        (SinkId::Led, _) => tr(lang, "светодиод по условию", "an LED on condition"),
        (SinkId::Buzzer, _) => tr(lang, "пищалка по порогу", "a buzzer on threshold"),
        (SinkId::Relay, _) => tr(lang, "реле с нагрузкой", "a relay with a load"),
        (SinkId::Lcd, _) => tr(lang, "вывод на LCD1602", "an LCD1602 readout"),
        (SinkId::Lcd2004, _) => tr(lang, "вывод на LCD2004", "an LCD2004 readout"),
        (SinkId::Oled, _) => tr(lang, "вывод на OLED", "an OLED readout"),
        (SinkId::Servo, _) => tr(lang, "серво по условию", "a servo on condition"),
        (SinkId::Motor, _) => tr(lang, "мотор по условию", "a motor on condition"),
        (SinkId::Rgb, _) => tr(lang, "цвет по условию", "a colour on condition"),
    }
}

fn level_name(n: u8, lang: Lang) -> String {
    match (n, lang) {
        (1, Lang::Ru) => "низкая".to_string(),
        (1, Lang::En) => "low".to_string(),
        (2, Lang::Ru) => "средняя".to_string(),
        (2, Lang::En) => "medium".to_string(),
        (_, Lang::Ru) => "высокая".to_string(),
        (_, Lang::En) => "high".to_string(),
    }
}

fn number(v: f32, decimals: u8) -> String {
    if decimals == 0 {
        format!("{:.0}", v)
    } else {
        format!("{:.*}", decimals as usize, v)
    }
}

fn choice(r: &Resolved, lang: Lang) -> Choice {
    let mut cons = Vec::new();
    if r.behavior != Behavior::Show && r.spoken.is_some() && r.variant.logic == Logic::Flag {
        cons.push(L::new(
            "Число из фразы не попадает в скетч: этот порог задаёт подстроечник на модуле",
            "The number from the sentence does not reach the sketch: the trimmer on the module sets this threshold",
        ));
    }
    let current = format!(
        "{} мА",
        r.variant_current() + sink(r.sink).current_ma
    );
    let effort = templates::spec_line(
        &level_name(r.variant.level, lang),
        L::new("Сложность", "Effort"),
    );
    let (pros, specs) = if r.behavior == Behavior::Show {
        (
            vec![L::new(
                "Значение видно на экране, порт остаётся свободным",
                "The value shows on the screen and the port stays free",
            )],
            vec![
                templates::spec_line("I2C: A4, A5", L::new("Шина", "Bus")),
                templates::spec_line(sink(r.sink).title.get(lang), L::new("Экран", "Screen")),
                templates::spec_line(&current, L::new("Ток", "Current")),
                effort,
            ],
        )
    } else {
        let decimals = r.variant.decimals;
        let unit = r.variant.unit.get(lang);
        let threshold = match r.condition {
            Condition::Above(v) | Condition::Below(v) => {
                format!("{} {}", number(v, decimals), unit)
            }
            Condition::AboveBand { on, off } | Condition::BelowBand { on, off } => {
                format!(
                    "{} → {} {}",
                    number(on, decimals),
                    number(off, decimals),
                    unit
                )
            }
            Condition::Between { low, high } => format!(
                "{}…{} {}",
                number(low, decimals),
                number(high, decimals),
                unit
            ),
            Condition::Active => unit.to_string(),
        };
        (
            vec![
                L::new(
                    "Значение видно в мониторе порта",
                    "The value goes to the serial monitor",
                ),
                r.condition.label(),
            ],
            vec![
                templates::spec_line(&threshold, L::new("Порог", "Threshold")),
                templates::spec_line(&r.sink_pins.join(", "), L::new("Выход", "Output")),
                templates::spec_line(&current, L::new("Ток", "Current")),
                effort,
            ],
        )
    };
    Choice {
        name: r.variant.name,
        blurb: r.variant.blurb,
        pros,
        cons,
        specs,
        level: r.variant.level,
    }
}

impl Resolved<'_> {
    fn variant_current(&self) -> u16 {
        match self.source.place {
            Place::Board { name, .. } => crate::modules::find(name).map(|m| m.current_ma).unwrap_or(0),
            _ => 0,
        }
    }
}

pub fn plan(c: Composition, lang: Lang) -> Option<Plan> {
    let source = source(c.source);
    let mut choices = Vec::new();
    for i in 0..source.variants.len() {
        if let Some(r) = resolve(&c, i) {
            choices.push(choice(&r, lang));
        }
    }
    if choices.is_empty() {
        return None;
    }
    let headline = format!(
        "{}: {}",
        source.title.get(lang),
        action_phrase(c.sink, c.behavior, lang)
    );
    Some(Plan {
        tid: Tid::Composed,
        title: L::new("Схема по описанию", "A composed circuit"),
        headline: Some(headline),
        choices,
        recommended: 0,
    })
}

pub fn build(c: Composition, choice: usize, lang: Lang) -> Option<Built> {
    let plan = plan(c, lang)?;
    let choice = choice.min(plan.choices.len() - 1);
    let r = resolve(&c, choice)?;
    let circuit = circuit(&r, lang)?;
    let code = sketch(&r, lang);
    let mut notes = vec![r.variant.note];
    notes.push(match r.sink {
        SinkId::Led => L::new(
            "Светодиод через 220 Ом: ток канала около 13 мА.",
            "The LED has a 220 Ω resistor: about 13 mA per channel.",
        ),
        SinkId::Buzzer => L::new(
            "Пьезо без генератора: частоту задаёт tone().",
            "The piezo has no oscillator: tone() sets the frequency.",
        ),
        SinkId::Relay => L::new(
            "Модуль реле активен низким уровнем; контакты COM и NO развязаны.",
            "The relay module is active low; COM and NO are isolated contacts.",
        ),
        SinkId::Lcd => L::new(
            "Экран на шине I2C: SDA на A4, SCL на A5, адрес 0x27 проверь сканером шины.",
            "The screen sits on I2C: SDA on A4, SCL on A5; check the 0x27 address with a bus scanner.",
        ),
        SinkId::Lcd2004 => L::new(
            "Экран 20x4 на шине I2C: SDA на A4, SCL на A5, адрес 0x27.",
            "The 20x4 screen sits on I2C: SDA on A4, SCL on A5, address 0x27.",
        ),
        SinkId::Oled => L::new(
            "OLED на шине I2C: SDA на A4, SCL на A5; адрес бывает 0x3C или 0x3D.",
            "The OLED sits on I2C: SDA on A4, SCL on A5; its address is 0x3C or 0x3D.",
        ),
        SinkId::Servo => L::new(
            "Серво поворачивается в 90° по условию и возвращается в 0. SG90 питается от 5 В.",
            "The servo turns to 90° while the condition holds and back to 0. An SG90 runs on 5 V.",
        ),
        SinkId::Motor => L::new(
            "Мотор включает MOSFET низкой стороны; диод гасит выброс обмотки.",
            "A low-side MOSFET switches the motor; the diode absorbs the coil's kickback.",
        ),
        SinkId::Rgb => L::new(
            "RGB-светодиод: красный и зелёный с общим катодом, по резистору на канал.",
            "The RGB LED has red and green channels and a common cathode, one resistor each.",
        ),
    });
    // A screen has no threshold to tune: the reading goes out as it is.
    if r.behavior != Behavior::Show {
        notes.push(match r.condition {
            Condition::Above(_) | Condition::Below(_) => L::new(
                "Порог можно подобрать по выводу в мониторе порта.",
                "Tune the threshold from the serial monitor.",
            ),
            Condition::AboveBand { .. } | Condition::BelowBand { .. } => L::new(
                "Гистерезис не даёт дребезжать у порога: вход на одном значении, выход на другом.",
                "Hysteresis stops chatter at the threshold: it comes on at one value and off at another.",
            ),
            Condition::Between { .. } => L::new(
                "Оба условия проверяются на одном чтении: срабатывание только внутри диапазона.",
                "Both conditions read the same sample: it fires only inside the range.",
            ),
            Condition::Active => L::new(
                "Условие — уровень с модуля: подстроечник задаёт его на месте.",
                "The condition is the module's level; its trimmer sets it on the spot.",
            ),
        });
    }
    Some(Built {
        tid: Tid::Composed,
        title: plan.title_text(lang),
        circuit,
        code,
        choice,
        summary: r.source.summary,
        notes,
    })
}

fn circuit(r: &Resolved, lang: Lang) -> Option<Circuit> {
    let mut b = B::new(lang);
    place_source(&mut b, r.source, r.variant)?;
    place_sink(&mut b, r)?;
    templates::feeds(&mut b);
    Some(b.finish())
}

/// Drop a ready-made board on the bench and wire its supply pins. Its signal
/// pins are left to the caller: a source and a screen bring their own.
fn place_board(b: &mut B, label: &str, name: &str, at: (f32, f32)) -> usize {
    let part = b.breakout(label, name, Pt::new(at.0, at.1));
    if let Some(m) = crate::modules::find(name) {
        for (pin, dx, _) in m.pins.iter().copied() {
            match pin {
                "GND" => {
                    let g = b.gnd(at.0 + dx);
                    b.w(b.mp(part, pin), g, NetClass::Gnd);
                }
                "VCC" | "+5V" | "+12V" | "+" => {
                    if m.three_three {
                        b.w(b.uno_pin("3V3"), b.mp(part, pin), NetClass::Vcc);
                    } else {
                        let p = b.vcc(at.0 + dx);
                        b.w(b.mp(part, pin), p, NetClass::Vcc);
                    }
                }
                "3V3" => {
                    b.w(b.uno_pin("3V3"), b.mp(part, pin), NetClass::Vcc);
                }
                _ => {}
            }
        }
    }
    part
}

fn place_source(b: &mut B, source: &SourceSpec, v: &Variant) -> Option<()> {
    match source.place {
        Place::Board { name, at } => {
            let part = place_board(b, "U1", name, at);
            for (pin, target) in v.signals.iter().copied() {
                if b.c.parts[part]
                    .pins
                    .iter()
                    .any(|&p| b.c.pins[p].name == pin)
                {
                    let class = if target.starts_with('A') {
                        NetClass::Analog
                    } else {
                        NetClass::Signal
                    };
                    b.w(b.mp(part, pin), b.uno_pin(target), class);
                }
            }
        }
        Place::Ldr => {
            let (a, z) = b.axial(Kind::Ldr, "LDR1", "GL5528", 10, Row::C, 2);
            let (ra, rb) = b.res("R1", "10 кОм", 10, Row::E, 3);
            let vcc = b.vcc(8.0);
            let gnd = b.gnd(16.0);
            b.w(vcc, a, NetClass::Vcc);
            b.w(z, ra, NetClass::Analog);
            b.w(rb, gnd, NetClass::Gnd);
            b.w(z, b.uno_pin("A0"), NetClass::Analog);
        }
        Place::Thermistor => {
            let (a, z) = b.axial(Kind::Thermistor, "TH1", "NTC 10 кОм", 10, Row::C, 2);
            let (ra, rb) = b.res("R1", "10 кОм", 10, Row::E, 3);
            let vcc = b.vcc(8.0);
            let gnd = b.gnd(16.0);
            b.w(vcc, a, NetClass::Vcc);
            b.w(z, ra, NetClass::Analog);
            b.w(rb, gnd, NetClass::Gnd);
            b.w(z, b.uno_pin("A0"), NetClass::Analog);
        }
    }
    Some(())
}

fn place_sink(b: &mut B, r: &Resolved) -> Option<()> {
    let pin = r.sink_pins.first().copied()?;
    match r.sink {
        SinkId::Led => {
            let (ra, _rb) = b.res("R1", "220 Ом", 20, Row::A, 3);
            let (_a, k) = b.led("D1", "красный 5 мм", 23, Row::E);
            b.w(b.uno_pin(pin), ra, NetClass::Signal);
            b.w(k, b.gnd(52.0), NetClass::Gnd);
        }
        SinkId::Buzzer => {
            // A bench part talks to the Uno headers directly, so its power
            // does not travel across the breadboard.
            let z = b.bench(Kind::Buzzer, "HA1", "пьезо 5 В", Pt::new(50.0, 54.0));
            b.w(b.uno_pin(pin), z[1], NetClass::Signal);
            b.w(z[0], b.uno_pin("GND1"), NetClass::Gnd);
        }
        SinkId::Relay => {
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(44.0, 54.0));
            let siren = b.bench(Kind::Buzzer, "HA1", "сирена 5 В", Pt::new(84.0, 80.0));
            b.w(b.uno_pin(pin), rl[0], NetClass::Signal);
            b.w(rl[1], b.uno_pin("5V"), NetClass::Vcc);
            b.w(rl[2], b.uno_pin("GND1"), NetClass::Gnd);
            b.w(rl[3], b.uno_pin("5V"), NetClass::Vcc);
            b.w(rl[4], siren[1], NetClass::Signal);
            b.w(siren[0], b.uno_pin("GND1"), NetClass::Gnd);
        }
        SinkId::Servo => {
            let s = b.bench(Kind::Servo, "S1", "SG90", Pt::new(50.0, 54.0));
            b.w(b.uno_pin(pin), s[2], NetClass::Signal);
            b.w(s[1], b.uno_pin("5V"), NetClass::Vcc);
            b.w(s[0], b.uno_pin("GND1"), NetClass::Gnd);
        }
        SinkId::Motor => {
            // The lesson's low-side switch, moved to the free half of the
            // bench so the source board keeps its corner.
            let (ra, rb) = b.res("R3", "220 Ом", 21, Row::A, 3);
            let q = b.to92(Kind::Nmos, "Q1", "IRLZ44N", 25, Row::E, ["G", "S", "D"]);
            let (_r4a, r4b) = b.res("R4", "10 кОм", 25, Row::A, 3);
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(52.0, 54.0));
            let d = b.diode("D1", "1N4007", 52.0, 82.0);
            let vcc = b.vcc(82.0);
            b.w(b.uno_pin(pin), ra, NetClass::Signal);
            b.w(rb, q[0], NetClass::Signal);
            b.w(r4b, b.gnd(74.0), NetClass::Gnd);
            b.w(q[1], b.gnd(70.0), NetClass::Gnd);
            b.w(q[2], m[1], NetClass::Signal);
            b.w(m[0], vcc, NetClass::Vcc);
            b.w(d.1, m[0], NetClass::Vcc);
            b.w(d.0, m[1], NetClass::Signal);
        }
        SinkId::Rgb => {
            // Three channels, three resistors: the strip joins each resistor
            // to its own leg of the LED.
            let (ra, rb) = b.res("R3", "220 Ом", 20, Row::A, 3);
            let (ga, gb) = b.res("R4", "220 Ом", 21, Row::B, 3);
            let (ba, bb) = b.res("R5", "220 Ом", 22, Row::C, 3);
            let led = b.rgb("D1", "RGB общий катод", 26, Row::E);
            for (target, start) in r.sink_pins.iter().zip([ra, ga, ba]) {
                b.w(b.uno_pin(target), start, NetClass::Signal);
            }
            for (end, leg) in [(rb, led[0]), (gb, led[1]), (bb, led[2])] {
                b.w(end, leg, NetClass::Signal);
            }
            b.w(led[3], b.gnd(52.0), NetClass::Gnd);
        }
        SinkId::Lcd | SinkId::Lcd2004 | SinkId::Oled => {
            let spec = sink(r.sink);
            let name = spec.module?;
            let part = place_board(b, "DS1", name, spec.at);
            // The supply is already on the rails; the bus lines go to the
            // shared A4/A5 pair, where the source may already sit.
            let sda = *r.sink_pins.first()?;
            let scl = *r.sink_pins.get(1)?;
            for (pin_name, target) in [("SDA", sda), ("SCL", scl)] {
                if b.c.parts[part]
                    .pins
                    .iter()
                    .any(|&p| b.c.pins[p].name == pin_name)
                {
                    b.w(b.mp(part, pin_name), b.uno_pin(target), NetClass::Signal);
                }
            }
        }
    }
    Some(())
}

/// Join two sketch fragments, keeping a line only once: an I2C source and an
/// I2C screen both ask for `#include <Wire.h>`.
fn join_unique(a: &str, b: &str) -> String {
    let mut out = String::new();
    for line in a.lines().chain(b.lines()) {
        if line.is_empty() || out.lines().any(|seen| seen == line) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn sketch(r: &Resolved, lang: Lang) -> String {
    let spec = sink(r.sink);
    let show = r.behavior == Behavior::Show;
    let (extra, active, action) = if show {
        // A screen takes the reading as it is: there is nothing to compare
        // and no output pin to drive.
        (String::new(), String::new(), spec.show.to_string())
    } else {
        let decimals = r.variant.decimals;
        let (persist, active) = match r.condition {
            Condition::Above(v) => (
                String::new(),
                format!("  bool active = value > {};\n", number(v, decimals)),
            ),
            Condition::Below(v) => (
                String::new(),
                format!("  bool active = value < {};\n", number(v, decimals)),
            ),
            Condition::Active => (String::new(), "  bool active = value != 0;\n".to_string()),
            // A band keeps its state between polls, so `active` lives next to
            // `last` instead of being recomputed from one reading.
            Condition::AboveBand { on, off } => (
                "bool active = false;\n".to_string(),
                format!(
                    "  if (!active && value > {}) active = true;\n  if (active && value < {}) active = false;\n",
                    number(on, decimals),
                    number(off, decimals)
                ),
            ),
            Condition::BelowBand { on, off } => (
                "bool active = false;\n".to_string(),
                format!(
                    "  if (!active && value < {}) active = true;\n  if (active && value > {}) active = false;\n",
                    number(on, decimals),
                    number(off, decimals)
                ),
            ),
            Condition::Between { low, high } => (
                String::new(),
                format!(
                    "  bool active = value > {} && value < {};\n",
                    number(low, decimals),
                    number(high, decimals)
                ),
            ),
        };
        let (extra, action) = match (r.sink, r.behavior) {
            (SinkId::Led, Behavior::Blink) => (
                "bool lit = false;\n",
                "  if (active) {\n    lit = !lit;\n    digitalWrite(SINK_PIN, lit ? HIGH : LOW);\n  } else {\n    lit = false;\n    digitalWrite(SINK_PIN, LOW);\n  }\n",
            ),
            (SinkId::Led, _) => (
                "",
                "  digitalWrite(SINK_PIN, active ? HIGH : LOW);\n",
            ),
            (SinkId::Buzzer, _) => (
                "",
                "  if (active) {\n    tone(SINK_PIN, 1200);\n  } else {\n    noTone(SINK_PIN);\n  }\n",
            ),
            (SinkId::Relay, _) => (
                "",
                "  digitalWrite(SINK_PIN, active ? LOW : HIGH);\n",
            ),
            (SinkId::Servo, _) => ("", "  arm.write(active ? 90 : 0);\n"),
            (SinkId::Motor, _) => ("", "  analogWrite(SINK_PIN, active ? 255 : 0);\n"),
            (SinkId::Rgb, _) => (
                "",
                "  digitalWrite(R_PIN, active ? HIGH : LOW);\n  digitalWrite(G_PIN, active ? LOW : HIGH);\n  digitalWrite(B_PIN, active ? LOW : HIGH);\n",
            ),
            // Screens are handled by the branch above; this keeps the match
            // exhaustive without pretending a display has a threshold.
            (SinkId::Lcd | SinkId::Lcd2004 | SinkId::Oled, _) => ("", ""),
        };
        (format!("{persist}{extra}"), active, action.to_string())
    };
    let (pins, output) = if show {
        (String::new(), String::new())
    } else if r.sink == SinkId::Rgb {
        let numbers: Vec<&str> = r
            .sink_pins
            .iter()
            .map(|p| p.trim_start_matches('D'))
            .collect();
        (
            format!(
                "const uint8_t R_PIN = {};\nconst uint8_t G_PIN = {};\nconst uint8_t B_PIN = {};\n",
                numbers[0], numbers[1], numbers[2]
            ),
            "  pinMode(R_PIN, OUTPUT);\n  pinMode(G_PIN, OUTPUT);\n  pinMode(B_PIN, OUTPUT);\n"
                .to_string(),
        )
    } else {
        // The servo library claims its pin in attach(), so a pinMode here
        // would only fight it.
        let output = if r.sink == SinkId::Servo {
            String::new()
        } else {
            "  pinMode(SINK_PIN, OUTPUT);\n".to_string()
        };
        (
            format!(
                "const uint8_t SINK_PIN = {};\n",
                r.sink_pins[0].trim_start_matches('D')
            ),
            output,
        )
    };
    let includes = join_unique(r.variant.includes, spec.includes);
    let globals = join_unique(r.variant.globals, spec.globals);
    let setup = format!("{}{}", r.variant.setup, spec.setup);
    let title = format!(
        "{}: {}",
        r.source.title.get(lang),
        action_phrase(r.sink, r.behavior, lang)
    );
    let mut s = String::from(
        r#"// <<C0>>
<<INCLUDES>><<GLOBALS>><<PINS>>const uint16_t PERIOD_MS = <<PERIOD>>;
uint32_t last = 0;
<<EXTRA>>
void setup() {
<<SETUP>><<SETUP_PIN>>  Serial.begin(9600);
}

void loop() {
  if (millis() - last < PERIOD_MS) return;
  last = millis();
<<SAMPLE>>
<<ACTIVE>>
<<ACTION>>
<<PRINT>>
}
"#,
    );
    for (k, v) in [
        ("C0", title.as_str()),
        ("INCLUDES", includes.as_str()),
        ("GLOBALS", globals.as_str()),
        ("PINS", pins.as_str()),
        ("EXTRA", extra.as_str()),
        ("SETUP", setup.as_str()),
        ("SETUP_PIN", output.as_str()),
        ("SAMPLE", r.variant.sample),
        ("ACTIVE", active.as_str()),
        ("ACTION", action.as_str()),
        ("PRINT", r.variant.print),
    ] {
        s = s.replace(&format!("<<{k}>>"), v);
    }
    s = s.replace("<<PERIOD>>", &r.source.period_ms.to_string());
    s = s.replace("<<NAME>>", r.source.short.get(lang));
    s = s.replace("<<DECIMALS>>", &r.variant.decimals.to_string());
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn unfilled(code: &str) -> bool {
        let mut rest = code;
        while let Some(i) = rest.find("<<") {
            let tail = &rest[i + 2..];
            let Some(j) = tail.find(">>") else { return false };
            let name = &tail[..j];
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            {
                return true;
            }
            rest = &tail[j + 2..];
        }
        false
    }

    /// The sentence from the brief is now an ordinary composition: MQ-2, a
    /// threshold and a blinking LED. Both ways of setting the threshold build
    /// and pass every check.
    #[test]
    fn the_gas_alarm_is_a_composition() {
        let p = parse("датчик газа который мигает когда много газа");
        let recipe = p.intent.recipe.expect("the sentence composes");
        assert_eq!(recipe.source, SourceId::Gas);
        assert_eq!(recipe.sink, SinkId::Led);
        assert_eq!(recipe.behavior, Behavior::Blink);
        let plan = plan(recipe, Lang::Ru).expect("the alarm plans");
        assert_eq!(plan.tid, Tid::Composed);
        assert_eq!(plan.choices.len(), 2);
        assert!(plan.title_text(Lang::Ru).contains("MQ-2"));
        assert!(plan.title_text(Lang::Ru).contains("мигающ"));
        for choice in 0..plan.choices.len() {
            for lang in [Lang::Ru, Lang::En] {
                let built = build(recipe, choice, lang).expect("builds");
                assert!(!unfilled(&built.code), "choice {choice}: unfilled token");
                assert!(built.circuit.parts.iter().any(|q| q.value == "MQ-2"));
                assert!(built.circuit.parts.iter().any(|q| q.kind == Kind::Led));
                let report = crate::checks::run(&built, lang);
                assert_eq!(
                    report.failed(),
                    0,
                    "choice {choice} ({}): {:?}",
                    lang.code(),
                    report
                        .checks
                        .iter()
                        .filter(|c| c.sev == crate::checks::Sev::Fail)
                        .map(|c| c.name.get(lang))
                        .collect::<Vec<_>>()
                );
            }
        }
        let analog = build(recipe, 0, Lang::Ru).unwrap();
        let digital = build(recipe, 1, Lang::Ru).unwrap();
        assert!(analog.code.contains("analogRead"));
        assert!(digital.code.contains("digitalRead"));
    }

    /// The matrix is the contract: every declared source, read its way, with
    /// every sink, must build and pass the checks in both languages.
    #[test]
    fn every_declared_combination_builds_and_passes() {
        let mut checked = 0;
        for source in SOURCES {
            for variant in 0..source.variants.len() {
                for (sink, behavior) in [
                    (SinkId::Led, Behavior::Blink),
                    (SinkId::Led, Behavior::Solid),
                    (SinkId::Buzzer, Behavior::Beep),
                    (SinkId::Relay, Behavior::Switch),
                    (SinkId::Lcd, Behavior::Show),
                    (SinkId::Lcd2004, Behavior::Show),
                    (SinkId::Oled, Behavior::Show),
                    (SinkId::Servo, Behavior::Turn),
                    (SinkId::Motor, Behavior::Spin),
                    (SinkId::Rgb, Behavior::Color),
                ] {
                    let c = Composition {
                        source: source.id,
                        sink,
                        behavior,
                        condition: None,
                    };
                    let plan = plan(c, Lang::Ru)
                        .unwrap_or_else(|| panic!("{}: {sink:?} plans", source.title.ru));
                    for choice in 0..plan.choices.len() {
                        for lang in [Lang::Ru, Lang::En] {
                            let built = build(c, choice, lang).unwrap_or_else(|| {
                                panic!("{}: {sink:?} builds", source.title.ru)
                            });
                            assert!(
                                !unfilled(&built.code),
                                "{}: unfilled token in {}",
                                source.title.ru,
                                lang.code()
                            );
                            assert!(
                                !built.circuit.parts.is_empty(),
                                "{}: empty circuit",
                                source.title.ru
                            );
                            let report = crate::checks::run(&built, lang);
                            let fails: Vec<String> = report
                                .checks
                                .iter()
                                .filter(|c| c.sev == crate::checks::Sev::Fail)
                                .map(|c| format!("{}: {}", c.name.get(lang), c.detail))
                                .collect();
                            assert!(
                                fails.is_empty(),
                                "{} / {sink:?} / {}: {}",
                                source.title.ru,
                                lang.code(),
                                fails.join("; ")
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked >= 80, "the matrix shrank to {checked} builds");
    }

    /// A source pin is never reused by the sink: the ranger's D9/D10 forces
    /// the LED onto another pin.
    #[test]
    fn the_sink_avoids_the_source_pins() {
        let c = Composition {
            source: SourceId::Range,
            sink: SinkId::Led,
            behavior: Behavior::Blink,
            condition: None,
        };
        let built = build(c, 0, Lang::Ru).unwrap();
        let uses = |pin: &str| {
            built
                .circuit
                .parts
                .iter()
                .find(|p| p.kind == Kind::Arduino)
                .map(|p| {
                    p.pins
                        .iter()
                        .any(|&i| built.circuit.pins[i].name == pin)
                })
                .unwrap_or(false)
        };
        assert!(uses("D9"));
        assert!(!built.code.contains("const uint8_t SINK_PIN = 9;"));
    }

    /// The sentence threshold is used, not the default.
    #[test]
    fn a_spoken_threshold_wins_over_the_default() {
        let p = parse("датчик газа мигает когда газ выше 700");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(recipe.condition, Some(Condition::Above(700.0)));
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("value > 700"));
        assert!(!built.code.contains("value > 400"));
    }

    /// A slow sensor keeps its own sampling period: the DS18B20 conversion
    /// takes 750 ms and the DHT dislikes faster than 2 s polls.
    #[test]
    fn each_source_keeps_its_sampling_period() {
        for source in SOURCES {
            let c = Composition {
                source: source.id,
                sink: SinkId::Led,
                behavior: Behavior::Solid,
                condition: None,
            };
            let built = build(c, 0, Lang::Ru).unwrap();
            assert!(
                built
                    .code
                    .contains(&format!("const uint16_t PERIOD_MS = {};", source.period_ms)),
                "{}: period not {} ms\n{}",
                source.title.ru,
                source.period_ms,
                built.code
            );
        }
    }

    /// A decimal threshold written with a comma still reads as a number.
    #[test]
    fn a_comma_threshold_is_a_number() {
        let p = parse("ток выше 1,5 ампер реле");
        let recipe = p.intent.recipe.expect("the current sensor composes");
        assert_eq!(recipe.source, SourceId::Current);
        assert_eq!(recipe.sink, SinkId::Relay);
        assert_eq!(recipe.condition, Some(Condition::Above(1.5)));
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("value > 1.50"), "{}", built.code);
    }

    /// A screen is a sink too: "температура на дисплее" composes an LCD, the
    /// sketch prints the reading, and there is no output pin to drive.
    #[test]
    fn a_screen_shows_the_reading() {
        let p = parse("температура на дисплее");
        let recipe = p.intent.recipe.expect("the sentence composes");
        assert_eq!(recipe.sink, SinkId::Lcd);
        assert_eq!(recipe.behavior, Behavior::Show);
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("LiquidCrystal_I2C lcd"));
        assert!(built.code.contains("lcd.print(value, 1)"));
        assert!(!built.code.contains("SINK_PIN"));
        assert!(!built.code.contains("bool active"));
        assert!(built.circuit.parts.iter().any(|q| q.value == "LCD1602"));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// An OLED has to be named: a bare "дисплей" means the plain LCD.
    #[test]
    fn an_oled_is_named() {
        let p = parse("давление покажи на OLED");
        let recipe = p.intent.recipe.expect("the sentence composes");
        assert_eq!(recipe.source, SourceId::Pressure);
        assert_eq!(recipe.sink, SinkId::Oled);
        assert_eq!(recipe.behavior, Behavior::Show);
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("Adafruit_SSD1306 display"));
        assert!(!built.code.contains("SINK_PIN"));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// Two I2C boards share A4/A5: the gyro and the screen are devices on one
    /// bus, so the builder must not refuse the second one.
    #[test]
    fn an_i2c_screen_shares_the_bus_with_an_i2c_source() {
        let c = Composition {
            source: SourceId::Tilt,
            sink: SinkId::Oled,
            behavior: Behavior::Show,
            condition: None,
        };
        assert!(fits(c));
        let built = build(c, 0, Lang::Ru).unwrap();
        let uses = |pin: &str| {
            built
                .circuit
                .parts
                .iter()
                .find(|p| p.kind == Kind::Arduino)
                .map(|p| {
                    p.pins
                        .iter()
                        .any(|&i| built.circuit.pins[i].name == pin)
                })
                .unwrap_or(false)
        };
        assert!(uses("A4") && uses("A5"));
        assert_eq!(built.code.matches("#include <Wire.h>").count(), 1);
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// A servo turns to its working angle while the condition holds and
    /// returns to zero when it clears.
    #[test]
    fn a_servo_turns_on_the_condition() {
        let p = parse("расстояние меньше 20 серво");
        let recipe = p.intent.recipe.expect("the ranger composes");
        assert_eq!(recipe.sink, SinkId::Servo);
        assert_eq!(recipe.behavior, Behavior::Turn);
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("#include <Servo.h>"));
        assert!(built.code.contains("arm.attach(SINK_PIN)"));
        assert!(built.code.contains("arm.write(active ? 90 : 0)"));
        assert!(!built.code.contains("pinMode(SINK_PIN"));
        assert!(built.circuit.parts.iter().any(|q| q.kind == Kind::Servo));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// A motor is switched by a MOSFET with a gate resistor, a pulldown and a
    /// flyback diode — the checks insist on all three.
    #[test]
    fn a_motor_gets_a_switch_and_a_diode() {
        let p = parse("температура выше 30 мотор");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(recipe.sink, SinkId::Motor);
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("analogWrite(SINK_PIN, active ? 255 : 0)"));
        for kind in [Kind::Motor, Kind::Nmos, Kind::Diode, Kind::Resistor] {
            assert!(
                built.circuit.parts.iter().any(|q| q.kind == kind),
                "{kind:?} missing"
            );
        }
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// An RGB LED takes three free pins and shows one colour for the
    /// condition, another without it.
    #[test]
    fn an_rgb_led_takes_three_pins() {
        let p = parse("газ выше 400 rgb");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(recipe.sink, SinkId::Rgb);
        let built = build(recipe, 0, Lang::Ru).unwrap();
        for pin in ["R_PIN", "G_PIN", "B_PIN"] {
            assert!(built.code.contains(&format!("const uint8_t {pin}")), "{pin}");
        }
        assert!(built.code.contains("digitalWrite(R_PIN, active ? HIGH : LOW)"));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// A spoken hysteresis turns one threshold into two: the relay comes on
    /// at 400 and lets go only below 350, so it does not chatter.
    #[test]
    fn a_hysteresis_band_keeps_its_state() {
        let p = parse("газ выше 400 реле с гистерезисом 50");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(
            recipe.condition,
            Some(Condition::AboveBand { on: 400.0, off: 350.0 })
        );
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("bool active = false;"));
        assert!(built.code.contains("value > 400"));
        assert!(built.code.contains("value < 350"));
        assert!(!built.code.contains("bool active = value >"));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// The band works from the low side too: "ниже 20" comes on under 20 and
    /// clears only above 23.
    #[test]
    fn a_low_band_returns_above_the_threshold() {
        let p = parse("температура ниже 20 серво с гистерезисом 3");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(
            recipe.condition,
            Some(Condition::BelowBand { on: 20.0, off: 23.0 })
        );
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("value < 20"));
        assert!(built.code.contains("value > 23"));
    }

    /// Two thresholds joined by "и" are two conditions on one reading: the
    /// relay has to sit inside the window.
    #[test]
    fn two_conditions_make_a_window() {
        let p = parse("температура выше 20 и ниже 30 реле");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(
            recipe.condition,
            Some(Condition::Between { low: 20.0, high: 30.0 })
        );
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("value > 20.0 && value < 30.0"));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// A window belongs to one reading: two sensors in one sentence are not
    /// squeezed into one range, the sentence stays a lesson.
    #[test]
    fn a_window_with_a_second_sensor_is_refused() {
        let p = parse("температура выше 20 и влажность ниже 60 реле");
        assert!(p.intent.recipe.is_none());
        assert!(crate::templates::plan(&p.intent, Lang::Ru).is_some());
    }

    /// A request to print in the serial port is not a request for a screen.
    #[test]
    fn a_serial_port_is_not_a_display() {
        let p = parse("вывести температуру в порт");
        assert!(p.intent.recipe.is_none());
        assert!(crate::templates::plan(&p.intent, Lang::Ru).is_some());
    }

    #[test]
    fn a_measure_verb_stays_a_lesson() {
        let p = parse("измерить температуру");
        assert!(p.intent.recipe.is_none());
        assert!(crate::templates::plan(&p.intent, Lang::Ru).is_some());
    }
}
