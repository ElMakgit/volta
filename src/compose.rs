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
    lesson: Option<usize>,
    action: Option<Action>,
    condition: Option<Condition>,
) -> Option<Composition> {
    let text = crate::words::normalize(raw);
    // Only a sentence that asks the circuit to do something is composed.
    // "Измерь температуру" stays the lesson that teaches the sensor. A named
    // relay or buzzer is itself a request to act: "влажность выше 60, реле".
    let explicit_sink = text.contains("реле")
        || text.contains("relay")
        || text.contains("пищал")
        || text.contains("сирен")
        || text.contains("зуммер")
        || text.contains("buzzer")
        || text.contains("siren");
    let asks_action = matches!(
        action,
        Some(Action::Blink | Action::Alarm | Action::Toggle)
    );
    if !asks_action && !explicit_sink {
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
    let sink = if text.contains("реле") || text.contains("relay") {
        SinkId::Relay
    } else if text.contains("пищал")
        || text.contains("сирен")
        || text.contains("зуммер")
        || text.contains("buzzer")
        || text.contains("siren")
    {
        SinkId::Buzzer
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
    sink_pin: &'static str,
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
    // A sink cannot land on a pin the source already uses. The first free
    // candidate wins, which keeps the drawing stable between runs.
    let sink_pin = sink
        .pins
        .iter()
        .copied()
        .find(|p| !v.pins.contains(p))?;
    Some(Resolved {
        source,
        variant: v,
        condition,
        sink: c.sink,
        sink_pin,
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
    let threshold = match r.condition {
        Condition::Above(v) | Condition::Below(v) => {
            format!("{} {}", number(v, r.variant.decimals), r.variant.unit.get(lang))
        }
        Condition::Active => r.variant.unit.get(lang).to_string(),
    };
    let mut cons = Vec::new();
    if r.spoken.is_some() && r.variant.logic == Logic::Flag {
        cons.push(L::new(
            "Число из фразы не попадает в скетч: этот порог задаёт подстроечник на модуле",
            "The number from the sentence does not reach the sketch: the trimmer on the module sets this threshold",
        ));
    }
    Choice {
        name: r.variant.name,
        blurb: r.variant.blurb,
        pros: vec![
            L::new(
                "Значение видно в мониторе порта",
                "The value goes to the serial monitor",
            ),
            r.condition.label(),
        ],
        cons,
        specs: vec![
            templates::spec_line(&threshold, L::new("Порог", "Threshold")),
            templates::spec_line(r.sink_pin, L::new("Выход", "Output")),
            templates::spec_line(
                &format!(
                    "{} мА",
                    r.variant_current() + sink(r.sink).current_ma
                ),
                L::new("Ток", "Current"),
            ),
            templates::spec_line(&level_name(r.variant.level, lang), L::new("Сложность", "Effort")),
        ],
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
    });
    notes.push(match r.condition {
        Condition::Above(_) | Condition::Below(_) => L::new(
            "Порог можно подобрать по выводу в мониторе порта.",
            "Tune the threshold from the serial monitor.",
        ),
        Condition::Active => L::new(
            "Условие — уровень с модуля: подстроечник задаёт его на месте.",
            "The condition is the module's level; its trimmer sets it on the spot.",
        ),
    });
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
    place_sink(&mut b, r.sink, r.sink_pin)?;
    templates::feeds(&mut b);
    Some(b.finish())
}

fn place_source(b: &mut B, source: &SourceSpec, v: &Variant) -> Option<()> {
    match source.place {
        Place::Board { name, at } => {
            let part = b.breakout("U1", name, Pt::new(at.0, at.1));
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

fn place_sink(b: &mut B, sink: SinkId, pin: &'static str) -> Option<()> {
    match sink {
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
    }
    Some(())
}

fn sketch(r: &Resolved, lang: Lang) -> String {
    let threshold = match r.condition {
        Condition::Above(v) | Condition::Below(v) => number(v, r.variant.decimals),
        Condition::Active => "0".to_string(),
    };
    let active = match r.condition {
        Condition::Above(_) => format!("  bool active = value > {threshold};\n"),
        Condition::Below(_) => format!("  bool active = value < {threshold};\n"),
        Condition::Active => "  bool active = value != 0;\n".to_string(),
    };
    let (extra, action) = match (r.sink, r.behavior) {
        (SinkId::Led, Behavior::Blink) => (
            "bool lit = false;\n",
            "  if (active) {\n    lit = !lit;\n    digitalWrite(OUT, lit ? HIGH : LOW);\n  } else {\n    lit = false;\n    digitalWrite(OUT, LOW);\n  }\n",
        ),
        (SinkId::Led, _) => (
            "",
            "  digitalWrite(OUT, active ? HIGH : LOW);\n",
        ),
        (SinkId::Buzzer, _) => (
            "",
            "  if (active) {\n    tone(OUT, 1200);\n  } else {\n    noTone(OUT);\n  }\n",
        ),
        (SinkId::Relay, _) => (
            "",
            "  digitalWrite(OUT, active ? LOW : HIGH);\n",
        ),
    };
    let title = format!(
        "{}: {}",
        r.source.title.get(lang),
        action_phrase(r.sink, r.behavior, lang)
    );
    let mut s = String::from(
        r#"// <<C0>>
<<INCLUDES>><<GLOBALS>>
const uint8_t OUT = <<OUTNUM>>;
const uint16_t PERIOD_MS = <<PERIOD>>;
uint32_t last = 0;
<<EXTRA>>
void setup() {
<<SETUP>>  pinMode(OUT, OUTPUT);
  Serial.begin(9600);
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
        ("INCLUDES", r.variant.includes),
        ("GLOBALS", r.variant.globals),
        ("OUTNUM", r.sink_pin.trim_start_matches('D')),
        ("EXTRA", extra),
        ("SETUP", r.variant.setup),
        ("SAMPLE", r.variant.sample),
        ("ACTIVE", active.as_str()),
        ("ACTION", action),
        ("PRINT", r.variant.print),
    ] {
        s = s.replace(&format!("<<{k}>>"), v);
    }
    s = s.replace("<<PERIOD>>", &r.source.period_ms.to_string());
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
        assert!(!built.code.contains("const uint8_t OUT = 9;"));
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

    #[test]
    fn a_measure_verb_stays_a_lesson() {
        let p = parse("измерить температуру");
        assert!(p.intent.recipe.is_none());
        assert!(crate::templates::plan(&p.intent, Lang::Ru).is_some());
    }
}
