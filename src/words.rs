//! Word-level tools for the offline parser.
//!
//! Russian is heavily inflected and people type quickly, so matching whole
//! keywords against the raw sentence misses most real phrasings. This module
//! does the cheap, predictable part: normalise the text, fold the common
//! endings, repair a wrong keyboard layout and a small typo, and compare two
//! words by their stem.

/// Fold the text to the form the parser compares against: lower case, `ё`
/// becomes `е`, Latin look-alikes inside a Cyrillic word become Cyrillic, and
/// every punctuation mark turns into a space.
pub fn normalize(raw: &str) -> String {
    let lower = raw.to_lowercase().replace('ё', "е");
    let mut out = String::with_capacity(lower.len());
    let mut word = String::new();
    for ch in lower.chars() {
        if ch.is_alphanumeric() || ch == '-' || ch == '.' || ch == '/' {
            word.push(ch);
        } else {
            flush(&mut word, &mut out);
            out.push(' ');
        }
    }
    flush(&mut word, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn flush(word: &mut String, out: &mut String) {
    if word.is_empty() {
        return;
    }
    out.push_str(&fix_homoglyphs(word));
    word.clear();
}

/// A Cyrillic word typed with a Latin look-alike (`cветодиод`) is a common
/// paste artefact. Only mixed words are repaired, so real Latin terms such as
/// `hc-05` stay untouched.
fn fix_homoglyphs(word: &str) -> String {
    let has_cyrillic = word.chars().any(|c| ('а'..='я').contains(&c));
    if !has_cyrillic {
        return word.to_string();
    }
    word.chars()
        .map(|c| match c {
            'a' => 'а',
            'b' => 'в',
            'c' => 'с',
            'e' => 'е',
            'h' => 'н',
            'k' => 'к',
            'm' => 'м',
            'o' => 'о',
            'p' => 'р',
            't' => 'т',
            'x' => 'х',
            'y' => 'у',
            other => other,
        })
        .collect()
}

/// The same text in the other keyboard layout, word by word. A Russian word
/// typed with the Latin layout (`rjyrf`) becomes readable again.
pub fn layout_fixes(text: &str) -> Vec<String> {
    const MAP: [(char, char); 26] = [
        ('q', 'й'),
        ('w', 'ц'),
        ('e', 'у'),
        ('r', 'к'),
        ('t', 'е'),
        ('y', 'н'),
        ('u', 'г'),
        ('i', 'ш'),
        ('o', 'щ'),
        ('p', 'з'),
        ('a', 'ф'),
        ('s', 'ы'),
        ('d', 'в'),
        ('f', 'а'),
        ('g', 'п'),
        ('h', 'р'),
        ('j', 'о'),
        ('k', 'л'),
        ('l', 'д'),
        ('z', 'я'),
        ('x', 'ч'),
        ('c', 'с'),
        ('v', 'м'),
        ('b', 'и'),
        ('n', 'т'),
        ('m', 'ь'),
    ];
    let mut out = Vec::new();
    for word in text.split_whitespace() {
        if !word.chars().all(|c| c.is_ascii_alphabetic()) || word.len() < 3 {
            continue;
        }
        let mut mapped = String::new();
        for c in word.chars() {
            match MAP.iter().find(|(from, _)| *from == c) {
                Some((_, to)) => mapped.push(*to),
                None => mapped.push(c),
            }
        }
        out.push(mapped);
    }
    out
}

/// The part of a Russian word that does not change with its case: strip a
/// common ending, but never shorten a word below four letters.
pub fn stem(word: &str) -> String {
    const ENDINGS: &[&str] = &[
        "иями", "ями", "ами", "ыми", "ими", "его", "ого", "ому", "ему", "ешь", "ишь",
        "ать", "ять", "ить", "еть", "ует", "уют", "ает", "ают", "ится", "ется", "ться",
        "ами", "ями", "ов", "ев", "ий", "ый", "ой", "ей", "ом", "ем", "ам", "ям", "ах",
        "ях", "ия", "ию", "ье", "ья", "ая", "ое", "ые", "ую", "юю", "ем", "им", "ым",
        "а", "я", "ы", "и", "е", "у", "ю", "о", "ь",
    ];
    let chars: Vec<char> = word.chars().collect();
    for e in ENDINGS {
        if chars.len() > 4 && word.ends_with(e) && chars.len() - e.chars().count() >= 4 {
            return chars[..chars.len() - e.chars().count()].iter().collect();
        }
    }
    word.to_string()
}

/// Whether two stems name the same thing. A missing letter in the middle
/// (`кнопк` and `кнопок`) is common after folding, so one-letter differences
/// at the tail are tolerated — but only when the lengths differ, which keeps
/// `ручн` and `ручк` apart.
pub fn alike(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let short = a.chars().count().min(b.chars().count());
    if short < 4 {
        return false;
    }
    // A short word must not swallow every word that starts with it:
    // "свет" is not "светочувствительность".
    // A four-letter stem is too short to stand in for a much longer word:
    // "ради" (from "радио") must not match "радиац" (from "радиация").
    let length_diff = a.chars().count().abs_diff(b.chars().count());
    if (a.starts_with(b) || b.starts_with(a))
        && length_diff <= 3
        && (short >= 5 || length_diff <= 1)
    {
        return true;
    }
    let common = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let diff = a.chars().count().abs_diff(b.chars().count());
    diff == 1 && common >= short - 1
}

/// Levenshtein distance with a hard cap: the parser only cares about "one or
/// two typos", so a larger distance can stop early.
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    if n.abs_diff(m) > 3 {
        return 4;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
}

/// Does a sentence mention the given keyword? The keyword is written as a
/// stem (`кнопк`, `мотор`, `max7219`); Latin and digit keywords are compared
/// literally, Cyrillic ones by stem, with one typo allowed on long words.
pub fn mentions(text: &str, key: &str) -> bool {
    // The key is normalised the same way as the sentence, so a lesson key
    // written as "16*2" still matches the sentence "16 2".
    let key = normalize(key);
    let tokens: Vec<&str> = text.split_whitespace().collect();
    // A short Latin key is a whole token: "oled" contains "led", but an OLED
    // is not an LED, and "gsm" must not match inside another word.
    if key.chars().count() <= 4 && key.chars().all(|c| c.is_ascii_alphabetic()) {
        return tokens
            .iter()
            .any(|t| t.trim_matches(|c: char| !c.is_alphanumeric()) == key);
    }
    // A long key may sit inside a word ("температур" in "температуру"). A
    // short plain word must not: "ток" is not inside "поток". A part number
    // such as "mq-2" keeps the substring test even when it is short.
    let short_word = key.chars().count() < 5 && key.chars().all(|c| c.is_alphabetic());
    if !short_word && text.contains(&key) {
        return true;
    }
    // A key with a digit or a hyphen is a whole phrase or a part number
    // (`max7219`, `hc-05`, `2 двигател`). Editing it by one letter would let
    // "двигателя" match "2 двигател", so such keys are exact.
    let words: Vec<&str> = key.split_whitespace().collect();
    if !words.iter().all(|w| w.chars().all(|c| c.is_alphabetic())) {
        return false;
    }
    // A phrase key ("датчик свет") is matched word by word: every word is
    // folded to its stem, so "по датчику света" still finds "датчик свет".
    if words.len() > 1 {
        return tokens
            .windows(words.len())
            .any(|window| window.iter().zip(&words).all(|(t, k)| word_matches(t, k)));
    }
    let Some(key) = words.first().copied() else {
        return false;
    };
    tokens.iter().any(|token| word_matches(token, key))
}

/// One word of a key against one word of the sentence: same stem, or one
/// typo on a real word.
fn word_matches(token: &str, key: &str) -> bool {
    let token_stem = stem(token);
    let key_stem = stem(key);
    if alike(&token_stem, &key_stem) || alike(&token_stem, key) {
        return true;
    }
    // A short key still takes its own case endings ("ток" → "тока", "час" →
    // "часы"), but the ending list is closed: the rest of a longer word is
    // not an inflection of the key.
    if key_stem.chars().count() <= 4 && token_stem.starts_with(&key_stem) {
        const ENDINGS: &[&str] = &[
            "", "а", "я", "ы", "и", "е", "у", "ю", "о", "ь", "й", "ом", "ем", "ам",
            "ям", "ах", "ях", "ов", "ев", "ами", "ями", "ет", "ут", "ют", "ат", "ят",
            "ит", "ешь", "ишь", "ал", "ил", "ла", "ло", "ли", "ть", "ться",
            "ется", "ится",
        ];
        if ENDINGS.contains(&&token_stem[key_stem.len()..]) {
            return true;
        }
    }
    // One typo is allowed on a real word of at least six letters. A long
    // word may have two, but only when the lengths differ: a skipped or
    // doubled letter is a typo, while two substitutions in words of the same
    // length are usually unrelated words ("приветствие" and "присутствие").
    // Shorter stems differ by a letter for unrelated reasons ("пульт" and
    // "пельтье"), and a typo there is a false hit.
    let key_len = key_stem.chars().count();
    let token_len = token_stem.chars().count();
    if key_len >= 6 && token_len >= 5 {
        let two = token_len >= 9 && key_len >= 9 && token_len != key_len;
        let cap = if two { 2 } else { 1 };
        if distance(&token_stem, &key_stem) <= cap {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_folds_case_punctuation_and_yo() {
        assert_eq!(normalize("  Мотор, Ёмкость!  "), "мотор емкость");
        assert_eq!(normalize("cветодиод"), "светодиод");
        assert_eq!(normalize("HC-05"), "hc-05");
    }

    #[test]
    fn phrases_match_through_cases() {
        assert!(mentions("по датчику света включай лампу", "датчик свет"));
        assert!(mentions("мощной нагрузке", "мощной нагрузки"));
        assert!(mentions("цветным светодиодом", "цветн светодиод"));
        assert!(!mentions("второго светодиода", "первый светодиод"));
        assert!(!mentions("свет датчик", "датчик свет"));
    }

    #[test]
    fn stems_cover_the_common_cases() {
        for (word, want) in [
            ("мотор", "мотор"),
            ("моторами", "мотор"),
            ("моторчиком", "моторчик"),
            ("кнопка", "кнопк"),
            ("кнопками", "кнопк"),
            ("лампочке", "лампочк"),
            ("светодиодами", "светодиод"),
            ("температуру", "температур"),
        ] {
            assert_eq!(stem(word), want, "{word}");
        }
    }

    #[test]
    fn a_wrong_layout_is_readable() {
        let fixed = layout_fixes("ryjgrf");
        assert_eq!(fixed, ["кнопка"]);
    }

    #[test]
    fn typos_and_missing_letters_still_match() {
        assert!(mentions("термоментр", "термометр"));
        assert!(mentions("кнопок", "кнопк"));
        assert!(mentions("светодиот", "светодиод"));
        assert!(!mentions("мотор", "серво"));
        assert!(!alike("ручн", "ручк"));
        assert!(alike("кнопк", "кнопок"));
        assert!(mentions("max7219 матрица", "max7219"));
    }

    #[test]
    fn a_short_latin_key_is_a_whole_word() {
        assert!(mentions("подключи oled экран", "oled"));
        assert!(!mentions("подключи oled экран", "led"));
        assert!(mentions("светодиод led", "led"));
    }
}