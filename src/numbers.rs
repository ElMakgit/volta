//! Numbers and durations as people write them.
//!
//! "Выше тридцати", "раз в полчаса", "каждые пять минут", "после трёх
//! нажатий": a spoken number is as real as a digit, but the parser used to
//! read only digits and silently dropped the rest of the sentence. This
//! module turns those words back into values, without a dictionary of every
//! inflection: numeric words are matched by their stable stem.

use crate::words::normalize;

/// The value of one numeral word, matched by its stem: "тридцати",
/// "тридцать" and "тридцатью" are the same 30.
fn token_value(word: &str) -> Option<u64> {
    let word = normalize(word);
    let word = word.trim_matches(|c: char| c == '.' || c == ',');
    if let Ok(n) = word.parse::<u64>() {
        return Some(n);
    }
    let hit = |prefixes: &[&str]| prefixes.iter().any(|p| word.starts_with(p));
    // Teens, tens, hundreds and thousands go first: they share their first
    // letters with the plain units ("четыреста" with "четыре").
    if hit(&["одиннадцат"]) {
        return Some(11);
    }
    if hit(&["двенадцат"]) {
        return Some(12);
    }
    if hit(&["тринадцат"]) {
        return Some(13);
    }
    if hit(&["четырнадцат"]) {
        return Some(14);
    }
    if hit(&["пятнадцат"]) {
        return Some(15);
    }
    if hit(&["шестнадцат"]) {
        return Some(16);
    }
    if hit(&["семнадцат"]) {
        return Some(17);
    }
    if hit(&["восемнадцат"]) {
        return Some(18);
    }
    if hit(&["девятнадцат"]) {
        return Some(19);
    }
    if hit(&["двадцат"]) {
        return Some(20);
    }
    if hit(&["тридцат"]) {
        return Some(30);
    }
    if hit(&["сорок"]) {
        return Some(40);
    }
    if hit(&["пятьдесят", "пятидесят"]) {
        return Some(50);
    }
    if hit(&["шестьдесят", "шестидесят"]) {
        return Some(60);
    }
    if hit(&["семьдесят", "семидесят"]) {
        return Some(70);
    }
    if hit(&["восемьдесят", "восьмидесят"]) {
        return Some(80);
    }
    if hit(&["девяност"]) {
        return Some(90);
    }
    if hit(&["двести", "двухсот"]) {
        return Some(200);
    }
    if hit(&["триста", "трехсот"]) {
        return Some(300);
    }
    if hit(&["четыреста", "четырехсот"]) {
        return Some(400);
    }
    if hit(&["пятьсот", "пятисот"]) {
        return Some(500);
    }
    if hit(&["шестьсот", "шестисот"]) {
        return Some(600);
    }
    if hit(&["семьсот", "семисот"]) {
        return Some(700);
    }
    if hit(&["восемьсот", "восьмисот"]) {
        return Some(800);
    }
    if hit(&["девятьсот", "девятисот"]) {
        return Some(900);
    }
    if hit(&["сто"]) {
        return Some(100);
    }
    if hit(&["тысяч"]) {
        return None;
    }
    if hit(&["один", "одн", "един"]) {
        return Some(1);
    }
    if hit(&["два", "две", "двух", "двум", "двоих", "двое", "двумя"]) {
        return Some(2);
    }
    if hit(&["три", "трех", "трем", "тремя", "троих", "трое"]) {
        return Some(3);
    }
    if hit(&["четыр", "четвер"]) {
        return Some(4);
    }
    if hit(&["пят"]) {
        return Some(5);
    }
    if hit(&["шест"]) {
        return Some(6);
    }
    if hit(&["сем"]) {
        return Some(7);
    }
    if hit(&["восем", "восьм"]) {
        return Some(8);
    }
    if hit(&["девят"]) {
        return Some(9);
    }
    if hit(&["десят"]) {
        return Some(10);
    }
    if hit(&["нол", "нул"]) {
        return Some(0);
    }
    None
}

fn is_thousand(word: &str) -> bool {
    normalize(word).starts_with("тысяч")
}

/// A decimal comma inside a number ("0,5 сек", "36,6 градуса") becomes a
/// dot, so normalisation keeps the number as one token.
pub fn fix_decimal_commas(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    for (i, &ch) in chars.iter().enumerate() {
        let decimal = ch == ','
            && i > 0
            && chars[i - 1].is_ascii_digit()
            && i + 1 < chars.len()
            && chars[i + 1].is_ascii_digit();
        out.push(if decimal { '.' } else { ch });
    }
    out
}

/// Read the number at the start of `words`: digits, "полтора", or a run of
/// numeral words ("двадцать пять"). Returns the value and how many words it
/// spans. A word that is not a numeral stops the run.
pub fn leading_value(words: &[&str]) -> Option<(f32, usize)> {
    if let Some(first) = words.first() {
        let raw = first.trim_matches(|c: char| c == '.' || c == ',' || c == '-');
        let decimal = raw.replace(',', ".");
        if !decimal.is_empty()
            && decimal.chars().all(|c| c.is_ascii_digit() || c == '.')
            && let Ok(value) = decimal.parse::<f32>()
            && value.is_finite()
        {
            return Some((value, 1));
        }
        let w = normalize(first);
        if w.starts_with("полтора") || w.starts_with("полторы") {
            return Some((1.5, 1));
        }
    }
    let mut total = 0.0f32;
    let mut used = 0;
    let mut seen = false;
    let mut thousands = false;
    for word in words.iter().take(5) {
        if is_thousand(word) {
            if !seen {
                return None;
            }
            thousands = true;
            used += 1;
            continue;
        }
        match token_value(word) {
            Some(value) => {
                total += value as f32;
                used += 1;
                seen = true;
            }
            None => break,
        }
    }
    if !seen {
        return None;
    }
    if thousands {
        total *= 1000.0;
    }
    Some((total, used))
}

/// The first number in `words`, skipping one or two filler words:
/// "выше порога 700" still reads 700, and "выше тридцати" reads 30.
pub fn first_value(words: &[&str]) -> Option<(f32, usize)> {
    for start in 0..words.len().min(4) {
        if let Some((value, used)) = leading_value(&words[start..]) {
            return Some((value, start + used));
        }
    }
    None
}

fn unit_ms(word: &str) -> Option<u64> {
    let w = normalize(word);
    if w.starts_with("миллисекунд") || w == "мс" || w.starts_with("ms") {
        return Some(1);
    }
    if w.starts_with("секунд") || w.starts_with("сек") || w.starts_with("second") {
        return Some(1_000);
    }
    if w.starts_with("минут") || w == "мин" || w.starts_with("minute") || w.starts_with("min.") {
        return Some(60_000);
    }
    if w.starts_with("час") || w.starts_with("hour") {
        return Some(3_600_000);
    }
    if w.starts_with("сут") || w.starts_with("дн") || w == "день" || w.starts_with("day") {
        return Some(86_400_000);
    }
    if w.starts_with("недел") || w.starts_with("week") {
        return Some(604_800_000);
    }
    None
}

/// Halves written as one word: "полчаса", "пол-часа", "полминуты".
fn half_ms(word: &str) -> Option<u64> {
    let w = normalize(word).replace('-', "");
    if w.starts_with("полчас") {
        return Some(1_800_000);
    }
    if w.starts_with("полминут") {
        return Some(30_000);
    }
    if w.starts_with("полсекунд") {
        return Some(500);
    }
    if w.starts_with("полдня") || w.starts_with("полсут") {
        return Some(43_200_000);
    }
    None
}

/// Read a duration at the start of `words`: "5 секунд", "пять минут",
/// "полчаса", "полтора часа", or a bare unit ("час").
pub fn leading_duration(words: &[&str]) -> Option<(u64, usize)> {
    let first = *words.first()?;
    if let Some(ms) = half_ms(first) {
        return Some((ms, 1));
    }
    if let Some((value, used)) = leading_value(words) {
        for (i, word) in words.iter().enumerate().skip(used).take(2) {
            if let Some(unit) = unit_ms(word) {
                let ms = (value as f64 * unit as f64).round() as u64;
                return (ms > 0).then_some((ms, i + 1));
            }
        }
        return None;
    }
    unit_ms(first).map(|unit| (unit, 1))
}

/// The duration right after a cue phrase: "раз в 5 секунд" -> 5000.
pub fn duration_after(text: &str, cue: &str) -> Option<u64> {
    let at = text.find(cue)?;
    let rest: Vec<&str> = text[at + cue.len()..].split_whitespace().take(6).collect();
    leading_duration(&rest).map(|(ms, _)| ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_numbers_are_read_by_their_stem() {
        assert_eq!(leading_value(&["двадцать", "пять"]), Some((25.0, 2)));
        assert_eq!(leading_value(&["сто", "двадцать", "пять"]), Some((125.0, 3)));
        assert_eq!(leading_value(&["тридцати"]), Some((30.0, 1)));
        assert_eq!(leading_value(&["трёх"]), Some((3.0, 1)));
        assert_eq!(leading_value(&["сорока"]), Some((40.0, 1)));
        assert_eq!(leading_value(&["полтора"]), Some((1.5, 1)));
        assert_eq!(leading_value(&["49"]), Some((49.0, 1)));
        assert_eq!(leading_value(&["порога", "700"]), None);
    }

    #[test]
    fn a_number_is_found_after_filler_words() {
        assert_eq!(first_value(&["порога", "700"]), Some((700.0, 2)));
        assert_eq!(first_value(&["тридцати"]), Some((30.0, 1)));
        assert_eq!(first_value(&["нажат", "нет"]), None);
    }

    #[test]
    fn durations_read_units_halves_and_bare_units() {
        assert_eq!(duration_after("раз в 5 секунд", "раз в"), Some(5_000));
        assert_eq!(duration_after("каждые пять минут", "каждые"), Some(300_000));
        assert_eq!(duration_after("раз в полчаса", "раз в"), Some(1_800_000));
        assert_eq!(duration_after("каждую минуту", "каждую"), Some(60_000));
        assert_eq!(duration_after("раз в час", "раз в"), Some(3_600_000));
        assert_eq!(leading_duration(&["полтора", "часа"]), Some((5_400_000, 2)));
        assert_eq!(duration_after("каждые два дня", "каждые"), Some(172_800_000));
        assert_eq!(duration_after("нет числа", "нет"), None);
    }
}
