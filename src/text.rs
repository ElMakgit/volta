//! Language handling.
//!
//! Omarchy keeps the chosen language in `~/.config/omarchy/language`. Every
//! visible string is either UI chrome (looked up in `i18n/*.json` by key) or
//! domain data that lives next to the data it describes (component names,
//! notes, sketch comments), which is why those carry both languages inline.

use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    pub fn from_tag(tag: &str) -> Lang {
        if tag.trim().to_lowercase().starts_with("ru") {
            Lang::Ru
        } else {
            Lang::En
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
        }
    }
    pub fn is_rtl(self) -> bool {
        false
    }
}

/// A string that carries its own translations, for data that is not UI chrome.
#[derive(Clone, Copy, Debug)]
pub struct L {
    pub ru: &'static str,
    pub en: &'static str,
}

impl L {
    pub const fn new(ru: &'static str, en: &'static str) -> Self {
        Self { ru, en }
    }
    pub fn get(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Ru => self.ru,
            Lang::En => self.en,
        }
    }
}

pub fn tr(lang: Lang, ru: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::Ru => ru,
        Lang::En => en,
    }
}

/// The parser's trace is developer evidence, but it is shown in the same
/// panel as the Russian UI. Translate the labels and the common phrases here
/// instead of leaking two languages into one window. Unknown lines pass
/// through untouched: a new trace label stays readable rather than vanishing.
pub fn localize_trace(line: &str, lang: Lang) -> String {
    if lang != Lang::Ru {
        return line.to_string();
    }
    const PREFIXES: &[(&str, &str)] = &[
        ("output: readings offered: ", "выход: прочтения: "),
        ("load group ", "группа нагрузки "),
        ("text screens: ", "экраны: "),
        ("unsupported: ", "отказ: "),
        ("alternative: ", "вариант: "),
        ("context: ", "контекст: "),
        ("output: ", "выход: "),
        ("control: ", "слот входа: "),
        ("input: ", "вход: "),
        ("action: ", "действие: "),
        ("recipe: ", "рецепт: "),
        ("lesson: ", "урок: "),
        ("route: ", "маршрут: "),
        ("task: ", "задача: "),
        ("book: ", "книга: "),
        ("text: ", "текст: "),
        ("load: ", "нагрузка: "),
        ("understand: ", "понято: "),
    ];
    const PHRASES: &[(&str, &str)] = &[
        (
            "IR receiver is not a 433 MHz radio",
            "ИК-приёмник — не радио 433 МГц",
        ),
        ("SD card is not an RFID tag", "SD-карта — не RFID-метка"),
        (
            "stepper is not a second DC motor",
            "шаговик не второй мотор постоянного тока",
        ),
        (
            "default (no load word)",
            "по умолчанию (слово нагрузки не названо)",
        ),
        (
            "near tie, readings offered",
            "близкий счёт, предложены прочтения",
        ),
        ("exact task", "точная задача"),
        ("coverage picks", "покрытие выбирает"),
        ("readings offered", "прочтения"),
        ("printed: ", "в книге: "),
        ("suggestion", "предложение"),
        ("first at", "впервые на"),
        ("evidence", "вес"),
        ("was", "было"),
        ("none", "нет"),
    ];
    // Debug names from the enums that appear in the trace. Longest first:
    // "Lcd2004" must win over "Lcd", and "Motors" over "Motor".
    const NAMES: &[(&str, &str)] = &[
        ("RadioNrf", "радио NRF24L01"),
        ("Radio433", "радио 433 МГц"),
        ("Bluetooth", "Bluetooth"),
        ("Ethernet", "Ethernet"),
        ("Stepper", "шаговый двигатель"),
        ("Motors", "драйвер двигателей"),
        ("Motor", "двигатель"),
        ("Servo", "сервопривод"),
        ("Buzzer", "звук"),
        ("Lamp", "лампа накаливания"),
        ("Rgb", "RGB-светодиод"),
        ("Relay", "реле"),
        ("Rfid", "RFID"),
        ("Lcd2004", "LCD 2004"),
        ("Lcd", "LCD"),
        ("Oled", "OLED"),
        ("Serial", "монитор порта"),
        ("Led", "светодиод"),
        ("Temperature", "температура"),
        ("Thermistor", "термистор"),
        ("Humidity", "влажность"),
        ("Pressure", "давление"),
        ("Soil", "почва"),
        ("Light", "свет"),
        ("Range", "расстояние"),
        ("Alcohol", "алкоголь"),
        ("Motion", "движение"),
        ("Sound", "звук"),
        ("Line", "линия"),
        ("Hall", "Холла"),
        ("Current", "ток"),
        ("Radiation", "радиация"),
        ("Tilt", "наклон"),
        ("Weight", "вес"),
        ("Color", "цвет"),
        ("Reader", "считыватель"),
        ("Transmitter", "передатчик"),
        ("Receiver", "приёмник"),
        ("Peer", "пара"),
        ("Gateway", "шлюз"),
        ("Blink", "мигание"),
        ("Solid", "ровный свет"),
        ("Beep", "писк"),
        ("Switch", "щёлкать"),
        ("Show", "показ"),
        ("Turn", "поворот"),
        ("Spin", "вращение"),
        ("Fade", "плавное свечение"),
        ("Measure", "измерение"),
        ("Display", "вывод на экран"),
        ("Alarm", "сигнал"),
        ("Toggle", "переключение"),
        ("Button", "кнопка"),
        ("Pot", "потенциометр"),
        ("Ldr", "фоторезистор"),
        ("outputs", "выходы"),
        ("controls", "входы"),
        ("sensors", "датчики"),
        ("screens", "экраны"),
        ("links", "ссылки"),
        ("kind:", "тип"),
        ("count:", "количество"),
        ("matrix:", "матрица"),
        ("segments:", "семисегментный"),
        ("true", "да"),
        ("false", "нет"),
        ("Any", "любой"),
    ];
    let mut out = line.to_string();
    for (en, ru) in PREFIXES {
        if let Some(rest) = out.strip_prefix(en) {
            out = format!("{ru}{rest}");
            break;
        }
    }
    for (en, ru) in PHRASES {
        out = out.replace(en, ru);
    }
    // `{:?}` prints options and tuples, not prose: unwrap them before the
    // enum names so both languages see the same value.
    out = strip_some(&out);
    if out.starts_with("рецепт: (") && out.ends_with(')') {
        let body = &out["рецепт: (".len()..out.len() - 1];
        let parts: Vec<String> = body
            .split(", ")
            .map(|part| name_in_ru(part, NAMES))
            .collect();
        out = format!("рецепт: {}", parts.join(" → "));
    }
    if let Some(inner) = out
        .split_once("Screens { ")
        .and_then(|(head, tail)| tail.strip_suffix(" }").map(|body| (head, body)))
    {
        out = format!("{}{}", inner.0, inner.1);
    }
    for (en, ru) in NAMES {
        out = out.replace(en, ru);
    }
    out = out.replace("None", "нет");
    out
}

/// Remove `Some(` and its matching `)` from a debug-printed option, keeping
/// the value inside. Other parentheses are left alone.
fn strip_some(text: &str) -> String {
    let mut rest = text;
    let mut out = String::new();
    while let Some(start) = rest.find("Some(") {
        out.push_str(&rest[..start]);
        let after = &rest[start + "Some(".len()..];
        let mut depth = 1usize;
        let mut end = after.len();
        for (i, ch) in after.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        out.push_str(&after[..end]);
        rest = after.get(end + 1..).unwrap_or("");
    }
    out.push_str(rest);
    out
}

/// One debug name from a tuple, translated when it is known.
fn name_in_ru(value: &str, names: &[(&str, &str)]) -> String {
    let mut out = value.to_string();
    for (en, ru) in names {
        out = out.replace(en, ru);
    }
    out
}


/// UI chrome, loaded from the JSON files compiled into the binary.
pub struct Tr {
    pub lang: Lang,
    map: BTreeMap<String, String>,
    fallback: BTreeMap<String, String>,
}

impl Tr {
    pub fn new(lang: Lang) -> Self {
        let (json, fallback) = match lang {
            Lang::Ru => (include_str!("i18n/ru.json"), include_str!("i18n/en.json")),
            Lang::En => (include_str!("i18n/en.json"), "{}"),
        };
        Self {
            lang,
            map: parse_json(json),
            fallback: parse_json(fallback),
        }
    }
    /// Missing translations fall back to English, then to the key itself, so a
    /// gap shows up as a readable word instead of an empty control.
    pub fn t(&self, key: &str) -> String {
        self.map
            .get(key)
            .or_else(|| self.fallback.get(key))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
}

fn parse_json(src: &str) -> BTreeMap<String, String> {
    let value: serde_json::Value = serde_json::from_str(src).unwrap_or_default();
    let mut out = BTreeMap::new();
    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                out.insert(k.clone(), s.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_define_the_same_keys() {
        let ru = Tr::new(Lang::Ru);
        let en = Tr::new(Lang::En);
        let ru_keys: Vec<&String> = ru.map.keys().collect();
        let en_keys: Vec<&String> = en.map.keys().collect();
        assert_eq!(ru_keys, en_keys, "ru.json and en.json drifted apart");
        assert!(!en.map.is_empty());
    }

    #[test]
    fn language_tags_parse() {
        assert_eq!(Lang::from_tag("ru"), Lang::Ru);
        assert_eq!(Lang::from_tag("ru-RU\n"), Lang::Ru);
        assert_eq!(Lang::from_tag("da"), Lang::En);
    }

    /// The trace panel sits inside a Russian window, so its labels and the
    /// debug names of the enums must not stay English.
    #[test]
    fn the_trace_is_russian_in_a_russian_window() {
        let lines = [
            "output: Led (evidence 1, first at 2) [светодиод]",
            "load: Led [светодиод]",
            "input: Some(Button) [кнопк]",
            "control: Button (evidence 1, first at 0) [кнопк]",
            "action: None",
            "lesson: Some(6)",
            "recipe: Some((Temperature, Relay, Switch))",
            "text screens: Some(Screens { kind: Any, count: 1, matrix: false, segments: false })",
            "task: outputs [Led], controls [Button], sensors [Temperature], screens 1, links [(Radio433, Transmitter)]",
            "route: coverage picks Motor + Some(Pot), was Led + Some(Button)",
            "context: stepper is not a second DC motor",
            "unsupported: none",
            "book: suggestion, 80%, printed: Светодиод",
        ];
        let bad = [
            "output",
            "load",
            "input",
            "control",
            "action",
            "lesson",
            "recipe",
            "screens",
            "task",
            "evidence",
            "first at",
            "Some",
            "None",
            "Button",
            "Led",
            "Motor",
            "Pot",
            "Temperature",
            "Transmitter",
            "stepper",
            "coverage",
            "picks",
            "was",
            "unsupported",
            "Screens",
        ];
        for line in lines {
            let shown = localize_trace(line, Lang::Ru);
            for word in bad {
                assert!(
                    !shown.contains(word),
                    "{line:?} -> {shown:?} still contains {word:?}"
                );
            }
        }
        assert_eq!(localize_trace("output: Led", Lang::En), "output: Led");
    }
}
