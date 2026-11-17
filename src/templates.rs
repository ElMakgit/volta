//! From an intent to a concrete circuit, a bill of materials, and a sketch.
//!
//! Each template is a small electrical design with a few real component
//! choices. The choices are not cosmetic: picking a relay instead of a MOSFET
//! changes the wiring, the code, and what the checks can promise.

use crate::model::*;
use crate::parse::{Action, Input, Intent, Load};
use crate::text::{tr, Lang, L};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tid {
    MotorSteps,
    /// Motor speed from a potentiometer.
    MotorPot,
    /// Motor simply on, as a test of the switch.
    MotorTest,
    /// Two motors behind an L298N or L293D board.
    MotorDriver,
    /// A 28BYJ-48 stepper with its ULN2003 board.
    Stepper,
    LedToggle,
    LedBlink,
    NightLight,
    ServoSweep,
    ServoPot,
    BuzzerTone,
    Rfid,
    Radio433,
    RadioNrf,
    Bluetooth,
    Gsm,
    Ethernet,
    /// A circuit composed from a sensor, a condition and a sink.
    Composed,
    /// A lesson of the course books, by index into `lessons::ALL`.
    Lesson(usize),
}

pub fn route(load: Load, input: Option<Input>) -> Option<Tid> {
    match (load, input) {
        (Load::Motor, Some(Input::Button)) => Some(Tid::MotorSteps),
        (Load::Motor, Some(Input::Pot)) => Some(Tid::MotorPot),
        (Load::Motor, _) => Some(Tid::MotorTest),
        (Load::Motors, _) => Some(Tid::MotorDriver),
        (Load::Stepper, _) => Some(Tid::Stepper),
        (Load::Servo, Some(Input::Pot)) => Some(Tid::ServoPot),
        (Load::Servo, _) => Some(Tid::ServoSweep),
        (Load::Led, Some(Input::Button)) => Some(Tid::LedToggle),
        (Load::Led, Some(Input::Ldr)) => Some(Tid::NightLight),
        (Load::Led, _) => Some(Tid::LedBlink),
        (Load::Buzzer, _) => Some(Tid::BuzzerTone),
        (Load::Rfid, _) => Some(Tid::Rfid),
        (Load::Radio433, _) => Some(Tid::Radio433),
        (Load::RadioNrf, _) => Some(Tid::RadioNrf),
        (Load::Bluetooth, _) => Some(Tid::Bluetooth),
        (Load::Gsm, _) => Some(Tid::Gsm),
        (Load::Ethernet, _) => Some(Tid::Ethernet),
    }
}

pub struct Choice {
    pub name: L,
    pub blurb: L,
    pub pros: Vec<L>,
    pub cons: Vec<L>,
    pub specs: Vec<(L, String)>,
    pub level: u8,
}

pub struct Plan {
    pub tid: Tid,
    /// A static header for the built-in templates.
    pub title: L,
    /// A header assembled from the source and the sink, for compositions.
    pub headline: Option<String>,
    pub choices: Vec<Choice>,
    pub recommended: usize,
    /// Why this plan was chosen, shown under the variants.
    pub why: Option<String>,
}

impl Plan {
    /// The header to show: the composed one when there is one, otherwise the
    /// translated static title.
    pub fn title_text(&self, lang: Lang) -> String {
        match &self.headline {
            Some(h) => h.clone(),
            None => self.title.get(lang).to_string(),
        }
    }
}

pub struct Built {
    pub tid: Tid,
    pub title: String,
    pub circuit: Circuit,
    pub code: String,
    pub choice: usize,
    pub summary: L,
    pub notes: Vec<L>,
}

pub(crate) fn spec_line(num: &str, unit: L) -> (L, String) {
    (unit, num.to_string())
}

/// A compact choice for the cases where one real editorial line is enough.
fn quick(lang: Lang, name: L, blurb: L, current: &str, price: &str, level: u8) -> Choice {
    Choice {
        name,
        blurb,
        pros: vec![],
        cons: vec![],
        specs: vec![
            spec_line(current, CURRENT),
            spec_line(price, PRICE),
            spec_line(&level_word(level, lang), LEVEL),
        ],
        level,
    }
}

const PRICE: L = L::new("Цена", "Price");
const CURRENT: L = L::new("Ток", "Current");
const VOLTAGE: L = L::new("Напряжение", "Voltage");
const LEVEL: L = L::new("Сложность", "Effort");

fn level_word(n: u8, lang: Lang) -> String {
    let words = match n {
        1 => L::new("низкая", "low"),
        2 => L::new("средняя", "medium"),
        _ => L::new("высокая", "high"),
    };
    words.get(lang).to_string()
}

/// A door with cards kept in EEPROM is a scene of its own: the reader is the
/// input, the lock the output, and the table of cards the memory the task
/// names. "Контроль доступа на 49 карт ... EEPROM" must not fall back to the
/// EEPROM counter lesson, which counts button presses.
pub(crate) fn wants_access(raw: &str) -> bool {
    let text = crate::words::normalize(raw);
    let door = text.contains("доступ")
        || text.contains("пропуск")
        || text.contains("турникет")
        || text.contains("замок")
        || text.contains("access");
    let store = text.contains("eeprom")
        || text.contains("энергонезависим")
        || text.contains("памят");
    door && store
}

/// The number of cards a table keeps: the task names it ("на 49 карт"),
/// otherwise the EEPROM of an Uno takes 49 four-byte UIDs with room to spare.
fn card_slots(raw: &str) -> Option<usize> {
    let text = crate::words::normalize(&crate::numbers::fix_decimal_commas(raw));
    let cut = text.find("карт")?;
    let before: Vec<&str> = text[..cut].split_whitespace().collect();
    // The number closest to the word "карт" is the one that matters;
    // reading the words backwards also accepts "на сорок девять карт".
    let nearest: Vec<&str> = before.iter().rev().copied().take(6).collect();
    crate::numbers::leading_value(&nearest)
        .map(|(value, _)| value.round() as usize)
        .filter(|n| (1..=200).contains(n))
}

fn rfid_plan(lang: Lang, raw: &str) -> (L, Vec<Choice>, usize) {
    let choices = vec![
        quick(
            lang,
            L::new("RC522, UID в порт", "RC522, UID to serial"),
            L::new(
                "Считыватель по SPI, UID карты выводится в терминал.",
                "The reader on SPI prints the card UID to the terminal.",
            ),
            "30 мА",
            "средняя",
            2,
        ),
        quick(
            lang,
            L::new("RC522 + LCD1602", "RC522 + LCD1602"),
            L::new(
                "UID показывается на экране 16×2 по I2C.",
                "The UID is shown on a 16×2 screen over I2C.",
            ),
            "55 мА",
            "средняя",
            3,
        ),
        quick(
            lang,
            L::new("RC522 + реле (замок)", "RC522 + relay (lock)"),
            L::new(
                "Своя карта открывает замок на 5 секунд, чужая — нет.",
                "A known card opens the lock for five seconds, any other does not.",
            ),
            "100 мА",
            "средняя",
            3,
        ),
        quick(
            lang,
            L::new(
                "Замок, карты в EEPROM, OLED 128×64",
                "Lock, cards in EEPROM, a 128×64 OLED",
            ),
            L::new(
                "Кнопка пишет карту в таблицу EEPROM, знакомая карта открывает замок, экран показывает запись и доступ.",
                "A button saves a card into an EEPROM table, a known card opens the lock, and the screen shows the saving and the access.",
            ),
            "115 мА",
            "высокая",
            4,
        ),
    ];
    let recommended = if wants_access(raw) { 3 } else { 0 };
    (
        L::new("RFID RC522: чтение метки", "RFID RC522: reading a tag"),
        choices,
        recommended,
    )
}

/// A short, visible reason for the chosen plan, built from the same roles the
/// parser used, so the UI never claims more than it understood.
fn explain(raw: &str, lang: Lang) -> Option<String> {
    let ru = lang == Lang::Ru;
    let book = {
        #[cfg(feature = "book-tasks")]
        {
            crate::book_tasks::closest(raw).map(|(_, _, score)| score)
        }
        #[cfg(not(feature = "book-tasks"))]
        {
            None
        }
    };
    let prefix = match book {
        Some(score) => format!(
            "{} {}%; ",
            if ru { "задача из книги, совпадение" } else { "a book task, match" },
            score
        ),
        None => String::new(),
    };
    let s = crate::semantics::analyse(raw);
    if s.confidence == 0.0 {
        return (!prefix.is_empty()).then(|| prefix.trim_end_matches("; ").to_string());
    }
    let source = s.source.unwrap_or_else(|| if ru { "не назван".into() } else { "unnamed".into() });
    let sinks = if s.sinks.is_empty() {
        if ru { "нет" } else { "none" }.to_string()
    } else {
        s.sinks.join(", ")
    };
    let extra = crate::parse::named_devices(raw);
    let mut parts = Vec::new();
    if extra.screens.is_some() { parts.push(if ru { "экран" } else { "display" }); }
    if extra.buzzer > 0 { parts.push(if ru { "пищалка" } else { "buzzer" }); }
    if extra.relay > 0 { parts.push(if ru { "реле" } else { "relay" }); }
    let add = if parts.is_empty() { String::new() } else {
        format!("; {}: {}", if ru { "добавлено" } else { "added" }, parts.join(", "))
    };
    let when = match s.period_ms {
        Some(ms) => format!("; {} {} мс", if ru { "период" } else { "period" }, ms),
        None => String::new(),
    };
    let conf = format!("; {} {:.0}%", if ru { "уверенность" } else { "confidence" }, s.confidence * 100.0);
    Some(format!("{prefix}{}: {} → {}{}{}{}", if ru { "Прочитано" } else { "Read as" }, source, sinks, add, when, conf))
}

pub fn plan(intent: &Intent, lang: Lang) -> Option<Plan> {
    if let Some(recipe) = intent.recipe {
        let mut p = crate::compose::plan(recipe, lang)?;
        p.why = explain(&intent.raw, lang);
        return Some(p);
    }
    if let Some(i) = intent.lesson {
        let mut p = display_lesson_plan(i, crate::parse::named_devices(&intent.raw).screens, lang);
        p.why = explain(&intent.raw, lang);
        return Some(p);
    }
    let tid = route(intent.load, intent.input)?;
    let (title, choices, recommended) = match tid {
        Tid::MotorSteps => (
            L::new(
                "Двигатель: ступени скорости по кнопке",
                "Motor: speed steps from a button",
            ),
            vec![
                Choice {
                    name: L::new("MOSFET IRLZ44N", "MOSFET IRLZ44N"),
                    blurb: L::new(
                        "N-канальный логический MOSFET в ключе низкой стороны. ШИМ отдаёт полный диапазон без нагрева ключа.",
                        "Logic-level N-channel MOSFET as a low-side switch. PWM gives the full range without cooking the part.",
                    ),
                    pros: vec![
                        L::new("Драйв от 5 В с пина напрямую", "Driven from a 5 V pin directly"),
                        L::new("ШИМ любой частоты, не греется", "Clean PWM at any frequency"),
                        L::new("Ток до 30 А — запас на рост", "Up to 30 A of headroom"),
                    ],
                    cons: vec![
                        L::new("Нужны резистор в затвор и подтяжка 10 кОм", "Needs a gate resistor and a 10 kΩ pulldown"),
                    ],
                    specs: vec![
                        spec_line("55 В", VOLTAGE),
                        spec_line("30 А", CURRENT),
                        spec_line("низкая", PRICE),
                        spec_line(&level_word(2, lang), LEVEL),
                    ],
                    level: 2,
                },
                Choice {
                    name: L::new("Биполярный 2N2222", "Bipolar 2N2222"),
                    blurb: L::new(
                        "Классический NPN в ключе низкой стороны. Дешёвый, но ток ограничен и ключ греется.",
                        "The classic NPN low-side switch. Cheap, but the current is limited and the part warms up.",
                    ),
                    pros: vec![
                        L::new("Есть в любом наборе, копеечный", "In every parts bin, nearly free"),
                        L::new("Простая схема на три детали", "Three parts and done"),
                    ],
                    cons: vec![
                        L::new("Ток коллектора до 0.8 А", "Collector current up to 0.8 A"),
                        L::new("Насыщение требует базового тока, ключ греется", "Needs base current, and it warms up"),
                        L::new("ШИМ выше 1 кГц звучит как писк", "PWM above 1 kHz starts to whine"),
                    ],
                    specs: vec![
                        spec_line("40 В", VOLTAGE),
                        spec_line("0.8 А", CURRENT),
                        spec_line("очень низкая", PRICE),
                        spec_line(&level_word(1, lang), LEVEL),
                    ],
                    level: 1,
                },
                Choice {
                    name: L::new("Реле SRD-05VDC", "Relay SRD-05VDC"),
                    blurb: L::new(
                        "Механический контакт вместо транзистора: развязка и коммутация 220 В, но не ШИМ.",
                        "A mechanical contact instead of a transistor: isolation and mains switching, but no PWM.",
                    ),
                    pros: vec![
                        L::new("Гальваническая развязка", "Galvanic isolation"),
                        L::new("Коммутирует 220 В и большие токи", "Switches mains and big loads"),
                    ],
                    cons: vec![
                        L::new("ШИМ невозможен: только вкл/выкл", "No PWM: on or off only"),
                        L::new("Щёлкает, ресурс контактов ограничен", "Clicks, and contacts wear out"),
                        L::new("Дребезг скорости на медленном окне", "Speed jitters on a slow window"),
                    ],
                    specs: vec![
                        spec_line("250 В AC", VOLTAGE),
                        spec_line("10 А", CURRENT),
                        spec_line("средняя", PRICE),
                        spec_line(&level_word(3, lang), LEVEL),
                    ],
                    level: 3,
                },
            ],
            0,
        ),
        Tid::LedToggle => (
            L::new(
                "Светодиод: вкл/выкл по кнопке",
                "LED: on and off from a button",
            ),
            vec![
                Choice {
                    name: L::new("Красный 5 мм, 220 Ω", "Red 5 mm, 220 Ω"),
                    blurb: L::new(
                        "Самый ходовой вариант: 20 мА, падение 1.8 В, хорошо видно днём.",
                        "The default: 20 mA, 1.8 V drop, readable in daylight.",
                    ),
                    pros: vec![L::new("Проще некуда", "Nothing simpler")],
                    cons: vec![L::new("Только красный", "Only red")],
                    specs: vec![
                        spec_line("1.8 В", VOLTAGE),
                        spec_line("20 мА", CURRENT),
                        spec_line("низкая", PRICE),
                        spec_line(&level_word(1, lang), LEVEL),
                    ],
                    level: 1,
                },
                Choice {
                    name: L::new("Зелёный 5 мм, 330 Ω", "Green 5 mm, 330 Ω"),
                    blurb: L::new(
                        "Падение 2.2 В, ток 10 мА: мягче для глаз и дружелюбнее к пину.",
                        "2.2 V drop at 10 mA: easier on the eyes and on the pin.",
                    ),
                    pros: vec![L::new("Меньше ток, дольше живёт", "Less current, longer life")],
                    cons: vec![L::new("Тусклее красного при том же резисторе", "Dimmer than red at the same resistor")],
                    specs: vec![
                        spec_line("2.2 В", VOLTAGE),
                        spec_line("10 мА", CURRENT),
                        spec_line("низкая", PRICE),
                        spec_line(&level_word(1, lang), LEVEL),
                    ],
                    level: 1,
                },
                Choice {
                    name: L::new("RGB 4 вывода", "RGB, 4 pins"),
                    blurb: L::new(
                        "Три кристалла в одном корпусе: любой цвет перебором каналов.",
                        "Three dies in one package: any colour by mixing channels.",
                    ),
                    pros: vec![L::new("Любой цвет без замены детали", "Any colour without swapping parts")],
                    cons: vec![
                        L::new("Три резистора и три пина", "Three resistors and three pins"),
                        L::new("Общий катод — не забудь про землю", "Common cathode needs its ground"),
                    ],
                    specs: vec![
                        spec_line("2.0–3.2 В", VOLTAGE),
                        spec_line("20 мА на канал", CURRENT),
                        spec_line("средняя", PRICE),
                        spec_line(&level_word(2, lang), LEVEL),
                    ],
                    level: 2,
                },
            ],
            0,
        ),
        Tid::LedBlink => (
            L::new("Светодиод: мигание", "LED: blinking"),
            vec![
                Choice {
                    name: L::new("Красный 5 мм, 220 Ω", "Red 5 mm, 220 Ω"),
                    blurb: L::new("Классическое «мигает раз в секунду».", "The classic once-a-second blink."),
                    pros: vec![L::new("Проверка платы за минуту", "Board check in a minute")],
                    cons: vec![L::new("Скучно", "Boring")],
                    specs: vec![spec_line("1.8 В", VOLTAGE), spec_line("20 мА", CURRENT), spec_line("низкая", PRICE), spec_line(&level_word(1, lang), LEVEL)],
                    level: 1,
                },
                Choice {
                    name: L::new("Синий 5 мм, 150 Ω", "Blue 5 mm, 150 Ω"),
                    blurb: L::new("Падение 3.2 В, нужен резистор поменьше.", "3.2 V drop, so a smaller resistor."),
                    pros: vec![L::new("Ярко и заметно", "Bright and obvious")],
                    cons: vec![L::new("Запас по току меньше", "Less current margin")],
                    specs: vec![spec_line("3.2 В", VOLTAGE), spec_line("20 мА", CURRENT), spec_line("низкая", PRICE), spec_line(&level_word(1, lang), LEVEL)],
                    level: 1,
                },
                Choice {
                    name: L::new("Три светодиода", "Three LEDs"),
                    blurb: L::new(
                        "Бегущий огонь на D9–D11: три канала и три резистора.",
                        "A chaser on D9–D11: three channels, three resistors.",
                    ),
                    pros: vec![L::new("Сразу видно порядок фаз", "Shows the phase order at a glance")],
                    cons: vec![L::new("Больше проводов на макетке", "More jumpers on the board")],
                    specs: vec![spec_line("1.8 В ×3", VOLTAGE), spec_line("60 мА всего", CURRENT), spec_line("низкая", PRICE), spec_line(&level_word(2, lang), LEVEL)],
                    level: 2,
                },
            ],
            0,
        ),
        Tid::NightLight => (
            L::new(
                "Ночник: фоторезистор и светодиод",
                "Night light: photoresistor and LED",
            ),
            vec![
                Choice {
                    name: L::new("GL5528 + 10 кОм", "GL5528 + 10 kΩ"),
                    blurb: L::new(
                        "Делитель из фоторезистора и 10 кОм: широкий диапазон, мягкий порог.",
                        "Photoresistor and 10 kΩ divider: wide range, gentle threshold.",
                    ),
                    pros: vec![L::new("Терпит смену лампы в комнате", "Copes with a change of room light")],
                    cons: vec![L::new("Порог плавает от температуры", "Threshold drifts with temperature")],
                    specs: vec![spec_line("5 В", VOLTAGE), spec_line("1 мА", CURRENT), spec_line("низкая", PRICE), spec_line(&level_word(1, lang), LEVEL)],
                    level: 1,
                },
                Choice {
                    name: L::new("GL5528 + 4.7 кОм", "GL5528 + 4.7 kΩ"),
                    blurb: L::new(
                        "Тот же делитель, но сдвинутый порог: включается раньше.",
                        "Same divider with a shifted threshold: it turns on earlier.",
                    ),
                    pros: vec![L::new("Срабатывает в сумерках, не в темноте", "Switches at dusk, not in the dark")],
                    cons: vec![L::new("Может дребезжать у окна", "Can chatter near a window")],
                    specs: vec![spec_line("5 В", VOLTAGE), spec_line("1 мА", CURRENT), spec_line("низкая", PRICE), spec_line(&level_word(1, lang), LEVEL)],
                    level: 2,
                },
                Choice {
                    name: L::new("LDR + реле", "LDR + relay"),
                    blurb: L::new(
                        "Фоторезистор управляет реле: можно включить настоящую лампу 220 В.",
                        "The photoresistor drives a relay, so a real mains lamp can be switched.",
                    ),
                    pros: vec![L::new("Нагрузка любой мощности", "Any load size")],
                    cons: vec![
                        L::new("Реле на макетке — только для слаботочки", "A relay on a breadboard is for low voltage only"),
                        L::new("Нет плавности, только порог", "No dimming, just a threshold"),
                    ],
                    specs: vec![spec_line("250 В AC", VOLTAGE), spec_line("10 А", CURRENT), spec_line("высокая", PRICE), spec_line(&level_word(3, lang), LEVEL)],
                    level: 3,
                },
            ],
            0,
        ),
        Tid::ServoSweep => (
            L::new("Сервопривод: проход по углам", "Servo: sweep between angles"),
            vec![
                Choice {
                    name: L::new("SG90, 5 В", "SG90, 5 V"),
                    blurb: L::new(
                        "Микросерво на 180°: питается от 5 В платы и почти не грузит её.",
                        "A micro servo over 180°: runs from the board's 5 V and barely loads it.",
                    ),
                    pros: vec![L::new("Работает от платы без БП", "Runs off the board, no PSU")],
                    cons: vec![L::new("Момент слабый, 1.8 кг·см", "Weak torque, 1.8 kg·cm")],
                    specs: vec![spec_line("5 В", VOLTAGE), spec_line("0.25 А", CURRENT), spec_line("низкая", PRICE), spec_line(&level_word(1, lang), LEVEL)],
                    level: 1,
                },
                Choice {
                    name: L::new("MG996R, VIN 6 В", "MG996R, VIN 6 V"),
                    blurb: L::new(
                        "Сильное серво: питание с VIN, земля общая с платой.",
                        "A strong servo: power from VIN, ground shared with the board.",
                    ),
                    pros: vec![L::new("Момент 10 кг·см", "10 kg·cm of torque")],
                    cons: vec![
                        L::new("VIN должен быть ≥ 6 В", "VIN must be at least 6 V"),
                        L::new("Пиковый ток до 2.5 А", "Peaks up to 2.5 A"),
                    ],
                    specs: vec![spec_line("6 В", VOLTAGE), spec_line("2.5 А пик", CURRENT), spec_line("средняя", PRICE), spec_line(&level_word(2, lang), LEVEL)],
                    level: 2,
                },
                Choice {
                    name: L::new("SG90 + потенциометр", "SG90 + potentiometer"),
                    blurb: L::new(
                        "Угол задаётся ручкой: потенциометр на A0, серво на D9.",
                        "The angle follows a knob: potentiometer on A0, servo on D9.",
                    ),
                    pros: vec![L::new("Живое управление без кода в прошивке", "Live control without reflashing")],
                    cons: vec![L::new("Дребезг АЦП надо сглаживать", "The ADC needs smoothing")],
                    specs: vec![spec_line("5 В", VOLTAGE), spec_line("0.25 А", CURRENT), spec_line("средняя", PRICE), spec_line(&level_word(2, lang), LEVEL)],
                    level: 2,
                },
            ],
            0,
        ),
        Tid::BuzzerTone => (
            L::new("Звук: мелодия по таймеру", "Sound: a timed melody"),
            vec![
                Choice {
                    name: L::new("Пассивный пьезо", "Passive piezo"),
                    blurb: L::new(
                        "Пьезоизлучатель без генератора: частоту задаёт плата.",
                        "A piezo element without its own oscillator: the board sets the pitch.",
                    ),
                    pros: vec![L::new("Любая нота и любая мелодия", "Any note, any melody")],
                    cons: vec![L::new("Нужен tone() и таймер", "Needs tone() and a timer")],
                    specs: vec![spec_line("5 В", VOLTAGE), spec_line("30 мА", CURRENT), spec_line("низкая", PRICE), spec_line(&level_word(1, lang), LEVEL)],
                    level: 1,
                },
                Choice {
                    name: L::new("Активный буззер", "Active buzzer"),
                    blurb: L::new(
                        "Внутри свой генератор: включается логическим уровнем.",
                        "Its own oscillator: a logic level turns it on.",
                    ),
                    pros: vec![L::new("Один digitalWrite", "One digitalWrite")],
                    cons: vec![L::new("Одна частота, мелодии нет", "One fixed pitch, no melody")],
                    specs: vec![spec_line("5 В", VOLTAGE), spec_line("25 мА", CURRENT), spec_line("низкая", PRICE), spec_line(&level_word(1, lang), LEVEL)],
                    level: 1,
                },
                Choice {
                    name: L::new("Динамик 8 Ω + 2N2222", "8 Ω speaker + 2N2222"),
                    blurb: L::new(
                        "Настоящий динамик через ключ: громче пьезо, но нужен транзистор.",
                        "A real speaker behind a switch: louder than a piezo, but needs a transistor.",
                    ),
                    pros: vec![L::new("Слышно в шумной комнате", "Audible in a noisy room")],
                    cons: vec![
                        L::new("Нельзя подключать динамик к пину напрямую", "Never wire a speaker straight to a pin"),
                        L::new("Больше деталей", "More parts"),
                    ],
                    specs: vec![spec_line("5 В", VOLTAGE), spec_line("0.5 А пик", CURRENT), spec_line("средняя", PRICE), spec_line(&level_word(2, lang), LEVEL)],
                    level: 2,
                },
            ],
            0,
        ),
        Tid::MotorPot => (
            L::new(
                "Двигатель: скорость с потенциометра",
                "Motor: speed from a potentiometer",
            ),
            vec![
                quick(
                    lang,
                    L::new("MOSFET IRLZ44N", "MOSFET IRLZ44N"),
                    L::new(
                        "Ручка задаёт скважность ШИМ: от 0 до 100 % без нагрева ключа.",
                        "The knob sets the PWM duty from 0 to 100 % without heating the switch.",
                    ),
                    "30 А",
                    "низкая",
                    1,
                ),
                quick(
                    lang,
                    L::new("Биполярный 2N2222", "Bipolar 2N2222"),
                    L::new(
                        "Дешёвый ключ для маломощного мотора, но ток ограничен 0.8 А.",
                        "A cheap switch for a small motor, limited to 0.8 A.",
                    ),
                    "0.8 А",
                    "низкая",
                    2,
                ),
                quick(
                    lang,
                    L::new("Модуль L298N", "L298N module"),
                    L::new(
                        "Готовый драйвер: мотор до 2 А и своя защита, но нужен внешний 12 В.",
                        "A ready driver: 2 A per channel and its own protection, but it wants an external 12 V.",
                    ),
                    "2 А",
                    "средняя",
                    2,
                ),
            ],
            0,
        ),
        Tid::MotorTest => (
            L::new(
                "Двигатель: тест ключом",
                "Motor: test through a switch",
            ),
            vec![
                quick(
                    lang,
                    L::new("MOSFET IRLZ44N", "MOSFET IRLZ44N"),
                    L::new(
                        "Мотор включается на полную с пина D9, ключ не греется.",
                        "The motor runs flat out from D9, and the switch stays cool.",
                    ),
                    "30 А",
                    "низкая",
                    1,
                ),
                quick(
                    lang,
                    L::new("Биполярный 2N2222", "Bipolar 2N2222"),
                    L::new(
                        "Тот же тест на NPN: база через 1 кОм, ток до 0.8 А.",
                        "The same test on an NPN: base through 1 kΩ, up to 0.8 A.",
                    ),
                    "0.8 А",
                    "низкая",
                    2,
                ),
                quick(
                    lang,
                    L::new("Реле SRD-05VDC", "Relay SRD-05VDC"),
                    L::new(
                        "Полная развязка и 220 В на контактах, но только вкл/выкл.",
                        "Full isolation and mains on the contacts, but on/off only.",
                    ),
                    "10 А",
                    "средняя",
                    3,
                ),
            ],
            0,
        ),
        Tid::MotorDriver => (
            L::new(
                "Драйвер: два двигателя вперёд-назад",
                "Driver: two motors forward and back",
            ),
            vec![
                quick(
                    lang,
                    L::new("L298N, 2 канала", "L298N, 2 channels"),
                    L::new(
                        "Два мотора до 2 А каждый, ШИМ по ENA/ENB, питание 12 В отдельно.",
                        "Two motors up to 2 A each, PWM on ENA/ENB, 12 V supply separately.",
                    ),
                    "2 А/канал",
                    "средняя",
                    2,
                ),
                quick(
                    lang,
                    L::new("L293D, 2 канала", "L293D, 2 channels"),
                    L::new(
                        "Меньше и дешевле, но 0.6 А на канал и заметное падение напряжения.",
                        "Smaller and cheaper, but 0.6 A per channel and a noticeable voltage drop.",
                    ),
                    "0.6 А/канал",
                    "низкая",
                    1,
                ),
            ],
            0,
        ),
        Tid::Stepper => (
            L::new(
                "Шаговый двигатель 28BYJ-48",
                "28BYJ-48 stepper motor",
            ),
            vec![
                quick(
                    lang,
                    L::new("ULN2003 + 28BYJ-48", "ULN2003 + 28BYJ-48"),
                    L::new(
                        "Родная плата из набора: четыре ключа Дарлингтона и разъём мотора.",
                        "The board from the kit: four Darlington switches and the motor connector.",
                    ),
                    "240 мА",
                    "низкая",
                    1,
                ),
                quick(
                    lang,
                    L::new("ULN2003 + кнопка и ручка", "ULN2003 + button and knob"),
                    L::new(
                        "Кнопка меняет направление, потенциометр — скорость шагов.",
                        "The button reverses direction, the potentiometer sets the step rate.",
                    ),
                    "240 мА",
                    "средняя",
                    2,
                ),
            ],
            0,
        ),
        Tid::ServoPot => (
            L::new(
                "Сервопривод по потенциометру",
                "Servo from a potentiometer",
            ),
            vec![
                quick(
                    lang,
                    L::new("SG90", "SG90"),
                    L::new(
                        "Малый сервопривод: угол следует за ручкой, питание 5 В.",
                        "A small servo: the angle follows the knob, powered from 5 V.",
                    ),
                    "250 мА",
                    "низкая",
                    1,
                ),
                quick(
                    lang,
                    L::new("MG996R", "MG996R"),
                    L::new(
                        "Металлические шестерни и 2.5 А в броске: только от отдельного питания.",
                        "Metal gears and 2.5 A peaks: needs its own supply.",
                    ),
                    "2.5 А пик",
                    "средняя",
                    2,
                ),
            ],
            0,
        ),
        Tid::Rfid => rfid_plan(lang, &intent.raw),
        Tid::Radio433 => (
            L::new("Радио 433 МГц: передатчик и приёмник", "433 MHz radio: transmitter and receiver"),
            vec![
                quick(
                    lang,
                    L::new("FS1000A + XY-MK-5V", "FS1000A + XY-MK-5V"),
                    L::new(
                        "Пара с одной платы: данные идут в одну сторону, антенна обязательна.",
                        "The usual pair: one-way data, and an antenna is mandatory.",
                    ),
                    "30 мА",
                    "низкая",
                    2,
                ),
                quick(
                    lang,
                    L::new("Приёмник + реле", "Receiver + relay"),
                    L::new(
                        "Принятая команда включает реле: готовый радиоудлинитель.",
                        "A received command clicks a relay: a ready radio extender.",
                    ),
                    "100 мА",
                    "средняя",
                    3,
                ),
                quick(
                    lang,
                    L::new("Передатчик + светодиод", "Transmitter + LED"),
                    L::new(
                        "Передатчик мигает светодиодом при отправке '1' и '0'.",
                        "The transmitter blinks an LED while sending '1' and '0'.",
                    ),
                    "50 мА",
                    "низкая",
                    2,
                ),
            ],
            0,
        ),
        Tid::RadioNrf => (
            L::new("NRF24L01: обмен по 2.4 ГГц", "NRF24L01: 2.4 GHz link"),
            vec![
                quick(
                    lang,
                    L::new("NRF24L01, данные в порт", "NRF24L01, data to serial"),
                    L::new(
                        "Два числа по SPI, приём выводится в терминал.",
                        "Two numbers over SPI, printed by the receiver.",
                    ),
                    "15 мА",
                    "средняя",
                    2,
                ),
                quick(
                    lang,
                    L::new("NRF24L01 + LCD1602", "NRF24L01 + LCD1602"),
                    L::new(
                        "Метеостанция: температура и свет на экране 16×2.",
                        "A weather station: temperature and light on a 16×2 screen.",
                    ),
                    "40 мА",
                    "высокая",
                    3,
                ),
            ],
            0,
        ),
        Tid::Bluetooth => (
            L::new("Bluetooth HC-05: команды с телефона", "Bluetooth HC-05: commands from a phone"),
            vec![
                quick(
                    lang,
                    L::new("HC-05", "HC-05"),
                    L::new(
                        "Классический модуль: команды '1' и '0' включают светодиод.",
                        "The classic module: '1' and '0' switch an LED.",
                    ),
                    "40 мА",
                    "средняя",
                    2,
                ),
                quick(
                    lang,
                    L::new("HC-06", "HC-06"),
                    L::new(
                        "Только ведомый, зато проще: тот же обмен командами.",
                        "Slave only, but simpler: the same command exchange.",
                    ),
                    "40 мА",
                    "средняя",
                    2,
                ),
                quick(
                    lang,
                    L::new("HC-05 + реле", "HC-05 + relay"),
                    L::new(
                        "Телефон управляет нагрузкой через реле, а не только светодиодом.",
                        "The phone switches a load through a relay, not just an LED.",
                    ),
                    "110 мА",
                    "средняя",
                    3,
                ),
            ],
            0,
        ),
        Tid::Gsm => (
            L::new("GSM NEOWAY M590: SMS и звонки", "GSM NEOWAY M590: SMS and calls"),
            vec![
                quick(
                    lang,
                    L::new("M590, проверка AT", "M590, AT check"),
                    L::new(
                        "Модем отвечает на AT, ответы идут в терминал.",
                        "The modem answers AT and the replies go to the terminal.",
                    ),
                    "500 мА",
                    "высокая",
                    3,
                ),
                quick(
                    lang,
                    L::new("M590 + кнопка: звонок", "M590 + button: a call"),
                    L::new(
                        "Кнопка запускает звонок на заданный номер.",
                        "A button places a call to a fixed number.",
                    ),
                    "2 А пик",
                    "высокая",
                    3,
                ),
                quick(
                    lang,
                    L::new("M590 + реле: SMS-команда", "M590 + relay: SMS command"),
                    L::new(
                        "Принятая SMS с кодовым словом переключает реле.",
                        "An incoming SMS with the code word toggles the relay.",
                    ),
                    "2 А пик",
                    "высокая",
                    3,
                ),
            ],
            0,
        ),
        Tid::Ethernet => (
            L::new("Ethernet: страница в браузере", "Ethernet: a page in the browser"),
            vec![
                quick(
                    lang,
                    L::new("W5100 Shield", "W5100 shield"),
                    L::new(
                        "Готовая плата-бутерброд: веб-сервер на порту 80 из коробки.",
                        "A ready shield: a web server on port 80 out of the box.",
                    ),
                    "200 мА",
                    "средняя",
                    2,
                ),
                quick(
                    lang,
                    L::new("ENC28J60", "ENC28J60"),
                    L::new(
                        "Отдельный модуль по SPI: дешевле, но нужна библиотека EtherCard.",
                        "A separate SPI module: cheaper, but it needs the EtherCard library.",
                    ),
                    "180 мА",
                    "высокая",
                    3,
                ),
                quick(
                    lang,
                    L::new("W5100 + реле", "W5100 + relay"),
                    L::new(
                        "Кнопка на веб-странице включает реле в локальной сети.",
                        "A button on the web page switches a relay on the local network.",
                    ),
                    "270 мА",
                    "высокая",
                    3,
                ),
            ],
            0,
        ),
        Tid::Lesson(_) | Tid::Composed => return None,
    };
    Some(Plan {
        tid,
        title,
        headline: None,
        choices,
        recommended,
        why: explain(&intent.raw, lang),
    })
}

/// The display types offered when a sentence names a screen but not its
/// model. `choice` picks one; a second and third screen reuse it.
fn display_modules(screens: crate::parse::Screens) -> Vec<(&'static str, &'static str)> {
    use crate::parse::ScreenKind;
    match screens.kind {
        ScreenKind::Oled => vec![("OLED 128×64 (I2C)", "OLED 0.96")],
        ScreenKind::Lcd => vec![("LCD1602 16×2 (I2C)", "LCD1602")],
        ScreenKind::Lcd2004 => vec![("LCD2004 20×4 (I2C)", "LCD2004")],
        ScreenKind::Any => vec![
            ("OLED 128×64 (I2C)", "OLED 0.96"),
            ("LCD1602 16×2 (I2C)", "LCD1602"),
            ("LCD2004 20×4 (I2C)", "LCD2004"),
        ],
    }
}

/// A lesson plus an explicitly named display is no longer a single card: the
/// reader gets to choose which screen to put on the bench.
fn display_lesson_plan(i: usize, screens: Option<crate::parse::Screens>, lang: Lang) -> Plan {
    let base = lesson_plan(i, lang);
    let Some(screens) = screens else { return base };
    let level = base.choices[0].level;
    let choices = display_modules(screens)
        .into_iter()
        .map(|(name, _)| Choice {
            name: L::new(name, name),
            blurb: L::new(
                "Экран подключён к шине I2C: SDA на A4, SCL на A5.",
                "The screen sits on I2C: SDA on A4, SCL on A5.",
            ),
            pros: Vec::new(),
            cons: Vec::new(),
            specs: vec![
                spec_line("I2C · A4/A5", L::new("Шина", "Bus")),
                spec_line(&screens.count.to_string(), L::new("Экранов", "Screens")),
            ],
            level,
        })
        .collect();
    Plan {
        tid: base.tid,
        title: L::new("Экран для урока", "A display for the lesson"),
        headline: Some(format!(
            "{} {}",
            screens.count,
            if lang == Lang::Ru { "экранов на I2C" } else { "I2C displays" }
        )),
        choices,
        recommended: 0,
        why: None,
    }
}

/// A lesson is a single ready-made design, so the plan window shows one card:
/// the title, what it teaches, and what sits on the bench.
fn lesson_plan(i: usize, lang: Lang) -> Plan {
    let l = &crate::lessons::ALL[i];
    let mut benches = String::new();
    for spec in l.boards {
        if !benches.is_empty() {
            benches.push_str(", ");
        }
        benches.push_str(spec.name);
    }
    if benches.is_empty() {
        benches.push_str("макетка");
    }
    let mut specs = vec![spec_line(&benches, L::new("На столе", "On the bench"))];
    let roman = ["I", "II", "III", "IV"];
    let part = roman
        .get((l.book as usize).saturating_sub(1))
        .copied()
        .unwrap_or("I");
    specs.push(spec_line(
        &format!("{part}, {}", l.number),
        L::new("Книга, урок", "Book, lesson"),
    ));
    specs.push(spec_line(&level_word(l.level, lang), L::new("Сложность", "Effort")));
    Plan {
        tid: Tid::Lesson(i),
        title: L::new("Урок из книги", "A lesson from the book"),
        headline: None,
        choices: vec![Choice {
            name: l.title,
            blurb: l.summary,
            pros: Vec::new(),
            cons: Vec::new(),
            specs,
            level: l.level,
        }],
        recommended: 0,
        why: None,
    }
}

/// Build a lesson: the boards it names, their supply from the module table,
/// the extras with their resistors, and the sketch the book prints.
fn lesson_build(
    lang: Lang,
    i: usize,
    screens: Option<crate::parse::Screens>,
    choice: usize,
) -> (Circuit, String, L, Vec<L>) {
    use crate::lessons::{Extra, LoadKind};

    let l = &crate::lessons::ALL[i];
    let mut b = B::new(lang);
    let mut boards = Vec::new();
    let mut placed: Vec<(Pt, f32)> = Vec::new();
    let mut label = 2u8;
    for spec in l.boards {
        let name = format!("U{label}");
        label += 1;
        // A wide board would run into the Uno at x = 96, so it moves one
        // row down instead of being drawn on top of the controller.
        let (w, h) = crate::modules::find(spec.name)
            .map(|m| (m.w, m.h))
            .unwrap_or((40.0, 30.0));
        let mut at = Pt::new(spec.at.0, spec.at.1);
        if at.x + w > 92.0 {
            let below = placed
                .iter()
                .filter(|(p, _)| p.x + w > 92.0)
                .map(|(p, ph)| p.y + ph + 6.0)
                .fold(66.0f32, f32::max);
            at = Pt::new(2.0, below);
        }
        let p = b.breakout(&name, spec.name, at);
        if let Some(m) = crate::modules::find(spec.name) {
            for (pin, dx, _) in m.pins.iter().copied() {
                match pin {
                    "GND" => {
                        let g = b.gnd(at.x + dx);
                        b.w(b.mp(p, pin), g, NetClass::Gnd);
                    }
                    "VCC" | "+5V" | "+12V" | "3V3" | "+" => {
                        if m.three_three {
                            b.w(b.uno_pin("3V3"), b.mp(p, pin), NetClass::Vcc);
                        } else {
                            let v = b.vcc(at.x + dx);
                            b.w(b.mp(p, pin), v, NetClass::Vcc);
                        }
                    }
                    _ => {}
                }
            }
        }
        placed.push((at, h));
        boards.push(p);
    }

    // Signal wiring from the table. A board pin that the table names but the
    // module does not have is a data bug; skip it instead of panicking.
    for (pin, target) in l.wires {
        let source = boards.iter().copied().find(|&p| {
            b.c.parts[p]
                .pins
                .iter()
                .any(|&q| b.c.pins[q].name == *pin)
        });
        let Some(part) = source else { continue };
        let a = b.mp(part, pin);
        let z = b.target(target);
        let class = match *target {
            "GND" => NetClass::Gnd,
            "5V" | "VCC" | "3V3" => NetClass::Vcc,
            _ => NetClass::Signal,
        };
        b.w(a, z, class);
    }

    // Bench parts go under the boards, clear of the widest one in the lesson.
    let mut bench_y = 58.0f32;
    for (at, h) in &placed {
        bench_y = bench_y.max(at.y + h + 8.0);
    }

    for extra in l.extras {
        match *extra {
            Extra::Button { pin } => {
                let btn = b.button("B1", 4, Row::E);
                let g = b.gnd(10.0);
                b.w(b.uno_pin(pin), btn[3], NetClass::Signal);
                b.w(btn[0], g, NetClass::Gnd);
            }
            Extra::Led { pin, colour } => {
                let (ra, _rb) = b.res("R1", "220 Ом", 21, Row::H, 3);
                let (_a, k) = b.led("D1", colour, 24, Row::H);
                b.w(b.uno_pin(pin), ra, NetClass::Signal);
                b.w(k, b.gnd(30.0), NetClass::Gnd);
            }
            Extra::Rgb { r, g, b: blue } => {
                let legs = b.rgb("D1", "RGB 5 мм", 8, Row::H);
                for (n, (pin, res)) in [r, g, blue]
                    .into_iter()
                    .zip(["R1", "R2", "R3"])
                    .enumerate()
                {
                    let (ra, rb) = b.res(res, "220 Ом", 12 + n as u8 * 4, Row::A, 3);
                    b.w(b.uno_pin(pin), ra, NetClass::Signal);
                    b.w(rb, legs[n], NetClass::Signal);
                }
                b.w(legs[3], b.gnd(28.0), NetClass::Gnd);
            }
            Extra::Ldr { pin } => {
                let (ra, rb) = b.res("R2", "10 кОм", 12, Row::H, 3);
                let (_la, lb) = b.axial(Kind::Ldr, "R3", "фоторезистор", 15, Row::H, 1);
                b.w(b.vcc(10.0), ra, NetClass::Vcc);
                b.w(rb, b.uno_pin(pin), NetClass::Signal);
                b.w(lb, b.gnd(20.0), NetClass::Gnd);
            }
            Extra::Pot { pin } => {
                let pot = b.bench(Kind::Pot, "RV1", "10 кОм", Pt::new(2.0, bench_y));
                b.w(pot[0], b.vcc(12.0), NetClass::Vcc);
                b.w(pot[1], b.uno_pin(pin), NetClass::Signal);
                b.w(pot[2], b.gnd(18.0), NetClass::Gnd);
            }
            Extra::Buzzer { pin } => {
                let z = b.bench(Kind::Buzzer, "BZ1", "пьезо", Pt::new(2.0, bench_y));
                b.w(b.uno_pin(pin), z[1], NetClass::Signal);
                b.w(z[0], b.gnd(16.0), NetClass::Gnd);
            }
            Extra::IrLed { pin } => {
                let (ra, _rb) = b.res("R1", "220 Ом", 21, Row::H, 3);
                let (_a, k) = b.led("D2", "ИК-светодиод", 24, Row::H);
                b.w(b.uno_pin(pin), ra, NetClass::Signal);
                b.w(k, b.gnd(30.0), NetClass::Gnd);
            }
            Extra::Reed { pin } => {
                let (ra, rb) = b.axial(Kind::Reed, "R2", "геркон", 12, Row::H, 3);
                b.w(b.uno_pin(pin), ra, NetClass::Signal);
                b.w(rb, b.gnd(20.0), NetClass::Gnd);
            }
            Extra::Mosfet { gate, load, value } => {
                let (ra, rb) = b.res("R1", "1 кОм", 20, Row::A, 3);
                let q = b.to92(Kind::Nmos, "Q1", "IRLZ44N", 20, Row::E, ["G", "S", "D"]);
                b.w(b.uno_pin(gate), ra, NetClass::Signal);
                b.w(rb, q[0], NetClass::Signal);
                b.w(q[1], b.gnd(26.0), NetClass::Gnd);
                match load {
                    LoadKind::Motor => {
                        let m = b.bench(Kind::Motor, "M1", value, Pt::new(2.0, bench_y));
                        let d = b.diode("D1", "1N4007", 2.0, bench_y + 24.0);
                        b.w(m[0], b.vcc(34.0), NetClass::Vcc);
                        b.w(m[1], q[2], NetClass::Signal);
                        b.w(d.1, m[0], NetClass::Vcc);
                        b.w(d.0, m[1], NetClass::Signal);
                    }
                    LoadKind::Peltier => {
                        let t = b.bench(Kind::Peltier, "TE1", value, Pt::new(2.0, bench_y));
                        b.w(t[0], b.vcc(30.0), NetClass::Vcc);
                        b.w(t[1], q[2], NetClass::Signal);
                    }
                }
            }
            Extra::Relay { pin, load, value } => {
                let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC-SL-C", Pt::new(2.0, bench_y));
                b.w(b.uno_pin(pin), rl[0], NetClass::Signal);
                b.w(rl[1], b.vcc(16.0), NetClass::Vcc);
                b.w(rl[2], b.gnd(22.0), NetClass::Gnd);
                match load {
                    LoadKind::Motor => {
                        let m = b.bench(Kind::Motor, "M1", value, Pt::new(50.0, bench_y));
                        let d = b.diode("D1", "1N4007", 50.0, bench_y + 24.0);
                        b.w(m[0], b.vcc(56.0), NetClass::Vcc);
                        b.w(m[1], rl[4], NetClass::Signal);
                        b.w(rl[3], b.gnd(28.0), NetClass::Gnd);
                        b.w(d.1, m[0], NetClass::Vcc);
                        b.w(d.0, m[1], NetClass::Signal);
                    }
                    LoadKind::Peltier => {
                        let t = b.bench(Kind::Peltier, "TE1", value, Pt::new(50.0, bench_y));
                        b.w(t[0], b.vcc(56.0), NetClass::Vcc);
                        b.w(t[1], rl[4], NetClass::Signal);
                        b.w(rl[3], b.gnd(28.0), NetClass::Gnd);
                    }
                }
            }
        }
    }

    // A sentence that names a screen adds it even when the rest of the task
    // routes to a lesson: "часы ... на любой тип дисплея" is a clock *with* a
    // display, and "два экрана" means two of them.
    if let Some(wish) = screens {
        let modules = display_modules(wish);
        let module = modules[choice.min(modules.len() - 1)].1;
        for n in 0..wish.count {
            let name = format!("U{}", label);
            label += 1;
            let at = Pt::new(2.0 + n as f32 * 36.0, bench_y + 36.0);
            let disp = b.breakout(&name, module, at);
            if let Some(m) = crate::modules::find(module) {
                for (pin, dx, _) in m.pins.iter().copied() {
                    match pin {
                        "GND" => b.w(b.mp(disp, pin), b.gnd(at.x + dx), NetClass::Gnd),
                        "VCC" | "+5V" | "3V3" | "+" => {
                            b.w(b.mp(disp, pin), b.vcc(at.x + dx), NetClass::Vcc)
                        }
                        "SDA" => b.w(b.mp(disp, pin), b.uno_pin("A4"), NetClass::Signal),
                        "SCL" => b.w(b.mp(disp, pin), b.uno_pin("A5"), NetClass::Signal),
                        _ => {}
                    }
                }
            }
            placed.push((at, 30.0));
        }
    }

    feeds(&mut b);
    let mut notes = l.notes.to_vec();
    if let Some(m) = l.boards.first().and_then(|s| crate::modules::note(s.name)) {
        notes.push(m);
    }
    let base_code = l.code.render(l.title.get(lang));
    let code = if i == 31 && screens.is_some() {
        clock_display_code(lang, screens.unwrap(), choice.min(display_modules(screens.unwrap()).len() - 1))
    } else if screens.is_some() {
        format!("{}\n// Экран по I2C: SDA A4, SCL A5\n{}", base_code, display_note(screens.unwrap(), lang))
    } else {
        base_code
    };
    (b.finish(), code, l.summary, notes)
}

fn display_note(screens: crate::parse::Screens, lang: Lang) -> &'static str {
    use crate::parse::ScreenKind;
    match screens.kind {
        ScreenKind::Oled => if lang == Lang::Ru {
            "// OLED: #include <Adafruit_SSD1306.h>, SDA A4, SCL A5, адрес 0x3C."
        } else {
            "// OLED: #include <Adafruit_SSD1306.h>, SDA A4, SCL A5, address 0x3C."
        },
        _ => if lang == Lang::Ru {
            "// LCD I2C: #include <LiquidCrystal_I2C.h>, SDA A4, SCL A5, адрес 0x27."
        } else {
            "// LCD I2C: #include <LiquidCrystal_I2C.h>, SDA A4, SCL A5, address 0x27."
        },
    }
}

/// A complete sketch for the clock lesson with one or more I2C displays.
fn clock_display_code(lang: Lang, screens: crate::parse::Screens, choice: usize) -> String {
    use crate::parse::ScreenKind;
    let oled = match screens.kind {
        ScreenKind::Oled => true,
        ScreenKind::Any => choice == 0,
        _ => false,
    };
    let mut includes = String::from("#include <Wire.h>\n#include <RTClib.h>\n");
    if oled {
        includes.push_str("#include <Adafruit_GFX.h>\n#include <Adafruit_SSD1306.h>\n");
    } else {
        includes.push_str("#include <LiquidCrystal_I2C.h>\n");
    }
    let mut objects = String::from("RTC_DS3231 rtc;\nconst uint32_t PERIOD_MS = 1000;\nuint32_t last = 0;\n");
    let mut setup = String::from("  Wire.begin();\n  Serial.begin(9600);\n  if (!rtc.begin()) Serial.println(F(\"no RTC\"));\n");
    let mut show = String::new();
    for n in 0..screens.count {
        let name = format!("screen{n}");
        if oled {
            let addr = if n == 0 { "0x3C" } else { "0x3D" };
            objects.push_str(&format!("Adafruit_SSD1306 {name}(128, 64, &Wire, -1);\n"));
            setup.push_str(&format!("  {name}.begin(SSD1306_SWITCHCAPVCC, {addr});\n  {name}.setTextColor(SSD1306_WHITE);\n"));
            show.push_str(&format!(
                "  {name}.clearDisplay();\n  {name}.setCursor(0, 0);\n  {name}.print(buf);\n  {name}.display();\n"
            ));
        } else {
            let addr = if n == 0 { "0x27" } else { "0x26" };
            let cols = if screens.kind == ScreenKind::Lcd2004 { 20 } else { 16 };
            let rows = if screens.kind == ScreenKind::Lcd2004 { 4 } else { 2 };
            objects.push_str(&format!("LiquidCrystal_I2C {name}({addr}, {cols}, {rows});\n"));
            setup.push_str(&format!("  {name}.init();\n  {name}.backlight();\n"));
            show.push_str(&format!("  {name}.clear();\n  {name}.setCursor(0, 0);\n  {name}.print(buf);\n"));
        }
    }
    let comment = if lang == Lang::Ru {
        "// Часы DS3231 с выводом даты и времени на экран по I2C."
    } else {
        "// DS3231 clock with the date and time on one or more I2C displays."
    };
    format!(
        "{comment}\n{includes}{objects}\nvoid setup() {{\n{setup}}}\n\nvoid loop() {{\n  if (millis() - last >= PERIOD_MS) {{\n    last = millis();\n    DateTime now = rtc.now();\n    char buf[16];\n    sprintf(buf, \"%02d:%02d  %02d.%02d\", now.hour(), now.minute(), now.day(), now.month());\n{show}  }}\n}}\n"
    )
}

/// Pin placement for parts that sit on the bench, not on the breadboard.
/// Offsets are millimetres from the part's top-left corner; the same numbers
/// drive the wiring and the painter.
pub fn bench_pins(kind: Kind, value: &str) -> &'static [(&'static str, f32, f32)] {
    match kind {
        // The header of a ready-made board comes from the module table, so a
        // new board only has to be described there.
        Kind::Module => match crate::modules::find(value) {
            Some(m) => m.pins,
            None => &[],
        },
        // Terminals come out of the gear end, as on a hobby gearmotor.
        Kind::Motor => &[("+", 4.0, 15.6), ("-", 9.0, 15.6)],
        Kind::Tmp36 => &[("+", 1.0, -3.0), ("O", 1.9, -3.0), ("-", 2.8, -3.0)],
        Kind::Servo => &[("-", 3.0, -3.0), ("+", 9.0, -3.0), ("S", 15.0, -3.0)],
        Kind::Buzzer | Kind::Speaker => &[("-", 5.0, -3.0), ("+", 11.0, -3.0)],
        Kind::Relay => &[
            ("IN", 5.0, -3.0),
            ("VCC", 11.0, -3.0),
            ("GND", 17.0, -3.0),
            ("COM", 34.0, 7.0),
            ("NO", 34.0, 15.0),
        ],
        Kind::Pot => &[("1", 3.0, 15.0), ("2", 6.0, 15.0), ("3", 9.0, 15.0)],
        // The same parts can also be laid out loose on the bench, where the
        // user places them by hand instead of plugging them into holes.
        Kind::Resistor | Kind::Diode => &[("1", 0.0, 0.0), ("2", 9.0, 0.0)],
        Kind::Led => {
            if value.to_lowercase().contains("rgb") {
                &[("R", 0.0, 0.0), ("G", 2.54, 0.0), ("B", 5.08, 0.0), ("-", 7.62, 0.0)]
            } else {
                &[("1", 0.0, 0.0), ("2", 5.08, 0.0)]
            }
        }
        Kind::Ldr | Kind::Thermistor => &[("1", 0.0, 0.0), ("2", 5.08, 0.0)],
        Kind::Reed => &[("1", 0.0, 0.0), ("2", 7.62, 0.0)],
        Kind::Peltier => &[("+", 0.0, 6.0), ("-", 0.0, 14.0)],
        // Both capacitors share one footprint, so picking another type does
        // not pull the part out of its holes.
        Kind::Capacitor | Kind::Electrolytic => &[("1", 0.0, 0.0), ("2", 2.54, 0.0)],
        Kind::Nmos => &[("G", 0.0, 0.0), ("S", 2.54, 0.0), ("D", 5.08, 0.0)],
        Kind::Npn => &[("B", 0.0, 0.0), ("C", 2.54, 0.0), ("E", 5.08, 0.0)],
        Kind::Button => &[
            ("1", 0.0, 0.0),
            ("2", 6.5, 0.0),
            ("3", 0.0, 6.5),
            ("4", 6.5, 6.5),
        ],
        _ => &[],
    }
}

/// What a bench part's terminal actually is. A piezo's `+` is a drive input,
/// a speaker's `+` is a supply, and a motor's `-` is the switched side: the
/// checks are only as good as these distinctions.
pub fn bench_pin_kind(kind: Kind, _value: &str, name: &str) -> PinKind {
    match (kind, name) {
        (Kind::Module, "GND") => PinKind::Ground,
        (Kind::Module, "VCC" | "+5V" | "+12V" | "3V3" | "+") => PinKind::Power,
        (Kind::Module, _) => PinKind::Signal,
        (Kind::Motor, "+") => PinKind::Power,
        (Kind::Motor, _) => PinKind::Passive,
        (Kind::Servo, "+") => PinKind::Power,
        (Kind::Servo, "-") => PinKind::Ground,
        (Kind::Buzzer, "-") => PinKind::Ground,
        (Kind::Buzzer, _) => PinKind::Signal,
        (Kind::Speaker, "+") => PinKind::Power,
        (Kind::Speaker, _) => PinKind::Passive,
        (Kind::Relay, "VCC") => PinKind::Power,
        (Kind::Relay, "GND") => PinKind::Ground,
        (Kind::Relay, "IN") => PinKind::Signal,
        (Kind::Relay, _) => PinKind::Passive,
        (Kind::Pot, "1") => PinKind::Power,
        (Kind::Pot, "3") => PinKind::Ground,
        (Kind::Pot, _) => PinKind::Signal,
        (Kind::Peltier, "+") => PinKind::Power,
        _ => PinKind::Passive,
    }
}

pub(crate) struct B {
    pub(crate) c: Circuit,
    uno: usize,
    lang: Lang,
}

impl B {
    pub(crate) fn new(lang: Lang) -> Self {
        let mut c = Circuit::new();
        let uno = c.add_arduino(uno_spot());
        Self { c, uno, lang }
    }

    pub(crate) fn uno_pin(&self, name: &str) -> End {
        End::Pin(self.c.pin(self.uno, name))
    }

    /// The x of a header socket, i.e. the column it lines up with.
    fn uno_x(&self, name: &str) -> f32 {
        let pin = self.c.pin(self.uno, name);
        self.c.pins[pin].pos.x
    }

    fn hole(&self, col: u8, row: Row) -> End {
        End::Hole(Hole::Col(col, row))
    }

    fn rail(&self, rail: Rail, x: f32) -> End {
        let b = &self.c.board;
        let mut best = (f32::MAX, RailHalf::Left, 0u8);
        for half in [RailHalf::Left, RailHalf::Right] {
            for i in 0..Board::RAIL_HOLES {
                let d = (b.rail_x(half, i) - x).abs();
                if d < best.0 {
                    best = (d, half, i);
                }
            }
        }
        End::Hole(Hole::Rail(rail, best.1, best.2))
    }

    /// The rails nearest the Uno are the bottom pair, so the 5 V and GND
    /// jumpers are short instead of crossing the whole breadboard.
    pub(crate) fn vcc(&self, x: f32) -> End {
        self.rail(Rail::BotPlus, x)
    }
    pub(crate) fn gnd(&self, x: f32) -> End {
        self.rail(Rail::BotMinus, x)
    }

    /// An endpoint named in a lesson's wiring table: an Uno pin, a rail, or
    /// the 3.3 V socket.
    fn target(&self, name: &str) -> End {
        match name {
            "GND" => self.gnd(72.0),
            "5V" | "VCC" => self.vcc(78.0),
            "3V3" => self.uno_pin("3V3"),
            other => self.uno_pin(other),
        }
    }

    pub(crate) fn w(&mut self, a: End, b: End, class: NetClass) {
        self.c.wire(a, b, class);
    }

    /// Axial part lying along a row, spanning `span` holes.
    pub(crate) fn axial(
        &mut self,
        kind: Kind,
        label: &str,
        value: &str,
        col: u8,
        row: Row,
        span: u8,
    ) -> (End, End) {
        let b = self.c.board;
        let a = b.hole(col, row);
        let z = b.hole(col + span, row);
        let p = self.c.add_part(
            kind,
            label,
            value,
            a.mid(z),
            vec![
                PinSpec { name: "1", pos: a, hole: Hole::Col(col, row), kind: PinKind::Passive },
                PinSpec { name: "2", pos: z, hole: Hole::Col(col + span, row), kind: PinKind::Passive },
            ],
            vec![],
        );
        (End::Pin(self.c.pin(p, "1")), End::Pin(self.c.pin(p, "2")))
    }

    pub(crate) fn res(&mut self, label: &str, value: &str, col: u8, row: Row, span: u8) -> (End, End) {
        self.axial(Kind::Resistor, label, value, col, row, span)
    }

    pub(crate) fn led(&mut self, label: &str, value: &str, col: u8, row: Row) -> (End, End) {
        self.axial(Kind::Led, label, value, col, row, 1)
    }

    pub(crate) fn diode(&mut self, label: &str, value: &str, x: f32, y: f32) -> (End, End) {
        let a = Pt::new(x, y);
        let k = Pt::new(x + 10.0, y);
        let p = self.c.add_part(
            Kind::Diode,
            label,
            value,
            a.mid(k),
            vec![
                PinSpec { name: "A", pos: a, hole: Hole::OffBoard, kind: PinKind::Passive },
                PinSpec { name: "K", pos: k, hole: Hole::OffBoard, kind: PinKind::Passive },
            ],
            vec![],
        );
        (End::Pin(self.c.pin(p, "A")), End::Pin(self.c.pin(p, "K")))
    }

    /// TO-92 transistor standing legs-down in three adjacent columns.
    pub(crate) fn to92(
        &mut self,
        kind: Kind,
        label: &str,
        value: &str,
        col: u8,
        row: Row,
        names: [&'static str; 3],
    ) -> Vec<End> {
        let b = self.c.board;
        let mut pins = Vec::new();
        let mut ends = Vec::new();
        for i in 0..3u8 {
            let pos = b.hole(col + i, row);
            pins.push(PinSpec {
                name: names[i as usize],
                pos,
                hole: Hole::Col(col + i, row),
                kind: if i == 0 { PinKind::Signal } else { PinKind::Passive },
            });
        }
        let at = b.hole(col, row).mid(b.hole(col + 2, row));
        let p = self.c.add_part(kind, label, value, at, pins, vec![]);
        for n in names {
            ends.push(End::Pin(self.c.pin(p, n)));
        }
        ends
    }

    /// Four-leg tactile switch straddling the centre channel; the two legs on
    /// each side of the gap are one contact, and pressing joins the sides.
    fn button(&mut self, label: &str, col: u8, row: Row) -> Vec<End> {
        let b = self.c.board;
        let tops = [b.hole(col, row), b.hole(col + 3, row)];
        let bots = [b.hole(col, row.mirror()), b.hole(col + 3, row.mirror())];
        let at = tops[0].mid(bots[1]);
        let pins = vec![
            PinSpec { name: "1", pos: tops[0], hole: Hole::Col(col, row), kind: PinKind::Passive },
            PinSpec { name: "2", pos: tops[1], hole: Hole::Col(col + 3, row), kind: PinKind::Passive },
            PinSpec { name: "3", pos: bots[0], hole: Hole::Col(col, row.mirror()), kind: PinKind::Passive },
            PinSpec { name: "4", pos: bots[1], hole: Hole::Col(col + 3, row.mirror()), kind: PinKind::Passive },
        ];
        let p = self.c.add_part(
            Kind::Button,
            label,
            "такт 6×6",
            at,
            pins,
            vec![("1", "2"), ("3", "4")],
        );
        vec![
            End::Pin(self.c.pin(p, "1")),
            End::Pin(self.c.pin(p, "2")),
            End::Pin(self.c.pin(p, "3")),
            End::Pin(self.c.pin(p, "4")),
        ]
    }

    pub(crate) fn bench(&mut self, kind: Kind, label: &str, value: &str, at: Pt) -> Vec<End> {
        let mut pins = Vec::new();
        for (name, dx, dy) in bench_pins(kind, value) {
            pins.push(PinSpec {
                name,
                pos: Pt::new(at.x + dx, at.y + dy),
                hole: Hole::OffBoard,
                kind: bench_pin_kind(kind, value, name),
            });
        }
        let p = self.c.add_part(kind, label, value, at, pins, vec![]);
        bench_pins(kind, value)
            .iter()
            .map(|(n, _, _)| End::Pin(self.c.pin(p, n)))
            .collect()
    }

    /// Drop a ready-made board on the bench and return its part index, so
    /// the wiring can name its header pins.
    pub(crate) fn breakout(&mut self, label: &str, value: &str, at: Pt) -> usize {
        let mut pins = Vec::new();
        for (name, dx, dy) in bench_pins(Kind::Module, value) {
            pins.push(PinSpec {
                name,
                pos: Pt::new(at.x + dx, at.y + dy),
                hole: Hole::OffBoard,
                kind: bench_pin_kind(Kind::Module, value, name),
            });
        }
        self.c.add_part(Kind::Module, label, value, at, pins, vec![])
    }

    /// A named header pin of a board placed with [`B::breakout`].
    pub(crate) fn mp(&self, part: usize, name: &str) -> End {
        End::Pin(self.c.pin(part, name))
    }

    pub(crate) fn rgb(&mut self, label: &str, value: &str, col: u8, row: Row) -> Vec<End> {
        let b = self.c.board;
        let names = ["R", "G", "B", "-"];
        let mut pins = Vec::new();
        for (i, n) in names.iter().enumerate() {
            pins.push(PinSpec {
                name: n,
                pos: b.hole(col + i as u8, row),
                hole: Hole::Col(col + i as u8, row),
                kind: PinKind::Passive,
            });
        }
        let at = b.hole(col, row).mid(b.hole(col + 3, row));
        let p = self.c.add_part(Kind::Led, label, value, at, pins, vec![]);
        names.iter().map(|n| End::Pin(self.c.pin(p, n))).collect()
    }

    /// Connect the two halves of a power rail when both are in use, the way a
    /// real build bridges the gap under the middle of the board.
    fn bridge_rails(&mut self) {
        for rail in [Rail::TopPlus, Rail::TopMinus, Rail::BotPlus, Rail::BotMinus] {
            let mut left = false;
            let mut right = false;
            for w in self.c.wires() {
                for e in [w.a, w.b] {
                    if let End::Hole(Hole::Rail(r, h, _)) = e {
                        if r == rail {
                            match h {
                                RailHalf::Left => left = true,
                                RailHalf::Right => right = true,
                            }
                        }
                    }
                }
            }
            for p in self.c.pins() {
                if let Hole::Rail(r, h, _) = p.hole {
                    if r == rail {
                        match h {
                            RailHalf::Left => left = true,
                            RailHalf::Right => right = true,
                        }
                    }
                }
            }
            if left && right {
                let class = if matches!(rail, Rail::TopPlus | Rail::BotPlus) {
                    NetClass::Vcc
                } else {
                    NetClass::Gnd
                };
                self.c.wire(
                    End::Hole(Hole::Rail(rail, RailHalf::Left, Board::RAIL_HOLES - 1)),
                    End::Hole(Hole::Rail(rail, RailHalf::Right, 0)),
                    class,
                );
            }
        }
    }

    pub(crate) fn finish(mut self) -> Circuit {
        self.bridge_rails();
        self.c
    }

    fn lang(&self) -> Lang {
        self.lang
    }
}

fn fill(t: &str, vars: &[(&str, &str)]) -> String {
    let mut s = t.to_string();
    for (k, v) in vars {
        s = s.replace(&format!("<<{k}>>"), v);
    }
    s
}

fn motor_mosfet(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// D2 — кнопка, D9 — ШИМ, Q1 — IRLZ44N (ключ низкой стороны)
const uint8_t BTN = 2;
const uint8_t PWM_PIN = 9;
const uint8_t STEPS = 6;          // 0..5 ступеней
const uint16_t DEBOUNCE_MS = 30;
const uint16_t RAMP_MS = 180;

uint8_t step = 0;
uint8_t duty = 0;
uint8_t target = 0;
bool pressed = false;
uint32_t lastEdge = 0;
uint32_t lastRamp = 0;

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  pinMode(PWM_PIN, OUTPUT);
  analogWrite(PWM_PIN, 0);
  Serial.begin(9600);
  Serial.println(F("steps 0..5"));
}

void loop() {
  bool now = digitalRead(BTN) == LOW;
  if (now != pressed && millis() - lastEdge > DEBOUNCE_MS) {
    lastEdge = millis();
    pressed = now;
    if (now) {
      step = (step + 1) % STEPS;
      target = (uint16_t)step * 255 / (STEPS - 1);
      Serial.print(F("step "));
      Serial.println(step);
    }
  }
  // Плавный разгон: мотор не дёргается и не тянет бросок тока.
  if (millis() - lastRamp >= RAMP_MS / 255) {
    lastRamp = millis();
    if (duty < target) { analogWrite(PWM_PIN, ++duty); }
    else if (duty > target) { analogWrite(PWM_PIN, --duty); }
  }
}
"#,
        &[("C0", tr(lang, "Ступени скорости двигателя от кнопки", "Motor speed steps from a button"))],
    )
}

fn motor_bjt(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// D2 — кнопка, D9 — ШИМ, Q1 — 2N2222, R1 1 кОм в базу
const uint8_t BTN = 2;
const uint8_t PWM_PIN = 9;
const uint8_t STEPS = 6;
const uint16_t DEBOUNCE_MS = 30;

uint8_t step = 0;
bool pressed = false;
uint32_t lastEdge = 0;

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  pinMode(PWM_PIN, OUTPUT);
  analogWrite(PWM_PIN, 0);
  Serial.begin(9600);
}

void loop() {
  bool now = digitalRead(BTN) == LOW;
  if (now != pressed && millis() - lastEdge > DEBOUNCE_MS) {
    lastEdge = millis();
    pressed = now;
    if (now) {
      step = (step + 1) % STEPS;
      // 2N2222 входит в насыщение до ~150 мА: не гони ток выше.
      uint8_t duty = (uint16_t)step * 255 / (STEPS - 1);
      analogWrite(PWM_PIN, duty);
      Serial.print(F("duty "));
      Serial.println(duty);
    }
  }
}
"#,
        &[("C0", tr(lang, "Ступени скорости на биполярном ключе", "Speed steps on a bipolar switch"))],
    )
}

fn motor_relay(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// D2 — кнопка, D7 — реле. ШИМ невозможен: скорость = скважность окна 100 мс
const uint8_t BTN = 2;
const uint8_t RELAY = 7;
const uint8_t STEPS = 5;
const uint16_t WINDOW_MS = 100;
const uint16_t DEBOUNCE_MS = 30;

uint8_t step = 0;
bool pressed = false;
uint32_t lastEdge = 0;

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  pinMode(RELAY, OUTPUT);
  digitalWrite(RELAY, LOW);
  Serial.begin(9600);
}

void loop() {
  bool now = digitalRead(BTN) == LOW;
  if (now != pressed && millis() - lastEdge > DEBOUNCE_MS) {
    lastEdge = millis();
    pressed = now;
    if (now) {
      step = (step + 1) % STEPS;
      Serial.print(F("step "));
      Serial.println(step);
    }
  }
  // Реле щёлкает на 10 Гц: мотор гудит, но обороты видны.
  uint16_t on = (uint32_t)step * WINDOW_MS / (STEPS - 1);
  digitalWrite(RELAY, (millis() % WINDOW_MS) < on ? HIGH : LOW);
}
"#,
        &[("C0", tr(lang, "Ступени скорости через реле", "Speed steps through a relay"))],
    )
}

fn led_toggle_sketch(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
const uint8_t BTN = 2;
const uint8_t LED = 9;
const uint16_t DEBOUNCE_MS = 30;

bool on = false;
bool pressed = false;
uint32_t lastEdge = 0;

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  pinMode(LED, OUTPUT);
  digitalWrite(LED, LOW);
}

void loop() {
  bool now = digitalRead(BTN) == LOW;
  if (now != pressed && millis() - lastEdge > DEBOUNCE_MS) {
    lastEdge = millis();
    pressed = now;
    if (now) {
      on = !on;
      digitalWrite(LED, on ? HIGH : LOW);
    }
  }
}
"#,
        &[("C0", tr(lang, "Светодиод включается и выключается кнопкой", "The button toggles the LED"))],
    )
}

fn led_rgb(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// D9/D10/D11 — красный/зелёный/синий, у каждого свой резистор
const uint8_t BTN = 2;
const uint8_t R = 9, G = 10, B = 11;
const uint16_t DEBOUNCE_MS = 30;

uint8_t colour = 0;
bool pressed = false;
uint32_t lastEdge = 0;

void setColour(uint8_t c) {
  // Восемь цветов перебором трёх каналов.
  digitalWrite(R, (c & 0b001) ? HIGH : LOW);
  digitalWrite(G, (c & 0b010) ? HIGH : LOW);
  digitalWrite(B, (c & 0b100) ? HIGH : LOW);
}

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  pinMode(R, OUTPUT);
  pinMode(G, OUTPUT);
  pinMode(B, OUTPUT);
  setColour(0);
}

void loop() {
  bool now = digitalRead(BTN) == LOW;
  if (now != pressed && millis() - lastEdge > DEBOUNCE_MS) {
    lastEdge = millis();
    pressed = now;
    if (now) {
      colour = (colour + 1) % 8;
      setColour(colour);
    }
  }
}
"#,
        &[("C0", tr(lang, "Цвет по кнопке: три канала RGB", "Button cycles the RGB channels"))],
    )
}

fn led_blink_sketch(lang: Lang, pin: u8, period: u16) -> String {
    fill(
        r#"// <<C0>>
const uint8_t LED = <<PIN>>;
const uint16_t HALF_MS = <<PERIOD>>;

bool on = false;
uint32_t lastToggle = 0;

void setup() {
  pinMode(LED, OUTPUT);
}

void loop() {
  if (millis() - lastToggle >= HALF_MS) {
    lastToggle = millis();
    on = !on;
    digitalWrite(LED, on ? HIGH : LOW);
  }
}
"#,
        &[
            ("C0", tr(lang, "Мигание без delay(): плата остаётся свободной", "Blinking without delay(): the board stays free")),
            ("PIN", &pin.to_string()),
            ("PERIOD", &period.to_string()),
        ],
    )
}

fn led_chase(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
const uint8_t LEDS[3] = {9, 10, 11};
const uint16_t STEP_MS = 220;

uint8_t phase = 0;
uint32_t lastStep = 0;

void setup() {
  for (uint8_t i = 0; i < 3; i++) pinMode(LEDS[i], OUTPUT);
}

void loop() {
  if (millis() - lastStep >= STEP_MS) {
    lastStep = millis();
    for (uint8_t i = 0; i < 3; i++) {
      digitalWrite(LEDS[i], i == phase ? HIGH : LOW);
    }
    phase = (phase + 1) % 3;
  }
}
"#,
        &[("C0", tr(lang, "Бегущий огонь на трёх каналах", "A chaser across three channels"))],
    )
}

fn nightlight(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// A0 — делитель с фоторезистором, D9 — ШИМ светодиода
const uint8_t SENSE = A0;
const uint8_t LED = 9;
const int THRESHOLD = 520;
const int HYSTERESIS = 40;       // без него светодиод мигает на пороге
const uint16_t SAMPLE_MS = 50;

int level = 0;
uint32_t lastSample = 0;

void setup() {
  pinMode(LED, OUTPUT);
  Serial.begin(9600);
}

void loop() {
  if (millis() - lastSample >= SAMPLE_MS) {
    lastSample = millis();
    int raw = analogRead(SENSE);
    if (raw > THRESHOLD + HYSTERESIS) level = 1;
    else if (raw < THRESHOLD - HYSTERESIS) level = 0;
    // Плавное зажигание, чтобы не резало глаз.
    static int shown = 0;
    int target = level ? 255 : 0;
    shown += (target - shown) / 8;
    analogWrite(LED, shown);
    Serial.println(raw);
  }
}
"#,
        &[("C0", tr(lang, "Ночник: порог с гистерезисом", "Night light with a hysteresis threshold"))],
    )
}

fn nightlight_relay(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// A0 — фоторезистор, D7 — реле на лампу
const uint8_t SENSE = A0;
const uint8_t RELAY = 7;
const int THRESHOLD = 550;
const int HYSTERESIS = 50;

bool lampOn = false;
uint32_t lastSample = 0;

void setup() {
  pinMode(RELAY, OUTPUT);
  digitalWrite(RELAY, LOW);
  Serial.begin(9600);
}

void loop() {
  if (millis() - lastSample < 200) return;
  lastSample = millis();
  int raw = analogRead(SENSE);
  if (!lampOn && raw > THRESHOLD + HYSTERESIS) lampOn = true;
  if (lampOn && raw < THRESHOLD - HYSTERESIS) lampOn = false;
  digitalWrite(RELAY, lampOn ? HIGH : LOW);
  Serial.print(F("raw "));
  Serial.println(raw);
}
"#,
        &[("C0", tr(lang, "Ночник с реле: порог и гистерезис", "Relay night light with hysteresis"))],
    )
}

fn servo_sweep_sketch(lang: Lang, pin: u8) -> String {
    fill(
        r#"// <<C0>>
#include <Servo.h>

Servo arm;
const uint8_t SERVO_PIN = <<PIN>>;
const uint8_t STEP_DEG = 4;
const uint16_t STEP_MS = 20;

int angle = 0;
int direction = 1;
uint32_t lastStep = 0;

void setup() {
  arm.attach(SERVO_PIN);
  arm.write(angle);
}

void loop() {
  if (millis() - lastStep < STEP_MS) return;
  lastStep = millis();
  angle += direction * STEP_DEG;
  if (angle >= 180) { angle = 180; direction = -1; }
  if (angle <= 0)   { angle = 0;   direction = 1; }
  arm.write(angle);
}
"#,
        &[
            ("C0", tr(lang, "Плавный проход сервопривода по углам", "A smooth sweep across the servo range")),
            ("PIN", &pin.to_string()),
        ],
    )
}

fn servo_pot(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
#include <Servo.h>

Servo arm;
const uint8_t SERVO_PIN = 9;
const uint8_t KNOB = A0;

void setup() {
  arm.attach(SERVO_PIN);
  Serial.begin(9600);
}

void loop() {
  // Сглаживание: АЦП шумит, без него серво дёргается.
  static int smooth = 512;
  smooth = (smooth * 3 + analogRead(KNOB)) / 4;
  int angle = map(smooth, 0, 1023, 0, 180);
  arm.write(angle);
  delay(15);
}
"#,
        &[("C0", tr(lang, "Угол серво следует за ручкой потенциометра", "The servo follows the knob"))],
    )
}

fn buzzer_tone_sketch(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
const uint8_t BUZZER = 8;

// Частоты нот, Гц
const uint16_t NOTES[] = {262, 294, 330, 349, 392, 440, 494, 523};
const uint8_t LEN = sizeof(NOTES) / sizeof(NOTES[0]);

void setup() {
  pinMode(BUZZER, OUTPUT);
}

void loop() {
  for (uint8_t i = 0; i < LEN; i++) {
    tone(BUZZER, NOTES[i], 180);
    delay(220);
  }
  noTone(BUZZER);
  delay(800);
}
"#,
        &[("C0", tr(lang, "Гамма на пьезоизлучателе", "A scale on the piezo"))],
    )
}

fn buzzer_active(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
const uint8_t BUZZER = 8;

void setup() {
  pinMode(BUZZER, OUTPUT);
  digitalWrite(BUZZER, LOW);
}

void loop() {
  // Внутри буззера свой генератор: частота не выбирается.
  digitalWrite(BUZZER, HIGH);
  delay(150);
  digitalWrite(BUZZER, LOW);
  delay(850);
}
"#,
        &[("C0", tr(lang, "Активный буззер: короткий сигнал раз в секунду", "Active buzzer: one beep per second"))],
    )
}

fn speaker_npn(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// D8 -> R 100 Ом -> база 2N2222, динамик между +5 В и коллектором
const uint8_t SPEAKER = 8;

void setup() {
  pinMode(SPEAKER, OUTPUT);
}

void loop() {
  for (uint16_t f = 200; f < 1200; f += 20) {
    tone(SPEAKER, f, 30);
    delay(35);
  }
  noTone(SPEAKER);
  delay(600);
}
"#,
        &[("C0", tr(lang, "Свип по частоте через транзисторный ключ", "A frequency sweep behind a transistor"))],
    )
}

/// Add every device the sentence named but the plan did not include. This
/// runs on both lessons and templates, so "пьезопищалка + экран" is honoured
/// wherever the wording also matches a ready-made task.
fn augment_devices(
    c: &mut Circuit,
    code: &mut String,
    named: crate::parse::NamedDevices,
    lang: Lang,
    ledger: &mut crate::resources::Resources,
) {
    let Some(uno) = c.arduino else { return };
    for n in 0..named.buzzer {
        let Some(pin) = ledger.claim_pin(&["D8", "D7", "D6", "D5", "D4", "D3", "D2", "A0"]) else { break };
        if !ledger.claim_current(15) {
            break;
        }
        let at = Pt::new(2.0 + n as f32 * 18.0, 104.0);
        let specs: Vec<PinSpec> = bench_pins(Kind::Buzzer, "пьезо")
            .iter()
            .map(|(name, dx, dy)| PinSpec {
                name,
                pos: Pt::new(at.x + dx, at.y + dy),
                hole: Hole::OffBoard,
                kind: crate::templates::bench_pin_kind(Kind::Buzzer, "пьезо", name),
            })
            .collect();
        let part = c.add_part(Kind::Buzzer, &format!("BZ{}", n + 1), "пьезо", at, specs, vec![]);
        let signal = c.pin(part, "+");
        let ground = c.pin(part, "-");
        c.wire(End::Pin(signal), End::Pin(c.pin(uno, &pin)), NetClass::Signal);
        c.wire(End::Pin(ground), End::Pin(c.pin(uno, "GND2")), NetClass::Gnd);
    }
    for n in 0..named.relay {
        let Some(pin) = ledger.claim_pin(&["D8", "D7", "D6", "D5", "D4", "D3", "D2", "A0"]) else { break };
        if !ledger.claim_current(30) {
            break;
        }
        let at = Pt::new(2.0 + n as f32 * 40.0, 112.0);
        let specs: Vec<PinSpec> = bench_pins(Kind::Relay, "SRD-05VDC")
            .iter()
            .map(|(name, dx, dy)| PinSpec {
                name,
                pos: Pt::new(at.x + dx, at.y + dy),
                hole: Hole::OffBoard,
                kind: crate::templates::bench_pin_kind(Kind::Relay, "SRD-05VDC", name),
            })
            .collect();
        let part = c.add_part(Kind::Relay, &format!("K{}", n + 1), "SRD-05VDC", at, specs, vec![]);
        c.wire(End::Pin(c.pin(part, "IN")), End::Pin(c.pin(uno, &pin)), NetClass::Signal);
        c.wire(End::Pin(c.pin(part, "VCC")), End::Pin(c.pin(uno, "5V")), NetClass::Vcc);
        c.wire(End::Pin(c.pin(part, "GND")), End::Pin(c.pin(uno, "GND2")), NetClass::Gnd);
    }
}

/// Apply the numbers the user actually said to a generated sketch.
fn apply_spoken_numbers(code: &mut String, raw: &str) {
    let sem = crate::semantics::analyse(raw);
    if let Some(presses) = sem.after_presses {
        *code = code.replace("STEPS = 6", &format!("STEPS = {}", presses.min(255)));
    }
    if let Some(ms) = sem.period_ms {
        *code = code.replace("PERIOD_MS = 1000", &format!("PERIOD_MS = {ms}"));
    }
}

// new build body, spliced into templates.rs
pub fn build(intent: &Intent, choice: usize, lang: Lang) -> Option<Built> {
    if let Some(recipe) = intent.recipe {
        return crate::compose::build(recipe, choice, lang);
    }
    if let Some(i) = intent.lesson {
        let screens = crate::parse::named_devices(&intent.raw).screens;
        let plan = display_lesson_plan(i, screens, lang);
        let choice = if plan.choices.is_empty() { 0 } else { choice.min(plan.choices.len() - 1) };
        let (mut circuit, mut code, summary, mut notes) = lesson_build(lang, i, screens, choice);
        let named = crate::parse::named_devices(&intent.raw);
        let mut ledger = crate::resources::Resources::new(&circuit);
        augment_devices(&mut circuit, &mut code, named, lang, &mut ledger);
        crate::codegen::augment_from_circuit(&circuit, &mut code, named.hourly, lang);
        apply_spoken_numbers(&mut code, &intent.raw);
        notes.push(L::new(
            "Явно названные в задаче детали добавлены к схеме урока.",
            "Parts named in the sentence were added to the lesson circuit.",
        ));
        return Some(Built {
            tid: Tid::Lesson(i),
            title: plan.title_text(lang),
            circuit,
            code,
            choice: 0,
            summary,
            notes,
        });
    }
    let tid = route(intent.load, intent.input)?;
    let plan = plan(intent, lang)?;
    let choice = choice.min(plan.choices.len() - 1);
    let mut notes = Vec::new();
    if intent.presses {
        notes.push(L::new(
            "Нажатия считаются по фронту с антидребезгом 30 мс — удержание не добавляет ступеней.",
            "Presses are counted on the falling edge with 30 ms debounce; holding adds no steps.",
        ));
    }
    let (circuit, mut code, summary, extra) = match tid {
        Tid::MotorSteps => motor_steps(lang, choice),
        Tid::MotorPot => motor_pot(lang, choice),
        Tid::MotorTest => motor_test(lang, choice),
        Tid::MotorDriver => motor_driver(lang, choice),
        Tid::Stepper => stepper(lang, choice),
        Tid::LedToggle => led_toggle(lang, choice),
        Tid::LedBlink => led_blink(lang, choice),
        Tid::NightLight => night_light(lang, choice),
        Tid::ServoSweep => servo_sweep(lang, choice),
        Tid::ServoPot => servo_sweep(lang, if choice == 1 { 1 } else { 2 }),
        Tid::BuzzerTone => buzzer_tone(lang, choice),
        Tid::Rfid => rfid(lang, choice, card_slots(&intent.raw).unwrap_or(49)),
        Tid::Radio433 => radio433(lang, choice),
        Tid::RadioNrf => radio_nrf(lang, choice),
        Tid::Bluetooth => bluetooth(lang, choice),
        Tid::Gsm => gsm(lang, choice),
        Tid::Ethernet => ethernet(lang, choice),
        Tid::Composed => return crate::compose::build(intent.recipe?, choice, lang),
        Tid::Lesson(_) => return None,
    };
    apply_spoken_numbers(&mut code, &intent.raw);
    notes.extend(extra);
    Some(Built {
        tid,
        title: plan.title_text(lang),
        circuit,
        code,
        choice,
        summary,
        notes,
    })
}

/// Power the breadboard rails from the Uno, landing the jumpers directly under
/// the 5 V and GND sockets so they stay short.
pub(crate) fn feeds(b: &mut B) {
    // Land the jumpers directly under the sockets, so the rails are fed where
    // the board is, not at the far end of the bench.
    let vcc_near = b.vcc(b.uno_x("5V"));
    let gnd_near = b.gnd(b.uno_x("GND1"));
    b.w(b.uno_pin("5V"), vcc_near, NetClass::Vcc);
    b.w(b.uno_pin("GND1"), gnd_near, NetClass::Gnd);
}

/// Motor speed in steps. The driver stage is what the choice really changes:
/// a MOSFET, a bipolar transistor, or a relay.
fn motor_steps(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let btn = b.button("B1", 16, Row::E);
    let gnd = b.gnd(46.0);
    match choice {
        2 => {
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(6.0, 76.0));
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(8.0, 54.0));
            let d = b.diode("D1", "1N4007", 3.0, 76.0);
            let vcc = b.vcc(82.0);
            b.w(b.uno_pin("D7"), rl[0], NetClass::Signal);
            b.w(rl[1], vcc, NetClass::Vcc);
            b.w(rl[2], b.gnd(72.0), NetClass::Gnd);
            b.w(m[0], b.vcc(82.0), NetClass::Vcc);
            b.w(m[1], rl[3], NetClass::Signal);
            b.w(rl[4], b.gnd(74.0), NetClass::Gnd);
            // The contacts arc without a path for the motor's coil.
            b.w(d.1, m[0], NetClass::Vcc);
            b.w(d.0, m[1], NetClass::Signal);
            b.w(b.uno_pin("D2"), btn[3], NetClass::Signal);
            b.w(btn[0], gnd, NetClass::Gnd);
            feeds(&mut b);
            (
                b.finish(),
                motor_relay(lang),
                L::new(
                    "Реле замыкает землю двигателя: пять ступеней скважностью окна 100 мс.",
                    "The relay grounds the motor: five steps by duty inside a 100 ms window.",
                ),
                vec![
                    L::new(
                        "Реле не умеет ШИМ: скорость задаётся частыми включениями, мотор гудит.",
                        "A relay cannot PWM: speed comes from rapid switching, and the motor hums.",
                    ),
                    L::new(
                        "Контакт развязывает цепь, но обмотка реле всё равно требует диод — он на модуле.",
                        "The contact isolates the load, but the relay coil still needs a diode — the module has one.",
                    ),
                ],
            )
        }
        1 => {
            let (r1a, r1b) = b.res("R1", "1 кОм", 21, Row::A, 3);
            let q = b.to92(Kind::Npn, "Q1", "2N2222", 25, Row::E, ["B", "C", "E"]);
            let (_r2a, r2b) = b.res("R2", "10 кОм", 25, Row::A, 3);
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(8.0, 54.0));
            let d = b.diode("D1", "1N4007", 3.0, 76.0);
            let vcc = b.vcc(82.0);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(r1b, q[0], NetClass::Signal);
            // Each ground lands on the rail hole under its own column.
            b.w(r2b, b.gnd(74.0), NetClass::Gnd);
            b.w(q[2], b.gnd(70.0), NetClass::Gnd);
            b.w(q[1], m[1], NetClass::Signal);
            b.w(m[0], vcc, NetClass::Vcc);
            b.w(d.1, m[0], NetClass::Vcc);
            b.w(d.0, m[1], NetClass::Signal);
            b.w(b.uno_pin("D2"), btn[3], NetClass::Signal);
            b.w(btn[0], gnd, NetClass::Gnd);
            feeds(&mut b);
            (
                b.finish(),
                motor_bjt(lang),
                L::new(
                    "Двигатель на 5 В, скорость ступенями 0…100 %, база через 1 кОм.",
                    "A 5 V motor in six steps from 0 to 100 %, base driven through 1 kΩ.",
                ),
                vec![
                    L::new(
                        "Ключ греется: при токе выше 150 мА 2N2222 уходит из насыщения.",
                        "The switch warms up: above 150 mA the 2N2222 leaves saturation.",
                    ),
                    L::new(
                        "Диод D1 обязателен — без него выброс катушки убьёт транзистор.",
                        "D1 is not optional: without it the coil's kickback kills the transistor.",
                    ),
                ],
            )
        }
        _ => {
            // R1 drops the gate drive, R2 pulls the gate down; both lie in row A
            // so neither hides under the transistor body.
            let (r1a, r1b) = b.res("R1", "220 Ω", 21, Row::A, 3);
            let q = b.to92(Kind::Nmos, "Q1", "IRLZ44N", 25, Row::E, ["G", "S", "D"]);
            let (_r2a, r2b) = b.res("R2", "10 кОм", 25, Row::A, 3);
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(8.0, 54.0));
            let d = b.diode("D1", "1N4007", 3.0, 76.0);
            let vcc = b.vcc(82.0);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(r1b, q[0], NetClass::Signal);
            b.w(r2b, b.gnd(74.0), NetClass::Gnd);
            b.w(q[1], b.gnd(70.0), NetClass::Gnd);
            b.w(q[2], m[1], NetClass::Signal);
            b.w(m[0], vcc, NetClass::Vcc);
            b.w(d.1, m[0], NetClass::Vcc);
            b.w(d.0, m[1], NetClass::Signal);
            b.w(b.uno_pin("D2"), btn[3], NetClass::Signal);
            b.w(btn[0], gnd, NetClass::Gnd);
            feeds(&mut b);
            (
                b.finish(),
                motor_mosfet(lang),
                L::new(
                    "Двигатель на 5 В, шесть ступеней скорости по нажатию, ШИМ на D9.",
                    "A 5 V motor, six speed steps per press, PWM on D9.",
                ),
                vec![
                    L::new(
                        "R2 держит затвор у земли, пока плата загружается.",
                        "R2 holds the gate at ground while the board boots.",
                    ),
                    L::new(
                        "Затвор IRLZ44N открывается от 5 В — обычный IRF540 здесь не подойдёт.",
                        "The IRLZ44N gate opens at 5 V; a plain IRF540 would not.",
                    ),
                ],
            )
        }
    }
}

fn led_toggle(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let gnd = b.gnd(52.0);
    let btn = b.button("B1", 16, Row::E);
    match choice {
        2 => {
            // Each resistor ends in its own column, and that column is where
            // the matching LED leg sits: the strip does the joining.
            let (r1a, r1b) = b.res("R1", "220 Ω", 20, Row::A, 3);
            let (r2a, r2b) = b.res("R2", "220 Ω", 21, Row::B, 3);
            let (r3a, r3b) = b.res("R3", "220 Ω", 22, Row::C, 3);
            let led = b.rgb("D1", "RGB общий катод", 26, Row::E);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(b.uno_pin("D10"), r2a, NetClass::Signal);
            b.w(b.uno_pin("D11"), r3a, NetClass::Signal);
            b.w(r1b, led[0], NetClass::Signal);
            b.w(r2b, led[1], NetClass::Signal);
            b.w(r3b, led[2], NetClass::Signal);
            b.w(b.uno_pin("D2"), btn[3], NetClass::Signal);
            b.w(btn[0], gnd, NetClass::Gnd);
            b.w(led[3], gnd, NetClass::Gnd);
            let _vcc = b.vcc(70.0);
            feeds(&mut b);
            (
                b.finish(),
                led_rgb(lang),
                L::new(
                    "Кнопка перебирает восемь цветов на трёх каналах RGB.",
                    "The button cycles eight colours across three RGB channels.",
                ),
                vec![L::new(
                    "Четвёртый вывод RGB — общий катод, он идёт на землю.",
                    "The fourth RGB leg is the common cathode and goes to ground.",
                )],
            )
        }
        c if c == 1 => {
            let (r1a, _r1b) = b.res("R1", "330 Ω", 20, Row::A, 3);
            let (_la, lk) = b.led("D1", "зелёный 5 мм", 23, Row::E);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(lk, gnd, NetClass::Gnd);
            b.w(b.uno_pin("D2"), btn[3], NetClass::Signal);
            b.w(btn[0], gnd, NetClass::Gnd);
            let _vcc = b.vcc(70.0);
            feeds(&mut b);
            (
                b.finish(),
                led_toggle_sketch(lang),
                L::new(
                    "Кнопка на D2 переключает светодиод на D9 через 330 Ω.",
                    "A button on D2 toggles the LED on D9 through 330 Ω.",
                ),
                vec![],
            )
        }
        _ => {
            let (r1a, _r1b) = b.res("R1", "220 Ω", 20, Row::A, 3);
            let (_la, lk) = b.led("D1", "красный 5 мм", 23, Row::E);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(lk, gnd, NetClass::Gnd);
            b.w(b.uno_pin("D2"), btn[3], NetClass::Signal);
            b.w(btn[0], gnd, NetClass::Gnd);
            let _vcc = b.vcc(70.0);
            feeds(&mut b);
            (
                b.finish(),
                led_toggle_sketch(lang),
                L::new(
                    "Кнопка на D2 переключает светодиод на D9 через 220 Ω.",
                    "A button on D2 toggles the LED on D9 through 220 Ω.",
                ),
                vec![],
            )
        }
    }
}

fn led_blink(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let gnd = b.gnd(52.0);
    match choice {
        2 => {
            let mut pins = Vec::new();
            for (i, (pin, col, row)) in [
                ("D9", 11u8, Row::A),
                ("D10", 16, Row::A),
                ("D11", 21, Row::A),
            ]
            .into_iter()
            .enumerate()
            {
                let (ra, _rb) = b.res(&format!("R{}", i + 1), "220 Ω", col, row, 3);
                let (_a, k) = b.led(&format!("D{}", i + 1), "красный 5 мм", col + 3, Row::E);
                b.w(b.uno_pin(pin), ra, NetClass::Signal);
                b.w(k, gnd, NetClass::Gnd);
                pins.push(pin);
            }
            let _vcc = b.vcc(70.0);
            feeds(&mut b);
            (
                b.finish(),
                led_chase(lang),
                L::new(
                    "Три светодиода на D9–D11: бегущий огонь.",
                    "Three LEDs on D9–D11 run a chaser pattern.",
                ),
                vec![L::new(
                    "Три канала — это 60 мА суммарно, пин тянет 20 мА на канал.",
                    "Three channels draw 60 mA total, 20 mA per pin.",
                )],
            )
        }
        c if c == 1 => {
            let (r1a, _r1b) = b.res("R1", "150 Ω", 20, Row::A, 3);
            let (_la, lk) = b.led("D1", "синий 5 мм", 23, Row::E);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(lk, gnd, NetClass::Gnd);
            let _vcc = b.vcc(70.0);
            feeds(&mut b);
            (
                b.finish(),
                led_blink_sketch(lang, 9, 500),
                L::new(
                    "Синий светодиод мигает раз в секунду без delay().",
                    "A blue LED blinks once a second without delay().",
                ),
                vec![],
            )
        }
        _ => {
            let (r1a, _r1b) = b.res("R1", "220 Ω", 20, Row::A, 3);
            let (_la, lk) = b.led("D1", "красный 5 мм", 23, Row::E);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(lk, gnd, NetClass::Gnd);
            let _vcc = b.vcc(70.0);
            feeds(&mut b);
            (
                b.finish(),
                led_blink_sketch(lang, 9, 500),
                L::new(
                    "Красный светодиод мигает раз в секунду.",
                    "A red LED blinks once a second.",
                ),
                vec![],
            )
        }
    }
}

/// MQ-2 and one LED: the sensor's own comparator (DO) or the sketch's
/// threshold (AO) decides when the lamp blinks.
fn night_light(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let gnd = b.gnd(52.0);
    let vcc = b.vcc(44.0);
    match choice {
        2 => {
            let (d1a, d1b) = b.axial(Kind::Ldr, "LDR1", "GL5528", 10, Row::C, 2);
            let (r1a, r1b) = b.res("R1", "10 кОм", 10, Row::E, 3);
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(6.0, 76.0));
            b.w(vcc, d1a, NetClass::Vcc);
            b.w(d1b, r1a, NetClass::Analog);
            b.w(r1b, gnd, NetClass::Gnd);
            b.w(d1b, b.uno_pin("A0"), NetClass::Analog);
            b.w(b.uno_pin("D7"), rl[0], NetClass::Signal);
            b.w(rl[1], vcc, NetClass::Vcc);
            b.w(rl[2], gnd, NetClass::Gnd);
            feeds(&mut b);
            (
                b.finish(),
                nightlight_relay(lang),
                L::new(
                    "Фоторезистор в делителе с 10 кОм на A0, реле на D7.",
                    "A photoresistor and 10 kΩ divider on A0, relay on D7.",
                ),
                vec![L::new(
                    "Реле щёлкает только при переходе порога — гистерезис 50 отсчётов.",
                    "The relay only flips at the threshold crossing, with 50 counts of hysteresis.",
                )],
            )
        }
        c if c == 1 => {
            let (d1a, d1b) = b.axial(Kind::Ldr, "LDR1", "GL5528", 10, Row::B, 2);
            let (r1a, r1b) = b.res("R1", "4.7 кОм", 10, Row::D, 3);
            let (r2a, _r2b) = b.res("R2", "220 Ω", 20, Row::A, 3);
            let (_la, lk) = b.led("D1", "красный 5 мм", 23, Row::E);
            b.w(vcc, d1a, NetClass::Vcc);
            b.w(d1b, r1a, NetClass::Analog);
            b.w(r1b, gnd, NetClass::Gnd);
            b.w(d1b, b.uno_pin("A0"), NetClass::Analog);
            b.w(b.uno_pin("D9"), r2a, NetClass::Signal);
            b.w(lk, gnd, NetClass::Gnd);
            feeds(&mut b);
            (
                b.finish(),
                nightlight(lang),
                L::new(
                    "Светодиод зажигается в сумерках: делитель LDR + 4.7 кОм на A0.",
                    "The LED lights at dusk: an LDR + 4.7 kΩ divider on A0.",
                ),
                vec![L::new(
                    "С 4.7 кОм порог сдвинут: срабатывает раньше, но у окна может дребезжать.",
                    "With 4.7 kΩ the threshold moves earlier, and it can chatter near a window.",
                )],
            )
        }
        _ => {
            let (d1a, d1b) = b.axial(Kind::Ldr, "LDR1", "GL5528", 10, Row::B, 2);
            let (r1a, r1b) = b.res("R1", "10 кОм", 10, Row::D, 3);
            let (r2a, _r2b) = b.res("R2", "220 Ω", 20, Row::A, 3);
            let (_la, lk) = b.led("D1", "красный 5 мм", 23, Row::E);
            b.w(vcc, d1a, NetClass::Vcc);
            b.w(d1b, r1a, NetClass::Analog);
            b.w(r1b, gnd, NetClass::Gnd);
            b.w(d1b, b.uno_pin("A0"), NetClass::Analog);
            b.w(b.uno_pin("D9"), r2a, NetClass::Signal);
            b.w(lk, gnd, NetClass::Gnd);
            feeds(&mut b);
            (
                b.finish(),
                nightlight(lang),
                L::new(
                    "Делитель LDR + 10 кОм на A0, светодиод на D9.",
                    "An LDR + 10 kΩ divider on A0, LED on D9.",
                ),
                vec![L::new(
                    "Порог 520 отсчётов — половина шкалы АЦП при 5 В.",
                    "The 520-count threshold is half of the ADC range at 5 V.",
                )],
            )
        }
    }
}

fn servo_sweep(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let gnd = b.gnd(56.0);
    let vcc = b.vcc(50.0);
    match choice {
        1 => {
            let s = b.bench(Kind::Servo, "S1", "MG996R", Pt::new(8.0, 58.0));
            let vin = b.vcc(66.0);
            b.w(b.uno_pin("D9"), s[2], NetClass::Signal);
            b.w(b.uno_pin("VIN"), vin, NetClass::Vcc);
            b.w(s[1], vin, NetClass::Vcc);
            b.w(s[0], gnd, NetClass::Gnd);
            b.w(b.uno_pin("GND1"), gnd, NetClass::Gnd);
            (
                b.finish(),
                servo_sweep_sketch(lang, 9),
                L::new(
                    "Серво на VIN (нужно ≥ 6 В), сигнал с D9, земля общая.",
                    "The servo runs from VIN (6 V or more), signal on D9, shared ground.",
                ),
                vec![L::new(
                    "Питать MG996R от 5 В платы нельзя: бросок тока до 2.5 А перезагрузит Uno.",
                    "Do not power an MG996R from the board's 5 V: a 2.5 A spike resets the Uno.",
                )],
            )
        }
        2 => {
            let s = b.bench(Kind::Servo, "S1", "SG90", Pt::new(8.0, 58.0));
            let p = b.bench(Kind::Pot, "RV1", "10 кОм", Pt::new(8.0, 84.0));
            b.w(b.uno_pin("D9"), s[2], NetClass::Signal);
            b.w(s[1], vcc, NetClass::Vcc);
            b.w(s[0], gnd, NetClass::Gnd);
            b.w(p[0], vcc, NetClass::Vcc);
            b.w(p[2], gnd, NetClass::Gnd);
            b.w(p[1], b.uno_pin("A0"), NetClass::Analog);
            feeds(&mut b);
            (
                b.finish(),
                servo_pot(lang),
                L::new(
                    "Ручка потенциометра задаёт угол: A0 → D9.",
                    "The knob sets the angle: A0 to D9.",
                ),
                vec![L::new(
                    "Сглаживание по четырём отсчётам убирает дрожание серво.",
                    "Smoothing over four samples removes the servo jitter.",
                )],
            )
        }
        _ => {
            let s = b.bench(Kind::Servo, "S1", "SG90", Pt::new(8.0, 58.0));
            b.w(b.uno_pin("D9"), s[2], NetClass::Signal);
            b.w(s[1], vcc, NetClass::Vcc);
            b.w(s[0], gnd, NetClass::Gnd);
            feeds(&mut b);
            (
                b.finish(),
                servo_sweep_sketch(lang, 9),
                L::new(
                    "SG90 плавно проходит 0…180° и обратно, сигнал с D9.",
                    "An SG90 sweeps 0–180° and back, signal on D9.",
                ),
                vec![],
            )
        }
    }
}

fn buzzer_tone(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let gnd = b.gnd(56.0);
    match choice {
        1 => {
            let z = b.bench(Kind::Buzzer, "BZ1", "активный 5 В", Pt::new(8.0, 58.0));
            b.w(b.uno_pin("D8"), z[1], NetClass::Signal);
            b.w(z[0], gnd, NetClass::Gnd);
            let _vcc = b.vcc(50.0);
            feeds(&mut b);
            (
                b.finish(),
                buzzer_active(lang),
                L::new(
                    "Активный буззер: включается уровнем с D8.",
                    "An active buzzer switches on with a level from D8.",
                ),
                vec![],
            )
        }
        2 => {
            let (r1a, r1b) = b.res("R1", "100 Ω", 20, Row::A, 3);
            let q = b.to92(Kind::Npn, "Q1", "2N2222", 23, Row::E, ["B", "C", "E"]);
            let sp = b.bench(Kind::Speaker, "LS1", "8 Ω", Pt::new(8.0, 58.0));
            let vcc = b.vcc(70.0);
            b.w(b.uno_pin("D8"), r1a, NetClass::Signal);
            b.w(r1b, q[0], NetClass::Signal);
            b.w(q[2], gnd, NetClass::Gnd);
            b.w(q[1], sp[0], NetClass::Signal);
            b.w(sp[1], vcc, NetClass::Vcc);
            feeds(&mut b);
            (
                b.finish(),
                speaker_npn(lang),
                L::new(
                    "Динамик 8 Ω через 2N2222: пин управляет базой, не катушкой.",
                    "An 8 Ω speaker behind a 2N2222: the pin drives the base, not the coil.",
                ),
                vec![L::new(
                    "Динамик напрямую к пину — это 0.6 А, пин умрёт.",
                    "A speaker straight to a pin would pull 0.6 A and kill it.",
                )],
            )
        }
        _ => {
            let z = b.bench(Kind::Buzzer, "BZ1", "пьезо пассивный", Pt::new(8.0, 58.0));
            b.w(b.uno_pin("D8"), z[1], NetClass::Signal);
            b.w(z[0], gnd, NetClass::Gnd);
            let _vcc = b.vcc(50.0);
            feeds(&mut b);
            (
                b.finish(),
                buzzer_tone_sketch(lang),
                L::new(
                    "Пассивный пьезо на D8: частоту задаёт tone().",
                    "A passive piezo on D8: tone() sets the pitch.",
                ),
                vec![],
            )
        }
    }
}

/// Bill of materials, grouped by component kind and value.
pub fn bom(c: &Circuit, lang: Lang) -> Vec<(String, String, usize)> {
    let mut out: Vec<(String, String, usize)> = Vec::new();
    for p in &c.parts {
        let kind = kind_name(p.kind, lang).to_string();
        if let Some(slot) = out.iter_mut().find(|(v, k, _)| *v == p.value && *k == kind) {
            slot.2 += 1;
        } else {
            out.push((p.value.clone(), kind, 1));
        }
    }
    out
}

/// One line of the component list, drawn the way Tinkercad draws it: a name
/// or a stack of names, how many of them, and one component description.
pub struct ComponentRow {
    pub names: Vec<String>,
    pub quantity: usize,
    pub component: String,
}

/// The component list for any circuit: parts grouped by value and kind, with
/// their designators stacked, plus the breadboard itself.
pub fn component_rows(c: &Circuit, lang: Lang) -> Vec<ComponentRow> {
    let mut out: Vec<ComponentRow> = Vec::new();
    for p in &c.parts {
        let kind = kind_name(p.kind, lang);
        let component = if p.value.is_empty() {
            kind.to_string()
        } else {
            format!("{kind} {}", p.value)
        };
        if let Some(slot) = out.iter_mut().find(|r| r.component == component) {
            slot.names.push(p.label.clone());
            slot.quantity += 1;
        } else {
            out.push(ComponentRow {
                names: vec![p.label.clone()],
                quantity: 1,
                component,
            });
        }
    }
    let columns = if c.board.cols >= 30 { 30 } else { 15 };
    let board = match lang {
        Lang::Ru => format!("Макетка {columns} отверстий"),
        Lang::En => format!("Breadboard ({columns} columns)"),
    };
    if !c.bare {
        out.push(ComponentRow {
            // The breadboard has no designator of its own, and Tinkercad
            // leaves the name cell empty for it.
            names: Vec::new(),
            quantity: 1,
            component: board,
        });
    }
    out
}

/// The same list as Tinkercad's "Download CSV": one header and one row per
/// component, names separated by a space.
pub fn component_csv(rows: &[ComponentRow]) -> String {
    let quote = |s: &str| {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut out = String::from("Name,Quantity,Component\n");
    for r in rows {
        out.push_str(&format!(
            "{},{},{}\n",
            quote(&r.names.join(" ")),
            r.quantity,
            quote(&r.component)
        ));
    }
    out
}

pub fn kind_name(kind: Kind, lang: Lang) -> &'static str {
    match kind {
        Kind::Arduino => tr(lang, "плата", "board"),
        Kind::Resistor => tr(lang, "резистор", "resistor"),
        Kind::Led => tr(lang, "светодиод", "LED"),
        Kind::Diode => tr(lang, "диод", "diode"),
        Kind::Nmos => tr(lang, "MOSFET", "MOSFET"),
        Kind::Npn => tr(lang, "транзистор NPN", "NPN transistor"),
        Kind::Button => tr(lang, "кнопка", "button"),
        Kind::Motor => tr(lang, "двигатель", "motor"),
        Kind::Servo => tr(lang, "сервопривод", "servo"),
        Kind::Buzzer => tr(lang, "излучатель", "buzzer"),
        Kind::Speaker => tr(lang, "динамик", "speaker"),
        Kind::Pot => tr(lang, "потенциометр", "potentiometer"),
        Kind::Ldr => tr(lang, "фоторезистор", "photoresistor"),
        Kind::Tmp36 => tr(lang, "датчик температуры", "temperature sensor"),
        Kind::Relay => tr(lang, "реле", "relay"),
        Kind::Capacitor => tr(lang, "конденсатор", "capacitor"),
        Kind::Electrolytic => tr(lang, "электролит", "electrolytic"),
        Kind::Reed => tr(lang, "геркон", "reed switch"),
        Kind::Thermistor => tr(lang, "термистор", "thermistor"),
        Kind::Peltier => tr(lang, "элемент Пельтье", "Peltier element"),
        Kind::Module => tr(lang, "модуль", "module"),
    }
}


// ---------------------------------------------------------------- lesson 1

/// Motor speed from a potentiometer: the same switch as `motor_steps` with a
/// knob instead of a button.
fn motor_pot(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let pot = b.bench(Kind::Pot, "RV1", "10 кОм", Pt::new(8.0, 84.0));
    let vcc = b.vcc(82.0);
    let gnd = b.gnd(78.0);
    b.w(pot[0], vcc, NetClass::Vcc);
    b.w(pot[2], gnd, NetClass::Gnd);
    b.w(pot[1], b.uno_pin("A0"), NetClass::Analog);
    match choice {
        1 => {
            let (r1a, r1b) = b.res("R1", "1 кОм", 21, Row::A, 3);
            let q = b.to92(Kind::Npn, "Q1", "2N2222", 25, Row::E, ["B", "C", "E"]);
            let (_r2a, r2b) = b.res("R2", "10 кОм", 25, Row::A, 3);
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(8.0, 54.0));
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(r1b, q[0], NetClass::Signal);
            b.w(r2b, gnd, NetClass::Gnd);
            b.w(q[2], gnd, NetClass::Gnd);
            b.w(q[1], m[1], NetClass::Signal);
            b.w(m[0], vcc, NetClass::Vcc);
            feeds(&mut b);
            (
                b.finish(),
                motor_pot_sketch(lang, 9),
                L::new(
                    "Ручка задаёт ШИМ на базе 2N2222, мотор до 150 мА.",
                    "The knob sets PWM at the 2N2222 base, the motor draws up to 150 mA.",
                ),
                vec![L::new(
                    "Выше 150 мА транзистор уходит из насыщения и греется — тогда нужен MOSFET.",
                    "Above 150 mA the transistor leaves saturation and warms up, which calls for a MOSFET.",
                )],
            )
        }
        2 => {
            let drv = b.breakout("U2", "L298N", Pt::new(2.0, 56.0));
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(56.0, 58.0));
            b.w(b.uno_pin("A0"), b.mp(drv, "IN2"), NetClass::Analog);
            b.w(b.uno_pin("D9"), b.mp(drv, "ENA"), NetClass::Signal);
            b.w(b.uno_pin("D7"), b.mp(drv, "IN1"), NetClass::Signal);
            b.w(b.mp(drv, "OUT1"), m[0], NetClass::Signal);
            b.w(b.mp(drv, "OUT2"), m[1], NetClass::Signal);
            b.w(b.mp(drv, "GND"), b.gnd(74.0), NetClass::Gnd);
            b.w(b.mp(drv, "+12V"), b.uno_pin("VIN"), NetClass::Vcc);
            feeds(&mut b);
            (
                b.finish(),
                motor_pot_sketch(lang, 9),
                L::new(
                    "L298N: скважность на ENA, направление на IN1/IN2, питание 12 В.",
                    "L298N: duty on ENA, direction on IN1/IN2, 12 V supply.",
                ),
                vec![L::new(
                    "Перемычку ENA с модуля снимают: иначе ШИМ с потенциометра не дойдёт.",
                    "Remove the ENA jumper, or the potentiometer's PWM never reaches the bridge.",
                )],
            )
        }
        _ => {
            let (r1a, r1b) = b.res("R1", "220 Ω", 21, Row::A, 3);
            let q = b.to92(Kind::Nmos, "Q1", "IRLZ44N", 25, Row::E, ["G", "S", "D"]);
            let (_r2a, r2b) = b.res("R2", "10 кОм", 25, Row::A, 3);
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(8.0, 54.0));
            let d = b.diode("D1", "1N4007", 3.0, 76.0);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(r1b, q[0], NetClass::Signal);
            b.w(r2b, gnd, NetClass::Gnd);
            b.w(q[1], gnd, NetClass::Gnd);
            b.w(q[2], m[1], NetClass::Signal);
            b.w(m[0], vcc, NetClass::Vcc);
            b.w(d.1, m[0], NetClass::Vcc);
            b.w(d.0, m[1], NetClass::Signal);
            feeds(&mut b);
            (
                b.finish(),
                motor_pot_sketch(lang, 9),
                L::new(
                    "Ручка задаёт скважность ШИМ на затворе IRLZ44N.",
                    "The knob sets the PWM duty on the IRLZ44N gate.",
                ),
                vec![L::new(
                    "D1 гасит выброс обмотки; без него MOSFET выйдет из строя.",
                    "D1 clamps the coil's kickback; without it the MOSFET dies.",
                )],
            )
        }
    }
}

/// Motor simply on: the test from the first lesson.
fn motor_test(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let vcc = b.vcc(82.0);
    let gnd = b.gnd(78.0);
    let feed = feeds(&mut b);
    let _ = feed;
    match choice {
        2 => {
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(6.0, 76.0));
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(8.0, 54.0));
            let d = b.diode("D1", "1N4007", 3.0, 76.0);
            b.w(b.uno_pin("D7"), rl[0], NetClass::Signal);
            b.w(rl[1], vcc, NetClass::Vcc);
            b.w(rl[2], gnd, NetClass::Gnd);
            b.w(m[0], vcc, NetClass::Vcc);
            b.w(m[1], rl[3], NetClass::Signal);
            b.w(rl[4], gnd, NetClass::Gnd);
            b.w(d.1, m[0], NetClass::Vcc);
            b.w(d.0, m[1], NetClass::Signal);
            (
                b.finish(),
                motor_test_sketch(lang, 7),
                L::new(
                    "Реле замыкает мотор на землю, D7 держит его включённым.",
                    "The relay grounds the motor, D7 keeps it on.",
                ),
                vec![L::new(
                    "Реле не умеет ШИМ: это только тест включения.",
                    "A relay cannot PWM: this is an on/off test only.",
                )],
            )
        }
        1 => {
            let (r1a, r1b) = b.res("R1", "1 кОм", 21, Row::A, 3);
            let q = b.to92(Kind::Npn, "Q1", "2N2222", 25, Row::E, ["B", "C", "E"]);
            let (_r2a, r2b) = b.res("R2", "10 кОм", 25, Row::A, 3);
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(8.0, 54.0));
            let d = b.diode("D1", "1N4007", 3.0, 76.0);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(r1b, q[0], NetClass::Signal);
            b.w(r2b, gnd, NetClass::Gnd);
            b.w(q[2], gnd, NetClass::Gnd);
            b.w(q[1], m[1], NetClass::Signal);
            b.w(m[0], vcc, NetClass::Vcc);
            b.w(d.1, m[0], NetClass::Vcc);
            b.w(d.0, m[1], NetClass::Signal);
            (
                b.finish(),
                motor_test_sketch(lang, 9),
                L::new(
                    "NPN-ключ: база через 1 кОм от D9, мотор на полную.",
                    "NPN switch: base through 1 kΩ from D9, motor flat out.",
                ),
                vec![L::new(
                    "Для теста p-n-p транзистора схему зеркалят: эмиттер к +5 В, нагрузка в коллекторе.",
                    "A p-n-p test mirrors the circuit: emitter to +5 V, load in the collector.",
                )],
            )
        }
        _ => {
            let (r1a, r1b) = b.res("R1", "220 Ω", 21, Row::A, 3);
            let q = b.to92(Kind::Nmos, "Q1", "IRLZ44N", 25, Row::E, ["G", "S", "D"]);
            let (_r2a, r2b) = b.res("R2", "10 кОм", 25, Row::A, 3);
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(8.0, 54.0));
            let d = b.diode("D1", "1N4007", 3.0, 76.0);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(r1b, q[0], NetClass::Signal);
            b.w(r2b, gnd, NetClass::Gnd);
            b.w(q[1], gnd, NetClass::Gnd);
            b.w(q[2], m[1], NetClass::Signal);
            b.w(m[0], vcc, NetClass::Vcc);
            b.w(d.1, m[0], NetClass::Vcc);
            b.w(d.0, m[1], NetClass::Signal);
            (
                b.finish(),
                motor_test_sketch(lang, 9),
                L::new(
                    "Полевой n-канальный ключ: мотор включается уровнем с D9.",
                    "An n-channel MOSFET switch: D9 turns the motor on.",
                ),
                vec![L::new(
                    "R2 прижимает затвор к земле, пока плата загружается.",
                    "R2 holds the gate down while the board boots.",
                )],
            )
        }
    }
}

// ---------------------------------------------------------------- lesson 2

/// Two DC motors behind a driver board, forward and reverse.
fn motor_driver(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let m1 = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(56.0, 58.0));
    let m2 = b.bench(Kind::Motor, "M2", "5 В DC", Pt::new(56.0, 86.0));
    let vcc = b.vcc(78.0);
    let gnd = b.gnd(74.0);
    if choice == 1 {
        let d = b.breakout("U2", "L293D", Pt::new(6.0, 58.0));
        b.w(b.uno_pin("D5"), b.mp(d, "IN1"), NetClass::Signal);
        b.w(b.uno_pin("D6"), b.mp(d, "IN2"), NetClass::Signal);
        b.w(b.uno_pin("D7"), b.mp(d, "IN3"), NetClass::Signal);
        b.w(b.uno_pin("D8"), b.mp(d, "IN4"), NetClass::Signal);
        b.w(b.uno_pin("D9"), b.mp(d, "EN1"), NetClass::Signal);
        b.w(b.uno_pin("D10"), b.mp(d, "EN2"), NetClass::Signal);
        b.w(b.mp(d, "OUT1"), m1[0], NetClass::Signal);
        b.w(b.mp(d, "OUT2"), m1[1], NetClass::Signal);
        b.w(b.mp(d, "OUT3"), m2[0], NetClass::Signal);
        b.w(b.mp(d, "OUT4"), m2[1], NetClass::Signal);
        b.w(b.mp(d, "VCC1"), vcc, NetClass::Vcc);
        b.w(b.mp(d, "VCC2"), vcc, NetClass::Vcc);
        b.w(b.mp(d, "GND"), gnd, NetClass::Gnd);
        feeds(&mut b);
        (
            b.finish(),
            motor_driver_sketch(lang, "L293D", true),
            L::new(
                "L293D: два канала по 0.6 А, ENA/ENB на D9/D10.",
                "L293D: two 0.6 A channels, ENA/ENB on D9/D10.",
            ),
            vec![L::new(
                "Микросхема греется: при 0.6 А на канал нужен радиатор или L298N.",
                "The chip warms up: at 0.6 A per channel it needs a heatsink or the L298N.",
            )],
        )
    } else {
        let d = b.breakout("U2", "L298N", Pt::new(2.0, 56.0));
        b.w(b.uno_pin("D5"), b.mp(d, "IN1"), NetClass::Signal);
        b.w(b.uno_pin("D6"), b.mp(d, "IN2"), NetClass::Signal);
        b.w(b.uno_pin("D7"), b.mp(d, "IN3"), NetClass::Signal);
        b.w(b.uno_pin("D8"), b.mp(d, "IN4"), NetClass::Signal);
        b.w(b.uno_pin("D9"), b.mp(d, "ENA"), NetClass::Signal);
        b.w(b.uno_pin("D10"), b.mp(d, "ENB"), NetClass::Signal);
        b.w(b.mp(d, "OUT1"), m1[0], NetClass::Signal);
        b.w(b.mp(d, "OUT2"), m1[1], NetClass::Signal);
        b.w(b.mp(d, "OUT3"), m2[0], NetClass::Signal);
        b.w(b.mp(d, "OUT4"), m2[1], NetClass::Signal);
        b.w(b.mp(d, "+12V"), b.uno_pin("VIN"), NetClass::Vcc);
        b.w(b.mp(d, "GND"), gnd, NetClass::Gnd);
        feeds(&mut b);
        (
            b.finish(),
            motor_driver_sketch(lang, "L298N", true),
            L::new(
                "L298N: два мотора до 2 А, перемычки ENA/ENB сняты для ШИМ.",
                "L298N: two motors up to 2 A, ENA/ENB jumpers removed for PWM.",
            ),
            vec![
                L::new(
                    "Питание 12 В подаётся на +12V и GND, земля общая с платой.",
                    "The 12 V supply goes to +12V and GND, with a shared ground.",
                ),
                L::new(
                    "Выход +5V модуля можно не подключать: стабилизатор уже питает логику.",
                    "The module's +5V output can stay free: its regulator already feeds the logic.",
                ),
            ],
        )
    }
}

// ---------------------------------------------------------------- lesson 3

/// 28BYJ-48 with the ULN2003 board, with or without controls.
fn stepper(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let drv = b.breakout("U2", "ULN2003", Pt::new(2.0, 56.0));
    let m = b.breakout("M1", "28BYJ-48", Pt::new(52.0, 56.0));
    let vcc = b.vcc(78.0);
    let gnd = b.gnd(74.0);
    b.w(b.uno_pin("D8"), b.mp(drv, "IN1"), NetClass::Signal);
    b.w(b.uno_pin("D9"), b.mp(drv, "IN2"), NetClass::Signal);
    b.w(b.uno_pin("D10"), b.mp(drv, "IN3"), NetClass::Signal);
    b.w(b.uno_pin("D11"), b.mp(drv, "IN4"), NetClass::Signal);
    for (a, c) in [("OUT1", "1"), ("OUT2", "2"), ("OUT3", "3"), ("OUT4", "4")] {
        b.w(b.mp(drv, a), b.mp(m, c), NetClass::Signal);
    }
    b.w(b.mp(drv, "+5V"), vcc, NetClass::Vcc);
    b.w(b.mp(drv, "COM"), vcc, NetClass::Vcc);
    b.w(b.mp(drv, "GND"), gnd, NetClass::Gnd);
    b.w(b.mp(m, "+"), vcc, NetClass::Vcc);
    feeds(&mut b);
    if choice == 1 {
        let btn = b.button("B1", 16, Row::E);
        let pot = b.bench(Kind::Pot, "RV1", "10 кОм", Pt::new(2.0, 100.0));
        b.w(b.uno_pin("D2"), btn[3], NetClass::Signal);
        b.w(btn[0], gnd, NetClass::Gnd);
        b.w(pot[0], vcc, NetClass::Vcc);
        b.w(pot[2], b.gnd(78.0), NetClass::Gnd);
        b.w(pot[1], b.uno_pin("A0"), NetClass::Analog);
        (
            b.finish(),
            stepper_sketch(lang, true),
            L::new(
                "Кнопка на D2 меняет направление, ручка на A0 — скорость шагов.",
                "The button on D2 reverses, the knob on A0 sets the step rate.",
            ),
            vec![L::new(
                "Мотор и ULN2003 питаются от 5 В платы; при просадке лучше внешние 5 В.",
                "The motor and ULN2003 run from the board's 5 V; an external 5 V helps if it sags.",
            )],
        )
    } else {
        (
            b.finish(),
            stepper_sketch(lang, false),
            L::new(
                "Тест: полный оборот вперёд, пауза, полный оборот назад.",
                "A test: one full turn forward, a pause, then one back.",
            ),
            vec![L::new(
                "28BYJ-48 делает 2048 шагов на оборот в полушаговом режиме.",
                "The 28BYJ-48 takes 2048 half-steps per revolution.",
            )],
        )
    }
}

// ---------------------------------------------------------------- lessons 5-10

fn rfid(lang: Lang, choice: usize, slots: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let rc = b.breakout("U2", "RC522", Pt::new(2.0, 54.0));
    let gnd = b.gnd(74.0);
    b.w(b.uno_pin("D10"), b.mp(rc, "SDA"), NetClass::Signal);
    b.w(b.uno_pin("D13"), b.mp(rc, "SCK"), NetClass::Signal);
    b.w(b.uno_pin("D11"), b.mp(rc, "MOSI"), NetClass::Signal);
    b.w(b.uno_pin("D12"), b.mp(rc, "MISO"), NetClass::Signal);
    b.w(b.uno_pin("D9"), b.mp(rc, "RST"), NetClass::Signal);
    b.w(b.uno_pin("3V3"), b.mp(rc, "3V3"), NetClass::Vcc);
    b.w(b.mp(rc, "GND"), gnd, NetClass::Gnd);
    feeds(&mut b);
    match choice {
        1 => {
            let lcd = b.breakout("DS1", "LCD1602", Pt::new(2.0, 122.0));
            b.w(b.uno_pin("5V"), b.mp(lcd, "VCC"), NetClass::Vcc);
            b.w(b.mp(lcd, "GND"), b.gnd(78.0), NetClass::Gnd);
            b.w(b.uno_pin("A4"), b.mp(lcd, "SDA"), NetClass::Signal);
            b.w(b.uno_pin("A5"), b.mp(lcd, "SCL"), NetClass::Signal);
            (
                b.finish(),
                rfid_sketch(lang, 1),
                L::new("UID карты печатается в порт и выводится на LCD1602.", "The card UID goes to the port and to the LCD1602."),
                vec![L::new(
                    "Экран и считыватель сидят на разных шинах: I2C на A4/A5, SPI на D9–D13.",
                    "The screen and reader sit on different buses: I2C on A4/A5, SPI on D9–D13.",
                )],
            )
        }
        2 => {
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(4.0, 122.0));
            b.w(b.uno_pin("D8"), rl[0], NetClass::Signal);
            b.w(rl[1], b.vcc(78.0), NetClass::Vcc);
            b.w(rl[2], gnd, NetClass::Gnd);
            (
                b.finish(),
                rfid_sketch(lang, 2),
                L::new("Своя карта замыкает реле-замок на 5 секунд.", "A known card closes the lock relay for five seconds."),
                vec![L::new(
                    "Контакты COM/NO идут в цепь замка; питание замка — отдельное.",
                    "COM/NO switch the lock circuit; the lock has its own supply.",
                )],
            )
        }
        3 => {
            let oled = b.breakout("DS1", "OLED 0.96", Pt::new(2.0, 122.0));
            b.w(b.uno_pin("5V"), b.mp(oled, "VCC"), NetClass::Vcc);
            b.w(b.mp(oled, "GND"), b.gnd(78.0), NetClass::Gnd);
            b.w(b.uno_pin("A4"), b.mp(oled, "SDA"), NetClass::Signal);
            b.w(b.uno_pin("A5"), b.mp(oled, "SCL"), NetClass::Signal);
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(44.0, 122.0));
            b.w(b.uno_pin("D8"), rl[0], NetClass::Signal);
            b.w(rl[1], b.vcc(82.0), NetClass::Vcc);
            b.w(rl[2], gnd, NetClass::Gnd);
            let btn = b.button("B1", 16, Row::E);
            b.w(b.uno_pin("D2"), btn[3], NetClass::Signal);
            b.w(btn[0], gnd, NetClass::Gnd);
            (
                b.finish(),
                access_sketch(lang, slots),
                L::new(
                    "Таблица карт живёт в EEPROM, кнопка пишет карту, замок открывается знакомой картой.",
                    "The card table lives in EEPROM, a button saves a card, and a known card opens the lock.",
                ),
                vec![
                    L::new(
                        "EEPROM Uno — 1024 байта: заголовок 2 байта и по 4 байта на UID карты.",
                        "An Uno's EEPROM is 1024 bytes: a 2-byte header and 4 bytes per card UID.",
                    ),
                    L::new(
                        "Пишутся только карты на 4 байта (MIFARE Classic); метки на 7 байт не сохраняются.",
                        "Only 4-byte UIDs (MIFARE Classic) are saved; 7-byte tags are refused.",
                    ),
                    L::new(
                        "Модуль реле активен низким уровнем: экран и замок питаются от 5 В платы.",
                        "The relay module is active low; the screen and the lock run from the board's 5 V.",
                    ),
                ],
            )
        }
        _ => (
            b.finish(),
            rfid_sketch(lang, 0),
            L::new("RC522 по SPI, UID метки выводится в терминал.", "The RC522 on SPI prints the tag UID to the terminal."),
            vec![L::new(
                "RC522 питается только от 3.3 В: от 5 В модуль выходит из строя.",
                "The RC522 runs on 3.3 V only: 5 V kills it.",
            )],
        ),
    }
}

fn radio433(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let gnd = b.gnd(74.0);
    let vcc = b.vcc(78.0);
    feeds(&mut b);
    match choice {
        1 => {
            let rx = b.breakout("U2", "XY-MK-5V", Pt::new(2.0, 56.0));
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(44.0, 58.0));
            b.w(b.uno_pin("D2"), b.mp(rx, "DATA"), NetClass::Signal);
            b.w(b.mp(rx, "VCC"), vcc, NetClass::Vcc);
            b.w(b.mp(rx, "GND"), gnd, NetClass::Gnd);
            b.w(b.uno_pin("D8"), rl[0], NetClass::Signal);
            b.w(rl[1], b.vcc(82.0), NetClass::Vcc);
            b.w(rl[2], gnd, NetClass::Gnd);
            (
                b.finish(),
                radio433_sketch(lang, 1),
                L::new("Приёмник ловит код и щёлкает реле — радиоудлинитель.", "The receiver catches a code and clicks a relay — a radio extender."),
                vec![L::new(
                    "Антенна 17 см на DATA-модуле обязательна, иначе дальность — метры.",
                    "A 17 cm antenna on the module is mandatory, or range drops to metres.",
                )],
            )
        }
        2 => {
            let tx = b.breakout("U2", "FS1000A", Pt::new(2.0, 56.0));
            let (ra, rb) = b.res("R1", "220 Ω", 12, Row::A, 3);
            let (la, lb) = b.led("D1", "красный 5 мм", 17, Row::A);
            b.w(b.uno_pin("D12"), b.mp(tx, "DATA"), NetClass::Signal);
            b.w(b.mp(tx, "VCC"), vcc, NetClass::Vcc);
            b.w(b.mp(tx, "GND"), gnd, NetClass::Gnd);
            b.w(b.uno_pin("D13"), ra, NetClass::Signal);
            b.w(rb, la, NetClass::Signal);
            b.w(lb, b.gnd(82.0), NetClass::Gnd);
            (
                b.finish(),
                radio433_sketch(lang, 2),
                L::new("Передатчик шлёт '1' и '0', светодиод показывает отправку.", "The transmitter sends '1' and '0', an LED shows the sending."),
                vec![L::new(
                    "Один передатчик может вещать нескольким приёмникам: кода хватает.",
                    "One transmitter can feed several receivers: the code is enough.",
                )],
            )
        }
        _ => {
            let tx = b.breakout("U2", "FS1000A", Pt::new(2.0, 56.0));
            let rx = b.breakout("U3", "XY-MK-5V", Pt::new(44.0, 60.0));
            b.w(b.uno_pin("D12"), b.mp(tx, "DATA"), NetClass::Signal);
            b.w(b.mp(tx, "VCC"), vcc, NetClass::Vcc);
            b.w(b.mp(tx, "GND"), gnd, NetClass::Gnd);
            b.w(b.uno_pin("D2"), b.mp(rx, "DATA"), NetClass::Signal);
            b.w(b.mp(rx, "VCC"), vcc, NetClass::Vcc);
            b.w(b.mp(rx, "GND"), gnd, NetClass::Gnd);
            (
                b.finish(),
                radio433_sketch(lang, 0),
                L::new("Передатчик на D12, приёмник на D2: код идёт в одну сторону.", "Transmitter on D12, receiver on D2: the code goes one way."),
                vec![L::new(
                    "Модули дешёвые и без проверки: дальность сильно зависит от антенны и питания.",
                    "These modules are cheap and unchecked: range depends on the antenna and supply.",
                )],
            )
        }
    }
}

fn radio_nrf(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let nrf = b.breakout("U2", "NRF24L01", Pt::new(2.0, 60.0));
    b.w(b.uno_pin("D9"), b.mp(nrf, "CE"), NetClass::Signal);
    b.w(b.uno_pin("D10"), b.mp(nrf, "CSN"), NetClass::Signal);
    b.w(b.uno_pin("D13"), b.mp(nrf, "SCK"), NetClass::Signal);
    b.w(b.uno_pin("D11"), b.mp(nrf, "MOSI"), NetClass::Signal);
    b.w(b.uno_pin("D12"), b.mp(nrf, "MISO"), NetClass::Signal);
    b.w(b.uno_pin("3V3"), b.mp(nrf, "VCC"), NetClass::Vcc);
    b.w(b.mp(nrf, "GND"), b.gnd(74.0), NetClass::Gnd);
    feeds(&mut b);
    if choice == 1 {
        let lcd = b.breakout("DS1", "LCD1602", Pt::new(2.0, 88.0));
        b.w(b.uno_pin("5V"), b.mp(lcd, "VCC"), NetClass::Vcc);
        b.w(b.mp(lcd, "GND"), b.gnd(78.0), NetClass::Gnd);
        b.w(b.uno_pin("A4"), b.mp(lcd, "SDA"), NetClass::Signal);
        b.w(b.uno_pin("A5"), b.mp(lcd, "SCL"), NetClass::Signal);
        (
            b.finish(),
            radio_nrf_sketch(lang, 1),
            L::new("Принятые числа показываются на LCD1602 по I2C.", "Received numbers are shown on an I2C LCD1602."),
            vec![L::new(
                "NRF24L01 требует 3.3 В и 10 мкФ прямо на разъёме: иначе связь рвётся.",
                "The NRF24L01 wants 3.3 V and a 10 µF cap right at its header, or the link drops.",
            )],
        )
    } else {
        (
            b.finish(),
            radio_nrf_sketch(lang, 0),
            L::new("Приёмник печатает массив из трёх чисел в терминал.", "The receiver prints an array of three numbers to the terminal."),
            vec![L::new(
                "Без усилителя мощности дальность в комнате — 10–20 метров.",
                "Without a power amplifier the indoor range is 10–20 metres.",
            )],
        )
    }
}

fn bluetooth(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let name = if choice == 1 { "HC-06" } else { "HC-05" };
    let bt = b.breakout("U2", name, Pt::new(2.0, 58.0));
    let gnd = b.gnd(74.0);
    b.w(b.uno_pin("5V"), b.mp(bt, "VCC"), NetClass::Vcc);
    b.w(b.mp(bt, "GND"), gnd, NetClass::Gnd);
    b.w(b.mp(bt, "TXD"), b.uno_pin("D2"), NetClass::Signal);
    let (r1a, r1b) = b.res("R1", "1 кОм", 8, Row::A, 3);
    let (_r2a, r2b) = b.res("R2", "1 кОм", 13, Row::A, 3);
    b.w(b.uno_pin("D3"), r1a, NetClass::Signal);
    b.w(r1b, b.mp(bt, "RXD"), NetClass::Signal);
    b.w(r2b, b.gnd(78.0), NetClass::Gnd);
    feeds(&mut b);
    if choice == 2 {
        let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(48.0, 58.0));
        b.w(b.uno_pin("D4"), rl[0], NetClass::Signal);
        b.w(rl[1], b.vcc(82.0), NetClass::Vcc);
        b.w(rl[2], gnd, NetClass::Gnd);
        return (
            b.finish(),
            bluetooth_sketch(lang, 2),
            L::new("Команда с телефона включает реле на D4.", "A phone command switches the relay on D4."),
            vec![L::new(
                "Делитель 1 кОм + 1 кОм даёт 2.5 В на RXD — безопасно для 3.3 В входа.",
                "A 1 kΩ + 1 kΩ divider gives 2.5 V at RXD, safe for the 3.3 V input.",
            )],
        );
    }
    (
        b.finish(),
        bluetooth_sketch(lang, 0),
        L::new("HC-05 на D2/D3: команды из терминала телефона.", "The HC-05 on D2/D3: commands from the phone's terminal."),
        vec![L::new(
            "RXD модуля — вход на 3.3 В, поэтому линия D3 идёт через делитель.",
            "The module's RXD is a 3.3 V input, so D3 reaches it through a divider.",
        )],
    )
}

fn gsm(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let m = b.breakout("U2", "NEOWAY M590", Pt::new(2.0, 56.0));
    let gnd = b.gnd(74.0);
    b.w(b.mp(m, "VCC"), b.uno_pin("VIN"), NetClass::Vcc);
    b.w(b.mp(m, "TXD"), b.uno_pin("D2"), NetClass::Signal);
    b.w(b.uno_pin("D3"), b.mp(m, "RXD"), NetClass::Signal);
    b.w(b.uno_pin("D4"), b.mp(m, "PWR"), NetClass::Signal);
    b.w(b.mp(m, "GND"), gnd, NetClass::Gnd);
    feeds(&mut b);
    match choice {
        1 => {
            let btn = b.button("B1", 16, Row::E);
            b.w(b.uno_pin("D5"), btn[3], NetClass::Signal);
            b.w(btn[0], b.gnd(78.0), NetClass::Gnd);
            (
                b.finish(),
                gsm_sketch(lang, 1),
                L::new("Кнопка на D5 запускает звонок на заданный номер.", "A button on D5 places a call to a fixed number."),
                vec![L::new(
                    "M590 в момент звонка берёт до 2 А: только отдельный блок питания.",
                    "The M590 draws up to 2 A while calling: use its own supply.",
                )],
            )
        }
        2 => {
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(58.0, 58.0));
            b.w(b.uno_pin("D5"), rl[0], NetClass::Signal);
            b.w(rl[1], b.vcc(78.0), NetClass::Vcc);
            b.w(rl[2], gnd, NetClass::Gnd);
            (
                b.finish(),
                gsm_sketch(lang, 2),
                L::new("SMS с кодовым словом переключает реле, модем отвечает подтверждением.", "An SMS with the code word toggles the relay and the modem texts back."),
                vec![L::new(
                    "SIM-карта должна быть без PIN-кода, иначе модем не выйдет в сеть.",
                    "The SIM card must have no PIN, or the modem never joins the network.",
                )],
            )
        }
        _ => (
            b.finish(),
            gsm_sketch(lang, 0),
            L::new("Проверка AT: модем отвечает OK, версия и уровень сети — в терминал.", "An AT check: the modem answers OK, with version and signal in the terminal."),
            vec![L::new(
                "VCC модема намеренно не подключён к 5 В платы: нужен внешний блок.",
                "The modem's VCC is deliberately not on the board's 5 V: it needs its own supply.",
            )],
        ),
    }
}

fn ethernet(lang: Lang, choice: usize) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let gnd = b.gnd(74.0);
    feeds(&mut b);
    if choice == 1 {
        let eth = b.breakout("U2", "ENC28J60", Pt::new(2.0, 56.0));
        b.w(b.uno_pin("D10"), b.mp(eth, "CS"), NetClass::Signal);
        b.w(b.uno_pin("D13"), b.mp(eth, "SCK"), NetClass::Signal);
        b.w(b.uno_pin("D11"), b.mp(eth, "MOSI"), NetClass::Signal);
        b.w(b.uno_pin("D12"), b.mp(eth, "MISO"), NetClass::Signal);
        b.w(b.uno_pin("D2"), b.mp(eth, "INT"), NetClass::Signal);
        b.w(b.uno_pin("5V"), b.mp(eth, "VCC"), NetClass::Vcc);
        b.w(b.mp(eth, "GND"), gnd, NetClass::Gnd);
        return (
            b.finish(),
            ethernet_sketch(lang, 1),
            L::new("ENC28J60 на SPI, страница с часами и адресом.", "The ENC28J60 on SPI serves a page with a clock and the address."),
            vec![L::new(
                "ENC28J60 — только 3.3 В на самом чипе; плата с перемычкой принимает 5 В.",
                "The ENC28J60 chip is 3.3 V only; the board takes 5 V through its jumper.",
            )],
        );
    }
    let eth = b.breakout("U2", "W5100", Pt::new(2.0, 56.0));
    b.w(b.uno_pin("D10"), b.mp(eth, "CS"), NetClass::Signal);
    b.w(b.uno_pin("D13"), b.mp(eth, "SCK"), NetClass::Signal);
    b.w(b.uno_pin("D11"), b.mp(eth, "MOSI"), NetClass::Signal);
    b.w(b.uno_pin("D12"), b.mp(eth, "MISO"), NetClass::Signal);
    b.w(b.uno_pin("5V"), b.mp(eth, "VCC"), NetClass::Vcc);
    b.w(b.mp(eth, "GND"), gnd, NetClass::Gnd);
    if choice == 2 {
        let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(58.0, 58.0));
        b.w(b.uno_pin("D7"), rl[0], NetClass::Signal);
        b.w(rl[1], b.vcc(78.0), NetClass::Vcc);
        b.w(rl[2], gnd, NetClass::Gnd);
        return (
            b.finish(),
            ethernet_sketch(lang, 2),
            L::new("Кнопка на веб-странице включает реле в локальной сети.", "A button on the web page switches a relay on the local network."),
            vec![L::new(
                "W5100 занимает D10–D13 и SPI целиком: эти пины больше ничем не занять.",
                "The W5100 owns D10–D13 and all of SPI: nothing else can use those pins.",
            )],
        );
    }
    (
        b.finish(),
        ethernet_sketch(lang, 0),
        L::new("W5100 поднимает веб-сервер на 10.0.0.42, страница отдаёт статус.", "The W5100 brings up a web server on 10.0.0.42; the page returns status."),
        vec![L::new(
            "Адрес 10.0.0.42 должен быть в подсети роутера и свободен.",
            "10.0.0.42 must sit in the router's subnet and be free.",
        )],
    )
}


fn motor_pot_sketch(lang: Lang, pin: u8) -> String {
    fill(
        r#"// <<C0>>
const uint8_t MOTOR = <<PIN>>;
const uint8_t KNOB = A0;

void setup() {
  pinMode(MOTOR, OUTPUT);
  Serial.begin(9600);
}

void loop() {
  int raw = analogRead(KNOB);
  int duty = map(raw, 0, 1023, 0, 255);
  analogWrite(MOTOR, duty);
  Serial.println(duty);
  delay(20);
}
"#,
        &[
            ("C0", tr(lang, "Скорость мотора следует за ручкой потенциометра", "Motor speed follows the knob")),
            ("PIN", &pin.to_string()),
        ],
    )
}

fn motor_test_sketch(lang: Lang, pin: u8) -> String {
    fill(
        r#"// <<C0>>
const uint8_t MOTOR = <<PIN>>;

void setup() {
  pinMode(MOTOR, OUTPUT);
  digitalWrite(MOTOR, HIGH);   // тест: мотор крутится сразу после сброса
}

void loop() {
  // Ничего: ключ открыт постоянно.
}
"#,
        &[
            ("C0", tr(lang, "Тест ключа: мотор включён постоянно", "Switch test: the motor runs continuously")),
            ("PIN", &pin.to_string()),
        ],
    )
}

fn motor_driver_sketch(lang: Lang, name: &str, two: bool) -> String {
    let _ = two;
    fill(
        r#"// <<C0>>
// <<NAME>>: ENA/ENB — скорость, IN1…IN4 — направление
const uint8_t ENA = 9, ENB = 10;
const uint8_t IN1 = 5, IN2 = 6, IN3 = 7, IN4 = 8;
const uint16_t RUN_MS = 2000;

void forward() {
  digitalWrite(IN1, HIGH); digitalWrite(IN2, LOW);
  digitalWrite(IN3, HIGH); digitalWrite(IN4, LOW);
}

void backward() {
  digitalWrite(IN1, LOW); digitalWrite(IN2, HIGH);
  digitalWrite(IN3, LOW); digitalWrite(IN4, HIGH);
}

void stop() {
  digitalWrite(IN1, LOW); digitalWrite(IN2, LOW);
  digitalWrite(IN3, LOW); digitalWrite(IN4, LOW);
}

void setup() {
  for (uint8_t p : {ENA, ENB, IN1, IN2, IN3, IN4}) pinMode(p, OUTPUT);
  // ШИМ на оба канала: перемычки ENA/ENB на модуле сняты.
  analogWrite(ENA, 200);
  analogWrite(ENB, 200);
  stop();
}

void loop() {
  forward();
  delay(RUN_MS);
  stop();
  delay(500);
  backward();
  delay(RUN_MS);
  stop();
  delay(500);
}
"#,
        &[
            ("C0", tr(lang, "Два мотора: вперёд, стоп, назад", "Two motors: forward, stop, back")),
            ("NAME", name),
        ],
    )
}

fn stepper_sketch(lang: Lang, controls: bool) -> String {
    if controls {
        fill(
            r#"// <<C0>>
// 28BYJ-48 + ULN2003: D8…D11 — обмотки, D2 — кнопка, A0 — скорость
const uint8_t PINS[4] = {8, 9, 10, 11};
const uint8_t BTN = 2;
const uint8_t KNOB = A0;

// Полушаг: восемь состояний на четыре обмотки.
const uint8_t LOOKUP[8] = {B01000, B01100, B00100, B00110,
                           B00010, B00011, B00001, B01001};
bool direction = true;
bool pressed = false;
uint32_t lastEdge = 0;

void setOutput(uint8_t step) {
  for (uint8_t i = 0; i < 4; i++) {
    digitalWrite(PINS[i], bitRead(LOOKUP[step], 3 - i));
  }
}

void setup() {
  for (uint8_t p : PINS) pinMode(p, OUTPUT);
  pinMode(BTN, INPUT_PULLUP);
}

void loop() {
  bool now = digitalRead(BTN) == LOW;
  if (now && !pressed && millis() - lastEdge > 30) {
    direction = !direction;
    lastEdge = millis();
  }
  pressed = now;

  // Ручка задаёт паузу между шагами: 1200…200 мкс.
  int pause = map(analogRead(KNOB), 0, 1023, 1200, 200);
  for (uint8_t i = 0; i < 8; i++) {
    uint8_t step = direction ? i : 7 - i;
    setOutput(step);
    delayMicroseconds(pause);
  }
}
"#,
            &[("C0", tr(lang, "Направление кнопкой, скорость ручкой", "Direction by button, speed by knob"))],
        )
    } else {
        fill(
            r#"// <<C0>>
#include <Stepper.h>

const uint16_t STEPS_PER_TURN = 2048;   // 28BYJ-48, полушаг
Stepper motor(STEPS_PER_TURN, 8, 10, 9, 11);

void setup() {
  motor.setSpeed(12);                  // оборотов в минуту
}

void loop() {
  motor.step(STEPS_PER_TURN);          // полный оборот вперёд
  delay(1000);
  motor.step(-STEPS_PER_TURN);         // полный оборот назад
  delay(1000);
}
"#,
            &[("C0", tr(lang, "Тест шаговика через библиотеку Stepper", "Stepper test through the Stepper library"))],
        )
    }
}

fn rfid_sketch(lang: Lang, choice: usize) -> String {
    let mut vars: Vec<(&str, &str)> = vec![(
        "C0",
        tr(lang, "RC522: чтение UID карты по SPI", "RC522: reading a card UID over SPI"),
    )];
    let extra = match choice {
        1 => "#include <Wire.h>\n#include <LiquidCrystal_I2C.h>\nLiquidCrystal_I2C lcd(0x27, 16, 2);\n",
        2 => "const uint8_t LOCK = 8;   // реле замка\n",
        _ => "",
    };
    vars.push(("EXTRA", extra));
    let body = match choice {
        1 => "  lcd.setCursor(0, 0);\n  lcd.print(\"UID:\");\n  lcd.setCursor(0, 1);\n  for (byte i = 0; i < mfrc522.uid.size; i++) lcd.print(mfrc522.uid.uidByte[i], HEX);\n",
        2 => "  // Своя карта открывает замок на пять секунд.\n  digitalWrite(LOCK, HIGH);\n  delay(5000);\n  digitalWrite(LOCK, LOW);\n",
        _ => "  for (byte i = 0; i < mfrc522.uid.size; i++) {\n    Serial.print(mfrc522.uid.uidByte[i] < 0x10 ? \" 0\" : \" \");\n    Serial.print(mfrc522.uid.uidByte[i], HEX);\n  }\n  Serial.println();\n",
    };
    vars.push(("BODY", body));
    fill(
        r#"// <<C0>>
#include <SPI.h>
#include <MFRC522.h>
<<EXTRA>>
const uint8_t SS_PIN = 10;
const uint8_t RST_PIN = 9;
MFRC522 mfrc522(SS_PIN, RST_PIN);

void setup() {
  Serial.begin(9600);
  SPI.begin();
  mfrc522.PCD_Init();
  pinMode(8, OUTPUT);
}

void loop() {
  if (!mfrc522.PICC_IsNewCardPresent()) return;
  if (!mfrc522.PICC_ReadCardSerial()) return;
<<BODY>>
  mfrc522.PICC_HaltA();
  delay(300);
}
"#,
        &vars,
    )
}

fn access_sketch(lang: Lang, slots: usize) -> String {
    fill(
        r#"// <<C0>>
// D2 — кнопка записи, D8 — реле замка, SPI — RC522, I2C — OLED 128x64
#include <SPI.h>
#include <MFRC522.h>
#include <Wire.h>
#include <Adafruit_GFX.h>
#include <Adafruit_SSD1306.h>
#include <EEPROM.h>

const uint8_t SS_PIN = 10;
const uint8_t RST_PIN = 9;
const uint8_t BTN = 2;
const uint8_t LOCK = 8;
const uint8_t MAX_CARDS = <<SLOTS>>;
const uint8_t MAGIC = 0xA5;
const uint8_t COUNT = 1;              // число карт в таблице
const uint8_t TABLE = 2;              // начало таблицы UID
const uint8_t UID_LEN = 4;
const uint16_t OPEN_MS = 5000;
const uint16_t GAP_MS = 300;

MFRC522 mfrc522(SS_PIN, RST_PIN);
Adafruit_SSD1306 oled(128, 64, &Wire, -1);
uint32_t openedAt = 0;
uint32_t seenAt = 0;

uint8_t stored() {
  if (EEPROM.read(0) != MAGIC) return 0;
  uint8_t n = EEPROM.read(COUNT);
  return n > MAX_CARDS ? MAX_CARDS : n;
}

bool sameUid(uint8_t slot, const byte *uid) {
  for (uint8_t k = 0; k < UID_LEN; k++)
    if (EEPROM.read(TABLE + slot * UID_LEN + k) != uid[k]) return false;
  return true;
}

int8_t findCard(const byte *uid) {
  for (uint8_t i = 0; i < stored(); i++)
    if (sameUid(i, uid)) return (int8_t)i;
  return -1;
}

void saveCard(uint8_t slot, const byte *uid) {
  for (uint8_t k = 0; k < UID_LEN; k++)
    EEPROM.update(TABLE + slot * UID_LEN + k, uid[k]);
  if (slot >= stored()) EEPROM.update(COUNT, slot + 1);
}

void show(const char *line, const byte *uid) {
  oled.clearDisplay();
  oled.setCursor(0, 0);
  oled.println(line);
  oled.print(F("cards "));
  oled.print(stored());
  oled.print('/');
  oled.println(MAX_CARDS);
  if (uid) {
    for (uint8_t i = 0; i < UID_LEN; i++) {
      if (uid[i] < 0x10) oled.print('0');
      oled.print(uid[i], HEX);
      oled.print(' ');
    }
    oled.println();
  }
  oled.display();
}

void setup() {
  pinMode(LOCK, OUTPUT);
  digitalWrite(LOCK, HIGH);           // модуль реле активен низким
  pinMode(BTN, INPUT_PULLUP);
  Serial.begin(9600);
  SPI.begin();
  mfrc522.PCD_Init();
  Wire.begin();
  oled.begin(SSD1306_SWITCHCAPVCC, 0x3C);
  oled.setTextSize(1);
  oled.setTextColor(SSD1306_WHITE);
  if (EEPROM.read(0) != MAGIC) {
    EEPROM.update(0, MAGIC);
    EEPROM.update(COUNT, 0);
  }
  show("READY", nullptr);
}

void loop() {
  if (digitalRead(LOCK) == LOW && millis() - openedAt >= OPEN_MS) {
    digitalWrite(LOCK, HIGH);
  }
  if (millis() - seenAt < GAP_MS) return;
  if (!mfrc522.PICC_IsNewCardPresent()) return;
  if (!mfrc522.PICC_ReadCardSerial()) return;
  seenAt = millis();

  bool save = digitalRead(BTN) == LOW;
  int8_t slot = findCard(mfrc522.uid.uidByte);
  if (mfrc522.uid.size != UID_LEN) {
    show("UID IS NOT 4 BYTES", mfrc522.uid.uidByte);
  } else if (save && slot >= 0) {
    show("ALREADY SAVED", mfrc522.uid.uidByte);
  } else if (save && stored() >= MAX_CARDS) {
    show("TABLE IS FULL", mfrc522.uid.uidByte);
  } else if (save) {
    saveCard(stored(), mfrc522.uid.uidByte);
    show("CARD SAVED", mfrc522.uid.uidByte);
  } else if (slot >= 0) {
    digitalWrite(LOCK, LOW);
    openedAt = millis();
    show("ACCESS OPEN", mfrc522.uid.uidByte);
  } else {
    show("ACCESS DENIED", mfrc522.uid.uidByte);
  }
  mfrc522.PICC_HaltA();
  mfrc522.PCD_StopCrypto1();
}
"#,
        &[
            ("C0", tr(lang, "Контроль доступа: таблица карт в EEPROM", "Access control: the card table in EEPROM")),
            ("SLOTS", &slots.to_string()),
        ],
    )
}

fn radio433_sketch(lang: Lang, choice: usize) -> String {
    match choice {
        1 => fill(
            r#"// <<C0>>
#include <RCSwitch.h>
RCSwitch radio = RCSwitch();
const uint8_t RELAY = 8;

void setup() {
  Serial.begin(9600);
  radio.enableReceive(0);          // приёмник на D2 (прерывание 0)
  pinMode(RELAY, OUTPUT);
}

void loop() {
  if (!radio.available()) return;
  unsigned long code = radio.getReceivedValue();
  Serial.print(F("code "));
  Serial.println(code);
  if (code == 12345) {
    digitalWrite(RELAY, !digitalRead(RELAY));
  }
  radio.resetAvailable();
}
"#,
            &[("C0", tr(lang, "Приёмник переключает реле по коду", "The receiver toggles a relay by code"))],
        ),
        2 => fill(
            r#"// <<C0>>
#include <RCSwitch.h>
RCSwitch radio = RCSwitch();
const uint8_t LED = 13;

void setup() {
  radio.enableTransmit(12);        // передатчик на D12
  pinMode(LED, OUTPUT);
}

void loop() {
  digitalWrite(LED, HIGH);
  radio.send(1, 24);
  delay(2000);
  digitalWrite(LED, LOW);
  radio.send(0, 24);
  delay(2000);
}
"#,
            &[("C0", tr(lang, "Передатчик шлёт 1 и 0, светодиод показывает отправку", "The transmitter sends 1 and 0, an LED shows it"))],
        ),
        _ => fill(
            r#"// <<C0>>
#include <RCSwitch.h>
RCSwitch radio = RCSwitch();

void setup() {
  Serial.begin(9600);
  radio.enableTransmit(12);
  radio.enableReceive(0);          // D2
}

void loop() {
  // Одна и та же плата и шлёт, и слушает: удобно для проверки пары.
  radio.send(1, 24);
  delay(2000);
  radio.send(0, 24);
  delay(2000);

  if (radio.available()) {
    Serial.print(F("received "));
    Serial.println(radio.getReceivedValue());
    radio.resetAvailable();
  }
}
"#,
            &[("C0", tr(lang, "Передатчик на D12 и приёмник на D2", "Transmitter on D12, receiver on D2"))],
        ),
    }
}

fn radio_nrf_sketch(lang: Lang, choice: usize) -> String {
    let extra = if choice == 1 {
        "#include <Wire.h>\n#include <LiquidCrystal_I2C.h>\nLiquidCrystal_I2C lcd(0x27, 16, 2);\n"
    } else {
        ""
    };
    let show = if choice == 1 {
        "  lcd.setCursor(0, 0);\n  lcd.print(\"a=\"); lcd.print(values[0]);\n  lcd.setCursor(0, 1);\n  lcd.print(\"b=\"); lcd.print(values[1]);\n  lcd.print(\" c=\"); lcd.print(values[2]);\n"
    } else {
        "  Serial.print(values[0]); Serial.print(\", \");\n  Serial.print(values[1]); Serial.print(\", \");\n  Serial.println(values[2]);\n"
    };
    fill(
        r#"// <<C0>>
#include <SPI.h>
#include <nRF24L01.h>
#include <RF24.h>
<<EXTRA>>
RF24 radio(9, 10);                 // CE, CSN
const byte ADDRESS[6] = "00001";
int values[3];

void setup() {
  Serial.begin(9600);
<<LCD>>
  radio.begin();
  radio.openReadingPipe(0, ADDRESS);
  radio.setPALevel(RF24_PA_MIN);
  radio.startListening();
}

void loop() {
  if (!radio.available()) return;
  radio.read(&values, sizeof(values));
<<SHOW>>
}
"#,
        &[
            ("C0", tr(lang, "NRF24L01: приём массива из трёх чисел", "NRF24L01: receiving an array of three numbers")),
            ("EXTRA", extra),
            ("LCD", if choice == 1 { "  lcd.init();\n  lcd.backlight();\n" } else { "" }),
            ("SHOW", show),
        ],
    )
}

fn bluetooth_sketch(lang: Lang, choice: usize) -> String {
    let extra = if choice == 2 {
        "const uint8_t RELAY = 4;\n"
    } else {
        "const uint8_t LED = 13;\n"
    };
    let action = if choice == 2 {
        "      digitalWrite(RELAY, c == '1' ? HIGH : LOW);\n"
    } else {
        "      digitalWrite(LED, c == '1' ? HIGH : LOW);\n"
    };
    fill(
        r#"// <<C0>>
#include <SoftwareSerial.h>
SoftwareSerial bt(2, 3);          // TX модуля на D2, RX через делитель с D3
<<EXTRA>>
void setup() {
  Serial.begin(9600);
  bt.begin(9600);
  pinMode(13, OUTPUT);
  pinMode(4, OUTPUT);
}

void loop() {
  while (bt.available()) {
    char c = bt.read();
    Serial.write(c);
    if (c == '1' || c == '0') {
<<ACTION>>      bt.print(F("ok: "));
      bt.println(c);
    }
  }
}
"#,
        &[
            ("C0", tr(lang, "HC-05: команды '1' и '0' из терминала телефона", "HC-05: '1' and '0' from the phone's terminal")),
            ("EXTRA", extra),
            ("ACTION", action),
        ],
    )
}

fn gsm_sketch(lang: Lang, choice: usize) -> String {
    let (extra, loop_body, title) = match choice {
        1 => (
            "const uint8_t BTN = 5;\nconst char NUMBER[] = \"+79000000000\";\n",
            r#"  if (digitalRead(BTN) == LOW) {
    delay(30);
    if (digitalRead(BTN) == LOW) {
      modem.print(F("ATD"));
      modem.print(NUMBER);
      modem.println(F(";"));
      while (digitalRead(BTN) == LOW) delay(10);
    }
  }
"#,
            tr(lang, "Кнопка звонит на заданный номер", "A button calls a fixed number"),
        ),
        2 => (
            "const uint8_t RELAY = 5;\nbool armed = false;\n",
            r#"  if (modem.available()) {
    String line = modem.readStringUntil('\n');
    Serial.println(line);
    if (line.indexOf("+CMTI:") >= 0) {
      // Новое SMS: читаем и ищем кодовое слово OPEN.
      modem.println(F("AT+CMGL=\"REC UNREAD\""));
    }
    if (line.indexOf("OPEN") >= 0) {
      armed = !armed;
      digitalWrite(RELAY, armed ? HIGH : LOW);
      modem.println(F("AT+CMGS=\"+79000000000\""));
      modem.print(armed ? F("ON") : F("OFF"));
      modem.write(26);
    }
  }
"#,
            tr(lang, "SMS с кодом OPEN переключает реле", "An SMS with the code OPEN toggles the relay"),
        ),
        _ => (
            "",
            r#"  if (modem.available()) Serial.write(modem.read());
  if (Serial.available()) modem.write(Serial.read());
"#,
            tr(lang, "Мост между терминалом и модемом: проверка AT вручную", "A bridge between terminal and modem: AT by hand"),
        ),
    };
    fill(
        r#"// <<C0>>
#include <SoftwareSerial.h>
SoftwareSerial modem(2, 3);        // TX модема на D2, RX через делитель с D3
<<EXTRA>>
void setup() {
  Serial.begin(9600);
  modem.begin(9600);
  pinMode(4, OUTPUT);
  pinMode(5, INPUT_PULLUP);
  // Импульс на PWR включает модем.
  digitalWrite(4, HIGH);
  delay(1200);
  digitalWrite(4, LOW);
  delay(3000);
  modem.println(F("AT"));
}

void loop() {
<<LOOP>>}
"#,
        &[
            ("C0", title),
            ("EXTRA", extra),
            ("LOOP", loop_body),
        ],
    )
}

fn ethernet_sketch(lang: Lang, choice: usize) -> String {
    if choice == 1 {
        return fill(
            r#"// <<C0>>
#include <EtherCard.h>

static byte mymac[] = {0x74, 0x69, 0x69, 0x2D, 0x30, 0x31};
byte Ethernet::buffer[700];
uint32_t timer;

void setup() {
  Serial.begin(9600);
  if (ether.begin(sizeof Ethernet::buffer, mymac, 10) == 0) {
    Serial.println(F("ENC28J60 not found"));
  }
  ether.staticSetup("10.0.0.43");
}

void loop() {
  ether.packetLoop(ether.packetReceive());
  if (millis() - timer > 1000) {
    timer = millis();
    Serial.println(millis() / 1000);
  }
}
"#,
            &[("C0", tr(lang, "ENC28J60: страница с секундами с запуска", "ENC28J60: a page with uptime in seconds"))],
        );
    }
    let relay = if choice == 2 {
        "  if (request.indexOf(\"GET /on\") >= 0) { digitalWrite(7, HIGH); }\n  if (request.indexOf(\"GET /off\") >= 0) { digitalWrite(7, LOW); }\n"
    } else {
        ""
    };
    fill(
        r#"// <<C0>>
#include <SPI.h>
#include <Ethernet.h>

byte mac[] = {0xDE, 0xAD, 0xBE, 0xEF, 0xFE, 0xED};
IPAddress ip(10, 0, 0, 42);
EthernetServer server(80);

void setup() {
  Serial.begin(9600);
  Ethernet.begin(mac, ip);
  server.begin();
  pinMode(7, OUTPUT);
  Serial.print(F("server: "));
  Serial.println(Ethernet.localIP());
}

void loop() {
  EthernetClient client = server.available();
  if (!client) return;
  String request = client.readStringUntil('\r');
<<RELAY>>
  client.println(F("HTTP/1.1 200 OK"));
  client.println(F("Content-Type: text/html"));
  client.println();
  client.print(F("<h1>volta</h1><p>uptime "));
  client.print(millis() / 1000);
  client.println(F(" s</p>"));
  client.stop();
}
"#,
        &[
            ("C0", tr(lang, "W5100: веб-страница со статусом", "W5100: a web page with status")),
            ("RELAY", relay),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{parse, Input, Load};

    #[test]
    fn a_clock_naming_a_display_gets_one_and_two_gets_two() {
        use crate::text::Lang;
        let task = "Составить программу электронных часов с выводом даты и времени с выводом на любой тип дисплея.";
        let parsed = crate::parse::parse(task);
        let plan = plan(&parsed.intent, Lang::Ru).expect("plans");
        assert_eq!(plan.choices.len(), 3, "any display offers a choice");
        let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
        let screens = built.circuit.parts().filter(|p| p.value == "OLED 0.96").count();
        assert_eq!(screens, 1);
        assert!(built.code.contains("Adafruit_SSD1306"), "{}", built.code);
        assert!(built.code.contains("rtc.now()"));

        let two = "Составить программу электронных часов с выводом даты и времени на два экрана.";
        let parsed = crate::parse::parse(two);
        let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
        let screens = built.circuit.parts().filter(|p| p.value == "OLED 0.96").count();
        assert_eq!(screens, 2, "two screens means two boards");
        assert!(built.code.contains("screen0") && built.code.contains("screen1"));
    }

    /// A ladder from nonsense to precise multi-device tasks: every rung must
    /// answer without panicking, and the precise ones must add what they name.
    #[test]
    fn spoken_numbers_reach_the_sketch() {
        use crate::text::Lang;
        let presses = crate::parse::parse("увеличить скорость после 3 нажатий кнопкой на MOSFET");
        let built = build(&presses.intent, 0, Lang::Ru).expect("builds");
        assert!(built.code.contains("STEPS = 3"), "{}", built.code);

        let period = crate::parse::parse(
            "часы DS3231 с выводом времени на экран раз в 5 секунд",
        );
        let built = build(&period.intent, 0, Lang::Ru).expect("builds");
        assert!(built.code.contains("PERIOD_MS = 5000"), "{}", built.code);
    }

    #[test]
    fn auto_assembly_from_dumb_to_precise() {
        use crate::text::Lang;
        let cases: &[(&str, bool, fn(&Built) -> bool)] = &[
            ("", false, |_| true),
            ("как дела", false, |_| true),
            ("сделай что-нибудь", false, |_| true),
            ("светодиод", true, |b| b.code.contains("void loop()")),
            ("зажечь светодиод", true, |b| b.code.contains("void loop()")),
            ("часы реального времени", true, |b| !b.code.is_empty()),
            (
                "Составить программу электронных часов с выводом даты и времени с выводом на любой тип дисплея.",
                true,
                |b| b.circuit.parts().any(|p| p.value == "OLED 0.96"),
            ),
            (
                "Составить программу электронных часов с выводом даты и времени на два экрана.",
                true,
                |b| b.circuit.parts().filter(|p| p.value == "OLED 0.96").count() == 2,
            ),
            (
                "Составить программу электронных часов с пьезопищалкой для выдачи звука каждый час с выводом на любой тип дисплея.",
                true,
                |b| b.circuit.parts().any(|p| p.value == "пьезо")
                    && b.circuit.parts().any(|p| p.value == "OLED 0.96")
                    && b.code.contains("tone("),
            ),
            (
                "Составить программу для МК контроля доступа на 49 карт с их записью в энергозависимую память EEPROM и выводом информации записи и доступа на дисплей 128x64",
                true,
                |b| b.code.contains("MAX_CARDS = 49"),
            ),
            ("температура выше 30 включи вентилятор", true, |b| !b.code.is_empty()),
            ("газ выше 400 включи сирену", true, |b| !b.code.is_empty()),
            ("включи музыку", false, |_| true),
        ];
        for (task, should_build, check) in cases {
            let parsed = crate::parse::parse(task);
            let built = plan(&parsed.intent, Lang::Ru).and_then(|p| {
                build(&parsed.intent, p.recommended, Lang::Ru)
            });
            match (should_build, built) {
                (false, _) => {}
                (true, Some(b)) => assert!(check(&b), "unexpected build for {task:?}"),
                (true, None) => panic!("{task:?} should build"),
            }
        }
    }

    #[test]
    fn every_choice_builds_and_is_connected() {
        let cases = [
            (Load::Motor, Some(Input::Button)),
            (Load::Led, Some(Input::Button)),
            (Load::Led, None),
            (Load::Led, Some(Input::Ldr)),
            (Load::Servo, None),
            (Load::Buzzer, None),
        ];
        for (load, input) in cases {
            let p = parse("x");
            let intent = Intent {
                load,
                input,
                transistor: false,
                presses: false,
                lesson: None,
                action: None,
                raw: p.intent.raw,
                condition: None,
                recipe: None,
            };
            let plan = plan(&intent, Lang::Ru).expect("routable");
            assert_eq!(plan.choices.len(), 3);
            let mut signatures = Vec::new();
            for choice in 0..3 {
                let built = build(&intent, choice, Lang::Ru).expect("builds");
                assert!(!built.code.contains("<<"), "unfilled token in code");
                assert!(built.code.contains("void setup()"));
                assert!(built.code.contains("void loop()"));
                assert!(built.circuit.wires.len() >= 3);
                assert_eq!(built.choice, choice);
                assert!(built.circuit.arduino.is_some());
                assert!(!built.summary.ru.is_empty() && !built.summary.en.is_empty());
                signatures.push((
                    built.circuit.parts.len(),
                    built.circuit.wires.len(),
                    built.circuit.parts().map(|p| p.value.clone()).collect::<Vec<_>>(),
                ));
            }
            assert!(
                signatures.windows(2).any(|w| w[0] != w[1]),
                "the three choices are the same circuit"
            );
        }
    }

    /// "Контроль доступа на 49 карт ... EEPROM ... дисплей 128x64" is a lock
    /// with a card table, not the EEPROM counter lesson: the reader is named,
    /// the table is remembered, and the screen shows both saving and access.
    #[test]
    fn an_access_task_builds_the_card_table() {
        let text = "Составить программу для МК контроля доступа на 49 карт с их записью в энергозависимую память EEPROM и выводом информации записи и доступа на дисплей 128x64";
        let p = parse(text);
        assert_eq!(p.intent.load, Load::Rfid, "reader must be recognised");
        assert!(p.intent.lesson.is_none(), "the EEPROM lesson must not win");
        let plan = plan(&p.intent, Lang::Ru).expect("plans");
        assert_eq!(plan.recommended, 3, "the access variant is the default");
        let built = build(&p.intent, plan.recommended, Lang::Ru).expect("builds");
        assert!(built.code.contains("#include <EEPROM.h>"), "no EEPROM");
        assert!(built.code.contains("MAX_CARDS = 49"), "wrong table size");
        assert!(built.code.contains("Adafruit_SSD1306"), "no OLED");
        assert!(built.code.contains("MFRC522"), "no reader");
        let values: Vec<&str> = built.circuit.parts().map(|p| p.value.as_str()).collect();
        assert!(values.iter().any(|v| v.contains("OLED")), "{values:?}");
        assert!(values.iter().any(|v| *v == "RC522"), "{values:?}");
        assert!(!built.code.contains("delay("), "loop must not block");
    }

    /// Every lesson from the course book has to plan and build without a
    /// human picking pins, and each offered choice must differ.
    #[test]
    fn the_book_lessons_all_build() {
        let cases = [
            ("тест двигателя постоянного тока", Load::Motor),
            ("регулировка скорости двигателя потенциометром", Load::Motor),
            ("драйвер L298N для двух двигателей", Load::Motors),
            ("шаговый двигатель 28BYJ-48 с ULN2003", Load::Stepper),
            ("шаговый двигатель, потенциометр и кнопка", Load::Stepper),
            ("сервопривод с потенциометром", Load::Servo),
            ("считыватель RFID RC522, UID в порт", Load::Rfid),
            ("передатчик и приёмник 433 МГц", Load::Radio433),
            ("NRF24L01 2.4 ГГц, передача структуры", Load::RadioNrf),
            ("блютуз модуль HC-05 с телефона", Load::Bluetooth),
            ("GSM NEOWAY M590, отправка SMS", Load::Gsm),
            ("Ethernet W5100, веб-страница", Load::Ethernet),
            ("Ethernet ENC28J60, страница с часами", Load::Ethernet),
        ];
        for (text, load) in cases {
            let p = parse(text);
            assert_eq!(p.intent.load, load, "{text}");
            assert!(p.unsupported.is_none(), "{text}: {:?}", p.unsupported);
            let plan = plan(&p.intent, Lang::Ru).expect(text);
            assert!(!plan.choices.is_empty(), "{text}");
            let mut signatures = Vec::new();
            for choice in 0..plan.choices.len() {
                let built = build(&p.intent, choice, Lang::Ru).expect(text);
                assert!(!built.code.contains("<<"), "{text}: unfilled token");
                assert!(built.code.contains("void setup()"), "{text}");
                assert!(built.circuit.wires.len() >= 3, "{text}: too few wires");
                assert!(built.circuit.arduino.is_some(), "{text}");
                signatures.push((
                    built.circuit.parts.len(),
                    built.circuit.wires.len(),
                    built.circuit.parts().map(|q| q.value.clone()).collect::<Vec<_>>(),
                ));
            }
            if signatures.len() > 1 {
                assert!(
                    signatures.windows(2).any(|w| w[0] != w[1]),
                    "{text}: every choice is the same circuit"
                );
            }
        }
    }

    /// A left shift is not a template token: only an upper-case name between
    /// angle brackets counts as an unfilled one.
    fn unfilled(code: &str) -> bool {
        let mut rest = code;
        while let Some(i) = rest.find("<<") {
            let tail = &rest[i + 2..];
            let Some(j) = tail.find(">>") else { return false };
            let name = &tail[..j];
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            {
                return true;
            }
            rest = &tail[j + 2..];
        }
        false
    }

    /// The 51 lessons of parts one to three: each must build on both
    /// languages, carry no unfilled template token, and pass every fail-level
    /// check. Warnings are allowed — the books themselves warn about hungry
    /// panels and long warm-ups.
    #[test]
    fn every_lesson_of_books_one_to_three_passes_its_checks() {
        for (i, l) in crate::lessons::ALL.iter().enumerate() {
            let intent = Intent {
                load: Load::Led,
                input: None,
                transistor: false,
                presses: false,
                lesson: Some(i),
                action: None,
                raw: String::new(),
                condition: None,
                recipe: None,
            };
            let plan = plan(&intent, Lang::Ru).expect("the lesson plans");
            assert_eq!(plan.choices.len(), 1, "{}", l.title.ru);
            for lang in [Lang::Ru, Lang::En] {
                let built = build(&intent, 0, lang).unwrap_or_else(|| panic!("{}", l.title.ru));
                assert_eq!(built.tid, Tid::Lesson(i));
                assert!(
                    !unfilled(&built.code),
                    "{}: unfilled token in the sketch",
                    l.title.ru
                );
                assert!(built.code.contains("void setup()"), "{}", l.title.ru);
                assert!(built.code.contains("void loop()"), "{}", l.title.ru);
                let report = crate::checks::run(&built, lang);
                let fails: Vec<String> = report
                    .checks
                    .iter()
                    .filter(|c| c.sev == crate::checks::Sev::Fail)
                    .map(|c| format!("{}: {}", c.name.get(lang), c.detail))
                    .collect();
                assert!(
                    fails.is_empty(),
                    "{} ({}) — {}",
                    l.title.ru,
                    lang.code(),
                    fails.join("; ")
                );
            }
        }
    }

    /// No lesson may draw a board on top of the Uno or on top of another
    /// board: that overlap hides pins and makes the picture lie about what
    /// gets wired.
    #[test]
    fn nothing_overlaps_in_a_lesson() {
        use crate::model::Kind;
        for (i, l) in crate::lessons::ALL.iter().enumerate() {
            let intent = Intent {
                load: Load::Led,
                input: None,
                transistor: false,
                presses: false,
                lesson: Some(i),
                action: None,
                raw: String::new(),
                condition: None,
                recipe: None,
            };
            let built = build(&intent, 0, Lang::Ru).unwrap_or_else(|| panic!("{}", l.title.ru));
            let c = &built.circuit;
            let rects: Vec<(String, crate::model::Rect)> = c
                .parts
                .iter()
                .filter(|p| {
                    matches!(
                        p.kind,
                        Kind::Module
                            | Kind::Arduino
                            | Kind::Motor
                            | Kind::Relay
                            | Kind::Peltier
                            | Kind::Servo
                            | Kind::Speaker
                            | Kind::Buzzer
                    )
                })
                .map(|p| (format!("{} {}", p.label, p.value), c.part_rect(p)))
                .collect();
            for (a, (na, ra)) in rects.iter().enumerate() {
                for (nb, rb) in &rects[a + 1..] {
                    assert!(
                        !ra.inflate(-0.5).intersects(rb.inflate(-0.5)),
                        "{}: {na} overlaps {nb}",
                        l.title.ru
                    );
                }
            }
        }
    }

    /// A phrase built from a lesson's own keywords has to route to that
    /// lesson and build, which is what a user typing into the box gets.
    #[test]
    fn a_lesson_phrase_routes_and_builds() {
        for (i, l) in crate::lessons::ALL.iter().enumerate() {
            let text = format!("{} к ардуино", l.keys[0]);
            let p = parse(&text);
            assert_eq!(p.intent.lesson, Some(i), "{text} → {:?}", p.intent.lesson);
            assert!(p.unsupported.is_none(), "{text}");
            assert!(build(&p.intent, 0, Lang::Ru).is_some(), "{text}");
        }
    }

    /// The sentence from the brief, all the way through: keywords route it,
    /// the plan offers three parts, each builds, and the checks pass.
    #[test]
    fn the_brief_sentence_builds_every_choice() {
        let text = "Составить программу увеличения скорости двигателя с помощью \
                    неоднократного нажатия кнопки на полевом n-канальном транзисторе.";
        let parsed = parse(text);
        assert_eq!(parsed.intent.load, Load::Motor);
        let plan = plan(&parsed.intent, Lang::Ru).expect("the brief is routable");
        assert_eq!(plan.choices.len(), 3);
        assert!(
            plan.choices[plan.recommended].name.ru.contains("MOSFET"),
            "the recommended part should be the logic-level MOSFET"
        );
        for choice in 0..3 {
            let built = build(&parsed.intent, choice, Lang::Ru).expect("builds");
            let report = crate::checks::run(&built, Lang::Ru);
            assert_eq!(
                report.failed(),
                0,
                "choice {choice}: {}",
                report
                    .checks
                    .iter()
                    .filter(|c| c.sev == crate::checks::Sev::Fail)
                    .map(|c| format!("{}: {}", c.name.ru, c.detail))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
            // Every choice keeps the button on D2 and drives the motor from
            // some output pin. The relay cannot do PWM: it switches a window
            // instead, so only the two analogue switches may use `analogWrite`.
            assert!(built.code.contains("= 2;"), "choice {choice}: button left D2");
            assert!(
                built.code.contains("analogWrite") || built.code.contains("digitalWrite"),
                "choice {choice}: the motor is never driven"
            );
            if choice < 2 {
                assert!(
                    built.code.contains("analogWrite"),
                    "choice {choice}: a PWM switch cannot change speed"
                );
            }
            assert!(
                built.circuit.parts().any(|p| p.kind == Kind::Motor),
                "choice {choice} lost the motor"
            );
        }
    }

    /// The sentence from the brief is a composition, not a lesson: the MQ-2
    /// plus a lamp that blinks at the gas threshold. Both ways of setting that
    /// threshold build, and both pass every check.
    #[test]
    fn a_gas_alarm_composes_sensor_and_lamp() {
        for text in [
            "датчик газа который мигает когда много газа",
            "газовый датчик мигает лампой",
            "mq-2 включает светодиод при газе",
        ] {
            let p = parse(text);
            assert_eq!(p.intent.lesson, Some(crate::lessons::GAS), "{text}");
            assert!(
                matches!(
                    p.intent.action,
                    Some(Action::Blink | Action::Alarm | Action::Toggle)
                ),
                "{text}: action {:?}",
                p.intent.action
            );
            assert!(p.unsupported.is_none(), "{text}");
            let plan = plan(&p.intent, Lang::Ru).expect("the alarm plans");
            assert_eq!(plan.tid, Tid::Composed, "{text}");
            assert_eq!(plan.choices.len(), 2, "{text}");
            for choice in 0..plan.choices.len() {
                for lang in [Lang::Ru, Lang::En] {
                    let built = build(&p.intent, choice, lang)
                        .unwrap_or_else(|| panic!("{text}: choice {choice} builds"));
                    assert_eq!(built.tid, Tid::Composed);
                    assert!(!unfilled(&built.code), "{text}: unfilled token");
                    let has_mq2 = built
                        .circuit
                        .parts
                        .iter()
                        .any(|q| q.value == "MQ-2");
                    assert!(has_mq2, "{text}: no MQ-2 in the circuit");
                    let has_led = built
                        .circuit
                        .parts
                        .iter()
                        .any(|q| q.kind == Kind::Led);
                    assert!(has_led, "{text}: no LED in the circuit");
                    let report = crate::checks::run(&built, lang);
                    assert_eq!(
                        report.failed(),
                        0,
                        "{text}: choice {choice} ({}): {:?}",
                        lang.code(),
                        report
                            .checks
                            .iter()
                            .filter(|c| c.sev == crate::checks::Sev::Fail)
                            .map(|c| c.name.get(lang))
                            .collect::<Vec<_>>()
                    );
                }
            }
            // The two options really differ: one reads the analogue output,
            // the other the module comparator.
            let analog = build(&p.intent, 0, Lang::Ru).unwrap();
            let digital = build(&p.intent, 1, Lang::Ru).unwrap();
            assert!(analog.code.contains("analogRead"));
            assert!(digital.code.contains("digitalRead"));
            assert!(!analog.code.contains("digitalRead(ALARM)"));
        }
    }

    /// A sensor word plus a verb becomes a composition: the photoresistor is
    /// in the source table, so it is no longer a plain lesson.
    #[test]
    fn a_light_sensor_composes_with_a_lamp() {
        let p = parse("датчик света который мигает");
        assert_eq!(p.intent.lesson, Some(crate::lessons::PHOTORESISTOR));
        let recipe = p.intent.recipe.expect("the light sensor composes");
        assert_eq!(recipe.source, crate::spec::SourceId::Light);
        assert_eq!(plan(&p.intent, Lang::Ru).unwrap().tid, Tid::Composed);
        assert!(build(&p.intent, 0, Lang::Ru).is_some());
    }

    /// A sensor the table does not describe yet still builds the plain
    /// lesson instead of being refused.
    #[test]
    fn an_undeclared_sensor_falls_back_to_the_lesson() {
        let p = parse("геркон который мигает");
        assert_eq!(p.intent.lesson, Some(4));
        assert!(p.intent.recipe.is_none());
        let plan = plan(&p.intent, Lang::Ru).expect("the lesson plans");
        assert_eq!(plan.tid, Tid::Lesson(4));
        assert!(build(&p.intent, 0, Lang::Ru).is_some());
    }

    #[test]
    fn every_part_on_the_board_is_in_the_bill() {
        let intent = Intent {
            load: Load::Motor,
            input: Some(Input::Button),
            transistor: true,
            presses: true,
            lesson: None,
            action: None,
            raw: String::new(),
            condition: None,
            recipe: None,
        };
        let built = build(&intent, 0, Lang::Ru).unwrap();
        let bom = bom(&built.circuit, Lang::Ru);
        let total: usize = bom.iter().map(|(_, _, n)| n).sum();
        assert_eq!(total, built.circuit.parts.len());
        assert!(bom.iter().any(|(v, _, _)| v.contains("IRLZ44N")));
        assert!(bom.iter().any(|(v, _, _)| v.contains("Arduino")));
    }

    /// The Tinkercad-style list groups identical parts and stacks their
    /// designators in one name cell; the breadboard is a row of its own.
    #[test]
    fn the_component_list_stacks_names() {
        let p = parse("газ выше 400 rgb");
        let built = build(&p.intent, 0, Lang::Ru).unwrap();
        let rows = component_rows(&built.circuit, Lang::Ru);
        let resistors = rows
            .iter()
            .find(|r| r.component.starts_with("резистор") && r.component.contains("220"))
            .expect("three 220 ohm resistors");
        assert_eq!(resistors.quantity, 3);
        assert_eq!(resistors.names, vec!["R3", "R4", "R5"]);
        let board = rows.last().expect("a breadboard row");
        assert_eq!(board.quantity, 1);
        assert!(board.names.is_empty());
        assert!(board.component.starts_with("Макетка 30"));

        // Every part is accounted for exactly once.
        let counted: usize = rows.iter().map(|r| r.quantity).sum();
        assert_eq!(counted, built.circuit.parts.len() + 1);
    }

    #[test]
    fn the_component_list_downloads_like_tinkercad() {
        let p = parse("газ выше 400 rgb");
        let built = build(&p.intent, 0, Lang::Ru).unwrap();
        let rows = component_rows(&built.circuit, Lang::Ru);
        let csv = component_csv(&rows);
        let mut lines = csv.lines();
        assert_eq!(lines.next(), Some("Name,Quantity,Component"));
        assert_eq!(lines.count(), rows.len());
        assert!(csv.contains("R3 R4 R5,3,"));
        assert!(csv.ends_with('\n'));

        // A comma inside a component is quoted instead of splitting a cell.
        let odd = ComponentRow {
            names: vec!["D1".into()],
            quantity: 1,
            component: "светодиод, красный".into(),
        };
        let csv = component_csv(&[odd]);
        assert!(csv.contains("D1,1,\"светодиод, красный\""));
    }
}

