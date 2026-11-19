//! Did the built circuit keep everything the sentence named?
//!
//! The parser chooses one shape, and a wrong shape used to look like a
//! finished build. This module reads the concrete parts out of the sentence
//! and compares them with the parts actually placed. A missing sensor or a
//! missing `struct` is then a visible failure, not a plausible picture.

use crate::checks::{Check, Sev};
use crate::model::{Circuit, Kind};
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

fn module_starts(c: &Circuit, prefix: &str) -> bool {
    c.parts.iter().any(|p| {
        p.kind == Kind::Module && p.value.to_uppercase().starts_with(prefix)
    })
}

fn any_module(c: &Circuit, names: &[&str]) -> bool {
    names.iter().any(|n| module(c, n))
}

/// Everyday words for heat. "Температура" is also a printed value in a task
/// that only relays it, so the check accepts any board that measures it.
/// "Градус" is deliberately absent: in a servo task it is an angle.
fn names_temperature(text: &str) -> bool {
    ["температур", "термометр", "термодатч", "temperature", "thermometer"]
        .iter()
        .any(|k| text.contains(k))
}

/// Humidity, but not the soil probe: "влажность почвы" is a different part.
fn names_humidity(text: &str) -> bool {
    (text.contains("влажност") || text.contains("humidity")) && !text.contains("почв")
}

/// A short Latin word is a whole token: "sd" must not match inside another
/// word, while "microsd" and "sd-карта" still name the card.
fn names_card(text: &str) -> bool {
    text.split_whitespace().any(|t| {
        let t = t.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
        t == "sd" || t.starts_with("sd-") || t == "microsd" || t == "sd card"
    }) || text.contains("карта памяти")
}

/// The concrete parts the sentence names, reduced to the questions the build
/// can answer: a kind with a count, a named module, or a piece of code.
pub fn missing(built: &Built, raw: &str) -> Vec<Missing> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    let named = crate::parse::named_devices(raw);
    let c = &built.circuit;
    let text = crate::words::normalize(raw);
    let mut out = Vec::new();

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
    let phone_screen = text.contains("телефон") || text.contains("сотов");
    if let Some(screens) = named.screens {
        let got = c
            .parts
            .iter()
            .filter(|p| {
                p.kind == Kind::Module
                    && (p.value.starts_with("LCD")
                        || p.value.starts_with("OLED")
                        || p.value.starts_with("SSD1306"))
            })
            .count();
        if !phone_screen {
            need(L::new("экран", "display"), screens.count, got);
        }
    }

    // A named board is a requirement of its own: "DHT22" must not silently
    // become another humidity sensor, and "FS1000A" must not disappear from
    // a radio pair.
    if text.contains("dht22") {
        need(
            L::new("DHT22", "DHT22"),
            1,
            any_module(c, &["DHT22"]) as usize,
        );
    } else if text.contains("dht11") {
        need(
            L::new("DHT11", "DHT11"),
            1,
            any_module(c, &["DHT11"]) as usize,
        );
    } else if names_humidity(&text) {
        need(
            L::new("датчик влажности", "humidity sensor"),
            1,
            any_module(c, &["DHT11", "DHT22", "BME280"]) as usize,
        );
    }
    if names_temperature(&text) {
        let got = count_kind(c, &[Kind::Tmp36, Kind::Thermistor])
            + ["DS18B20", "DHT11", "DHT22", "BMP180", "BMP280", "BME280"]
                .iter()
                .filter(|n| module(c, n))
                .count();
        need(L::new("датчик температуры", "temperature sensor"), 1, got);
    }
    if text.contains("почв") {
        need(
            L::new("датчик почвы", "soil sensor"),
            1,
            any_module(c, &["SOIL"]) as usize,
        );
    }
    if text.contains("fs1000a") {
        need(
            L::new("FS1000A", "FS1000A"),
            1,
            any_module(c, &["FS1000A"]) as usize,
        );
    }
    if text.contains("xy-mk") {
        need(
            L::new("XY-MK-5V", "XY-MK-5V"),
            1,
            any_module(c, &["XY-MK-5V"]) as usize,
        );
    }
    if text.contains("rc522") || text.contains("rfid") || text.contains("метк") {
        need(
            L::new("RC522", "RC522"),
            1,
            any_module(c, &["RC522"]) as usize,
        );
    }
    if text.contains("tsop") || text.contains("ик приемник") || text.contains("ик-приемник") {
        need(
            L::new("TSOP1838", "TSOP1838"),
            1,
            module_starts(c, "TSOP") as usize,
        );
    }
    if names_card(&text) && !text.contains("доступ") {
        need(
            L::new("SD-карта", "SD card"),
            1,
            any_module(c, &["SD"]) as usize,
        );
    }
    if text.contains("семисегмент") || text.contains("7seg") {
        need(
            L::new("семисегментный индикатор", "seven-segment display"),
            1,
            module_starts(c, "7SEG") as usize,
        );
    }
    if text.contains("хлоп") || text.contains("хлопк") {
        need(
            L::new("датчик звука", "sound sensor"),
            1,
            any_module(c, &["SOUND"]) as usize,
        );
    }

    // Two roles in the sentence are two roles in the sketch. A one-board
    // drawing is fine when the code really holds both halves, as the 433 MHz
    // link does with SENDER; a plan that quietly keeps only the transmitter
    // is not.
    let radio_words = text.contains("радио")
        || text.contains("433")
        || text.contains("nrf")
        || text.contains("fs1000")
        || text.contains("xy-mk");
    let both_roles = radio_words
        && (text.contains("передатчик") || text.contains("transmitter"))
        && (text.contains("приемник")
            || text.contains("приёмник")
            || text.contains("receiver"));
    if both_roles {
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

pub fn check(built: &Built, raw: &str, lang: Lang) -> Vec<Check> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    let missing = missing(built, raw);
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
    use crate::templates;

    #[test]
    fn a_missing_named_part_is_reported() {
        let p = crate::parse::parse("Датчик температуры DHT22 и реле");
        let built = templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        let mut broken = built.clone();
        broken.circuit.parts.retain(|part| part.value != "DHT22");
        let miss = missing(&broken, "Датчик температуры DHT22 и реле");
        assert!(miss.iter().any(|m| m.name.ru == "DHT22"));
    }

    #[test]
    fn a_struct_is_required_by_the_word() {
        let p = crate::parse::parse(
            "Составить программу для МК последовательно, через каждые 2 секунды отправлять значения номера передатчика, температуры и влажности на разных платах от 1 платы с передатчиком на 1 приёмник с по-мощью структуры.",
        );
        let built = templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        assert!(missing(&built, &p.intent.raw).is_empty());
        let mut broken = built.clone();
        broken.code = "// nothing\n".to_string();
        assert!(missing(&broken, &p.intent.raw)
            .iter()
            .any(|m| m.name.ru.contains("структура")));
    }

    #[test]
    fn sd_is_a_token_not_a_fragment() {
        assert!(names_card("записать данные на sd карту"));
        assert!(names_card("карта памяти"));
        assert!(!names_card("использовать датчик"));
    }

    /// The checklist is a ratchet: the printed corpus has 41 tasks where the
    /// book's own answer loses a part the task names. Fixing them is parser
    /// work; this test makes sure the number never grows, and prints the list
    /// when run with `--nocapture`. Lower the number as cases are fixed.
    #[test]
    fn the_checklist_does_not_lose_more_parts_than_today() {
        let known = 41;
        let mut bad = 0;
        for (task, _) in crate::book_tasks::TASKS {
            let parsed = crate::parse::parse(task);
            if parsed.unsupported.is_some() {
                continue;
            }
            let Some(plan) = crate::templates::plan(&parsed.intent, Lang::Ru) else {
                continue;
            };
            let Some(built) = crate::templates::build(&parsed.intent, plan.recommended, Lang::Ru)
            else {
                continue;
            };
            let miss = missing(&built, task);
            if !miss.is_empty() {
                bad += 1;
                let names: Vec<&str> = miss.iter().map(|m| m.name.ru).collect();
                eprintln!("{task} -> {}: {:?}", plan.title_text(Lang::Ru), names);
            }
        }
        eprintln!("book tasks with missing parts: {bad}");
        assert!(
            bad <= known,
            "{bad} book tasks lose a named part, was {known}: a new silent drop?"
        );
    }
}
