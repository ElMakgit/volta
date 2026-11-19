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
    /// An incandescent lamp switched by a transistor or a relay. It is a
    /// heavy load, not a light-emitting diode, so it never gets a series
    /// resistor or a "blink" sketch.
    Lamp,
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
    /// The sentence asks for a text message on one or more displays and
    /// names nothing else to drive. `screens.count` is the number of boards.
    pub text_screens: Option<Screens>,
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
        let rest: Vec<&str> = words.iter().skip(i + 1).take(4).map(String::as_str).collect();
        crate::numbers::first_value(&rest).map(|(value, _)| value)
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
    /// The words behind every decision, for `volta explain`. Technical, so
    /// it stays in one language instead of being translated with the UI.
    pub trace: Vec<String>,
}

/// Another way to read the same sentence: the label names the difference, the
/// intent is what would be built if the user picks it.
#[derive(Clone, Debug)]
pub struct Alternative {
    pub label: L,
    pub intent: Intent,
    /// Extra context for the hover, such as the printed task a fuzzy match
    /// came from. Free text, shown as is.
    pub note: Option<String>,
}

struct Group {
    words: &'static [&'static str],
    label: L,
}

const MOTOR: Group = Group {
    words: &[
        "мотор", "моторчик", "двигател", "скорост", "оборот", "вентилятор",
        "насос", "кулер", "вертушк", "редуктор", "коллекторн", "motor", "speed",
        "rpm", "fan", "pump", "полив", "поливалк", "полива", "орошен",
        "капельн", "фонтан", "скважин",
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
        "радиомодул", "одноканальн", "радио",
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
    words: &[
        "серво", "серва", "servo", "поворот", "поверн", "угол", "стрелк", "заслонк",
        "машинк", "sg90", "sg-90", "mg90", "mg995", "mg996", "fs90", "towerpro",
        "привод заслонк",
    ],
    label: L::new("сервопривод", "servo"),
};

/// An incandescent lamp is a heavy load, not an LED. The words are narrow on
/// purpose: a bare "лампа" still means a small light and stays an LED, while
/// "накаливания", "галоген" and a mains voltage name the real thing.
const LAMP: Group = Group {
    words: &[
        "накалив",
        "галоген",
        "диммер",
        "incandescent",
        "лампа накаливания",
        "лампа на 12",
        "лампа на 220",
        "лампочка на 12",
        "лампочка на 220",
    ],
    label: L::new("лампа накаливания", "incandescent lamp"),
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
        "photoresistor", "вечер", "ночь", "рассвет", "закат", "twilight",
        "dusk", "dawn", "ночник", "night light",
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
        "включ", "выключ", "переключ", "откры", "закры", "отвор", "затвор",
        "toggle", "on off", "switch on", "turn on", "turn off",
        "turns on", "turns off", "enable", "disable", "open", "close",
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

/// Token positions a keyword explains. Used to score a group without
/// counting two near-identical keys for the same word twice: the LED group
/// lists both the correct spelling and a common typo, and on the sentence
/// "RFID и светодиод" that must not outweigh the RFID the user named first.
fn match_indices(text: &str, key: &str) -> Vec<usize> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let n = crate::words::normalize(key).split_whitespace().count().max(1);
    if n == 1 {
        return tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| crate::words::mentions(t, key))
            .map(|(i, _)| i)
            .collect();
    }
    if tokens.len() < n {
        return Vec::new();
    }
    (0..=tokens.len() - n)
        .filter(|&i| crate::words::mentions(&tokens[i..i + n].join(" "), key))
        .collect()
}

/// Every group that matched, best first: the group with more distinct words
/// wins, and the order of the groups breaks a tie. The order puts a named
/// board ahead of the generic word for the same thing, so "драйвер
/// двигателей" is not read as a single motor.
fn votes(text: &str, groups: &[&Group]) -> Vec<(usize, Vec<String>)> {
    let mut found: Vec<(usize, Vec<String>, usize)> = groups
        .iter()
        .enumerate()
        .filter_map(|(i, g)| {
            let h = hits(text, g);
            if h.is_empty() {
                return None;
            }
            let mut claimed: Vec<usize> = Vec::new();
            let mut fresh = 0usize;
            for w in &h {
                let at = match_indices(text, w);
                if at.is_empty() || at.iter().all(|j| claimed.contains(j)) {
                    continue;
                }
                claimed.extend(at);
                fresh += 1;
            }
            Some((i, h, fresh.max(1)))
        })
        .collect();
    found.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    found.into_iter().map(|(i, h, _)| (i, h)).collect()
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
        11 => Load::Lamp,
        _ => Load::Led,
    }
}

fn load_group(load: Load) -> &'static Group {
    match load {
        Load::Motor => &MOTOR,
        Load::Motors => &MOTORS,
        Load::Stepper => &STEPPER,
        Load::Servo => &SERVO,
        Load::Lamp => &LAMP,
        Load::Led => &LED,
        Load::Buzzer => &BUZZER,
        Load::Rfid => &RFID,
        Load::Radio433 => &RADIO433,
        Load::RadioNrf => &RADIO_NRF,
        Load::Bluetooth => &BLUETOOTH,
        Load::Gsm => &GSM,
        Load::Ethernet => &ETHERNET,
    }
}

fn load_label(load: Load) -> L {
    match load {
        Load::Motor => MOTOR.label,
        Load::Motors => MOTORS.label,
        Load::Stepper => STEPPER.label,
        Load::Servo => SERVO.label,
        Load::Lamp => LAMP.label,
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

/// An infrared part is not a 433 MHz radio, although both are called a
/// "receiver" in everyday speech. The parser has separate lessons for the
/// TSOP1838 and the IR emitter, so a bare "приёмник" must not claim them.
fn ir_part(text: &str) -> bool {
    text.split_whitespace().any(|t| {
        t == "ик"
            || t.starts_with("ик-")
            || t.starts_with("инфракрас")
            || t == "tsop"
            || t.starts_with("tsop")
            || t.starts_with("ir-")
            || t == "ir"
    })
}

/// Words that make the sentence a real 433 MHz radio task. An IR receiver
/// with a remote control is not one.
fn radio_433_context(text: &str) -> bool {
    [
        "433",
        "fs1000a",
        "xy-mk",
        "радиомодул",
        "радиопередатчик",
        "радиоприем",
        "радиочастот",
    ]
    .iter()
    .any(|k| text.contains(k))
}

/// A card in a reader is not a memory card. The SD lesson is a separate
/// circuit, so the RFID group must not win merely on the word "карта".
fn sd_storage(text: &str) -> bool {
    let card = text.contains("sd")
        || text.contains("microsd")
        || text.contains("флеш")
        || text.contains("flash")
        || text.contains("карта памяти");
    let access = [
        "доступ", "пропуск", "турникет", "замок", "домофон", "rfid", "rc522",
        "метк", "считыват",
    ]
    .iter()
    .any(|k| text.contains(k));
    card && !access
}

/// A seven-segment indicator keeps its own lesson. "Часы на семисегментном
/// индикаторе" must not be turned into the DS3231 demo just because the word
/// "часы" is present.
fn seven_segment(text: &str) -> bool {
    ["семисегмент", "7seg", "7-сегмент", "tm1650", "tm1637"]
        .iter()
        .any(|k| text.contains(k))
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
    pub rgb: usize,
    pub motor: usize,
    pub servo: usize,
    pub button: usize,
    pub hourly: bool,
}

fn number_word(word: &str) -> Option<usize> {
    match word {
        "четыре" | "четырёх" | "4" => Some(4),
        "три" | "трёх" | "3" => Some(3),
        "два" | "две" | "двух" | "2" => Some(2),
        _ => None,
    }
}

/// Whether a token names a part. Used to stop a number from applying to the
/// next part in the list: in "два светодиода, реле" the relay is one part,
/// not two.
fn names_a_part(token: &str) -> bool {
    const PARTS: &[&str] = &[
        "светодиод", "led", "реле", "relay", "пьезо", "пищал", "зуммер", "сирен",
        "buzzer", "серво", "servo", "sg90", "мотор", "motor", "двигател",
        "вентилятор", "насос", "кулер", "кнопк", "button", "rgb", "экран",
        "диспле", "oled", "lcd",
    ];
    PARTS.iter().any(|w| crate::words::mentions(token, w))
}

fn count_of(text: &str, words: &[&str]) -> usize {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let Some(at) = tokens
        .iter()
        .position(|t| words.iter().any(|w| crate::words::mentions(t, w)))
    else {
        return 0;
    };
    // A numeral two words back still counts ("два красных светодиода"), but
    // never across another named part.
    for j in (at.saturating_sub(2)..at).rev() {
        let Some(n) = number_word(tokens[j]) else { continue };
        if tokens[j + 1..at].iter().any(|t| names_a_part(t)) {
            break;
        }
        return n;
    }
    1
}

/// "Шаговый двигатель" is a stepper, not a DC motor: the word "двигатель"
/// must not add a second, transistor-driven motor to the stepper lesson.
fn motor_count(text: &str) -> usize {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut kept = String::new();
    for (i, token) in tokens.iter().enumerate() {
        let stepper = i > 0
            && crate::words::mentions(token, "двигател")
            && crate::words::mentions(tokens[i - 1], "шагов");
        if stepper {
            continue;
        }
        if !kept.is_empty() {
            kept.push(' ');
        }
        kept.push_str(token);
    }
    count_of(
        &kept,
        &["двигател", "мотор", "motor", "вентилятор", "насос", "кулер"],
    )
}

pub fn named_devices(raw: &str) -> NamedDevices {
    let text = crate::words::normalize(raw);
    let rgb = count_of(&text, &["rgb", "трёхцвет", "трехцвет", "цветной светодиод"]);
    NamedDevices {
        screens: screens_in(raw),
        buzzer: count_of(&text, &["пьезо", "пищал", "зуммер", "сирен", "buzzer", "gong"]),
        relay: count_of(&text, &["реле", "relay"]),
        // "RGB" is a light-emitting diode too, but it is one part with three
        // channels: counting it twice would draw a second, needless LED.
        led: if rgb > 0 {
            0
        } else {
            count_of(
                &text,
                &["светодиод", "led", "индик", "ночник", "фонар", "гирлянд"],
            )
        },
        rgb,
        motor: motor_count(&text),
        // "Поворот" alone is too vague: it also shows up in servo lessons'
        // wording and in phrases about turning a knob.
        servo: count_of(
            &text,
            &[
                "серво", "servo", "sg90", "sg-90", "mg90", "mg995", "mg996", "fs90",
                "towerpro",
            ],
        ),
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
    } else if text.contains("lcd") || text.contains("16x2") || text.contains("1602") {
        ScreenKind::Lcd
    } else {
        ScreenKind::Any
    };
    let count = if ["два", "две", "двух", "2", "two", "pair"]
        .iter()
        .any(|w| text.split_whitespace().any(|t| t == *w))
    {
        2
    } else if ["три", "трёх", "трех", "3", "three"]
        .iter()
        .any(|w| text.split_whitespace().any(|t| t == *w))
    {
        3
    } else if ["четыре", "четырёх", "четырех", "4", "four"]
        .iter()
        .any(|w| text.split_whitespace().any(|t| t == *w))
    {
        4
    } else {
        1
    };
    Some(Screens { kind, count })
}

/// A sentence that asks to show a text message on a display and names no
/// sensor, storage or radio. "Текст приветствия на двух экранах" is such a
/// scene: the displays are the whole circuit, so the parser must not pull a
/// motion sensor out of a similar-looking word.
pub fn text_screen_task(raw: &str) -> Option<Screens> {
    let text = crate::words::normalize(raw);
    let screens = match screens_in(raw) {
        Some(screens) => screens,
        // "Текст и светодиод" names no display type, but a bare message next
        // to an output is still a text scene. On its own ("переведи текст")
        // the word is not a screen.
        None if text.contains("текст")
            && ["светодиод", "пьезо", "пищал", "реле", "мотор", "серво", "ламп"]
                .iter()
                .any(|w| text.contains(w)) =>
        {
            Screens { kind: ScreenKind::Any, count: 1 }
        }
        None => return None,
    };
    let message = [
        "приветств", "привет", "надпис", "сообщен", "greeting", "hello", "caption",
    ]
    .iter()
    .any(|w| text.contains(w));
    let show_verb = [
        "вывест", "вывод", "показат", "показыв", "показать", "написа", "напечат", "show", "print",
        "write",
    ]
    .iter()
    .any(|w| text.contains(w));
    let plain_text = ["текст", "text", "слов", "букв"].iter().any(|w| text.contains(w));
    // "Текстовый дисплей LCD1602" is the lesson's own name, not a request to
    // print a message: only a message word or a verb makes it a text task.
    let model_name = ["текстовый дисплей", "символьный дисплей", "text display"]
        .iter()
        .any(|w| text.contains(w));
    if !message && !(plain_text && (show_verb || !model_name)) {
        return None;
    }
    let s = crate::semantics::analyse(raw);
    if s.source.is_some() {
        return None;
    }
    // A relay, a buzzer or a motor named next to the screen can be added to
    // the text scene: "текст на экране и пьезо" prints the message and beeps.
    // A radio, storage or another board is a different circuit, not an extra.
    if s.sinks.iter().any(|sink| {
        !matches!(
            sink.as_str(),
            "display" | "led" | "buzzer" | "relay" | "motor" | "servo"
        )
    }) {
        return None;
    }
    Some(screens)
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
fn fuzzy_book_task(raw: &str) -> Option<(Parse, usize, u8)> {
    crate::book_tasks::closest(raw).map(|(i, solution, score)| {
        (from_book_solution(raw, solution), i, score)
    })
}

/// Model numbers the user typed. A fuzzy match to a printed task that never
/// mentions them is a different task, not a retyped one.
fn specific_parts(raw: &str) -> Vec<String> {
    const MODELS: &[&str] = &[
        "dht11", "dht22", "ds18b20", "bmp180", "bmp280", "bme280", "mq-2", "mq-3",
        "hc-sr04", "hc-sr501", "rc522", "fs1000a", "xy-mk", "nrf24", "hc-05",
        "hc-06", "neoway", "sd", "microsd", "tsop", "7seg", "tm1637", "max7219",
        "ws2812", "ds3231", "lcd1602", "lcd2004", "oled", "enc28j60", "w5100",
        "l298n", "l293d", "28byj", "uln2003", "acs712", "hx711", "mpu6050",
        "tcs3200", "dfplayer", "mt8870", "ttp223",
    ];
    let text = crate::words::normalize(raw);
    text.split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric() && c != '-'))
        .filter(|t| MODELS.contains(t))
        .map(|t| t.to_string())
        .collect()
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
            let seeded = parse_inner(seed, false);
            // The seed is the book's own answer, so it wins when the ordinary
            // reading found a different shape of circuit: a spurious recipe
            // ("по алгоритму движения" is not a motion sensor), a lesson, or
            // another load. When both readings agree on the shape, the
            // ordinary one is kept: it also keeps the button, knob or sensor
            // the task names.
            let same_shape = normal.intent.load == seeded.intent.load
                && normal.intent.lesson == seeded.intent.lesson
                && normal.intent.recipe.is_some() == seeded.intent.recipe.is_some()
                && normal.intent.text_screens.is_some()
                    == seeded.intent.text_screens.is_some();
            let keep_normal = normal.unsupported.is_none()
                && (seeded.unsupported.is_some() || same_shape);
            if keep_normal {
                normal
            } else {
                let mut parsed = seeded;
                parsed.intent.raw = raw.to_string();
                parsed
            }
        }
    }
}

#[cfg(not(feature = "book-tasks"))]
fn exact_book_task(_raw: &str) -> Option<Parse> {
    None
}

#[cfg(not(feature = "book-tasks"))]
fn fuzzy_book_task(_raw: &str) -> Option<(Parse, usize, u8)> {
    None
}

fn parse_inner(raw: &str, books: bool) -> Parse {
    let _ = books;
    // The user's own sentence stays in `raw`, so numbers in it ("на 49 карт")
    // still reach the sketch.
    let mut book_hint: Option<(Parse, usize, u8)> = None;
    if books {
        if let Some(mut parsed) = exact_book_task(raw) {
            parsed.trace.insert(0, "book: exact task".to_string());
            return parsed;
        }
        // A short ask such as "две надписи на двух OLED" can look like a
        // printed display task, but it is the text scene, not the lesson's
        // demo. A copied task still wins above, and a fuzzy match to a
        // sensor, radio or storage keeps its own circuit.
        if let Some((parsed, index, score)) = fuzzy_book_task(raw) {
            let own_text = text_screen_task(raw).is_some()
                && parsed
                    .intent
                    .lesson
                    .is_some_and(crate::lessons::is_text_display);
            // A near copy stays a copy. But if the user typed a model number
            // the printed task never mentions, the match answers a different
            // question: keep the book reading as an offer, and parse the
            // sentence as written.
            let printed = crate::book_tasks::TASKS[index].0.to_lowercase();
            let foreign = specific_parts(raw).iter().any(|p| !printed.contains(p));
            if !own_text && !foreign {
                return parsed;
            }
            if foreign {
                book_hint = Some((parsed, index, score));
            }
        }
    }
    let text = search_text(raw);
    let mut matched = Vec::new();
    let mut understood = Vec::new();
    let mut trace = vec![format!("text: {text}")];

    // Specific boards come before the generic word "мотор", otherwise an
    // L298N task would be read as a single-transistor motor switch.
    let load_groups = [
        &STEPPER, &MOTORS, &RFID, &RADIO_NRF, &RADIO433, &BLUETOOTH, &GSM, &ETHERNET,
        &MOTOR, &SERVO, &BUZZER, &LAMP, &LED,
    ];
    let load_votes = votes(&text, &load_groups);
    for (i, h) in &load_votes {
        trace.push(format!(
            "load group {}: [{}]",
            load_groups[*i].label.ru,
            h.join(", ")
        ));
    }
    let (mut load, mut load_hits) = match load_votes.first() {
        Some((i, h)) => (load_at(*i), h.clone()),
        None => (Load::Led, Vec::new()),
    };
    // A named board is the subject of the sentence; an LED next to it is the
    // indicator it drives. "RFID и светодиод" is an RFID task, not a blink,
    // and the LED is then added to the RFID circuit instead of replacing it.
    if load == Load::Led {
        if let Some((i, h)) = load_votes.iter().find(|(i, _)| {
            matches!(
                load_at(*i),
                Load::Rfid
                    | Load::Radio433
                    | Load::RadioNrf
                    | Load::Bluetooth
                    | Load::Gsm
                    | Load::Ethernet
                    | Load::Stepper
                    | Load::Motors
            )
        }) {
            load = load_at(*i);
            load_hits = h.clone();
        }
    }
    // An incandescent lamp is a heavy load, not an LED, although the LED
    // group also knows the word "лампа". It only overrides a plain light: a
    // named board still wins, and a light-sensor task that merely mentions a
    // lamp ("фотореле с лампами накаливания") keeps its sensor.
    let lamp_words =
        text.contains("ламп") || text.contains("галоген") || text.contains("диммер");
    let lamp_heavy = text.contains("накалив")
        || text.contains("галоген")
        || text.contains("диммер")
        || text.contains("12 в")
        || text.contains("220 в")
        || text.contains("вольт");
    let lamp_task = lamp_words
        && lamp_heavy
        && !text.contains("фотореле")
        && !text.contains("датчик свет")
        && !text.contains("освещ")
        && !text.contains("инкубатор")
        && !text.contains("термостат")
        && !text.contains("светодиодн");
    if lamp_task && (load == Load::Led || load_hits.is_empty()) {
        load = Load::Lamp;
        load_hits = hits(&text, &LAMP);
        if load_hits.is_empty() {
            load_hits.push("лампа".to_string());
        }
    }
    // A driver is a named board, not a single transistor switch. "Драйвер
    // двигателей и мотор" names the driver twice, and the generic "мотор"
    // wins the vote on its own; the named board is still the subject.
    if load == Load::Motor {
        if let Some((_, h)) = load_votes
            .iter()
            .find(|(i, _)| matches!(load_at(*i), Load::Motors))
        {
            load = Load::Motors;
            load_hits = h.clone();
        }
    }
    // "Приёмник" is also an IR receiver, and "карта" is also an SD card. Both
    // have their own lessons; letting the radio or RFID group win on the
    // shared word builds a plausible but wrong circuit. Drop the group and
    // let the lesson lookup name the real part.
    if load == Load::Radio433 && ir_part(&text) && !radio_433_context(&text) {
        trace.push("context: IR receiver is not a 433 MHz radio".to_string());
        load = Load::Led;
        load_hits.clear();
    }
    if load == Load::Rfid && sd_storage(&text) {
        trace.push("context: SD card is not an RFID tag".to_string());
        load = Load::Led;
        load_hits.clear();
    }
    let named_load = !load_hits.is_empty();
    if !named_load {
        trace.push("load: default (no load word)".to_string());
    }
    trace.push(format!("load: {:?} [{}]", load, load_hits.join(", ")));
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
    trace.push(format!(
        "input: {:?} [{}]",
        input,
        input_hits.join(", ")
    ));
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
    trace.push(format!("action: {:?}", action));

    let semantic = crate::templates::route(load, input);
    // A clock is a noun ("часы"), not a duration ("каждый час"). The
    // distinction matters: "часы и пьезо" is an alarm clock, while "мигать
    // каждый час" is just a timer.
    let clock_noun = [
        "часы", "часов", "часам", "часами", "часах", "часики", "будильник",
    ]
    .iter()
    .any(|w| text.split_whitespace().any(|t| t == *w));
    let clock_object = clock_noun
        || text.contains("clock")
        || text.contains("ds3231")
        || text.contains("ds1307")
        || text.contains("rtc")
        || text.contains("часы реальн");
    // A sentence that asks for text on displays needs no sensor or driver:
    // the displays are the circuit. A named LED, relay or buzzer next to the
    // screen is an output, not a different reading, and is added to it later.
    let text_screens = if !named_input && !transistor {
        text_screen_task(raw)
    } else {
        None
    };
    // A named lesson from the course books is more specific than the generic
    // templates: if the sentence names a board the books teach, build that.
    // Everyday words like "термометр" stand in for a part only when the
    // sentence names nothing else: "шуметь динамиком" must stay a buzzer.
    let mut lesson = crate::lessons::find_with(&text, !named_load && !transistor);
    let board_load = matches!(
        load,
        Load::Rfid
            | Load::Radio433
            | Load::RadioNrf
            | Load::Bluetooth
            | Load::Gsm
            | Load::Ethernet
            | Load::Stepper
            | Load::Motors
    );
    // "Часы на LCD1602" is the clock lesson with a screen, not the display
    // lesson: the model number must not hide what the circuit is for. The
    // same holds for an alarm clock: "часы и пьезо" is an RTC with a buzzer,
    // not the buzzer lesson.
    if clock_object
        && !board_load
        && !seven_segment(&text)
        && (screens_in(raw).is_some() || named_load)
    {
        if let Some(rtc) = crate::lessons::ALL
            .iter()
            .position(|l| l.keys.contains(&"ds3231"))
        {
            lesson = Some(rtc);
        }
    }
    if lesson.is_some_and(crate::lessons::is_text_display) && clock_object {
        if let Some(rtc) = crate::lessons::ALL
            .iter()
            .position(|l| l.keys.contains(&"ds3231"))
        {
            lesson = Some(rtc);
        }
    }
    // A lesson may not hide the load the sentence names. That used to apply
    // only to ready-made boards, so "полив по влажности почвы" turned into
    // the DHT lesson: the pump was named, but the lesson did not know the
    // word. The sensor the sentence itself names is the stronger evidence:
    // when the lesson teaches a different sensor, it is the wrong lesson.
    let lesson_source = lesson.and_then(crate::spec::source_for_lesson);
    let named_source = crate::compose::source_in_text(raw);
    let wrong_source = lesson_source
        .zip(named_source)
        .is_some_and(|(a, b)| a.id != b.id);
    if named_load && board_load {
        if let Some(i) = lesson {
            let l = &crate::lessons::ALL[i];
            let lesson_text =
                crate::words::normalize(&format!("{} {}", l.title.ru, l.keys.join(" ")));
            if hits(&lesson_text, load_group(load)).is_empty() {
                lesson = None;
            }
        }
    } else if wrong_source {
        lesson = None;
    }
    // EEPROM is a way to keep data, not the thing the circuit drives: in
    // "контроль доступа на 49 карт ... в EEPROM" the reader is named, so the
    // counter lesson (a button and EEPROM) must not win over the RFID lock.
    if named_load
        && lesson.is_some_and(|i| crate::lessons::ALL[i].keys.iter().any(|k| *k == "eeprom"))
    {
        lesson = None;
    }
    // A card table with a screen is the access scene: a display the sentence
    // names must not turn the whole task into the display lesson.
    if load == Load::Rfid && crate::templates::wants_access(raw) {
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
    // Text on a display is its own scene even when the sentence also names
    // the display type: the LCD lesson prints its own clock, not the text the
    // task asked for. A lesson about SD, radio or a sensor never reaches this
    // point: `text_screen_task` rejects those sinks and sources. A named
    // output is added to the text scene by `augment_devices`, not turned into
    // a lesson that loses the text.
    if text_screens.is_some() {
        lesson = None;
        understood.push(L::new("текст на экране", "text on a display"));
    }
    trace.push(format!("lesson: {:?}", lesson));
    if let Some(i) = lesson {
        understood.push(crate::lessons::ALL[i].title);
    }
    // The verb comes after the thing it applies to: "датчик газа · мигание"
    // reads better than the other way round. A text scene already says
    // "текст на экране", so the verb would only repeat it.
    if let Some(a) = action {
        if text_screens.is_none() {
            understood.push(a.label());
        }
    }
    // A sensor plus a verb is a composition, not merely the lesson that
    // teaches the sensor: "датчик газа, который мигает" becomes a circuit.
    let condition = spoken_condition(raw);
    // "пьезопищалка" in a clock task is a buzzer, not a sound sensor. Never
    // turn an RTC build into a microphone readout because of that word.
    let clock_tick = clock_object
        || text.contains("часы реальн")
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
    trace.push(format!(
        "recipe: {:?}",
        recipe.map(|r| (r.source, r.sink, r.behavior))
    ));
    trace.push(format!("text screens: {:?}", text_screens));
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
    } else if lesson.is_some() || text_screens.is_some() {
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

    trace.push(match unsupported {
        Some(ref msg) => format!("unsupported: {}", msg.ru),
        None => "unsupported: none".to_string(),
    });
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
        text_screens,
        raw: raw.to_string(),
    };
    let mut alternatives = Vec::new();
    // A tie is a tie, but so is a one-vote lead on a short match: "реле и
    // мотор" should not silently build a relay switch when the sentence names
    // both. A real lesson or recipe is specific enough to stand alone.
    let top_load = load_votes.first().map(|(_, h)| h.len()).unwrap_or(0);
    let second_load = load_votes.get(1).map(|(_, h)| h.len()).unwrap_or(0);
    let tied = load_votes.len() > 1 && second_load == top_load;
    let near_tie = load_votes.len() > 1
        && top_load <= 3
        && second_load + 1 >= top_load
        && lesson.is_none()
        && recipe.is_none()
        && text_screens.is_none();
    if tied || near_tie {
        if near_tie && !tied {
            trace.push("load: near tie, offering readings".to_string());
        }
        for (i, h) in load_votes.iter().skip(1).take(2) {
            let other = load_at(*i);
            if h.len() + 1 < top_load {
                continue;
            }
            if other != load
                && crate::templates::route(other, input).is_some()
                && alternatives.len() < 2
            {
                alternatives.push(Alternative {
                    label: plan_label(other, input),
                    intent: intent_of(other, input),
                    note: None,
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
                    note: None,
                });
            }
        }
    }
    // "По радио" names a link, not its band. Offer the 2.4 GHz board next to
    // the 433 MHz pair instead of silently choosing one.
    let bare_radio = text.split_whitespace().any(|t| t == "радио")
        && !radio_433_context(&text)
        && !text.contains("nrf");
    if bare_radio
        && load == Load::Radio433
        && alternatives.len() < 2
        && !alternatives
            .iter()
            .any(|a| a.intent.load == Load::RadioNrf)
        && crate::templates::route(Load::RadioNrf, input).is_some()
    {
        alternatives.push(Alternative {
            label: plan_label(Load::RadioNrf, input),
            intent: intent_of(Load::RadioNrf, input),
            note: None,
        });
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
                note: None,
            });
        }
    }

    // The printed task is a suggestion, not a silent replacement: the
    // sentence as written is what gets built, and the book reading waits one
    // click away with the printed text on hover.
    if let Some((parsed, index, score)) = book_hint {
        trace.push(format!(
            "book: suggestion, {}%, printed: {}",
            score, crate::book_tasks::TASKS[index].0
        ));
        if alternatives.len() < 2 {
            alternatives.insert(
                0,
                Alternative {
                    label: L::new("Так, как в книге", "The book's version"),
                    intent: parsed.intent,
                    note: Some(format!(
                        "{} {}%: {}",
                        "Задача из книги", score, crate::book_tasks::TASKS[index].0
                    )),
                },
            );
        }
    }
    for alt in &alternatives {
        if alt.note.is_none() {
            trace.push(format!("alternative: {}", alt.label.ru));
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
            text_screens,
            raw: raw.to_string(),
        },
        matched,
        understood,
        unsupported,
        alternatives,
        trace,
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
        Load::Lamp => L::new("лампа накаливания", "incandescent lamp"),
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
    fn word_stems_do_not_invent_sensors() {
        // "ради" is a stem of "радио", but not of "радиация".
        let radio = parse("радио 433 и реле");
        assert_eq!(radio.intent.load, Load::Radio433);
        assert!(radio.intent.recipe.is_none(), "{:?}", radio.intent.recipe);

        // "тока" in "постоянного тока" names the motor's supply.
        let motor = parse("двигатель постоянного тока");
        assert_eq!(motor.intent.load, Load::Motor);
        assert!(motor.intent.recipe.is_none(), "{:?}", motor.intent.recipe);

        // "по алгоритму движения" is a motion program, not a motion sensor.
        let program = parse("двух двигателей постоянного тока по алгоритму движения");
        assert!(program.intent.recipe.is_none(), "{:?}", program.intent.recipe);
    }

    /// A named driver board is the subject even when the sentence also uses
    /// the generic word "мотор": the board is what the circuit is built on.
    #[test]
    fn a_named_driver_beats_the_generic_motor() {
        for task in ["драйвер двигателей и мотор", "драйвер и мотор"] {
            let parsed = parse(task);
            assert_eq!(parsed.intent.load, Load::Motors, "{task}");
        }
    }

    #[test]
    fn a_stepper_is_not_counted_as_a_dc_motor() {
        assert_eq!(named_devices("шаговый двигатель и кнопка").motor, 0);
        assert_eq!(named_devices("шаговый двигатель и светодиод").motor, 0);
        assert_eq!(named_devices("два мотора").motor, 2);
        assert_eq!(named_devices("шаговый двигатель и мотор").motor, 1);
    }

    #[test]
    fn a_clock_on_a_named_display_keeps_its_clock() {
        let p = parse("часы на LCD1602");
        let lesson = p.intent.lesson.expect("the clock lesson wins");
        assert!(
            crate::lessons::ALL[lesson]
                .keys
                .contains(&"ds3231"),
            "picked {:?}",
            crate::lessons::ALL[lesson].title
        );
    }

    /// Every one of these sentences used to build a plausible circuit that
    /// answered a different question. They are kept together so a new group
    /// word cannot bring the old mistakes back.
    #[test]
    fn a_shared_word_does_not_steal_another_lesson() {
        // "Приёмник" is an IR receiver, not the 433 MHz radio pair.
        let p = parse("ИК приёмник и передатчик");
        assert_eq!(p.intent.load, Load::Led);
        let lesson = p.intent.lesson.expect("the IR lesson wins");
        assert!(
            crate::lessons::ALL[lesson].keys.contains(&"tsop"),
            "picked {:?}",
            crate::lessons::ALL[lesson].title
        );

        // A memory card is not an access card.
        let p = parse("записать данные на SD карту");
        assert_eq!(p.intent.load, Load::Led);
        let lesson = p.intent.lesson.expect("the SD lesson wins");
        assert!(
            crate::lessons::ALL[lesson].keys.contains(&"sd"),
            "picked {:?}",
            crate::lessons::ALL[lesson].title
        );

        // A clock on a seven-segment display is not the DS3231 demo.
        let p = parse("часы на семисегментном индикаторе");
        let lesson = p.intent.lesson.expect("the segment lesson wins");
        assert!(
            !crate::lessons::ALL[lesson].keys.contains(&"ds3231"),
            "picked {:?}",
            crate::lessons::ALL[lesson].title
        );
    }

    #[test]
    fn soil_moisture_composes_with_the_pump() {
        let p = parse("полив растений по влажности почвы");
        let recipe = p.intent.recipe.expect("the soil probe composes");
        assert_eq!(recipe.source, crate::spec::SourceId::Soil);
        assert_eq!(recipe.sink, crate::spec::SinkId::Motor);
        assert_ne!(
            p.intent.lesson,
            Some(crate::lessons::THERMISTOR),
            "the DHT lesson must not answer a soil question"
        );
        let built = crate::templates::build(&p.intent, 0, crate::text::Lang::Ru)
            .expect("the soil pump builds");
        assert!(built.circuit.parts().any(|part| part.value == "SOIL"));
        assert!(built
            .circuit
            .parts
            .iter()
            .any(|part| part.kind == crate::model::Kind::Motor));
        assert_eq!(
            crate::checks::run_with(&built, crate::text::Lang::Ru, &p.intent.raw).failed(),
            0
        );
    }

    #[test]
    fn a_clap_light_stays_on() {
        let p = parse("управление светом по хлопку");
        let recipe = p.intent.recipe.expect("the clap light composes");
        assert_eq!(recipe.source, crate::spec::SourceId::Sound);
        assert_eq!(recipe.sink, crate::spec::SinkId::Led);
        assert_eq!(recipe.behavior, crate::spec::Behavior::Solid);
        let built = crate::templates::build(&p.intent, 0, crate::text::Lang::Ru)
            .expect("the clap light builds");
        assert!(built.circuit.parts().any(|part| part.value == "SOUND"));
    }

    /// Two of each really means two parts, in the drawing and in the report.
    #[test]
    fn counts_are_checked_after_the_build() {
        let p = parse("две кнопки и два светодиода");
        let built = crate::templates::build(&p.intent, 0, crate::text::Lang::Ru)
            .expect("two buttons and two LEDs build");
        let count = |kind| built.circuit.parts.iter().filter(|x| x.kind == kind).count();
        assert_eq!(count(crate::model::Kind::Button), 2);
        assert_eq!(count(crate::model::Kind::Led), 2);
        let report = crate::checks::run_with(&built, crate::text::Lang::Ru, &p.intent.raw);
        assert_eq!(report.failed(), 0);
    }

    /// A near-tie is not a decision: the second reading is offered too.
    #[test]
    fn a_one_vote_lead_still_offers_the_other_reading() {
        let p = parse("светодиод и лампа и мотор");
        assert!(!p.alternatives.is_empty(), "understood {:?}", p.understood);
        assert!(
            p.trace.iter().any(|line| line.contains("near tie")),
            "{:?}",
            p.trace
        );
    }

    /// A printed task that never mentions the part the user typed is a
    /// suggestion, not the parse; the sentence as written still builds.
    #[test]
    fn a_foreign_part_turns_a_fuzzy_book_match_into_a_suggestion() {
        let p = parse(
            "Составить программу для МК последовательно, через каждые 2 секунды отправлять значения температуры и влажности на разных платах от 1 платы с передатчиком на 1 приёмник с помощью структуры и датчика DHT22.",
        );
        let book = p
            .alternatives
            .iter()
            .find(|a| a.note.is_some())
            .expect("the book reading is offered");
        assert_eq!(book.label.ru, "Так, как в книге");
        assert!(book.note.as_deref().unwrap_or("").contains("Задача из книги"));
    }


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
    fn a_threshold_written_in_words_still_builds() {
        let p = parse("температура выше тридцати — включи вентилятор");
        let condition = p.intent.condition.as_ref().expect("threshold was lost");
        assert!(matches!(condition, crate::spec::Condition::Above(v) if (*v - 30.0).abs() < 0.01));
        let plan = crate::templates::plan(&p.intent, crate::text::Lang::Ru).expect("no plan");
        assert!(
            plan.title_text(crate::text::Lang::Ru).contains("Термометр"),
            "{}",
            plan.title_text(crate::text::Lang::Ru)
        );
    }

    /// Everyday words name the same roles as the technical ones: a watering
    /// pump is a motor, evening and night ask for a light sensor, a lantern
    /// is an LED.
    #[test]
    fn everyday_words_name_the_same_roles() {
        let water = parse("полив раз в полчаса");
        assert_eq!(water.intent.load, Load::Motor);
        assert!(water.unsupported.is_none(), "{:?}", water.unsupported);

        for text in [
            "вечером включай фонарь",
            "ночью зажги свет на крыльце",
            "в темноте включи лампу",
        ] {
            let p = parse(text);
            assert_eq!(p.intent.load, Load::Led, "{text}");
            assert_eq!(p.intent.input, Some(Input::Ldr), "{text}");
            assert!(p.unsupported.is_none(), "{text}: {:?}", p.unsupported);
        }
    }

    #[test]
    fn an_inflected_phrase_key_is_heard() {
        let p = parse("по датчику света включай лампу вечером");
        assert_eq!(p.intent.load, Load::Led);
        assert_eq!(p.intent.input, Some(Input::Ldr));
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
            "lamp" => Load::Lamp,
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

    #[test]
    fn text_on_two_displays_is_its_own_scene() {
        let p = parse("текст приветствия на двух экранах");
        assert!(p.unsupported.is_none(), "{:?}", p.unsupported);
        assert_eq!(p.intent.lesson, None, "motion sensor won over the text");
        let screens = p.intent.text_screens.expect("two screens");
        assert_eq!(screens.count, 2);
        assert_eq!(screens.kind, ScreenKind::Any);
        assert_eq!(
            parse("текст на трёх экранах").intent.text_screens.map(|s| s.count),
            Some(3)
        );
    }

    /// The task from the bug report: an incandescent lamp is a heavy load,
    /// not a blinking LED, even though the LED group also knows "лампа".
    #[test]
    fn an_incandescent_lamp_is_not_an_led() {
        let task =
            "Составить программу для МК управления яркостью лампы накаливания на 12 Вольт.";
        let p = parse(task);
        assert_eq!(p.intent.load, Load::Lamp, "{:?}", p.understood);
        assert!(p.unsupported.is_none(), "{:?}", p.unsupported);
        let plan = crate::templates::plan(&p.intent, Lang::Ru).expect("a lamp has a plan");
        assert_eq!(plan.tid, crate::templates::Tid::Lamp);

        // A bare lamp with no filament and no mains voltage is still the
        // small light of the LED lessons.
        assert_eq!(parse("лампа мигает").intent.load, Load::Led);
    }

    /// Every named part has its own counter, so the circuit augmenter knows
    /// exactly what the sentence asked for.
    #[test]
    fn named_devices_count_every_part() {
        let n = named_devices(
            "два светодиода, реле, пьезо, сервопривод sg90, мотор, кнопка",
        );
        assert_eq!(n.led, 2);
        assert_eq!(n.relay, 1);
        assert_eq!(n.buzzer, 1);
        assert_eq!(n.servo, 1);
        assert_eq!(n.motor, 1);
        assert_eq!(n.button, 1);
        assert_eq!(n.rgb, 0);

        let rgb = named_devices("rgb светодиод");
        assert_eq!(rgb.rgb, 1);
        assert_eq!(rgb.led, 0, "an RGB LED is one part, not one plus three");

        assert_eq!(named_devices("поверни на угол 90").servo, 0);
        assert_eq!(named_devices("mg996").servo, 1);
        assert_eq!(named_devices("вентилятор").motor, 1);
        assert_eq!(named_devices("датчик движения и два светодиода").led, 2);
        assert_eq!(named_devices("два светодиода, реле").relay, 1);
        assert_eq!(named_devices("два красных светодиода").led, 2);
    }
}
