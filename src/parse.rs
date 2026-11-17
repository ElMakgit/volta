//! Turning a sentence into a circuit intent.
//!
//! This is deliberately a keyword scorer rather than a model call: it works
//! offline, it is predictable, and when it does not understand something it
//! says so instead of inventing a circuit.

use crate::text::{L, Lang};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Load {
    /// One DC motor on a transistor or relay.
    Motor,
    /// Two or four DC motors behind a driver board.
    Motors,
    /// A 28BYJ-48 stepper with a ULN2003 board.
    Stepper,
    Servo,
    Led,
    Buzzer,
    /// An RC522 RFID reader.
    Rfid,
    /// 433 MHz one-channel radio.
    Radio433,
    /// NRF24L01 2.4 GHz radio.
    RadioNrf,
    /// HC-05/06/07 Bluetooth.
    Bluetooth,
    /// NEOWAY M590 GSM/GPRS modem.
    Gsm,
    /// W5100 or ENC28J60 Ethernet.
    Ethernet,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Input {
    Button,
    Pot,
    Ldr,
}

/// What the sentence asks the circuit to do. The planner still picks the
/// circuit by load and input; the action explains the choice to the user and
/// feeds the "steps" flag where the words mean discrete presses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Blink,
    Fade,
    Measure,
    Display,
    Alarm,
    Toggle,
}

impl Action {
    /// How the verb is shown in the "I read it as" line.
    pub fn label(self) -> L {
        match self {
            Action::Blink => BLINK.label,
            Action::Fade => FADE.label,
            Action::Measure => MEASURE.label,
            Action::Display => DISPLAY.label,
            Action::Alarm => ALARM.label,
            Action::Toggle => TOGGLE.label,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Intent {
    pub load: Load,
    pub input: Option<Input>,
    /// The sentence asked for a transistor, which narrows the driver choice.
    pub transistor: bool,
    /// Asked for repeated presses / discrete steps rather than a hold.
    pub presses: bool,
    /// A lesson of the course books, as an index into `lessons::ALL`.
    pub lesson: Option<usize>,
    /// The verb of the task, when one was recognised.
    pub action: Option<Action>,
    /// A threshold the sentence named: "выше 700", "ниже 20".
    pub condition: Option<crate::spec::Condition>,
    /// A sensor, a condition and a sink: the sentence asks for a built
    /// circuit rather than for the lesson that teaches the sensor.
    pub recipe: Option<crate::spec::Composition>,
    pub raw: String,
}

/// The threshold a sentence names: "выше 700", "меньше 20 см", "ниже 1,5 А".
/// Only a number right after the comparison word counts, so an unrelated
/// number elsewhere in the sentence is not mistaken for a threshold.
fn spoken_condition(raw: &str) -> Option<crate::spec::Condition> {
    use crate::spec::Condition;
    let text = raw.to_lowercase().replace('ё', "е");
    let mut words: Vec<String> = Vec::new();
    let mut acc = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '.' || ch == ',' || ch == '-' {
            acc.push(ch);
        } else if !acc.is_empty() {
            words.push(std::mem::take(&mut acc));
        }
    }
    if !acc.is_empty() {
        words.push(acc);
    }
    let directions: [(&str, bool); 9] = [
        ("выше", true),
        ("больше", true),
        ("свыше", true),
        ("ниже", false),
        ("меньше", false),
        ("above", true),
        ("over", true),
        ("below", false),
        ("under", false),
    ];
    // Numbers stay within the next couple of words, so "выше порога 700"
    // still reads as a threshold.
    let number_after = |i: usize| -> Option<f32> {
        words.iter().skip(i + 1).take(2).find_map(|next| {
            let value = next.trim_end_matches(['.', '-']).replace(',', ".");
            value.parse::<f32>().ok()
        })
    };
    // The first threshold of each kind, so "выше 20 и ниже 30" keeps both.
    let mut above_at: Option<(usize, f32)> = None;
    let mut below_at: Option<(usize, f32)> = None;
    for (i, word) in words.iter().enumerate() {
        let Some((_, above)) = directions.iter().find(|(d, _)| word.starts_with(d)) else {
            continue;
        };
        let Some(n) = number_after(i) else { continue };
        let slot = if *above { &mut above_at } else { &mut below_at };
        if slot.is_none() {
            *slot = Some((i, n));
        }
    }
    // Two conditions on one reading, joined by "и": a window. A sentence
    // without the conjunction keeps its first threshold as before.
    let joined = words
        .iter()
        .any(|w| w == "и" || w == "and" || w == "между" || w == "between");
    if joined {
        if let (Some((_, a)), Some((_, b))) = (above_at, below_at) {
            return Some(Condition::Between {
                low: a.min(b),
                high: a.max(b),
            });
        }
    }
    // The earlier threshold is the one the plan leads with.
    let (above, n) = match (above_at, below_at) {
        (Some(a), Some(b)) => {
            if a.0 <= b.0 {
                (true, a.1)
            } else {
                (false, b.1)
            }
        }
        (Some(a), None) => (true, a.1),
        (None, Some(b)) => (false, b.1),
        (None, None) => return None,
    };
    // "с гистерезисом 50" names the width of the dead band, so the sink
    // turns on at the spoken threshold and off a band later.
    let band = words
        .iter()
        .position(|w| w.starts_with("гистерезис") || w.starts_with("hysteresis"))
        .and_then(number_after)
        .filter(|b| *b > 0.0 && (!above || *b < n));
    Some(match (above, band) {
        (true, Some(b)) => Condition::AboveBand { on: n, off: n - b },
        (false, Some(b)) => Condition::BelowBand { on: n, off: n + b },
        (true, None) => Condition::Above(n),
        (false, None) => Condition::Below(n),
    })
}

#[derive(Clone, Debug)]
pub struct Parse {
    pub intent: Intent,
    pub matched: Vec<String>,
    /// Everything that was recognised, phrased for the user.
    pub understood: Vec<L>,
    /// Filled when the sentence cannot be served by the built-in templates.
    pub unsupported: Option<L>,
    /// Two readings of an ambiguous sentence, offered as one-click choices.
    pub alternatives: Vec<Alternative>,
}

/// Another way to read the same sentence: the label names the difference, the
/// intent is what would be built if the user picks it.
#[derive(Clone, Debug)]
pub struct Alternative {
    pub label: L,
    pub intent: Intent,
}

struct Group {
    words: &'static [&'static str],
    label: L,
}

const MOTOR: Group = Group {
    words: &[
        "мотор", "моторчик", "двигател", "скорост", "оборот", "вентилятор",
        "насос", "кулер", "вертушк", "редуктор", "коллекторн", "motor", "speed",
        "rpm", "fan", "pump",
    ],
    label: L::new("двигатель", "motor"),
};
const MOTORS: Group = Group {
    words: &[
        "драйвер", "l298", "l293", "2 двигател", "два двигател", "двумя двигател",
        "двух двигател", "4 двигател", "четыре двигател", "motor driver",
    ],
    label: L::new("драйвер двигателей", "motor driver"),
};
const STEPPER: Group = Group {
    words: &[
        "шагов", "шаговик", "степпер", "stepper", "28byj", "uln2003",
        "биполярн", "униполярн",
    ],
    label: L::new("шаговый двигатель", "stepper"),
};
const RFID: Group = Group {
    words: &[
        "rfid", "rc522", "nfc", "метк", "карт", "считыват", "турникет", "пропуск",
    ],
    label: L::new("RFID-считыватель", "RFID reader"),
};
const RADIO433: Group = Group {
    words: &[
        "433", "fs1000a", "xy-mk", "радиопередатчик", "радиоприем", "приемник",
        "радиомодул", "одноканальн",
    ],
    label: L::new("радио 433 МГц", "433 MHz radio"),
};
const RADIO_NRF: Group = Group {
    words: &[
        "nrf24", "nrf", "2.4 ггц", "2.4 ghz", "многоканальн", "2.4g",
    ],
    label: L::new("радио NRF24L01", "NRF24L01 radio"),
};
const BLUETOOTH: Group = Group {
    words: &[
        "bluetooth", "блютуз", "блутуз", "hc-05", "hc05", "hc-06", "hc-07",
        "нс-05", "нс-06", "нс-07",
    ],
    label: L::new("Bluetooth-модуль", "Bluetooth module"),
};
const GSM: Group = Group {
    words: &[
        "gsm", "gprs", "neoway", "m590", "смс", "sms", "звонок", "сим-карт", "sim",
    ],
    label: L::new("GSM-модем", "GSM modem"),
};
const ETHERNET: Group = Group {
    words: &[
        "ethernet", "enc28j60", "w5100", "интернет", "веб", "web", "браузер",
        "ip-адрес", "html",
    ],
    label: L::new("Ethernet", "Ethernet"),
};
const LED: Group = Group {
    words: &[
        "светодиод", "светодиот", "лед", "led", "лампоч", "лампа", "подсветк",
        "ночник", "фонар", "гирлянд", "огонек", "индикатор", "rgb",
        "blink", "light",
    ],
    label: L::new("светодиод", "LED"),
};
const SERVO: Group = Group {
    words: &["серво", "серва", "servo", "поворот", "угол", "стрелк", "заслонк", "машинк"],
    label: L::new("сервопривод", "servo"),
};
const BUZZER: Group = Group {
    words: &[
        "пьезо", "динамик", "звук", "мелод", "сирен", "пища", "пищалка", "сигнал",
        "звонок", "будильник", "колонк", "спикер", "buzzer", "tone", "speaker",
        "beep",
    ],
    label: L::new("звук", "sound"),
};
const BUTTON: Group = Group {
    words: &["кнопк", "кнопочк", "нажат", "button", "press", "щелч", "клавиш", "такт"],
    label: L::new("кнопка", "button"),
};
const POT: Group = Group {
    words: &[
        "потенциометр", "регулятор", "крутилк", "подстроечн", "переменн резистор",
        "potentiometer", "pot",
    ],
    label: L::new("потенциометр", "potentiometer"),
};
const LDR: Group = Group {
    words: &[
        "фоторезистор", "фотоэлемент", "светочувств", "ldr", "освещ", "темно",
        "сумерк", "датчик свет", "свет датчик", "light sensor", "light level",
        "photoresistor",
    ],
    label: L::new("освещённость", "light level"),
};
const TRANSISTOR: Group = Group {
    words: &[
        "транзистор", "mosfet", "полев", "полевик", "мосфет", "ключ", "bjt",
        "npn", "pnp", "irf", "irlz", "irfz",
    ],
    label: L::new("транзисторный ключ", "transistor switch"),
};
const PRESSES: Group = Group {
    words: &[
        "неоднократн", "несколько нажат", "повторн", "каждое нажат", "ступен",
        "ступенями", "шаг", "по одному", "step", "repeated",
    ],
    label: L::new("ступени по нажатиям", "steps per press"),
};

/// The verbs, in the order they are tried: a sentence may say "плавно
/// мигать", and the fade is the more specific reading.
const FADE: Group = Group {
    words: &[
        "плавн", "затуха", "разгора", "затух", "погаса", "шим", "pwm", "fade",
        "шимм",
    ],
    label: L::new("плавное изменение", "fade"),
};
const BLINK: Group = Group {
    words: &["мига", "морга", "мигну", "вспышк", "blink", "flash"],
    label: L::new("мигание", "blinking"),
};
const MEASURE: Group = Group {
    words: &[
        "измер", "мерить", "замер", "считыва", "опрос", "measure", "read",
    ],
    label: L::new("измерение", "measuring"),
};
const DISPLAY: Group = Group {
    words: &[
        "вывест", "вывод на", "показать на", "показыва", "диспле", "экран",
        "табло", "display", "show",
    ],
    label: L::new("вывод на экран", "showing on a display"),
};
const ALARM: Group = Group {
    words: &[
        "сигнал", "тревог", "оповещ", "предупред", "пищ", "alarm", "alert", "warn",
    ],
    label: L::new("сигнал", "alarm"),
};
const TOGGLE: Group = Group {
    words: &[
        "включ", "выключ", "переключ", "toggle", "on off", "switch on", "turn on", "turn off",
        "turns on", "turns off", "enable", "disable",
    ],
    label: L::new("включение и выключение", "on and off"),
};

fn hits(text: &str, group: &Group) -> Vec<String> {
    group
        .words
        .iter()
        .filter(|w| crate::words::mentions(text, w))
        .map(|w| (*w).to_string())
        .collect()
}

/// Every group that matched, best first: the group with more distinct words
/// wins, and the order of the groups breaks a tie. The order puts a named
/// board ahead of the generic word for the same thing, so "драйвер
/// двигателей" is not read as a single motor.
fn votes(text: &str, groups: &[&Group]) -> Vec<(usize, Vec<String>)> {
    let mut found: Vec<(usize, Vec<String>)> = groups
        .iter()
        .enumerate()
        .filter_map(|(i, g)| {
            let h = hits(text, g);
            (!h.is_empty()).then_some((i, h))
        })
        .collect();
    found.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    found
}

fn load_at(i: usize) -> Load {
    match i {
        0 => Load::Stepper,
        1 => Load::Motors,
        2 => Load::Rfid,
        3 => Load::RadioNrf,
        4 => Load::Radio433,
        5 => Load::Bluetooth,
        6 => Load::Gsm,
        7 => Load::Ethernet,
        8 => Load::Motor,
        9 => Load::Servo,
        10 => Load::Buzzer,
        _ => Load::Led,
    }
}

fn load_label(load: Load) -> L {
    match load {
        Load::Motor => MOTOR.label,
        Load::Motors => MOTORS.label,
        Load::Stepper => STEPPER.label,
        Load::Servo => SERVO.label,
        Load::Led => LED.label,
        Load::Buzzer => BUZZER.label,
        Load::Rfid => RFID.label,
        Load::Radio433 => RADIO433.label,
        Load::RadioNrf => RADIO_NRF.label,
        Load::Bluetooth => BLUETOOTH.label,
        Load::Gsm => GSM.label,
        Load::Ethernet => ETHERNET.label,
    }
}

fn input_at(i: usize) -> Input {
    match i {
        0 => Input::Button,
        1 => Input::Pot,
        _ => Input::Ldr,
    }
}

fn input_label(input: Input) -> L {
    match input {
        Input::Button => BUTTON.label,
        Input::Pot => POT.label,
        Input::Ldr => LDR.label,
    }
}

/// A short name for what a load-plus-input pair builds, used on the buttons
/// that offer another reading of an ambiguous sentence.
fn plan_label(load: Load, input: Option<Input>) -> L {
    match (load, input) {
        (Load::Motor, Some(Input::Button)) => {
            L::new("двигатель: ступени кнопкой", "motor: steps by button")
        }
        (Load::Motor, Some(Input::Pot)) => {
            L::new("двигатель: скорость потенциометром", "motor: speed by knob")
        }
        (Load::Motor, _) => L::new("двигатель: тест ключом", "motor: switch test"),
        (Load::Servo, Some(Input::Pot)) => L::new("серво по потенциометру", "servo by knob"),
        (Load::Servo, _) => L::new("серво: плавный проход", "servo sweep"),
        (Load::Led, Some(Input::Button)) => L::new("светодиод по кнопке", "LED by button"),
        (Load::Led, Some(Input::Ldr)) => L::new("ночник по фоторезистору", "night light"),
        (Load::Led, _) => L::new("светодиод: мигание", "LED blink"),
        (other, _) => load_label(other),
    }
}

/// The sentence as the parser compares it: lower case, without punctuation,
/// with the common endings folded, plus the same words re-read in the other
/// keyboard layout. Everything is matched against this one string, so
/// "Моторчиком!" and "rjyrf" are both understood.
fn search_text(raw: &str) -> String {
    let norm = crate::words::normalize(raw);
    let mut out = norm.clone();
    for fixed in crate::words::layout_fixes(&norm) {
        out.push(' ');
        out.push_str(&fixed);
    }
    out
}

/// A display asked for in words. A book task names the lesson's sensor, but
/// the reader still has to honour "на любой тип дисплея" or "два экрана".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenKind {
    Any,
    Lcd,
    Lcd2004,
    Oled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screens {
    pub kind: ScreenKind,
    pub count: usize,
}

/// Everything the user named outright. A ready-made lesson or a template is
/// a starting point, not an excuse to drop parts the sentence asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NamedDevices {
    pub screens: Option<Screens>,
    pub buzzer: usize,
    pub relay: usize,
    pub led: usize,
    pub motor: usize,
    pub button: usize,
    pub hourly: bool,
}

fn count_of(text: &str, words: &[&str]) -> usize {
    if !words.iter().any(|w| text.contains(w)) {
        return 0;
    }
    for (word, n) in [
        ("четыре", 4usize), ("четырёх", 4), ("4", 4), ("три", 3), ("трёх", 3), ("3", 3),
        ("два", 2), ("две", 2), ("двух", 2), ("2", 2),
    ] {
        if text.split_whitespace().any(|t| t == word) {
            return n;
        }
    }
    1
}

pub fn named_devices(raw: &str) -> NamedDevices {
    let text = crate::words::normalize(raw);
    NamedDevices {
        screens: screens_in(raw),
        buzzer: count_of(&text, &["пьезо", "пищал", "зуммер", "сирен", "buzzer", "gong"]),
        relay: count_of(&text, &["реле", "relay"]),
        led: count_of(&text, &["светодиод", "led", "индик"]) ,
        motor: count_of(&text, &["двигател", "мотор", "motor", "вентилятор", "насос"]),
        button: count_of(&text, &["кнопк", "button", "переключател"]),
        hourly: text.contains("каждый час") || text.contains("раз в час")
            || text.contains("hourly") || text.contains("every hour"),
    }
}

pub fn screens_in(raw: &str) -> Option<Screens> {
    let text = crate::words::normalize(raw);
    let named = ["диспле", "экран", "табло", "lcd", "oled", "display", "128x64", "128 64", "16x2", "2004"];
    let is_screen = named.iter().any(|k| text.contains(k));
    if !is_screen || (text.contains("порт") && !text.contains("диспле") && !text.contains("экран")) {
        return None;
    }
    let kind = if text.contains("oled") || text.contains("128x64") || text.contains("128 64") {
        ScreenKind::Oled
    } else if text.contains("2004") || text.contains("20 4") {
        ScreenKind::Lcd2004
    } else if text.contains("lcd") || text.contains("16x2") {
        ScreenKind::Lcd
    } else {
        ScreenKind::Any
    };
    let count = if ["два", "две", "двух", "2", "two", "pair"]
        .iter()
        .any(|w| text.split_whitespace().any(|t| t == *w))
    {
        2
    } else if ["три", "трёх", "3", "three"]
        .iter()
        .any(|w| text.split_whitespace().any(|t| t == *w))
    {
        3
    } else {
        1
    };
    Some(Screens { kind, count })
}

pub fn parse(raw: &str) -> Parse {
    parse_inner(raw, true)
}

/// A task copied word for word is answered by its ready-made solution
/// instead of being guessed from a few keywords.
#[cfg(feature = "book-tasks")]
fn exact_book_task(raw: &str) -> Option<Parse> {
    crate::book_tasks::solution_for(raw).map(|solution| from_book_solution(raw, solution))
}

/// A sentence that is close to a printed task keeps that task's solution even
/// though a word or two changed. The similarity check is deliberately
/// conservative and never overrides a short keyword phrase.
#[cfg(feature = "book-tasks")]
fn fuzzy_book_task(raw: &str) -> Option<Parse> {
    crate::book_tasks::closest(raw).map(|(_, solution, _)| from_book_solution(raw, solution))
}

#[cfg(feature = "book-tasks")]
fn from_book_solution(raw: &str, solution: crate::book_tasks::Solution) -> Parse {
    use crate::book_tasks::Solution;
    match solution {
        Solution::Lesson(i) => {
            let mut parsed = parse_inner(crate::lessons::ALL[i].title.ru, false);
            // A bare title such as "Кнопка" is also an everyday word, so the
            // title alone may route elsewhere. The book already says which
            // lesson the task belongs to.
            parsed.intent.lesson = Some(i);
            parsed.unsupported = None;
            parsed.alternatives.clear();
            parsed.intent.raw = raw.to_string();
            parsed
        }
        Solution::Seed(seed) => {
            let normal = parse_inner(raw, false);
            if normal.unsupported.is_some() {
                let mut parsed = parse_inner(seed, false);
                parsed.intent.raw = raw.to_string();
                parsed
            } else {
                normal
            }
        }
    }
}

#[cfg(not(feature = "book-tasks"))]
fn exact_book_task(_raw: &str) -> Option<Parse> {
    None
}

#[cfg(not(feature = "book-tasks"))]
fn fuzzy_book_task(_raw: &str) -> Option<Parse> {
    None
}

fn parse_inner(raw: &str, books: bool) -> Parse {
    let _ = books;
    // The user's own sentence stays in `raw`, so numbers in it ("на 49 карт")
    // still reach the sketch.
    if books {
        if let Some(parsed) = exact_book_task(raw) {
            return parsed;
        }
        if let Some(parsed) = fuzzy_book_task(raw) {
            return parsed;
        }
    }
    let text = search_text(raw);
    let mut matched = Vec::new();
    let mut understood = Vec::new();

    // Specific boards come before the generic word "мотор", otherwise an
    // L298N task would be read as a single-transistor motor switch.
    let load_groups = [
        &STEPPER, &MOTORS, &RFID, &RADIO_NRF, &RADIO433, &BLUETOOTH, &GSM, &ETHERNET,
        &MOTOR, &SERVO, &BUZZER, &LED,
    ];
    let load_votes = votes(&text, &load_groups);
    let (load, load_hits) = match load_votes.first() {
        Some((i, h)) => (load_at(*i), h.clone()),
        None => (Load::Led, Vec::new()),
    };
    let named_load = !load_hits.is_empty();
    matched.extend(load_hits);
    if named_load {
        understood.push(load_label(load));
    }

    let input_groups = [&BUTTON, &POT, &LDR];
    let input_votes = votes(&text, &input_groups);
    let (input, input_hits) = match input_votes.first() {
        Some((i, h)) => (Some(input_at(*i)), h.clone()),
        None => (None, Vec::new()),
    };
    let named_input = !input_hits.is_empty();
    matched.extend(input_hits);
    if let Some(i) = input {
        understood.push(input_label(i));
    }

    let transistor = !hits(&text, &TRANSISTOR).is_empty();
    if transistor {
        matched.extend(hits(&text, &TRANSISTOR));
        understood.push(TRANSISTOR.label);
    }
    let presses = !hits(&text, &PRESSES).is_empty();
    if presses {
        matched.extend(hits(&text, &PRESSES));
        understood.push(PRESSES.label);
    }

    // The verb explains why the circuit looks the way it does. Fade is asked
    // first: "плавно мигать" is a fade, not a plain blink.
    let action_groups: [(Action, &Group); 6] = [
        (Action::Fade, &FADE),
        (Action::Blink, &BLINK),
        (Action::Measure, &MEASURE),
        (Action::Display, &DISPLAY),
        (Action::Alarm, &ALARM),
        (Action::Toggle, &TOGGLE),
    ];
    let action = action_groups
        .iter()
        .find_map(|(a, g)| (!hits(&text, g).is_empty()).then_some(*a));

    let semantic = crate::templates::route(load, input);
    // A named lesson from the course books is more specific than the generic
    // templates: if the sentence names a board the books teach, build that.
    // Everyday words like "термометр" stand in for a part only when the
    // sentence names nothing else: "шуметь динамиком" must stay a buzzer.
    let mut lesson = crate::lessons::find_with(&text, !named_load && !transistor);
    // EEPROM is a way to keep data, not the thing the circuit drives: in
    // "контроль доступа на 49 карт ... в EEPROM" the reader is named, so the
    // counter lesson (a button and EEPROM) must not win over the RFID lock.
    if named_load
        && lesson.is_some_and(|i| crate::lessons::ALL[i].keys.iter().any(|k| *k == "eeprom"))
    {
        lesson = None;
    }
    // "Пусть лампочка плавно гаснет" is the PWM lesson: same parts as the
    // blink, but the code fades instead of switching.
    if lesson.is_none()
        && load == Load::Led
        && input.is_none()
        && matches!(action, Some(Action::Fade))
    {
        lesson = Some(crate::lessons::PWM);
    }
    if let Some(i) = lesson {
        understood.push(crate::lessons::ALL[i].title);
    }
    // The verb comes after the thing it applies to: "датчик газа · мигание"
    // reads better than the other way round.
    if let Some(a) = action {
        understood.push(a.label());
    }
    // A sensor plus a verb is a composition, not merely the lesson that
    // teaches the sensor: "датчик газа, который мигает" becomes a circuit.
    let condition = spoken_condition(raw);
    // "пьезопищалка" in a clock task is a buzzer, not a sound sensor. Never
    // turn an RTC build into a microphone readout because of that word.
    let clock_tick = text.contains("часы реальн")
        || text.contains("ds3231")
        || text.contains("ds1307")
        || text.contains("rtc");
    let beeper = text.contains("пищал") || text.contains("buzzer") || text.contains("зуммер");
    let recipe = if clock_tick && beeper {
        None
    } else {
        crate::compose::detect(raw, load, named_load, lesson, action, condition)
    };
    let mut refusal = None;
    if let Some(r) = recipe {
        if !crate::compose::fits(r) {
            refusal = Some(L::new(
                "У этого датчика не осталось свободного выхода для приёмника: все его пины заняты. Выбери другой приёмник или другой способ чтения.",
                "This sensor leaves no free pin for the output: every pin it uses is taken. Pick another output or another way of reading it.",
            ));
        }
    }
    if let Some(c) = condition {
        understood.push(c.label());
    }
    // A composition is read back as its recipe: source · condition · action.
    // That is more useful than the lesson title and the bare verb.
    if let Some(r) = recipe {
        understood = crate::compose::recipe_words(r);
    }

    // A sentence that names only an input still builds when the plan really
    // uses that input: a button switches the LED, a photoresistor makes the
    // night light. A knob that the plan would ignore is not a plan.
    let input_matters =
        input.is_some() && semantic != crate::templates::route(load, None);
    let unsupported = if let Some(r) = refusal {
        Some(r)
    } else if lesson.is_some() {
        None
    } else if !named_load && !input_matters {
        Some(L::new(
            "Не нашёл, чем управлять. Назови, что должно работать: двигатель, шаговик, светодиод, сервопривод, RFID, радио, Bluetooth, GSM, Ethernet — или что измеряем: температура, влажность, свет, давление, расстояние, газ, звук.",
            "I could not tell what should be driven. Name it: a motor, a stepper, an LED, a servo, RFID, radio, Bluetooth, GSM, Ethernet — or what to measure: temperature, humidity, light, pressure, distance, gas, sound.",
        ))
    } else if semantic.is_none() {
        Some(match (load, input) {
            (Load::Servo, _) => L::new(
                "Сервопривод собирается с потенциометром или плавным проходом по углам. Убери лишний вход из описания: по кнопке серво не собирается.",
                "A servo is built with a potentiometer or as a sweep. Drop the extra input: a button-driven servo is not built.",
            ),
            _ => L::new(
                "Такая пара «что управляем + чем управляем» пока не собрана. Доступно: двигатель (ключ, потенциометр, кнопка), драйвер L298N, шаговик, серво, светодиод, звук, RFID, радио 433/NRF24, Bluetooth, GSM и Ethernet.",
                "That combination is not built yet. Available: a motor (switch, knob, button), the L298N driver, a stepper, a servo, an LED, a buzzer, RFID, 433 MHz/NRF24 radio, Bluetooth, GSM and Ethernet.",
            ),
        })
    } else {
        None
    };

    // Ambiguity is answered with one-click readings instead of a silent
    // coin toss: two loads named with equal force, or two inputs.
    let intent_of = |load: Load, input: Option<Input>| Intent {
        load,
        input,
        transistor,
        presses,
        lesson: None,
        action,
        condition,
        recipe,
        raw: raw.to_string(),
    };
    let mut alternatives = Vec::new();
    if load_votes.len() > 1 && load_votes[1].1.len() == load_votes[0].1.len() {
        for (i, _) in load_votes.iter().skip(1).take(2) {
            let other = load_at(*i);
            if other != load
                && crate::templates::route(other, input).is_some()
                && alternatives.len() < 2
            {
                alternatives.push(Alternative {
                    label: plan_label(other, input),
                    intent: intent_of(other, input),
                });
            }
        }
    }
    if input_votes.len() > 1 && input_votes[1].1.len() == input_votes[0].1.len() {
        for (i, _) in input_votes.iter().skip(1).take(2) {
            let other = Some(input_at(*i));
            if other != input
                && crate::templates::route(load, other).is_some()
                && alternatives.len() < 2
            {
                alternatives.push(Alternative {
                    label: input_label(input_at(*i)),
                    intent: intent_of(load, other),
                });
            }
        }
    }
    // Nothing was named as a load but an input was: offer the circuits that
    // can use that input instead of guessing one.
    if !named_load && named_input && alternatives.is_empty() {
        for candidate in [load, Load::Motor, Load::Servo, Load::Buzzer] {
            if candidate == load || crate::templates::route(candidate, input).is_none() {
                continue;
            }
            if alternatives.len() >= 2 {
                break;
            }
            alternatives.push(Alternative {
                label: plan_label(candidate, input),
                intent: intent_of(candidate, input),
            });
        }
    }

    matched.sort();
    matched.dedup();
    Parse {
        intent: Intent {
            load,
            input,
            transistor,
            presses,
            lesson,
            action,
            condition,
            recipe,
            raw: raw.to_string(),
        },
        matched,
        understood,
        unsupported,
        alternatives,
    }
}

/// Short label for the recognised task, used in the plan header.
pub fn describe(intent: &Intent, lang: Lang) -> String {
    let load = match intent.load {
        Load::Motor => L::new("двигатель", "motor"),
        Load::Motors => L::new("драйвер двигателей", "motor driver"),
        Load::Stepper => L::new("шаговый двигатель", "stepper"),
        Load::Servo => L::new("сервопривод", "servo"),
        Load::Led => L::new("светодиод", "LED"),
        Load::Buzzer => L::new("звук", "buzzer"),
        Load::Rfid => L::new("RFID", "RFID"),
        Load::Radio433 => L::new("радио 433 МГц", "433 MHz radio"),
        Load::RadioNrf => L::new("NRF24L01", "NRF24L01"),
        Load::Bluetooth => L::new("Bluetooth", "Bluetooth"),
        Load::Gsm => L::new("GSM", "GSM"),
        Load::Ethernet => L::new("Ethernet", "Ethernet"),
    };
    let input = match intent.input {
        Some(Input::Button) => Some(L::new("кнопка", "button")),
        Some(Input::Pot) => Some(L::new("потенциометр", "potentiometer")),
        Some(Input::Ldr) => Some(L::new("фоторезистор", "photoresistor")),
        None => None,
    };
    match input {
        Some(i) => format!(
            "{} + {}",
            load.get(lang),
            i.get(lang)
        ),
        None => load.get(lang).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_the_motor_button_example() {
        let p = parse(
            "Составить программу увеличения скорости двигателя с помощью неоднократного нажатия кнопки на полевом n-канальном транзисторе.",
        );
        assert_eq!(p.intent.load, Load::Motor);
        assert_eq!(p.intent.input, Some(Input::Button));
        assert!(p.intent.transistor);
        assert!(p.intent.presses);
        assert!(p.unsupported.is_none(), "{:?}", p.unsupported);
    }

    #[test]
    fn recognises_english_dimming() {
        let p = parse("Blink an LED, and let a button toggle it");
        assert_eq!(p.intent.load, Load::Led);
        assert_eq!(p.intent.input, Some(Input::Button));
    }

    #[test]
    fn night_light_needs_both_words() {
        let p = parse("ночник: фоторезистор и светодиод");
        assert_eq!(p.intent.load, Load::Led);
        assert_eq!(p.intent.input, Some(Input::Ldr));
    }

    #[test]
    fn recognises_every_lesson_from_the_book() {
        let cases = [
            ("Подключение к МК драйвера управления 2 или 4 двигателями", Load::Motors),
            ("Подключение шагового двигателя на 4 или 8 тактов", Load::Stepper),
            ("Считыватель RFID RC522, UID метки в порт", Load::Rfid),
            ("Радиопередатчик и приёмник 433 МГц", Load::Radio433),
            ("NRF24L01 + 2.4 ГГц, структура данных", Load::RadioNrf),
            ("Блютуз модуль HC-05 и телефон", Load::Bluetooth),
            ("GSM/GPRS NEOWAY M590, звонок по кнопке", Load::Gsm),
            ("Ethernet Shield W5100 или ENC28J60", Load::Ethernet),
            ("Тест двигателя на полевом n-канальном транзисторе", Load::Motor),
            ("Шаговый двигатель, потенциометр и кнопка", Load::Stepper),
        ];
        for (text, load) in cases {
            let p = parse(text);
            assert_eq!(p.intent.load, load, "{text}");
            assert!(p.unsupported.is_none(), "{text}: {:?}", p.unsupported);
        }
    }

    #[test]
    fn unknown_task_is_reported_not_invented() {
        let p = parse("посчитай что-нибудь на питоне");
        assert!(p.unsupported.is_some());
        for text in ["погода", "напиши стихи", "потенциометр"] {
            assert!(parse(text).unsupported.is_some(), "{text}");
        }
    }

    fn load_named(name: &str) -> Load {
        match name {
            "motor" => Load::Motor,
            "motors" => Load::Motors,
            "stepper" => Load::Stepper,
            "servo" => Load::Servo,
            "led" => Load::Led,
            "buzzer" => Load::Buzzer,
            "rfid" => Load::Rfid,
            "radio433" => Load::Radio433,
            "radionrf" => Load::RadioNrf,
            "bluetooth" => Load::Bluetooth,
            "gsm" => Load::Gsm,
            "ethernet" => Load::Ethernet,
            other => panic!("unknown load in the corpus: {other}"),
        }
    }

    fn input_named(name: &str) -> Input {
        match name {
            "button" => Input::Button,
            "pot" => Input::Pot,
            "ldr" => Input::Ldr,
            other => panic!("unknown input in the corpus: {other}"),
        }
    }

    /// The corpus is the contract: every line says what the sentence must
    /// become. It covers cases, synonyms, slang, typos, the wrong keyboard
    /// layout, English, refusals and ambiguity.
    #[test]
    fn corpus_phrases_parse_as_expected() {
        let corpus = include_str!("testdata/phrases.txt");
        let mut checked = 0;
        for (n, raw) in corpus.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            checked += 1;
            let at = n + 1;
            let (phrase, want) = line
                .split_once('|')
                .unwrap_or_else(|| panic!("line {at}: no | in {line:?}"));
            let phrase = phrase.trim();
            let want = want.trim();
            let p = parse(phrase);
            match want {
                "refuse" => {
                    assert!(
                        p.unsupported.is_some(),
                        "line {at}: {phrase:?} should be refused, understood {:?}",
                        p.understood
                    );
                    assert!(
                        p.alternatives.is_empty(),
                        "line {at}: {phrase:?} should be a plain refusal"
                    );
                }
                "alt" => assert!(
                    !p.alternatives.is_empty(),
                    "line {at}: {phrase:?} should offer readings, understood {:?}",
                    p.understood
                ),
                _ => {
                    assert!(
                        p.unsupported.is_none(),
                        "line {at}: {phrase:?} was refused: {:?}",
                        p.unsupported
                    );
                    if let Some(idx) = want.strip_prefix("lesson:") {
                        let idx: usize = idx.parse().unwrap();
                        assert_eq!(
                            p.intent.lesson,
                            Some(idx),
                            "line {at}: {phrase:?} went to {:?}",
                            p.intent.lesson
                        );
                    } else if let Some(spec) = want.strip_prefix("compose:") {
                        let r = p.intent.recipe.unwrap_or_else(|| {
                            panic!("line {at}: {phrase:?} did not compose, understood {:?}", p.understood)
                        });
                        let got = format!("{:?}/{:?}/{:?}", r.source, r.sink, r.behavior);
                        assert_eq!(got, spec, "line {at}: {phrase:?} composed as {got}");
                    } else if let Some(load) = want.strip_prefix("load:") {
                        let (l, i) = match load.split_once('+') {
                            Some((l, i)) => (l, Some(i)),
                            None => (load, None),
                        };
                        assert_eq!(
                            p.intent.load,
                            load_named(l),
                            "line {at}: {phrase:?} load, understood {:?}",
                            p.understood
                        );
                        assert_eq!(
                            p.intent.input,
                            i.map(input_named),
                            "line {at}: {phrase:?} input, understood {:?}",
                            p.understood
                        );
                    } else {
                        panic!("line {at}: unknown expectation {want:?}");
                    }
                }
            }
        }
        assert!(checked >= 300, "the corpus shrank to {checked} lines");
    }

    /// Every lesson title is understood, and every title but the plain
    /// "Кнопка" routes to its own lesson.
    #[test]
    fn every_lesson_title_is_understood() {
        for (i, lesson) in crate::lessons::ALL.iter().enumerate() {
            let p = parse(lesson.title.ru);
            assert!(
                p.unsupported.is_none(),
                "lesson {i} title {:?} was refused",
                lesson.title.ru
            );
            if i != 5 {
                assert_eq!(
                    p.intent.lesson,
                    Some(i),
                    "lesson {i} title {:?} went elsewhere",
                    lesson.title.ru
                );
            }
        }
    }

    /// A lamp named by its everyday word is still a load, and a light sensor
    /// plus that word is the night light, not a refusal.
    #[test]
    fn everyday_load_words_are_understood() {
        for text in ["ночник на фоторезисторе", "гирлянда", "фонарь по кнопке"] {
            let p = parse(text);
            assert_eq!(p.intent.load, Load::Led, "{text}");
            assert!(p.unsupported.is_none(), "{text}: {:?}", p.unsupported);
        }
        assert_eq!(parse("ночник на фоторезисторе").intent.input, Some(Input::Ldr));
    }
}
