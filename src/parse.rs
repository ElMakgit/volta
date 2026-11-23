//! Turning a sentence into a circuit intent.
//!
//! This is deliberately a keyword scorer rather than a model call: it works
//! offline, it is predictable, and when it does not understand something it
//! says so instead of inventing a circuit.

use crate::task::{ControlSlot, CountKind, LinkKind, LinkRole, OutputKind, OutputSlot, ScreenSlot, SensorSlot, Task};
use crate::text::{L, Lang};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Input {
    Button,
    Pot,
    Ldr,
}

/// What the sentence asks the circuit to do. The planner still picks the
/// circuit by load and input; the action explains the choice to the user and
/// feeds the "steps" flag where the words mean discrete presses.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
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
    /// The typed task behind this reading: every named part is a slot with
    /// its evidence, and the plan is judged by the slots it closes.
    pub task: crate::task::Task,
    /// The parts, sensors and roles the sentence names, extracted once by
    /// `named_devices`. The builder and the checklist both read this object,
    /// so they cannot disagree about what the task asked for.
    pub named: NamedDevices,
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
        "освещени", "ночник", "фонар", "гирлянд", "огонек", "индикатор", "rgb",
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
        "фоторезистор", "фотоэлемент", "светочувств", "ldr", "освещенност", "темно",
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

/// A template load as a typed output slot. The enum is wider because a relay
/// and a plain LED are different outputs even though both route through the
/// light templates.
fn output_kind(load: Load) -> OutputKind {
    match load {
        Load::Led => OutputKind::Led,
        Load::Lamp => OutputKind::Lamp,
        Load::Motor => OutputKind::Motor,
        Load::Motors => OutputKind::Motors,
        Load::Stepper => OutputKind::Stepper,
        Load::Servo => OutputKind::Servo,
        Load::Buzzer => OutputKind::Buzzer,
        Load::Rfid => OutputKind::Rfid,
        Load::Radio433 => OutputKind::Radio433,
        Load::RadioNrf => OutputKind::RadioNrf,
        Load::Bluetooth => OutputKind::Bluetooth,
        Load::Gsm => OutputKind::Gsm,
        Load::Ethernet => OutputKind::Ethernet,
    }
}

/// Evidence strength: a board or a model is stronger than a technical noun,
/// which is stronger than an everyday synonym. The numbers are weights, not
/// votes: "RFID" outweighs "светодиод" even when both match one keyword.
fn output_specificity(kind: OutputKind) -> u8 {
    match kind {
        OutputKind::Rfid
        | OutputKind::RadioNrf
        | OutputKind::Bluetooth
        | OutputKind::Gsm
        | OutputKind::Ethernet
        | OutputKind::Stepper
        | OutputKind::Motors
        | OutputKind::Lamp
        | OutputKind::Relay => 3,
        OutputKind::Motor
        | OutputKind::Servo
        | OutputKind::Buzzer
        | OutputKind::Radio433 => 2,
        OutputKind::Led => 1,
    }
}

fn output_count(kind: OutputKind, named: &NamedDevices) -> usize {
    let count = match kind {
        OutputKind::Led => named.led.max(named.rgb).max(1),
        OutputKind::Lamp => named.lamp.max(1),
        OutputKind::Motor => named.motor.max(1),
        OutputKind::Servo => named.servo.max(1),
        OutputKind::Buzzer => named.buzzer.max(1),
        OutputKind::Relay => named.relay.max(1),
        OutputKind::Rfid => 1,
        OutputKind::Radio433 | OutputKind::RadioNrf => {
            if named.radio_pair {
                2
            } else {
                1
            }
        }
        _ => 1,
    };
    count
}

fn control_count(kind: Input, named: &NamedDevices) -> usize {
    match kind {
        Input::Button => named.button.max(1),
        Input::Pot | Input::Ldr => 1,
    }
}

fn control_specificity(kind: Input) -> u8 {
    match kind {
        Input::Button => 1,
        Input::Pot | Input::Ldr => 2,
    }
}

/// Evidence, not a vote count. How many times a stem matched or how many
/// synonyms a table row holds must not outrank a named board: "драйвер
/// двигателей и мотор" says "двигател" twice, but the driver is still the
/// subject. Counts belong to the build, not to the reading.
fn output_score(slot: &OutputSlot) -> u32 {
    u32::from(slot.specificity) * 10
}

fn control_score(slot: &ControlSlot) -> u32 {
    u32::from(slot.specificity) * 10
}

/// The first token a group's keywords point at. Used only to order two equal
/// readings by what the sentence said first; it is not a group index.
fn first_hit(text: &str, hits: &[String]) -> usize {
    hits.iter()
        .filter_map(|word| match_indices(text, word).first().copied())
        .min()
        .unwrap_or(0)
}

fn first_word_position(text: &str, word: &str) -> usize {
    text.split_whitespace()
        .position(|t| crate::words::mentions(t, word))
        .unwrap_or(0)
}

/// The measured slots. A thermistor is its own slot, not a DS18B20: they read
/// the same quantity through different parts, and a task that names the NTC
/// must not be answered by the digital sensor.
fn sensor_slots(
    named: &NamedDevices,
    raw: &str,
    condition: Option<crate::spec::Condition>,
) -> Vec<SensorSlot> {
    use crate::spec::SourceId;
    let mut out: Vec<SensorSlot> = Vec::new();
    let mut add = |source: SourceId, count: usize, words: Vec<String>| {
        if count == 0 || out.iter().any(|s| s.source == source) {
            return;
        }
        out.push(SensorSlot {
            source,
            count,
            words,
            condition,
            at: 0,
        });
    };
    let thermistor = crate::compose::source_in_text(raw)
        .is_some_and(|s| s.id == SourceId::Thermistor);
    if thermistor {
        add(SourceId::Thermistor, named.temperature.max(1), vec!["термистор".into()]);
    } else if named.temperature > 0 {
        add(SourceId::Temperature, named.temperature, vec!["температура".into()]);
    }
    // "Два DHT22" names the humidity board without the word "влажность":
    // the model is the sensor, and its count is the count of the reading.
    let humidity = named.humidity.max(named.dht22).max(named.dht11);
    add(SourceId::Humidity, humidity, vec!["влажность".into()]);
    add(SourceId::Soil, named.soil, vec!["почва".into()]);
    add(SourceId::Pressure, named.pressure, vec!["давление".into()]);
    add(SourceId::Light, named.light, vec!["свет".into()]);
    add(SourceId::Range, named.range, vec!["расстояние".into()]);
    add(SourceId::Gas, named.gas, vec!["газ".into()]);
    out
}

fn link_slots(outputs: &[OutputSlot], named: &NamedDevices) -> Vec<(LinkKind, LinkRole)> {
    let mut out = Vec::new();
    let mut add = |kind: LinkKind, role: LinkRole| {
        if !out.iter().any(|(k, _)| *k == kind) {
            out.push((kind, role));
        }
    };
    for slot in outputs {
        match slot.kind {
            OutputKind::Rfid => add(LinkKind::Rfid, LinkRole::Reader),
            OutputKind::Radio433 => add(
                LinkKind::Radio433,
                if named.radio_pair {
                    LinkRole::Peer
                } else {
                    LinkRole::Transmitter
                },
            ),
            OutputKind::RadioNrf => add(
                LinkKind::RadioNrf,
                if named.radio_pair {
                    LinkRole::Peer
                } else {
                    LinkRole::Transmitter
                },
            ),
            OutputKind::Bluetooth => add(LinkKind::Bluetooth, LinkRole::Peer),
            OutputKind::Gsm => add(LinkKind::Gsm, LinkRole::Gateway),
            OutputKind::Ethernet => add(LinkKind::Ethernet, LinkRole::Gateway),
            _ => {}
        }
    }
    if named.tsop {
        add(LinkKind::Ir, LinkRole::Receiver);
    }
    out
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
    let norm = crate::words::normalize(&without_negated_parts(raw));
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

/// A display asked for in words. A book task names the lesson's sensor, but
/// the reader still has to honour "на любой тип дисплея" or "два экрана".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    /// "Матричный дисплей" is a MAX7219, not an LCD: the words name a kind
    /// the I2C display list cannot express by `kind` alone.
    pub matrix: bool,
    /// "Семисегментный индикатор" is a 7-segment board, not an I2C screen.
    pub segments: bool,
}

/// Everything the user named outright. A ready-made lesson or a template is
/// a starting point, not an excuse to drop parts the sentence asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedDevices {
    pub screens: Option<Screens>,
    pub buzzer: usize,
    pub relay: usize,
    pub led: usize,
    /// Incandescent lamps, counted apart from LEDs: a lamp is a heavy load
    /// with its own switch, not a light-emitting diode.
    pub lamp: usize,
    pub rgb: usize,
    pub motor: usize,
    pub servo: usize,
    pub button: usize,
    pub hourly: bool,
    /// Measured quantities, counted the same way as parts: "два датчика
    /// температуры" is two sensors, not one.
    pub temperature: usize,
    pub humidity: usize,
    pub soil: usize,
    pub pressure: usize,
    pub light: usize,
    pub range: usize,
    pub gas: usize,
    pub dht11: usize,
    pub dht22: usize,
    /// Named boards and displays that have no `Kind` of their own.
    pub fs1000a: bool,
    pub xy_mk: bool,
    pub rc522: bool,
    pub tsop: bool,
    /// Storage, not access: "записать на флешку" wants the SD lesson, while
    /// "доступ по SD-карте" is an RFID reader task.
    pub sd: bool,
    pub segments: usize,
    pub sound: usize,
    /// A clock is a part and a purpose: "часы на семисегментном индикаторе"
    /// needs the DS3231, not a lesson that merely counts digits.
    pub clock: bool,
    /// An explicit Uno pin from "к пину 9", if the sentence gave one.
    pub pin: Option<u8>,
    /// Every explicit pin in the order it was said: "на пины 12 и 13" names
    /// two, and both must reach the drawing.
    pub pins: Vec<u8>,
    /// The sentence asks for both halves of a radio link.
    pub radio_pair: bool,
    /// A phone screen is an SMS, not an LCD to wire.
    pub phone: bool,
}

/// A spoken count from zero to twenty, or its digits. The books use "10
/// светодиодов" as often as "три", so stopping at four silently turned a
/// ten-LED task into a one-LED blink.
fn number_word(word: &str) -> Option<usize> {
    let word = word.trim_matches(|c: char| !c.is_alphanumeric());
    Some(match word {
        "0" | "ноль" => 0,
        "1" | "один" | "одна" | "одно" | "одного" | "одной" => 1,
        "2" | "два" | "две" | "двух" | "пара" | "пару" => 2,
        "3" | "три" | "трех" => 3,
        "4" | "четыре" | "четырех" => 4,
        "5" | "пять" | "пяти" => 5,
        "6" | "шесть" | "шести" => 6,
        "7" | "семь" | "семи" => 7,
        "8" | "восемь" | "восьми" => 8,
        "9" | "девять" | "девяти" => 9,
        "10" | "десять" | "десяти" => 10,
        "11" | "одиннадцать" | "одиннадцати" => 11,
        "12" | "двенадцать" | "двенадцати" => 12,
        "13" | "тринадцать" | "тринадцати" => 13,
        "14" | "четырнадцать" | "четырнадцати" => 14,
        "15" | "пятнадцать" | "пятнадцати" => 15,
        "16" | "шестнадцать" | "шестнадцати" => 16,
        "17" | "семнадцать" | "семнадцати" => 17,
        "18" | "восемнадцать" | "восемнадцати" => 18,
        "19" | "девятнадцать" | "девятнадцати" => 19,
        "20" | "двадцать" | "двадцати" => 20,
        "one" => 1,
        "two" | "pair" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        _ => return None,
    })
}

/// Words that may sit between a count and the part it counts: "два красных
/// светодиода", "по 3 штуки". A sensor word counts as filler only when the
/// thing being counted is a sensor: "два датчика температуры" is two, while
/// "два датчика и светодиод" is still one LED.
fn is_count_filler(token: &str, sensor: bool) -> bool {
    let t = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
    if matches!(t, "по" | "с" | "из")
        || t.starts_with("штук")
        || (sensor && t.starts_with("датчик"))
    {
        return true;
    }
    // "4 разрядный семисегментный" is one display with four digits, not four
    // displays: the numeral belongs to the digit count, so the scan stops
    // here instead of carrying the 4 over to the module.
    if t.starts_with("разряд") {
        return false;
    }
    const ENDINGS: &[&str] = &[
        "ый", "ий", "ой", "ая", "яя", "ое", "ее", "ые", "ие", "ых", "их",
        "ого", "его", "ому", "ему", "ым", "им", "ую", "юю", "ыми", "ими",
    ];
    ENDINGS
        .iter()
        .any(|ending| t.chars().count() > ending.chars().count() && t.ends_with(ending))
}

/// A word that joins two groups of parts: "3 красных и 2 зелёных светодиода".
/// It is not filler like an adjective: a number on the far side of it needs
/// its own agreement before it counts.
fn is_conjunction(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !c.is_alphanumeric());
    matches!(t, "и" | "или" | "либо" | "and" | "or")
}

/// A word naming a pin, a port or a terminal. A number in front of it is a
/// pin number, not a count: "порт 3 и кнопка" asks for one button on port 3,
/// not three buttons.
fn is_pin_word(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !c.is_alphanumeric());
    const STEMS: &[&str] = &[
        "пин", "вывод", "порт", "контакт", "ног", "разъем", "клемм", "штыр", "pin",
    ];
    STEMS.iter().any(|stem| t.starts_with(stem))
}

/// How many parts the words in front of `start` count. A number belongs to
/// the noun it stands next to, so the scan stops at another noun and at a
/// pin name: "пин 13 и светодиод" is one LED, "3 красных и 2 зелёных
/// светодиода" is five. `sensor` lets "датчик" sit between the number and
/// the measured word ("два датчика температуры").
fn spoken_count_before(tokens: &[&str], start: usize, sensor: bool) -> Option<usize> {
    let mut sum = 0;
    let mut found = false;
    // A number after "и" counts only when the words between it and the
    // conjunction agree with it: "3 красных и 2 зелёных". Without that,
    // the number belongs to the noun before the conjunction.
    let mut crossed = false;
    let mut agreeing = false;
    let mut i = start;
    while i > 0 {
        i -= 1;
        let token = tokens[i];
        if let Some(n) = number_word(token) {
            if crossed && !agreeing {
                break;
            }
            sum += n;
            found = true;
            agreeing = false;
            continue;
        }
        if is_pin_word(token) {
            break;
        }
        if is_conjunction(token) {
            crossed = true;
            continue;
        }
        if is_count_filler(token, sensor) {
            if crossed {
                agreeing = true;
            }
            continue;
        }
        break;
    }
    if found { Some(sum) } else { None }
}

/// Count the parts a sentence names, summing split groups: "3 красных и 2
/// зелёных светодиода" is five LEDs, not the first number the scan meets.
/// Adjacent synonyms ("пьезо пищалка") are one part, not two.
pub(crate) fn count_of(text: &str, words: &[&str]) -> usize {
    count_of_inner(text, words, false)
}

/// The same count for sensors, where "датчик" may sit between the number and
/// the measured quantity: "два датчика температуры".
pub(crate) fn count_sensors(text: &str, words: &[&str]) -> usize {
    count_of_inner(text, words, true)
}

/// Count only the numerals written next to the words: a mention with no
/// number of its own adds nothing. "Два экрана" is two, "экран и дисплей"
/// is one screen described twice, not two boards.
fn number_of(text: &str, words: &[&str]) -> usize {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut total = 0;
    let mut last: Option<usize> = None;
    for (i, token) in tokens.iter().enumerate() {
        if !words.iter().any(|w| crate::words::mentions(token, w)) {
            continue;
        }
        if last.is_some_and(|prev| i <= prev + 1) {
            continue;
        }
        last = Some(i);
        total += spoken_count_before(&tokens, i, false).unwrap_or(0);
    }
    total
}

fn count_of_inner(text: &str, words: &[&str], sensor: bool) -> usize {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut starts: Vec<usize> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if !words.iter().any(|w| crate::words::mentions(token, w)) {
            continue;
        }
        if starts.last().is_some_and(|&prev| i <= prev + 1) {
            continue;
        }
        starts.push(i);
    }
    let mut total = 0;
    for start in starts {
        // A second mention without its own number is a synonym or a model
        // list, not another part: "BMP180, BMP280, BME280" names one
        // barometer, just as "светодиод, LED" names one light. Only an
        // explicit number adds to the count of an already counted part.
        match spoken_count_before(&tokens, start, sensor) {
            Some(n) => total += n,
            None if total == 0 => total += 1,
            None => {}
        }
    }
    total
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

/// A lamp that is a heavy load, not a small light: "накаливания", "галоген",
/// a dimmer or a named 12/220 V supply. The word "лампа" alone stays an LED.
fn heavy_lamp(text: &str) -> bool {
    text.contains("накалив")
        || text.contains("галоген")
        || text.contains("диммер")
        || text.contains("симистор")
        || text.contains("лампа на 12")
        || text.contains("лампа на 220")
        || text.contains("лампочка на 12")
        || text.contains("лампочка на 220")
        || text.contains("12 в")
        || text.contains("220 в")
        || text.contains("вольт")
}

/// "Две лампочки" is two lights, whether they are LEDs or incandescent
/// lamps. The light group knows many everyday synonyms; the count must know
/// the same ones, or a number silently collapses to one part.
fn light_count(text: &str) -> usize {
    if heavy_lamp(text) {
        // A heavy lamp is not an LED, even though the word "лампа" reaches
        // both groups. Only an explicitly named LED counts here.
        count_of(
            text,
            &["светодиод", "led", "индик", "ночник", "фонар", "гирлянд"],
        )
    } else {
        count_of(
            text,
            &[
                "светодиод",
                "led",
                "индик",
                "ночник",
                "фонар",
                "гирлянд",
                "ламп",
                "лампоч",
                "подсветк",
                "огонек",
            ],
        )
    }
}

/// Incandescent lamps counted by their own words, with the number that sits
/// in front of "лампа"/"лампочка": "две лампы накаливания" is two.
fn lamp_count(text: &str) -> usize {
    if !heavy_lamp(text) {
        return 0;
    }
    count_of(
        text,
        &["лампа", "лампоч", "накалив", "галоген", "диммер"],
    )
}

/// Text with the parts the sentence explicitly refuses removed. Negation is
/// handled once, here, so the drawing, the plan and the checklist all agree
/// that "реле не нужно" names no relay.
fn without_negated_parts(raw: &str) -> String {
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut drop = vec![false; tokens.len()];
    for (i, token) in tokens.iter().enumerate() {
        if !is_part_token(token) || !is_negated(&tokens, i) {
            continue;
        }
        drop[i] = true;
        // "датчик температуры не нужен": drop the measured word too, or the
        // count would still see it. The noun is inflected ("датчика",
        // "датчиков"), so compare the stem. A refused list loses every
        // quantity in it: "без датчика температуры и влажности" is neither.
        if bare(token).starts_with("датчик") {
            let mut j = i + 1;
            while let Some(next) = tokens.get(j) {
                let word = bare(next);
                if is_measure_word(next) {
                    drop[j] = true;
                    j += 1;
                    continue;
                }
                if matches!(word, "и" | "или")
                    && tokens.get(j + 1).is_some_and(|t| is_measure_word(t))
                {
                    drop[j] = true;
                    j += 1;
                    continue;
                }
                break;
            }
        }
    }
    if !drop.iter().any(|d| *d) {
        return raw.to_string();
    }
    // Drop the refusal next to the refused part as well. Otherwise "не" is
    // left in the sentence and can catch the part in the next clause:
    // "датчик не нужен, нужен светодиод" must keep the LED.
    for i in 0..tokens.len() {
        if drop[i] {
            continue;
        }
        let near = (i.saturating_sub(3)..=(i + 3).min(tokens.len() - 1)).any(|j| drop[j]);
        if near
            && (is_negation(tokens[i])
                || is_refusal_verb(tokens[i])
                || matches!(bare(tokens[i]), "без" | "кроме"))
        {
            drop[i] = true;
        }
    }
    tokens
        .iter()
        .enumerate()
        .filter(|(i, _)| !drop[*i])
        .map(|(_, token)| *token)
        .collect::<Vec<_>>()
        .join(" ")
}

/// A token without the punctuation that came with it.
fn bare(token: &str) -> &str {
    token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
}

/// A comma, a full stop or a semicolon ends a clause: a refusal on one side
/// does not reach a part on the other.
fn ends_clause(token: &str) -> bool {
    token.ends_with(',')
        || token.ends_with('.')
        || token.ends_with(';')
        || token.ends_with('!')
        || token.ends_with('?')
        || token.ends_with(':')
}

/// Words that name a part on the bench. Only these can be refused: dropping
/// "не" or "без" alone would let "не мигает светодиод" lose the LED.
fn is_part_token(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
    const PARTS: &[&str] = &[
        "светодиод", "led", "реле", "relay", "пьезо", "пищал", "зуммер", "сирен",
        "buzzer", "gong", "серво", "servo", "sg90", "мотор", "motor", "двигател",
        "вентилятор", "насос", "кулер", "кнопк", "button", "переключател",
        "потенциометр", "экран", "диспле", "oled", "lcd", "screen", "display",
        "датчик", "термометр", "термистор", "фоторезистор", "ldr", "дальномер",
        "барометр", "dht11", "dht22", "ds18b20", "bmp180", "bmp280", "bme280",
        "hc-sr04", "fs1000a", "xy-mk", "rc522", "tsop", "семисегмент", "7seg",
        "флешк", "флеш", "sd", "microsd", "карта", "клавиатур", "динамик",
        "спикер",
    ];
    PARTS.iter().any(|p| t.starts_with(p))
}

fn is_negation(token: &str) -> bool {
    matches!(bare(token), "не" | "нет" | "ни")
}

/// A verb that turns "не" into a refusal: "не нужно", "не использовать".
fn is_refusal_verb(token: &str) -> bool {
    const VERBS: &[&str] = &[
        "нуж", "над", "понад", "треб", "использ", "подключ", "став", "ставить",
        "будем", "надо", "обойтись", "обойдемся",
    ];
    let t = bare(token);
    VERBS.iter().any(|v| t.starts_with(v))
}

fn is_negated(tokens: &[&str], i: usize) -> bool {
    // A refusal belongs to the nearest part and stays inside its clause: in
    // "светодиод, реле не нужно" it rules out the relay, not the LED before.
    let mut start = i;
    while start > 0 {
        let token = tokens[start - 1];
        if is_part_token(token) || ends_clause(token) {
            break;
        }
        start -= 1;
    }
    let before = &tokens[start..i];
    // "без реле", "кроме реле".
    if before.iter().any(|t| matches!(bare(t), "без" | "кроме")) {
        return true;
    }
    // "не использовать реле": refusal before the part.
    if before.iter().any(|t| is_negation(t)) && before.iter().any(|t| is_refusal_verb(t)) {
        return true;
    }
    // "реле не нужно": refusal after the part, up to the clause end or the
    // next part. A plain "не мигает" is not a refusal: no verb of the list
    // follows the "не".
    let mut end = i + 1;
    while end < tokens.len() {
        let token = tokens[end];
        if is_part_token(token) {
            break;
        }
        end += 1;
        if ends_clause(token) {
            break;
        }
    }
    let after = &tokens[i + 1..end];
    after.iter().any(|t| is_negation(t)) && after.iter().any(|t| is_refusal_verb(t))
}

/// The word after "датчик" that names what it measures. Dropped together
/// with a refused "датчик", so "датчик температуры не нужен" leaves neither.
/// The stems are chosen not to swallow parts: "свет" would eat the LED in
/// "без датчика температуры, светодиод", so only "света" is a measurement.
fn is_measure_word(token: &str) -> bool {
    const WORDS: &[&str] = &[
        "температур", "влажност", "давлен", "освещ", "света", "расстоян", "движен",
        "газ", "дым", "звук", "почв", "грунт", "ток", "уровн",
    ];
    let t = token.trim_matches(|c: char| !c.is_alphanumeric());
    WORDS.iter().any(|w| t.starts_with(w))
}

/// A short Latin word is a whole token: "sd" must not match inside another
/// word, while "microsd" and "sd-карта" still name the card.
fn names_card(text: &str) -> bool {
    text.split_whitespace().any(|t| {
        let t = t.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
        t == "sd" || t.starts_with("sd-") || t == "microsd" || t == "sd card"
    }) || text.contains("карта памяти")
        || text.contains("флеш")
        || text.contains("накопител")
}

/// Everyday words for heat. "Символы термометра" on a screen are a drawing,
/// not a sensor; DS3231 has a temperature register, but that is the
/// checklist's business, not the parser's.
fn count_temperature(text: &str) -> usize {
    // "Термометр на термисторе" is one probe, not a thermometer plus a
    // thermistor. The named part wins; the ordinary word only describes it.
    let probe = count_sensors(
        text,
        &["термистор", "терморезистор", "ntc", "thermistor"],
    );
    if probe > 0 {
        return probe;
    }
    let mut n = count_sensors(
        text,
        &["температур", "термодатч", "temperature", "thermometer"],
    );
    if !text.contains("символ") {
        n += count_sensors(text, &["термометр"]);
    }
    // "Два DS18B20" names the part directly, without the word "датчик".
    n += count_sensors(text, &["ds18b20", "18b20"]);
    n
}

/// A clock is a noun ("часы"), not a duration ("два часа"). The same
/// distinction the parser makes for the routing table is needed by the
/// checklist, so it lives here with the other named parts.
fn names_clock(text: &str) -> bool {
    let chip = text.contains("clock")
        || text.contains("ds3231")
        || text.contains("ds1307")
        || text.contains("ds1302")
        || text.contains("rtc")
        || text.contains("часы реальн");
    // "Часы, минуты и секунды" are the fields of a time, not a clock part:
    // a task that prints them does not have to carry an RTC.
    let words: Vec<&str> = text
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|t| !t.is_empty() && *t != "и")
        .collect();
    let time_fields = words
        .windows(2)
        .any(|w| w[0].starts_with("час") && w[1].starts_with("минут"));
    if time_fields && !chip {
        return false;
    }
    let noun = [
        "часы", "часов", "часам", "часами", "часах", "часики", "будильник",
    ]
    .iter()
    .any(|w| text.split_whitespace().any(|t| t == *w));
    noun || chip
}

/// The Uno pins from "к пину 9", "на пины 12 и 13", "pin 9" or "D9". Only
/// numbers next to a pin word count: "9 светодиодов" is a count, not a pin.
/// A list after one pin word keeps going through "и", a comma or a dash.
fn named_pins(text: &str) -> Vec<u8> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let number = |t: &str| -> Option<u8> {
        let t = t.trim_matches(|c: char| !c.is_alphanumeric());
        let t = t.strip_prefix('d').or_else(|| t.strip_prefix('D')).unwrap_or(t);
        t.parse::<u8>().ok().filter(|n| *n <= 19)
    };
    let connector = |t: &str| matches!(t, "и" | "," | "-" | "по");
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let clean = tokens[i].trim_matches(|c: char| !c.is_alphanumeric());
        if clean.len() > 1 && (clean.starts_with('d') || clean.starts_with('D')) {
            if let Some(n) = number(clean) {
                out.push(n);
                i += 1;
                continue;
            }
        }
        if is_pin_word(clean) {
            // "пины 12 и 13": one pin word can introduce a whole list.
            let mut expect_number = true;
            let mut j = i + 1;
            while j < tokens.len() {
                let t = tokens[j].trim_matches(|c: char| !c.is_alphanumeric());
                if let Some(n) = number(t) {
                    if !expect_number {
                        break;
                    }
                    out.push(n);
                    expect_number = false;
                    j += 1;
                    continue;
                }
                if connector(t) {
                    expect_number = true;
                    j += 1;
                    continue;
                }
                break;
            }
            if j > i + 1 {
                i = j;
                continue;
            }
        }
        // "9 к пину": the pin word follows the number.
        if let Some(n) = number(clean) {
            let prev = i.checked_sub(1).and_then(|j| tokens.get(j));
            if prev.is_some_and(|p| is_pin_word(p.trim_matches(|c: char| !c.is_alphanumeric()))) {
                out.push(n);
            }
        }
        i += 1;
    }
    let mut seen = Vec::new();
    out.retain(|n| {
        if seen.contains(n) {
            false
        } else {
            seen.push(*n);
            true
        }
    });
    out
}

/// Soil moisture is a different part, not a DHT reading. "Грунт" and
/// "влажность земли" mean the same probe as "почва".
fn names_soil(text: &str) -> bool {
    text.contains("почв")
        || text.contains("грунт")
        || text.contains("soil")
        || (text.contains("влажност") && text.contains("земл"))
}

fn count_humidity(text: &str) -> usize {
    if names_soil(text) {
        return 0;
    }
    // "Термистор от влажности в комнате" names a cause, not a sensor: the
    // probe is the thermistor, and demanding a humidity board would be wrong.
    if text.contains("от влажност") || text.contains("зависит от влажност") {
        return 0;
    }
    count_sensors(text, &["влажност", "humidity", "гигрометр", "влагомер"])
}

fn count_light(text: &str) -> usize {
    let named = count_sensors(
        text,
        &["фоторезистор", "ldr", "освещенност", "light", "photoresistor"],
    );
    if named > 0 {
        return named;
    }
    // "Датчик света" is a phrase: a bare "свет" in "выключатель света" is the
    // lamp being switched, not a photoresistor. The words agree through
    // cases ("два датчика света"), so the phrase is matched by stem.
    if crate::words::mentions(text, "датчик свет") || crate::words::mentions(text, "свет датчик") {
        return count_sensors(text, &["света"]).max(1);
    }
    0
}

pub fn named_devices(raw: &str) -> NamedDevices {
    let text = crate::words::normalize(&without_negated_parts(raw));
    let rgb = count_of(&text, &["rgb", "трёхцвет", "трехцвет", "цветной светодиод"]);
    // The keys of a telephone are not parts on the bench: "все 16 кнопок
    // телефона" is a DTMF decoder task, not sixteen tact switches.
    let phone_keypad = text.contains("кнопок телефона")
        || text.contains("кнопки телефона")
        || text.contains("клавиш телефона");
    let radio_words = text.contains("радио")
        || text.contains("433")
        || text.contains("nrf")
        || text.contains("fs1000")
        || text.contains("xy-mk");
    let dht22 = count_sensors(&text, &["dht22"]);
    let dht11 = count_sensors(&text, &["dht11"]);
    let lamp = lamp_count(&text);
    let soil = if names_soil(&text) {
        count_sensors(&text, &["почв", "грунт", "soil", "fc-28", "yl-69", "yl-38"])
            .max(1)
    } else {
        0
    };
    let pins = named_pins(&text);
    NamedDevices {
        screens: screens_in(raw),
        // Every synonym the sound group knows must also count: "два
        // динамика" is two parts, not one speaker and a silent second one.
        buzzer: count_of(
            &text,
            &[
                "пьезо", "пищал", "зуммер", "сирен", "buzzer", "gong", "динамик",
                "спикер", "speaker", "колонк",
            ],
        ),
        relay: count_of(&text, &["реле", "relay"]),
        // "RGB" is a light-emitting diode too, but it is one part with three
        // channels: counting it twice would draw a second, needless LED. A
        // seven-segment or matrix indicator is a screen, not an LED either.
        led: if rgb > 0
            || (!text.contains("светодиод")
                && (text.contains("семисегмент")
                    || text.contains("7seg")
                    || text.contains("матричн")
                    || text.contains("max7219")
                    || text.contains("дисплей")
                    || text.contains("экран")
                    || text.contains("lcd")
                    || text.contains("oled")))
        {
            0
        } else {
            light_count(&text)
        },
        lamp,
        rgb,
        motor: motor_count(&text),
        // "Поворот" alone is too vague: it also shows up in servo lessons'
        // wording and in phrases about turning a knob.
        servo: count_of(
            &text,
            &[
                "серво", "servo", "sg90", "sg-90", "mg90", "mg995", "mg996", "fs90",
                "towerpro", "машинк",
            ],
        ),
        button: if phone_keypad {
            0
        } else {
            count_of(&text, &["кнопк", "button", "переключател", "клавиш"])
        },
        hourly: text.contains("каждый час") || text.contains("раз в час")
            || text.contains("hourly") || text.contains("every hour"),
        temperature: count_temperature(&text),
        humidity: count_humidity(&text),
        soil,
        pressure: count_sensors(
            &text,
            &["давлен", "барометр", "bmp180", "bmp280", "bme280", "pressure", "barometer"],
        ),
        light: count_light(&text),
        range: count_sensors(
            &text,
            &["расстоян", "дальномер", "ультразвук", "hc-sr04", "distance", "ranger", "ultrasonic"],
        ),
        gas: count_sensors(
            &text,
            &[
                "газ",
                "дым",
                "mq-2",
                "mq2",
                "gas",
                "газоанализатор",
                "газосигнализатор",
            ],
        ),
        dht11,
        dht22,
        fs1000a: text.contains("fs1000a"),
        xy_mk: text.contains("xy-mk"),
        rc522: text.contains("rc522") || text.contains("rfid") || text.contains("метк"),
        tsop: text.contains("tsop")
            || text.contains("ик приемник")
            || text.contains("ик-приемник"),
        sd: names_card(&text) && !text.contains("доступ"),
        segments: count_sensors(&text, &["семисегмент", "7seg", "tm1650", "tm1637"]),
        sound: count_sensors(&text, &["хлоп", "хлопк"]),
        clock: names_clock(&text),
        pin: pins.first().copied(),
        pins,
        radio_pair: radio_words
            && (text.contains("передатчик") || text.contains("transmitter"))
            && (text.contains("приемник")
                || text.contains("приёмник")
                || text.contains("receiver")),
        phone: text.contains("телефон") || text.contains("сотов"),
    }
}

pub fn screens_in(raw: &str) -> Option<Screens> {
    let text = crate::words::normalize(&without_negated_parts(raw));
    let named = [
        "диспле", "экран", "табло", "lcd", "oled", "display", "screen", "128x64", "128 64",
        "16x2", "2004", "матричн", "max7219", "семисегмент", "7seg", "tm1650", "tm1637",
    ];
    let is_screen = named.iter().any(|k| text.contains(k));
    if !is_screen || (text.contains("порт") && !text.contains("диспле") && !text.contains("экран")) {
        return None;
    }
    let matrix = (text.contains("матричн") || text.contains("max7219"))
        && !text.contains("клавиатур");
    let segments = text.contains("семисегмент")
        || text.contains("7seg")
        || text.contains("tm1650")
        || text.contains("tm1637");
    let kind = if matrix || segments {
        ScreenKind::Any
    } else if text.contains("oled") || text.contains("128x64") || text.contains("128 64") {
        ScreenKind::Oled
    } else if text.contains("2004") || text.contains("20 4") {
        ScreenKind::Lcd2004
    } else if text.contains("lcd") || text.contains("16x2") || text.contains("1602") {
        ScreenKind::Lcd
    } else {
        ScreenKind::Any
    };
    // The number belongs to the screen, not to any earlier noun: "две кнопки
    // и дисплей" is one display, while "два экрана" is two. A second mention
    // without its own number is the same screen said twice: "вывода на
    // дисплей ... на матричный дисплей" asks for one board, not two.
    let count = number_of(&text, &[
        "дисплей", "экран", "табло", "screen", "display", "lcd", "oled",
        "матричн", "max7219", "семисегмент", "7seg", "tm1650", "tm1637",
    ])
    .max(1);
    Some(Screens {
        kind,
        count,
        matrix,
        segments,
    })
}

/// A sentence that asks to show a text message on a display and names no
/// sensor, storage or radio. "Текст приветствия на двух экранах" is such a
/// scene: the displays are the whole circuit, so the parser must not pull a
/// motion sensor out of a similar-looking word.
pub fn text_screen_task(raw: &str) -> Option<Screens> {
    let text = crate::words::normalize(&without_negated_parts(raw));
    let screens = match screens_in(&text) {
        Some(screens) => screens,
        // "Текст и светодиод" names no display type, but a bare message next
        // to an output is still a text scene. On its own ("переведи текст")
        // the word is not a screen.
        None if text.contains("текст")
            && ["светодиод", "пьезо", "пищал", "реле", "мотор", "серво", "ламп"]
                .iter()
                .any(|w| text.contains(w)) =>
        {
            Screens {
                kind: ScreenKind::Any,
                count: 1,
                matrix: false,
                segments: false,
            }
        }
        None => return None,
    };
    // A matrix or a seven-segment board is a lesson with its own driver, not
    // the I2C text scene: printing "Privet!" on it would be a different task.
    if screens.matrix || screens.segments {
        return None;
    }
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

/// The same parser without the printed-task table. Tests use it to measure
/// how much of the book corpus the parser itself understands, instead of the
/// exact lookup answering for it.
#[cfg(test)]
pub(crate) fn parse_without_books(raw: &str) -> Parse {
    parse_inner(raw, false)
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
fn lesson_fits_named(i: usize, named: &NamedDevices) -> bool {
    let Some(lesson) = crate::lessons::ALL.get(i) else {
        return false;
    };
    let keys = |words: &[&str]| lesson.keys.iter().any(|k| words.iter().any(|w| k.contains(w)));
    let boards = |names: &[&str]| {
        lesson
            .boards
            .iter()
            .any(|b| names.iter().any(|n| b.name.starts_with(n)))
    };
    // A storage lesson keeps readings; it does not read them. "Записать
    // дневную влажность в EEPROM" is the EEPROM lesson even though that
    // lesson has no humidity sensor of its own, and the checklist still
    // demands the sensor from the build.
    let storage = keys(&["eeprom", "sd", "карта памяти", "флеш"]);
    // A display lesson shows whatever the code sends it: the printed rows
    // that put a clock on an LCD, a matrix, a Nokia panel or a TV are the
    // book's own answer, and the clock word describes the picture, not a
    // chip the lesson must draw. The checklist still counts the clock, so a
    // build without one is a recorded loss.
    let display = keys(&[
        "дисплей", "экран", "матриц", "nokia", "oled", "телевизор", "p10", "lcd", "tft",
    ]);
    if !storage && named.temperature > 0
        && !(keys(&[
            "температур",
            "термистор",
            "терморезистор",
            "ds18b20",
            "dht11",
            "dht22",
            "давлен",
            "bmp",
        ]) || boards(&["DS18B20", "DHT", "BMP", "BME"]))
    {
        return false;
    }
    if !storage && named.humidity > 0
        && !(keys(&["влажност", "dht"]) || boards(&["DHT", "BME", "BMP"]))
    {
        return false;
    }
    if !storage && named.soil > 0 && !(keys(&["почв", "грунт", "soil"]) || boards(&["SOIL"])) {
        return false;
    }
    if !storage && named.pressure > 0
        && !(keys(&["давлен", "bmp"]) || boards(&["BMP", "BME"]))
    {
        return false;
    }
    if !storage && named.light > 0
        && !(keys(&["фоторезистор", "ldr", "освещ", "свет"])
            || boards(&["GL5528"]))
    {
        return false;
    }
    if !storage && named.range > 0 && !keys(&["дальномер", "ультразвук", "hc-sr04", "расстоян"]) {
        return false;
    }
    if !storage && named.gas > 0 && !(keys(&["газ", "дым", "mq-2"]) || boards(&["MQ-2"])) {
        return false;
    }
    if !storage
        && !display
        && named.clock
        && !(keys(&["часы реальн", "ds3231", "ds1307", "rtc"]) || boards(&["DS3231"]))
    {
        return false;
    }
    // A named segment display is added to the lesson by the builder, so it
    // does not have to be in the lesson's own keys: "измерение тока ... на
    // 4 разрядный семисегментный дисплей" stays the current-sensor lesson
    // and gets the display drawn beside it.
    if named.sd && !keys(&["sd", "карта памяти"]) {
        return false;
    }
    if named.rc522 && !keys(&["rc522", "rfid", "метк"]) {
        return false;
    }
    if named.tsop && !keys(&["tsop", "ик приемник", "ик-приемник"]) {
        return false;
    }
    true
}

#[cfg(feature = "book-tasks")]
fn from_book_solution(raw: &str, solution: crate::book_tasks::Solution) -> Parse {
    use crate::book_tasks::Solution;
    match solution {
        Solution::Lesson(i) => {
            let named = named_devices(raw);
            // The printed table maps a rounded-off sentence to a lesson. That
            // entry can be worse than the ordinary reading: "термометр на
            // термисторе ... на экран" was filed under the TV OUT lesson.
            // When the lesson does not contain the sensor the sentence names,
            // the parser's own reading wins; a lookup is never allowed to
            // answer a different question.
            if !lesson_fits_named(i, &named) {
                return parse_inner(raw, false);
            }
            let mut parsed = parse_inner(crate::lessons::ALL[i].title.ru, false);
            // A bare title such as "Кнопка" is also an everyday word, so the
            // title alone may route elsewhere. The book already says which
            // lesson the task belongs to.
            parsed.intent.lesson = Some(i);
            parsed.unsupported = None;
            parsed.alternatives.clear();
            parsed.intent.raw = raw.to_string();
            parsed.intent.named = named_devices(raw);
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
                parsed.intent.named = named_devices(raw);
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
    // still reach the sketch. A refused part is removed before any reading,
    // so the plan and the checklist never see "реле" in "реле не нужно".
    let norm = crate::words::normalize(raw);
    let clean = crate::words::normalize(&without_negated_parts(raw));
    let negated = clean != norm;
    let mut book_hint: Option<(Parse, usize, u8)> = None;
    if books && !negated {
        if let Some(mut parsed) = exact_book_task(raw) {
            parsed.trace.insert(0, "book: exact task".to_string());
            return parsed;
        }
        // A short ask such as "две надписи на двух OLED" can look like a
        // printed display task, but it is the text scene, not the lesson's
        // demo. A copied task still wins above, and a fuzzy match to a
        // sensor, radio or storage keeps its own circuit.
        if let Some((parsed, index, score)) = fuzzy_book_task(raw) {
            let own_text = text_screen_task(&clean).is_some()
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
    let named = named_devices(raw);
    let text = search_text(raw);
    let mut matched = Vec::new();
    let mut understood = Vec::new();
    let mut trace = vec![format!("text: {text}")];

    // The keyword tables name parts. The decision is made by the typed slots
    // below: each slot carries its own pointing evidence, and the array order
    // is vocabulary order, never a tie-break.
    let load_groups = [
        &STEPPER, &MOTORS, &RFID, &RADIO_NRF, &RADIO433, &BLUETOOTH, &GSM, &ETHERNET,
        &MOTOR, &SERVO, &BUZZER, &LAMP, &LED,
    ];
    let load_votes = votes(&text, &load_groups);
    let mut outputs: Vec<OutputSlot> = load_votes
        .iter()
        .map(|(i, h)| {
            let kind = output_kind(load_at(*i));
            OutputSlot {
                kind,
                count: output_count(kind, &named),
                words: h.clone(),
                specificity: output_specificity(kind),
                at: first_hit(&text, h),
            }
        })
        .collect();
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
    if lamp_task {
        // A separately named LED is not the lamp: only drop the LED slot when
        // the sentence did not ask for one of its own.
        if named.led == 0 {
            outputs.retain(|s| s.kind != OutputKind::Led);
        }
        if !outputs.iter().any(|s| s.kind == OutputKind::Lamp) {
            outputs.push(OutputSlot {
                kind: OutputKind::Lamp,
                count: 1,
                words: vec!["лампа накаливания".to_string()],
                specificity: output_specificity(OutputKind::Lamp),
                at: 0,
            });
        }
    }
    // "Приёмник" is also an IR receiver, and "карта" is also an SD card. Both
    // have their own lessons; letting the radio or RFID group win on the
    // shared word builds a plausible but wrong circuit. Drop the slot and let
    // the lesson lookup name the real part.
    if ir_part(&text) && !radio_433_context(&text) {
        if outputs.iter().any(|s| s.kind == OutputKind::Radio433) {
            trace.push("context: IR receiver is not a 433 MHz radio".to_string());
        }
        outputs.retain(|s| s.kind != OutputKind::Radio433);
    }
    if sd_storage(&text) {
        if outputs.iter().any(|s| s.kind == OutputKind::Rfid) {
            trace.push("context: SD card is not an RFID tag".to_string());
        }
        outputs.retain(|s| s.kind != OutputKind::Rfid);
    }
    // A relay is a real output, but it has no template route of its own: it
    // is a sink of a composition or an added part. Keep it as a slot so a
    // plan that ignores it is visible, but never let it outrank a routable
    // load in the primary reading.
    if named.relay > 0 && !outputs.iter().any(|s| s.kind == OutputKind::Relay) {
        outputs.push(OutputSlot {
            kind: OutputKind::Relay,
            count: named.relay,
            words: vec!["реле".to_string()],
            specificity: 3,
            at: first_word_position(&text, "реле"),
        });
    }
    // "Шаговый двигатель" is one part: the shared word "двигатель" must not
    // add a second, transistor-driven motor next to the stepper. A separately
    // named motor ("шаговый двигатель и мотор") still counts.
    if named.motor == 0 && outputs.iter().any(|s| s.kind == OutputKind::Stepper) {
        if outputs.iter().any(|s| s.kind == OutputKind::Motor) {
            trace.push("context: stepper is not a second DC motor".to_string());
        }
        outputs.retain(|s| s.kind != OutputKind::Motor);
    }
    // Rank by evidence, then by what the sentence said first. Two equal
    // readings stay equal: that is an ambiguity to show, not a table index to
    // break.
    outputs.sort_by(|a, b| {
        output_score(b)
            .cmp(&output_score(a))
            .then(a.at.cmp(&b.at))
    });
    for slot in &outputs {
        trace.push(format!(
            "output: {} (evidence {}, first at {}) [{}]",
            slot.kind.label().ru,
            slot.specificity,
            slot.at,
            slot.words.join(", ")
        ));
    }
    let named_load = outputs.iter().any(|s| s.kind.load().is_some());
    if !named_load {
        trace.push("load: default (no load word)".to_string());
    }
    let mut load = outputs
        .iter()
        .find_map(|s| s.kind.load())
        .unwrap_or(Load::Led);
    let load_hits: Vec<String> = outputs
        .iter()
        .find(|s| s.kind.load() == Some(load))
        .map(|s| s.words.clone())
        .unwrap_or_default();
    if outputs.len() > 1 {
        let top = output_score(&outputs[0]);
        let second = output_score(&outputs[1]);
        if top.saturating_sub(second) <= 5 {
            trace.push(format!(
                "output: near tie, readings offered ({} vs {})",
                outputs[0].kind.label().ru, outputs[1].kind.label().ru
            ));
        }
    }
    trace.push(format!("load: {:?} [{}]", load, load_hits.join(", ")));
    matched.extend(load_hits.clone());
    if named_load {
        understood.push(load_label(load));
    }

    let input_groups = [&BUTTON, &POT, &LDR];
    let input_votes = votes(&text, &input_groups);
    let mut controls: Vec<ControlSlot> = input_votes
        .iter()
        .map(|(i, h)| {
            let kind = input_at(*i);
            ControlSlot {
                kind,
                count: control_count(kind, &named),
                words: h.clone(),
                specificity: control_specificity(kind),
                at: first_hit(&text, h),
            }
        })
        .collect();
    controls.sort_by(|a, b| {
        control_score(b)
            .cmp(&control_score(a))
            .then(a.at.cmp(&b.at))
    });
    let mut input = controls.first().map(|c| c.kind);
    let input_hits: Vec<String> = controls
        .first()
        .map(|c| c.words.clone())
        .unwrap_or_default();
    let named_input = !input_hits.is_empty();
    trace.push(format!(
        "input: {:?} [{}]",
        input,
        input_hits.join(", ")
    ));
    for slot in &controls {
        trace.push(format!(
            "control: {:?} (evidence {}, first at {}) [{}]",
            slot.kind,
            slot.specificity,
            slot.at,
            slot.words.join(", ")
        ));
    }
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

    let mut semantic = crate::templates::route(load, input);
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
        text_screen_task(&clean)
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
        && (screens_in(&clean).is_some() || named_load || named.segments > 0)
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
    let named_source = crate::compose::source_in_text(&clean);
    // A DHT board reports both temperature and humidity, so a task that asks
    // for either one is not a different lesson. A thermistor and the DS18B20
    // read the same quantity through different parts, and the lesson teaches
    // both: the part is the sentence's choice, not a different question.
    let same_reading = |a: crate::spec::SourceId, b: crate::spec::SourceId| {
        use crate::spec::SourceId::{Humidity, Temperature, Thermistor};
        a == b
            || matches!(
                (a, b),
                (Humidity, Temperature)
                    | (Temperature, Humidity)
                    | (Temperature, Thermistor)
                    | (Thermistor, Temperature)
            )
    };
    let wrong_source = lesson_source
        .zip(named_source)
        .is_some_and(|(a, b)| !same_reading(a.id, b.id));
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
        crate::compose::detect(&clean, load, named_load, lesson, action, condition)
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

    // Layer 4: every named part becomes a typed slot with its evidence. The
    // template and the checklist both read this object, so "the plan closes
    // what the sentence asked for" is a check on data, not on a vote count.
    let mut task = Task::new(raw);
    task.outputs = outputs.clone();
    task.controls = controls.clone();
    if let Some(screens) = named.screens {
        let words: Vec<String> = screens_in(raw)
            .map(|_| {
                ["дисплей", "экран", "lcd", "oled", "матричн", "семисегмент"]
                    .iter()
                    .filter(|w| text.contains(*w))
                    .map(|w| (*w).to_string())
                    .collect()
            })
            .unwrap_or_default();
        task.screens
            .push(ScreenSlot::from_screens(screens, words, 0));
    }
    if let Some(c) = condition {
        task.conditions.push(c);
    }
    if let Some(a) = action {
        task.behaviors.push(a);
    }
    task.named = named.clone();
    task.lesson = lesson;
    task.recipe = recipe;
    task.text_screens = text_screens;
    task.transistor = transistor;
    task.presses = presses;
    task.primary = outputs
        .iter()
        .find(|s| s.kind.load() == Some(load))
        .map(|s| s.kind);
    task.primary_control = input;
    task.sensors = sensor_slots(&named, raw, condition);
    // The multi-sensor builder can place several instances and read each one.
    // When it can, the sentence stops being a refusal; when it cannot, the
    // honest refusal below still stands rather than drawing one sensor and
    // failing the checklist.
    let multi = crate::compose::multi_request(&task);
    let single_board = task.sensors.len() == 2
        && task.sensors.iter().all(|s| {
            matches!(
                s.source,
                crate::spec::SourceId::Temperature | crate::spec::SourceId::Humidity
            )
        });
    let sensor_refusal = if !books {
        // The book table answers a printed task with its own canned solution,
        // whose lesson may deliberately be narrower than the words. Its
        // fallback reading is allowed to build the closest circuit; the
        // ordinary user sentence below is refused instead.
        None
    } else if multi.is_some() {
        None
    } else if task.sensors.iter().any(|s| s.count > 1) {
        Some(L::new(
            "Схема с несколькими одинаковыми датчиками пока не собирается: у каждого датчика на плате свой вход, а готовый шаблон ставит один. Назови один датчик или раздели задачу на части.",
            "A circuit with several identical sensors is not built yet: every sensor needs its own pin, and the ready-made template places one. Name one sensor or split the task.",
        ))
    } else if task.sensors.len() > 1 && !single_board {
        Some(L::new(
            "Схема с несколькими разными датчиками пока не собирается: готовый шаблон читает один. Назови один датчик или раздели задачу на части.",
            "A circuit with several different sensors is not built yet: the ready-made template reads one. Name one sensor or split the task.",
        ))
    } else {
        None
    };
    task.links = link_slots(&outputs, &named);
    task.counts.set(CountKind::Led, named.led);
    task.counts.set(CountKind::Lamp, named.lamp);
    task.counts.set(CountKind::Rgb, named.rgb);
    task.counts.set(CountKind::Button, named.button);
    task.counts.set(CountKind::Motor, named.motor);
    task.counts.set(CountKind::Servo, named.servo);
    task.counts.set(CountKind::Buzzer, named.buzzer);
    task.counts.set(CountKind::Relay, named.relay);
    task.counts
        .set(CountKind::Screen, named.screens.map(|s| s.count).unwrap_or(0));
    for slot in &task.sensors {
        task.counts.set(CountKind::Sensor(slot.source), slot.count);
    }
    trace.push(format!(
        "task: outputs {:?}, controls {:?}, sensors {:?}, screens {}, links {:?}",
        task.outputs.iter().map(|s| s.kind).collect::<Vec<_>>(),
        task.controls.iter().map(|s| s.kind).collect::<Vec<_>>(),
        task.sensors.iter().map(|s| s.source).collect::<Vec<_>>(),
        task.screens.len(),
        task.links
    ));

    // The template is chosen by the slots it closes, not by which keyword
    // group won. A lesson or a composition is already a complete reading;
    // only the built-in templates go through this comparison.
    if lesson.is_none() && recipe.is_none() && text_screens.is_none() && !task.outputs.is_empty()
    {
        if let Some((chosen_load, chosen_input)) = task.best_route() {
            if chosen_load != load || chosen_input != input {
                trace.push(format!(
                    "route: coverage picks {:?} + {:?}, was {:?} + {:?}",
                    chosen_load, chosen_input, load, input
                ));
                load = chosen_load;
                input = chosen_input;
                semantic = crate::templates::route(load, input);
                task.primary = task
                    .outputs
                    .iter()
                    .find(|s| s.kind.load() == Some(load))
                    .map(|s| s.kind);
                task.primary_control = input;
            }
        }
    }

    // A sentence that names only an input still builds when the plan really
    // uses that input: a button switches the LED, a photoresistor makes the
    // night light. A knob that the plan would ignore is not a plan.
    let input_matters =
        input.is_some() && semantic != crate::templates::route(load, None);
    let unsupported = if let Some(r) = sensor_refusal {
        Some(r)
    } else if let Some(r) = refusal {
        Some(r)
    } else if lesson.is_some() || text_screens.is_some() || recipe.is_some() {
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
    // Ambiguity is answered with one-click readings instead of a silent coin
    // toss. The alternatives are the other slots the sentence actually
    // named, never an unrelated load the parser happened to know.
    let intent_of = |load: Load, input: Option<Input>| {
        let mut reading = task.clone();
        reading.primary = reading
            .outputs
            .iter()
            .find(|s| s.kind.load() == Some(load))
            .map(|s| s.kind);
        reading.primary_control = input;
        Intent {
            load,
            input,
            task: reading,
            named: named.clone(),
            transistor,
            presses,
            lesson: None,
            action,
            condition,
            recipe,
            text_screens,
            raw: raw.to_string(),
        }
    };
    let mut alternatives = Vec::new();
    // Alternatives are the other readings of this sentence: each one is
    // another output or control the words actually named. A phrase that
    // names an LED and a button never gets a motor or a servo offered: those
    // are different questions, not a different reading of this one.
    if outputs.len() > 1 && lesson.is_none() && recipe.is_none() && text_screens.is_none() {
        for slot in outputs.iter().filter(|s| s.kind.load() != Some(load)) {
            let Some(kind) = slot.kind.load() else {
                continue;
            };
            if crate::templates::route(kind, input).is_none() {
                continue;
            }
            // A reading that silently drops a named screen is not an
            // alternative: it is the same "similar circuit" this layer
            // exists to stop. The access scene is the exception, because its
            // screen is part of the lock.
            if named.screens.is_some() && !matches!(kind, Load::Rfid) {
                continue;
            }
            if alternatives.len() >= 2 {
                break;
            }
            alternatives.push(Alternative {
                label: plan_label(kind, input),
                intent: intent_of(kind, input),
                note: Some(format!("другое прочтение: {}", slot.words.join(", "))),
            });
        }
        if !alternatives.is_empty() {
            trace.push(format!(
                "output: readings offered: {}",
                alternatives
                    .iter()
                    .map(|a| a.label.ru)
                    .collect::<Vec<_>>()
                    .join(" / ")
            ));
        }
    }
    // The other control is a different reading only when the named output can
    // actually use it and the sentence really named that control. A button
    // the user never mentioned is not an alternative to a potentiometer.
    if named_load
        && controls.len() > 1
        && lesson.is_none()
        && recipe.is_none()
        && text_screens.is_none()
    {
        for slot in controls.iter().filter(|c| Some(c.kind) != input) {
            let other = Some(slot.kind);
            if crate::templates::route(load, other).is_none() {
                continue;
            }
            if alternatives.len() >= 2 {
                break;
            }
            alternatives.push(Alternative {
                label: plan_label(load, other),
                intent: intent_of(load, other),
                note: Some(format!("другое прочтение: {}", slot.words.join(", "))),
            });
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
            note: Some("другой диапазон: 2.4 ГГц".to_string()),
        });
    }
    // Nothing is invented for an input-only phrase: a button with no output
    // is either the LED reading the template already offers or a genuinely
    // unsupported sentence. The old list (motor, servo, buzzer) was the
    // parser guessing loads the user never said.

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
            task,
            named,
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

        // A clock on a seven-segment display is a clock: the RTC keeps the
        // time and the segments show it. The segment lesson (counting digits)
        // used to win because its words matched better, and the clock was
        // silently dropped.
        let p = parse("часы на семисегментном индикаторе");
        let lesson = p.intent.lesson.expect("the clock lesson wins");
        assert!(
            crate::lessons::ALL[lesson].keys.contains(&"ds3231"),
            "picked {:?}",
            crate::lessons::ALL[lesson].title
        );
        let built = crate::templates::build(&p.intent, 0, crate::text::Lang::Ru)
            .expect("the clock builds");
        assert!(built.circuit.parts().any(|part| part.value == "DS3231"));
        assert!(
            built.circuit.parts().any(|part| part.value == "7SEG-4"),
            "the segments the sentence names must be drawn"
        );
        assert!(
            !built
                .circuit
                .parts
                .iter()
                .any(|part| part.kind == crate::model::Kind::Led),
            "a segment display is not an LED"
        );
        assert!(crate::requirements::missing(&built, &p.intent).is_empty());
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
            crate::checks::run_with(&built, crate::text::Lang::Ru, &p.intent).failed(),
            0
        );
    }

    /// "Влажность грунта" and "влажность земли" are the same probe as
    /// "влажность почвы". They used to fall through to the DHT22 lesson, and
    /// a measurement with a display was refused outright even when the recipe
    /// was found.
    #[test]
    fn soil_moisture_is_not_humidity_and_a_readout_is_enough() {
        for task in [
            "Измерить влажность грунта и вывести на экран",
            "Измерить влажность земли и вывести на LCD1602",
        ] {
            let p = parse(task);
            assert!(p.unsupported.is_none(), "{task}: {}", p.unsupported.as_ref().map(|u| &u.ru).cloned().unwrap_or_default());
            let recipe = p.intent.recipe.expect("the soil probe composes");
            assert_eq!(recipe.source, crate::spec::SourceId::Soil, "{task}");
            let built = crate::templates::build(&p.intent, 0, crate::text::Lang::Ru)
                .expect("the soil readout builds");
            assert!(
                built.circuit.parts().any(|part| part.value == "SOIL"),
                "{task} must wire the soil probe"
            );
            assert!(
                !built.circuit.parts().any(|part| part.value.starts_with("DHT")),
                "{task} is not a DHT22 question"
            );
        }
    }

    #[test]
    fn a_refused_part_is_not_named_or_built() {
        let p = parse("кнопка и светодиод, реле не нужно");
        assert_eq!(p.intent.named.relay, 0, "the relay was refused");
        assert_eq!(p.intent.named.led, 1, "the LED is still asked for");
        assert!(
            p.understood.iter().all(|l| !l.ru.contains("Реле")),
            "a refused part must not be understood: {:?}",
            p.understood.iter().map(|l| &l.ru).collect::<Vec<_>>()
        );
        let built = crate::templates::build(&p.intent, 0, crate::text::Lang::Ru)
            .expect("the button and LED build");
        assert!(
            !built
                .circuit
                .parts
                .iter()
                .any(|part| part.kind == crate::model::Kind::Relay),
            "a refused relay must not be drawn"
        );
        assert!(
            crate::requirements::missing(&built, &p.intent).is_empty(),
            "the checklist must not demand the refused relay"
        );
    }

    /// "Без датчика температуры и влажности" refuses both readings, and the
    /// LED after the comma is still a request. The word "свет" is a quantity
    /// only as "света": otherwise the LED itself would be eaten as one.
    #[test]
    fn a_refused_sensor_list_leaves_the_rest() {
        let p = parse("без датчика температуры и влажности, светодиод");
        assert_eq!(p.intent.named.temperature, 0);
        assert_eq!(p.intent.named.humidity, 0);
        assert_eq!(p.intent.named.led, 1);
        let built = crate::templates::build(&p.intent, 0, crate::text::Lang::Ru)
            .expect("the LED still builds");
        assert!(
            !built.circuit.parts().any(|part| part.value.starts_with("DS18B20")
                || part.value.starts_with("DHT")),
            "a refused sensor must not be drawn"
        );
        assert!(crate::requirements::missing(&built, &p.intent).is_empty());
    }

    /// A flash drive is storage, not an unknown load: the SD lesson answers
    /// "записать на флешку" instead of refusing the whole sentence.
    #[test]
    fn a_flash_drive_is_storage() {
        let p = parse("записать на флешку");
        assert!(p.unsupported.is_none(), "{:?}", p.unsupported.map(|u| u.ru));
        assert!(p.intent.named.sd, "the flash drive is storage");
        let built = crate::templates::build(&p.intent, 0, crate::text::Lang::Ru)
            .expect("the storage task builds");
        assert!(built.circuit.parts().any(|part| part.value == "SD"));
        assert!(crate::requirements::missing(&built, &p.intent).is_empty());
    }

    /// The parser extracts the sensors once, and the checklist reads that
    /// same object: two DHT22 and two temperature sensors are four readings,
    /// not one.
    #[test]
    fn sensor_counts_come_from_the_same_parse() {
        let p = parse("два DHT22 и два датчика температуры с выводом на экран");
        assert_eq!(p.intent.named.dht22, 2);
        assert_eq!(p.intent.named.temperature, 2);
        let p = parse("влажность грунта и насос");
        assert_eq!(p.intent.named.soil, 1);
        assert_eq!(p.intent.named.humidity, 0);
    }

    /// The refusal belongs to its clause. "Датчик температуры не нужен,
    /// нужен светодиод" asks for an LED and no sensor.
    #[test]
    fn a_refusal_does_not_reach_the_next_clause() {
        let p = parse("датчик температуры не нужен, нужен светодиод");
        assert_eq!(p.intent.named.temperature, 0);
        assert_eq!(p.intent.named.led, 1);
        assert!(p.unsupported.is_none(), "{:?}", p.unsupported.map(|u| u.ru));
        let p = parse("не нужен датчик температуры, нужен светодиод");
        assert_eq!(p.intent.named.temperature, 0);
        assert_eq!(p.intent.named.led, 1);
    }

    /// "Не мигает, а горит" is how the lamp behaves, not a refusal: dropping
    /// the LED there would lose the part the sentence is about.
    #[test]
    fn a_negative_verb_is_not_a_refusal() {
        let p = parse("светодиод не мигает, а горит");
        assert_eq!(p.intent.named.led, 1);
    }

    /// "Термистор от влажности в комнате" names a cause, not a humidity
    /// board: the checklist must not demand a sensor the task never asked for.
    #[test]
    fn a_humidity_word_can_name_a_cause() {
        let p = parse("Терменвокс на пьезопищалке и термисторе от влажности в комнате.");
        assert_eq!(p.intent.named.humidity, 0);
    }

    /// A humidity board reports temperature too, so asking for both readings
    /// keeps the DHT lesson instead of refusing the sentence as a mismatch.
    #[test]
    fn humidity_and_temperature_share_one_board() {
        let p = parse("датчик влажности и датчик температуры");
        assert!(p.unsupported.is_none(), "{:?}", p.unsupported.map(|u| u.ru));
        let built = crate::templates::build(&p.intent, 0, crate::text::Lang::Ru)
            .expect("the DHT lesson answers both readings");
        assert!(crate::requirements::missing(&built, &p.intent).is_empty());
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
        let report = crate::checks::run_with(&built, crate::text::Lang::Ru, &p.intent);
        assert_eq!(report.failed(), 0);
    }

    /// A phrase that names several outputs offers the other named outputs as
    /// readings. It never offers a load the sentence did not name.
    #[test]
    fn a_second_named_output_is_a_reading() {
        let p = parse("светодиод и лампа и мотор");
        assert!(!p.alternatives.is_empty(), "understood {:?}", p.understood);
        let labels: Vec<&str> = p
            .alternatives
            .iter()
            .map(|a| a.label.ru)
            .collect();
        assert!(
            labels.iter().any(|l| l.contains("двигатель") || l.contains("светодиод")),
            "{labels:?}"
        );
        assert!(
            p.trace.iter().any(|line| line.contains("readings offered")),
            "{:?}",
            p.trace
        );
    }

    /// An output the sentence never named is not an alternative. "Две кнопки
    /// и дисплей" used to offer a motor and a servo; those are different
    /// questions, not other readings.
    #[test]
    fn alternatives_never_invent_an_output() {
        for task in ["две кнопки и дисплей", "кнопка и дисплей"] {
            let p = parse(task);
            assert!(
                p.alternatives.iter().all(|a| {
                    !matches!(
                        a.intent.load,
                        Load::Motor | Load::Servo | Load::Buzzer | Load::Motors | Load::Stepper
                    )
                }),
                "{task}: {:?}",
                p.alternatives.iter().map(|a| a.label.ru).collect::<Vec<_>>()
            );
        }
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

    /// Two ways to say one thing must read the same way. The corpus catches
    /// drift between synonyms that the keyword tables treat separately.
    #[test]
    fn paraphrases_read_the_same_way() {
        for (n, line) in include_str!("testdata/paraphrases.txt").lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (a, b) = line
                .split_once('|')
                .unwrap_or_else(|| panic!("line {}: no | in {line:?}", n + 1));
            let (a, b) = (a.trim(), b.trim());
            let pa = parse(a);
            let pb = parse(b);
            assert_eq!(
                pa.unsupported.is_some(),
                pb.unsupported.is_some(),
                "{a:?} vs {b:?}"
            );
            assert_eq!(pa.intent.load, pb.intent.load, "{a:?} vs {b:?}");
            let sensors = |p: &Parse| {
                p.intent
                    .task
                    .sensors
                    .iter()
                    .map(|s| s.source)
                    .collect::<Vec<_>>()
            };
            let outputs = |p: &Parse| {
                p.intent
                    .task
                    .outputs
                    .iter()
                    .map(|s| s.kind)
                    .collect::<Vec<_>>()
            };
            assert_eq!(sensors(&pa), sensors(&pb), "{a:?} vs {b:?}");
            assert_eq!(outputs(&pa), outputs(&pb), "{a:?} vs {b:?}");
        }
    }

    /// The corpus is the contract: every line says what the sentence must
    /// become. It covers cases, synonyms, slang, typos, the wrong keyboard
    /// layout, English, refusals and ambiguity.
    #[test]
    fn counts_corpus_matches_the_words() {
        let corpus = include_str!("testdata/counts.txt");
        for (n, raw) in corpus.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let at = n + 1;
            let (phrase, want) = line
                .split_once('|')
                .unwrap_or_else(|| panic!("line {at}: no | in {line:?}"));
            let phrase = phrase.trim();
            let named = named_devices(phrase);
            for pair in want.split(';') {
                let (key, value) = pair
                    .trim()
                    .split_once('=')
                    .unwrap_or_else(|| panic!("line {at}: no = in {pair:?}"));
                let want: usize = value
                    .trim()
                    .parse()
                    .unwrap_or_else(|_| panic!("line {at}: bad number in {pair:?}"));
                let got = match key.trim() {
                    "led" => named.led,
                    "lamp" => named.lamp,
                    "rgb" => named.rgb,
                    "button" => named.button,
                    "motor" => named.motor,
                    "servo" => named.servo,
                    "buzzer" => named.buzzer,
                    "relay" => named.relay,
                    "screens" => named.screens.map(|s| s.count).unwrap_or(0),
                    "temperature" => named.temperature,
                    "humidity" => named.humidity,
                    "soil" => named.soil,
                    "pressure" => named.pressure,
                    "light" => named.light,
                    "range" => named.range,
                    "gas" => named.gas,
                    other => panic!("line {at}: unknown field {other:?}"),
                };
                assert_eq!(got, want, "line {at}: {phrase:?}, field {key}");
            }
        }
    }

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
                // Written by `volta feedback --corpus`: the case is still
                // understood, even though it was marked as a wrong build.
                "keep" => assert!(
                    p.unsupported.is_none(),
                    "line {at}: {phrase:?} should still be understood: {:?}",
                    p.unsupported
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

        // Everyday light words count too: a screenshot with two "лампочки"
        // drew one LED because the count table did not know the synonym.
        assert_eq!(named_devices("две лампочки").led, 2);
        assert_eq!(named_devices("две лампы").led, 2);
        assert_eq!(named_devices("пара лампочек").led, 2);
        assert_eq!(named_devices("две лампы накаливания").lamp, 2);
        assert_eq!(named_devices("две лампы накаливания").led, 0);
        assert_eq!(named_devices("лампа накаливания и светодиод").lamp, 1);
        assert_eq!(named_devices("лампа накаливания и светодиод").led, 1);
        // The same gap existed for other outputs' synonyms.
        assert_eq!(named_devices("два динамика").buzzer, 2);
        assert_eq!(named_devices("два спикера").buzzer, 2);
        assert_eq!(named_devices("две машинки").servo, 2);
        assert_eq!(named_devices("две клавиши").button, 2);
    }

    /// Counts must reach the numbers the books use and stay with their noun.
    /// A ten-LED task that becomes a one-LED blink is exactly the silent
    /// "similar, but not that" the checklist is meant to catch.
    #[test]
    fn counts_go_past_four_and_stay_with_their_noun() {
        let n = named_devices("10 светодиодов и 5 кнопок");
        assert_eq!(n.led, 10);
        assert_eq!(n.button, 5);

        let n = named_devices("3 красных и 2 зелёных светодиода");
        assert_eq!(n.led, 5);

        let n = named_devices("две кнопки и три светодиода");
        assert_eq!(n.button, 2);
        assert_eq!(n.led, 3);

        assert_eq!(named_devices("пьезо пищалка").buzzer, 1);
        assert_eq!(named_devices("3 раза мигнуть светодиодом").led, 1);
        assert_eq!(named_devices("два датчика и светодиод").led, 1);
        assert_eq!(named_devices("16 кнопок телефона").button, 0);
    }

    #[test]
    fn a_number_belongs_to_the_screen_it_counts() {
        assert_eq!(screens_in("две кнопки и дисплей").unwrap().count, 1);
        assert_eq!(screens_in("три режима и экран").unwrap().count, 1);
        assert_eq!(screens_in("два экрана").unwrap().count, 2);
        assert_eq!(screens_in("три OLED").unwrap().count, 3);
        assert_eq!(screens_in("экран 16x2").unwrap().count, 1);
    }

    /// A pin number before "и" is not a count of the next part.
    #[test]
    fn a_pin_number_is_not_a_part_count() {
        let n = named_devices("пин 13 и светодиод");
        assert_eq!(n.led, 1);
        assert_eq!(n.pin, Some(13));

        let n = named_devices("порт 3 и кнопка");
        assert_eq!(n.button, 1);

        let n = named_devices("контакт 4 и реле");
        assert_eq!(n.relay, 1);

        let n = named_devices("вывод 2 и датчик температуры");
        assert_eq!(n.temperature, 1);

        let p = parse("датчик температуры на пин 13 и дисплей");
        assert_eq!(p.intent.named.screens.map(|s| s.count), Some(1));
        assert_eq!(p.intent.named.temperature, 1);
    }

    /// The multi-sensor builder handles several named sensors; a part it
    /// cannot read twice (or a count beyond the pins and addresses available)
    /// is still refused by name instead of being drawn as one and failing the
    /// requirements checklist afterwards.
    #[test]
    fn an_undrawable_sensor_count_is_refused() {
        for task in ["пять датчиков температуры", "четыре датчика давления"] {
            let p = parse(task);
            assert!(p.unsupported.is_some(), "{task} should be refused");
        }
        // The builder places each of these and reads every one.
        for task in [
            "два датчика температуры",
            "два датчика света",
            "датчик температуры, влажности и давления",
            "датчик света и температуры",
        ] {
            let p = parse(task);
            assert!(p.unsupported.is_none(), "{task}: {:?}", p.unsupported);
            assert!(
                crate::compose::multi_request(&p.intent.task).is_some(),
                "{task} should take the multi-sensor route"
            );
        }
        // A single DHT covers temperature and humidity together: that pair is
        // one board, not a multi-sensor circuit.
        let p = parse("датчик температуры и влажности");
        assert!(p.unsupported.is_none(), "{:?}", p.unsupported);
        assert!(crate::compose::multi_request(&p.intent.task).is_none());
    }

    /// "Освещение" is what the circuit switches, not a light sensor.
    #[test]
    fn lighting_is_not_a_light_sensor() {
        for task in [
            "управление освещением",
            "регулировать яркость освещения",
            "включить освещение кнопкой",
        ] {
            let p = parse(task);
            assert_eq!(p.intent.named.light, 0, "{task}: {:?}", p.intent.named.light);
            assert!(
                !p.intent.task.sensors.iter().any(|s| {
                    s.source == crate::spec::SourceId::Light
                }),
                "{task}: a photoresistor was added"
            );
        }
        let p = parse("датчик освещённости");
        assert_eq!(p.intent.named.light, 1);
        let p = parse("два датчика света");
        assert_eq!(p.intent.named.light, 2);
    }
}
