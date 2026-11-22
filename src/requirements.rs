//! Did the built circuit keep everything the sentence named?
//!
//! The parser chooses one shape, and a wrong shape used to look like a
//! finished build. This module reads the concrete parts out of the sentence
//! and compares them with the parts actually placed. A missing sensor or a
//! missing `struct` is then a visible failure, not a plausible picture.

use crate::checks::{Check, Sev};
use crate::model::{Circuit, End, Kind};
use crate::templates::Built;
use crate::text::{tr, Lang, L};

/// One thing the sentence asked for that the circuit does not have.
pub struct Missing {
    pub name: L,
    pub want: usize,
    pub got: usize,
    /// A value for the name, such as the spoken period in milliseconds.
    pub extra: Option<String>,
}

fn count_kind(c: &Circuit, kinds: &[Kind]) -> usize {
    c.parts
        .iter()
        .filter(|p| kinds.contains(&p.kind))
        .count()
}

fn module(c: &Circuit, name: &str) -> bool {
    c.parts
        .iter()
        .any(|p| p.kind == Kind::Module && p.value.eq_ignore_ascii_case(name))
}

fn count_module(c: &Circuit, name: &str) -> usize {
    c.parts
        .iter()
        .filter(|p| p.kind == Kind::Module && p.value.eq_ignore_ascii_case(name))
        .count()
}

fn count_module_starts(c: &Circuit, prefix: &str) -> usize {
    c.parts
        .iter()
        .filter(|p| p.kind == Kind::Module && p.value.to_uppercase().starts_with(prefix))
        .count()
}

fn module_starts(c: &Circuit, prefix: &str) -> bool {
    c.parts.iter().any(|p| {
        p.kind == Kind::Module && p.value.to_uppercase().starts_with(prefix)
    })
}

fn any_module(c: &Circuit, names: &[&str]) -> bool {
    names.iter().any(|n| module(c, n))
}

/// A screen is not only an LCD or an OLED. The checklist has to count the
/// board the sentence asked for: a MAX7219 matrix, a Nokia panel and a
/// seven-segment module are screens too, and an LCD must not satisfy
/// "матричный дисплей".
fn is_screen_value(value: &str, screens: crate::parse::Screens) -> bool {
    use crate::modules::Look;
    let look = crate::modules::find(value).map(|m| m.look);
    let screen = matches!(
        look,
        Some(Look::Lcd | Look::Display | Look::Segments | Look::Matrix | Look::Panel)
    );
    if !screen {
        return false;
    }
    if screens.matrix {
        return value.starts_with("MAX7219");
    }
    if screens.segments {
        return value.starts_with("7SEG") || value.starts_with("TM");
    }
    match screens.kind {
        crate::parse::ScreenKind::Oled => {
            value.starts_with("OLED") || value.starts_with("SSD1306")
        }
        crate::parse::ScreenKind::Lcd => value.starts_with("LCD1602"),
        crate::parse::ScreenKind::Lcd2004 => value.starts_with("LCD2004"),
        crate::parse::ScreenKind::Any => true,
    }
}

fn count_screens(c: &Circuit, screens: crate::parse::Screens) -> usize {
    c.parts
        .iter()
        .filter(|p| p.kind == Kind::Module && is_screen_value(&p.value, screens))
        .count()
}

/// Whether the named Uno pin is actually part of the drawing. A pin the
/// sentence asked for and the build ignored is a missing part, not a detail.
pub(crate) fn pin_wired(c: &Circuit, n: u8) -> bool {
    let Some(uno) = c.arduino else { return false };
    let name = format!("D{n}");
    let Some(pin) = c.parts[uno]
        .pins
        .iter()
        .copied()
        .find(|&p| c.pins[p].name == name)
    else {
        return false;
    };
    c.wires()
        .any(|w| w.a == End::Pin(pin) || w.b == End::Pin(pin))
}

/// The concrete parts the sentence names, reduced to the questions the build
/// can answer: a kind with a count, a named module, or a piece of code.
/// `intent.named` is the single extraction done by the parser; this module
/// only compares it with the circuit. A second set of keyword lists here
/// would be a second reading, free to disagree with the drawing.
pub fn missing(built: &Built, intent: &crate::parse::Intent) -> Vec<Missing> {
    let raw = intent.raw.as_str();
    if raw.trim().is_empty() {
        return Vec::new();
    }
    let named = &intent.named;
    let c = &built.circuit;
    let text = crate::words::normalize(raw);
    let mut out = Vec::new();

    let pins: Vec<u8> = if named.pins.is_empty() {
        named.pin.into_iter().collect()
    } else {
        named.pins.clone()
    };
    for pin in pins {
        if pin_wired(c, pin) || built.explained_pins.contains(&pin) {
            continue;
        }
        out.push(Missing {
            name: L::new("пин", "pin"),
            want: 1,
            got: 0,
            extra: Some(format!("D{pin}")),
        });
    }

    let mut need = |name: L, want: usize, got: usize| {
        if got < want {
            out.push(Missing {
                name,
                want,
                got,
                extra: None,
            });
        }
    };

    need(
        L::new("светодиод", "LED"),
        named.led,
        count_kind(c, &[Kind::Led]),
    );
    need(
        L::new("лампа накаливания", "incandescent lamp"),
        named.lamp,
        count_kind(c, &[Kind::Lamp]),
    );
    need(
        L::new("кнопка", "button"),
        named.button,
        count_kind(c, &[Kind::Button]),
    );
    need(
        L::new("пьезоизлучатель", "buzzer"),
        named.buzzer,
        count_kind(c, &[Kind::Buzzer, Kind::Speaker]),
    );
    need(
        L::new("реле", "relay"),
        named.relay,
        count_kind(c, &[Kind::Relay]),
    );
    need(
        L::new("сервопривод", "servo"),
        named.servo,
        count_kind(c, &[Kind::Servo]),
    );
    need(
        L::new("двигатель", "motor"),
        named.motor,
        count_kind(c, &[Kind::Motor]),
    );
    // A phone screen is not a display we wire: "вывести на экран сотового
    // телефона" is an SMS, and the GSM circuit is complete without an LCD.
    if let Some(screens) = named.screens {
        let got = count_screens(c, screens);
        if !named.phone {
            need(L::new("экран", "display"), screens.count, got);
        }
    }

    // A named board is a requirement of its own: "DHT22" must not silently
    // become another humidity sensor, and "FS1000A" must not disappear from
    // a radio pair. Counts matter too: "два DHT22" is two boards.
    if named.dht22 > 0 {
        need(
            L::new("DHT22", "DHT22"),
            named.dht22,
            count_module(c, "DHT22"),
        );
    }
    if named.dht11 > 0 {
        need(
            L::new("DHT11", "DHT11"),
            named.dht11,
            count_module(c, "DHT11"),
        );
    }
    if named.dht22 == 0 && named.dht11 == 0 && named.humidity > 0 {
        let got = count_module(c, "DHT11")
            + count_module(c, "DHT22")
            + count_module(c, "BME280");
        need(
            L::new("датчик влажности", "humidity sensor"),
            named.humidity,
            got,
        );
    }
    if named.temperature > 0 {
        let got = count_kind(c, &[Kind::Tmp36, Kind::Thermistor])
            + count_module(c, "DS18B20")
            + count_module(c, "DS3231")
            + count_module(c, "DHT11")
            + count_module(c, "DHT22")
            + count_module(c, "BMP180")
            + count_module(c, "BMP280")
            + count_module(c, "BME280");
        need(
            L::new("датчик температуры", "temperature sensor"),
            named.temperature,
            got,
        );
    }
    // A thermistor is its own part, not a temperature reading: the DS18B20
    // measures the same quantity through a different circuit, so it cannot
    // close a request that named the NTC.
    if crate::compose::source_in_text(&intent.raw)
        .is_some_and(|s| s.id == crate::spec::SourceId::Thermistor)
    {
        need(
            L::new("термистор", "thermistor"),
            1,
            count_kind(c, &[Kind::Thermistor]),
        );
    }
    if named.clock {
        // The sentence names a clock, not one particular chip: DS1302,
        // DS1307 and DS3231 are all real-time clocks, and the book has a
        // lesson for each.
        let got = ["DS3231", "DS1307", "DS1302"]
            .iter()
            .filter(|name| module(c, name))
            .count();
        need(
            L::new("часы реального времени", "real-time clock"),
            1,
            got,
        );
    }
    if named.soil > 0 {
        need(
            L::new("датчик почвы", "soil sensor"),
            named.soil,
            count_module(c, "SOIL"),
        );
    }
    if named.pressure > 0 {
        need(
            L::new("датчик давления", "pressure sensor"),
            named.pressure,
            count_module_starts(c, "BMP") + count_module_starts(c, "BME"),
        );
    }
    if named.light > 0 {
        need(
            L::new("датчик света", "light sensor"),
            named.light,
            count_kind(c, &[Kind::Ldr]),
        );
    }
    if named.range > 0 {
        need(
            L::new("дальномер", "distance sensor"),
            named.range,
            count_module(c, "HC-SR04"),
        );
    }
    if named.gas > 0 {
        need(
            L::new("датчик газа", "gas sensor"),
            named.gas,
            count_module_starts(c, "MQ-"),
        );
    }
    if named.fs1000a {
        need(
            L::new("FS1000A", "FS1000A"),
            1,
            any_module(c, &["FS1000A"]) as usize,
        );
    }
    if named.xy_mk {
        need(
            L::new("XY-MK-5V", "XY-MK-5V"),
            1,
            any_module(c, &["XY-MK-5V"]) as usize,
        );
    }
    if named.rc522 {
        need(
            L::new("RC522", "RC522"),
            1,
            any_module(c, &["RC522"]) as usize,
        );
    }
    if named.tsop {
        need(
            L::new("TSOP1838", "TSOP1838"),
            1,
            module_starts(c, "TSOP") as usize,
        );
    }
    if named.sd {
        need(
            L::new("SD-карта", "SD card"),
            1,
            any_module(c, &["SD"]) as usize,
        );
    }
    if named.segments > 0 {
        need(
            L::new("семисегментный индикатор", "seven-segment display"),
            named.segments,
            module_starts(c, "7SEG") as usize,
        );
    }
    if named.sound > 0 {
        need(
            L::new("датчик звука", "sound sensor"),
            named.sound,
            any_module(c, &["SOUND"]) as usize,
        );
    }

    // Two roles in the sentence are two roles in the sketch. A one-board
    // drawing is fine when the code really holds both halves, as the 433 MHz
    // link does with SENDER; a plan that quietly keeps only the transmitter
    // is not.
    if named.radio_pair {
        let boards = count_kind(c, &[Kind::Arduino]);
        let code_roles = built.code.contains("SENDER")
            && (built.code.contains("enableReceive") || built.code.contains("RX_PIN"));
        if boards < 2 && !code_roles {
            out.push(Missing {
                name: L::new("вторая плата (приёмник)", "the second board (receiver)"),
                want: 2,
                got: boards.max(1),
                extra: None,
            });
        }
    }

    // A spoken period has to reach the sketch, not just the plan text. A
    // calendar period ("каждый час", "каждый день") is handled by a clock,
    // not by a millisecond constant, so only short periods are checked.
    if let Some(ms) = crate::semantics::analyse(raw).period_ms {
        if ms > 0 && ms < 3_600_000 && !built.code.contains(&ms.to_string()) {
            out.push(Missing {
                name: L::new("период", "period"),
                want: 1,
                got: 0,
                extra: Some(format!("{ms} ms")),
            });
        }
    }

    // A struct is a property of the sketch, not a part.
    if text.contains("структур") && !built.code.contains("struct ") {
        out.push(Missing {
            name: L::new("структура в скетче", "a struct in the sketch"),
            want: 1,
            got: 0,
            extra: None,
        });
    }
    out
}

pub fn check(built: &Built, intent: &crate::parse::Intent, lang: Lang) -> Vec<Check> {
    if intent.raw.trim().is_empty() {
        return Vec::new();
    }
    let missing = missing(built, intent);
    let name = L::new("Требования", "Requirements");
    if missing.is_empty() {
        return vec![Check {
            name,
            sev: Sev::Pass,
            detail: tr(
                lang,
                "Все названные в задаче детали есть в схеме.",
                "Every part named in the task is in the circuit.",
            )
            .to_string(),
            focus: None,
        }];
    }
    let mut parts = Vec::new();
    for m in &missing {
        let name = match &m.extra {
            Some(x) => format!("{} {x}", m.name.get(lang)),
            None => m.name.get(lang).to_string(),
        };
        if m.got == 0 {
            parts.push(name);
        } else {
            parts.push(format!(
                "{} {} ({} {})",
                name,
                m.want,
                tr(lang, "есть", "found"),
                m.got
            ));
        }
    }
    vec![Check {
        name,
        sev: Sev::Fail,
        detail: format!(
            "{}: {}",
            tr(lang, "в схеме нет", "missing from the circuit"),
            parts.join(", ")
        ),
        focus: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ratchet floor from the data file, not a literal buried in a test.
    fn baseline(key: &str) -> usize {
        include_str!("testdata/book_baseline.txt")
            .lines()
            .filter_map(|line| line.split_once('='))
            .find_map(|(k, v)| {
                (k.trim() == key).then(|| {
                    v.trim()
                        .parse()
                        .unwrap_or_else(|_| panic!("baseline {key} is not a number"))
                })
            })
            .unwrap_or_else(|| panic!("baseline {key} is missing"))
    }
    use crate::templates;

    #[test]
    fn a_missing_named_part_is_reported() {
        let p = crate::parse::parse("Датчик температуры DHT22 и реле");
        let built = templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        let mut broken = built.clone();
        broken.circuit.parts.retain(|part| part.value != "DHT22");
        let miss = missing(&broken, &p.intent);
        assert!(miss.iter().any(|m| m.name.ru == "DHT22"));
    }

    #[test]
    fn a_struct_is_required_by_the_word() {
        let p = crate::parse::parse(
            "Составить программу для МК последовательно, через каждые 2 секунды отправлять значения номера передатчика, температуры и влажности на разных платах от 1 платы с передатчиком на 1 приёмник с по-мощью структуры.",
        );
        let built = templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        assert!(missing(&built, &p.intent).is_empty());
        let mut broken = built.clone();
        broken.code = "// nothing\n".to_string();
        assert!(missing(&broken, &p.intent)
            .iter()
            .any(|m| m.name.ru.contains("структура")));
    }

    #[test]
    fn sd_is_a_token_not_a_fragment() {
        assert!(crate::parse::named_devices("записать данные на sd карту").sd);
        assert!(crate::parse::named_devices("карта памяти").sd);
        assert!(!crate::parse::named_devices("использовать датчик").sd);
    }

    /// One sensor is not "two sensors": a count the task names must reach the
    /// circuit, not be rounded down to the first board the plan happens to use.
    #[test]
    fn a_named_sensor_count_is_not_satisfied_by_one() {
        let task = "два датчика температуры и экран";
        let p = crate::parse::parse(task);
        let built = templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        let miss = missing(&built, &p.intent);
        assert!(
            miss.iter()
                .any(|m| m.name.ru == "датчик температуры" && m.want == 2),
            "two temperature sensors must be a requirement: {miss:?}",
            miss = miss.iter().map(|m| (m.name.ru, m.want, m.got)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn two_named_boards_are_two() {
        let task = "два DHT22 и экран";
        let p = crate::parse::parse(task);
        let built = templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        let miss = missing(&built, &p.intent);
        assert!(
            miss.iter().any(|m| m.name.ru == "DHT22" && m.want == 2),
            "two DHT22 boards must be a requirement: {miss:?}",
            miss = miss.iter().map(|m| (m.name.ru, m.want, m.got)).collect::<Vec<_>>()
        );
    }

    /// Two boards and two generic sensors are both counts, and both must be
    /// visible: the old checklist saw one DHT22 and one reading and passed.
    #[test]
    fn two_boards_and_two_sensors_are_four_readings() {
        let task = "два DHT22 и два датчика температуры с выводом на экран";
        let p = crate::parse::parse(task);
        let built = templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        let miss = missing(&built, &p.intent);
        assert!(
            miss.iter().any(|m| m.name.ru == "DHT22" && m.want == 2),
            "two DHT22 must be required: {miss:?}",
            miss = miss.iter().map(|m| (m.name.ru, m.want, m.got)).collect::<Vec<_>>()
        );
        assert!(
            miss.iter()
                .any(|m| m.name.ru == "датчик температуры" && m.want == 2),
            "two temperature sensors must be required: {miss:?}",
            miss = miss.iter().map(|m| (m.name.ru, m.want, m.got)).collect::<Vec<_>>()
        );
    }

    /// The book's weather-station answer keeps the clock and a screen but
    /// drops the barometer. That used to pass because pressure was not on the
    /// checklist at all.
    #[test]
    fn pressure_does_not_disappear_silently() {
        let task = "Составить программу метеостанции с часами+влажность+температура+барометр с выводом на любой тип дисплея.";
        let p = crate::parse::parse(task);
        let built = templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        let miss = missing(&built, &p.intent);
        assert!(
            miss.iter().any(|m| m.name.ru == "датчик давления"),
            "the missing barometer must be reported: {miss:?}",
            miss = miss.iter().map(|m| m.name.ru).collect::<Vec<_>>()
        );
    }

    /// A thermometer drawn on a screen is not a temperature sensor: the task
    /// asks for a symbol, not a measurement.
    #[test]
    fn a_drawn_thermometer_needs_no_sensor() {
        let task = "Составить программу вывода на дисплей своих символов термометра, капли дождя.";
        let p = crate::parse::parse(task);
        let built = templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        let miss = missing(&built, &p.intent);
        assert!(
            !miss.iter().any(|m| m.name.ru == "датчик температуры"),
            "a thermometer symbol is not a sensor: {miss:?}",
            miss = miss.iter().map(|m| m.name.ru).collect::<Vec<_>>()
        );
    }

    /// The checklist is a ratchet over the book's own canned answers. The
    /// printed tasks are combinations (sensor + display + storage), while an
    /// answer is one fixed lesson, so some losses are structural; the number
    /// must never grow. Skips are printed too: a task that never built is a
    /// loss the old count did not include.
    #[test]
    #[ignore = "slow: full book corpus, run in CI with --include-ignored"]
    fn the_checklist_does_not_lose_more_parts_than_today() {
        let known = baseline("checklist_losing");
        let mut bad = 0;
        let mut skipped = 0;
        for (task, _) in crate::book_tasks::TASKS {
            let parsed = crate::parse::parse(task);
            if parsed.unsupported.is_some() {
                skipped += 1;
                continue;
            }
            let Some(plan) = crate::templates::plan(&parsed.intent, Lang::Ru) else {
                skipped += 1;
                continue;
            };
            let Some(built) = crate::templates::build(&parsed.intent, plan.recommended, Lang::Ru)
            else {
                skipped += 1;
                continue;
            };
            let miss = missing(&built, &parsed.intent);
            if !miss.is_empty() {
                bad += 1;
                let names: Vec<&str> = miss.iter().map(|m| m.name.ru).collect();
                eprintln!("{task} -> {}: {:?}", plan.title_text(Lang::Ru), names);
            }
        }
        eprintln!("book tasks with missing parts: {bad}; never built: {skipped}");
        assert!(
            bad <= known,
            "{bad} book tasks lose a named part, was {known}: a new silent drop?"
        );
    }

    /// The table answers a printed task by exact lookup, so the green book
    /// run says nothing about the parser. This test parses the same corpus
    /// without the table and prints what the parser alone can do: understood,
    /// refused, built, and how many builds lose a part the task named.
    #[test]
    #[ignore = "slow: full book corpus, run in CI with --include-ignored"]
    fn the_parser_alone_is_measured_against_the_book_corpus() {
        // The numbers live in `testdata/book_baseline.txt`, so raising a
        // floor is a visible data change, not a magic constant in a test.
        // The build floor fell from 574 to 567 on purpose: eight tasks that
        // name a screen yet route to an LED, button or buzzer template are
        // now refused instead of drawn as a look-alike. The losses are
        // honest: a sensor lesson with no matrix drawable for it, an SD or
        // RTC lesson that lacks the other part, a clock test built as a
        // serial readout.
        let known_understood = baseline("parser_understood");
        let known_recipe = baseline("parser_recipe");
        let known_built = baseline("parser_built");
        let known_losing = baseline("parser_losing");
        let mut refused = 0;
        let mut no_plan = 0;
        let mut no_build = 0;
        let mut understood = 0;
        let mut built_count = 0;
        let mut losing = 0;
        let mut recipe = 0;
        let mut lesson = 0;
        let mut screens = 0;
        let mut template = 0;
        for (task, _) in crate::book_tasks::TASKS {
            let parsed = crate::parse::parse_without_books(task);
            if parsed.unsupported.is_some() {
                refused += 1;
                continue;
            }
            let Some(plan) = crate::templates::plan(&parsed.intent, Lang::Ru) else {
                no_plan += 1;
                continue;
            };
            understood += 1;
            if parsed.intent.recipe.is_some() {
                recipe += 1;
            } else if parsed.intent.lesson.is_some() {
                lesson += 1;
            } else if parsed.intent.text_screens.is_some() {
                screens += 1;
            } else {
                template += 1;
            }
            let Some(built) = crate::templates::build(&parsed.intent, plan.recommended, Lang::Ru)
            else {
                no_build += 1;
                if std::env::var_os("VOLTA_PARSER_LOSSES").is_some() {
                    eprintln!("parser-only no build: {task} -> {}", plan.title_text(Lang::Ru));
                }
                continue;
            };
            built_count += 1;
            let miss = missing(&built, &parsed.intent);
            if !miss.is_empty() {
                losing += 1;
                if std::env::var_os("VOLTA_PARSER_LOSSES").is_some() {
                    let names: Vec<&str> = miss.iter().map(|m| m.name.ru).collect();
                    eprintln!("parser-only loss: {task} -> {names:?}");
                }
            }
        }
        eprintln!(
            "parser-only: {understood} of {} tasks understood (recipe {recipe}, lesson {lesson}, screens {screens}, template {template}); refused {refused}, no plan {no_plan}; built {built_count}, no build {no_build}, of those losing a named part {losing}",
            crate::book_tasks::TASKS.len()
        );
        assert!(
            understood >= known_understood,
            "parser-only understanding fell to {understood}, was {known_understood}"
        );
        assert!(
            recipe >= known_recipe,
            "parser-only compositions fell to {recipe}, was {known_recipe}"
        );
        assert!(
            built_count >= known_built,
            "parser-only builds fell to {built_count}, was {known_built}"
        );
        assert!(
            losing <= known_losing,
            "parser-only drops rose to {losing}, was {known_losing}"
        );
    }
}
