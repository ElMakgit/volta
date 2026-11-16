//! Semantic roles of a task, extracted without an LLM.
//!
//! Keywords still route the quick paths, but a sentence is more than a bag of
//! words: it says what is measured, what is driven, under which condition and
//! on which schedule. This module reads those roles with a small dictionary of
//! inflections, synonyms, common misspellings and number patterns.

use crate::words::{normalize, stem};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Semantic {
    /// What the circuit observes: "температура", "газ", "часы".
    pub source: Option<String>,
    /// What it acts on: "реле", "пищалка", "дисплей", "двигатель".
    pub sinks: Vec<String>,
    /// The spoken condition, kept as text for the explanation.
    pub condition: Option<String>,
    /// "раз в 5 секунд" -> 5000.
    pub period_ms: Option<u32>,
    /// "после 3 нажатий" -> 3.
    pub after_presses: Option<u32>,
    /// How much of the sentence was understood, 0..1.
    pub confidence: f32,
}

const SOURCES: &[(&str, &[&str])] = &[
    ("clock", &["часы реальн", "часов", "часы", "время", "дата", "ds3231", "ds1307", "rtc", "clock"]),
    ("temperature", &["температур", "термометр", "термодатчик", "градус", "temperature", "температура"]),
    ("humidity", &["влажност", "humidity", "гигрометр"]),
    ("pressure", &["давлен", "барометр", "pressure", "bmp180", "bme280"]),
    ("gas", &["газ", "утечк", "mq-2", "mq2", "дым", "smoke"]),
    ("light", &["свет", "фоторезистор", "освещ", "люкс", "brightness"]),
    ("distance", &["расстоян", "дальномер", "ультразвук", "hc-sr04", "distance"]),
    ("motion", &["движен", "пиро", "hc-sr501", "motion"]),
    ("sound", &["звук", "шум", "громкост", "sound"]),
    ("card", &["карт", "rfid", "метк", "rc522", "card"]),
    ("alcohol", &["алкометр", "спирт", "алкогол", "mq-3", "alcohol"]),
];

const SINKS: &[(&str, &[&str])] = &[
    ("display", &["диспле", "экран", "табло", "lcd", "oled", "display", "индикатор"]),
    ("buzzer", &["пьезо", "пищал", "зуммер", "сирен", "buzzer", "gong"]),
    ("relay", &["реле", "relay"]),
    ("motor", &["двигател", "мотор", "вентилятор", "насос", "motor"]),
    ("servo", &["серво", "servo"]),
    ("led", &["светодиод", "индикатор", "led", "лампочк"]),
    ("button", &["кнопк", "button", "переключател"]),
    ("radio", &["радио", "nrf24", "433", "передатчик", "radio"]),
    ("gsm", &["gsm", "sms", "modem"]),
];

fn mentions_any(text: &str, words: &[&str]) -> bool {
    words.iter().any(|key| {
        if text.contains(key) {
            return true;
        }
        let k = stem(key);
        k.len() >= 4 && text.split_whitespace().any(|word| stem(word) == k)
    })
}

fn number_after(text: &str, cue: &str) -> Option<u32> {
    let at = text.find(cue)?;
    text[at + cue.len()..]
        .split_whitespace()
        .take(3)
        .find_map(|w| w.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok())
}

/// Read the roles of a request. Unknown words do not guess a device: they
/// lower the confidence so the caller can ask a clarifying question.
pub fn analyse(raw: &str) -> Semantic {
    let text = normalize(raw);
    if text.is_empty() {
        return Semantic::default();
    }
    let sinks: Vec<String> = SINKS
        .iter()
        .filter(|(_, keys)| mentions_any(&text, keys))
        .map(|(name, _)| (*name).to_string())
        .collect();
    let has_buzzer = sinks.iter().any(|s| s == "buzzer");
    let source = SOURCES
        .iter()
        .filter(|(name, _)| !(*name == "sound" && has_buzzer))
        .find(|(_, keys)| mentions_any(&text, keys))
        .map(|(name, _)| (*name).to_string());
    let condition = if ["выше", "ниже", "больше", "меньше", "равно", "когда", "если", "после", "above", "below"]
        .iter()
        .any(|c| text.contains(c))
    {
        Some("threshold".to_string())
    } else {
        None
    };
    let period_ms = if text.contains("каждый час") || text.contains("раз в час") || text.contains("hourly") {
        Some(3_600_000)
    } else {
        ["раз в", "каждые", "every"].iter().find_map(|cue| {
            let n = number_after(&text, cue)?;
            let unit = if text.contains("минут") || text.contains("minute") { 60_000 }
                else if text.contains("секунд") || text.contains("second") { 1000 }
                else { 1000 };
            Some(n.saturating_mul(unit))
        })
    };
    let after_presses = if text.contains("нажат") || text.contains("press") || text.contains("нажм") {
        ["после", "через", "after"].iter().find_map(|cue| number_after(&text, cue))
    } else {
        None
    };
    let hits = source.is_some() as usize + sinks.len() + condition.is_some() as usize
        + period_ms.is_some() as usize + after_presses.is_some() as usize;
    let words = text.split_whitespace().count().max(1) as f32;
    let confidence = if source.is_none() && sinks.is_empty() {
        0.0
    } else {
        (hits as f32 / words).min(1.0)
    };
    Semantic { source, sinks, condition, period_ms, after_presses, confidence }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_are_read_with_inflections_and_typos() {
        let s = analyse("температура выше 30 включи вентилятор раз в 5 секунд");
        assert_eq!(s.source.as_deref(), Some("temperature"));
        assert!(s.sinks.iter().any(|x| x == "motor"));
        assert_eq!(s.condition.as_deref(), Some("threshold"));
        assert_eq!(s.period_ms, Some(5000));
    }

    #[test]
    fn numbers_and_press_counts_are_extracted() {
        let s = analyse("после 3 нажатий зажечь светодиод");
        assert_eq!(s.after_presses, Some(3));
        assert!(s.sinks.iter().any(|x| x == "led"));
        let hourly = analyse("часы с пьезопищалкой каждый час на экран");
        assert_eq!(hourly.period_ms, Some(3_600_000));
        assert!(hourly.sinks.iter().any(|x| x == "buzzer"));
        assert!(hourly.sinks.iter().any(|x| x == "display"));
    }

    #[test]
    fn nonsense_has_low_confidence() {
        assert!(analyse("как дела").confidence < 0.2);
        assert!(analyse("").sinks.is_empty());
    }
}
