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
}
