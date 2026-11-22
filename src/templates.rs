//! From an intent to a concrete circuit, a bill of materials, and a sketch.
//!
//! Each template is a small electrical design with a few real component
//! choices. The choices are not cosmetic: picking a relay instead of a MOSFET
//! changes the wiring, the code, and what the checks can promise.

use crate::model::*;
use crate::parse::{Input, Intent, Load, ScreenKind};
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
    /// An incandescent lamp: PWM through a MOSFET, or a relay for mains.
    Lamp,
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
    /// A text message on one or more I2C displays.
    TextScreens,
    /// Buttons counted on an I2C display: the screen shows what the controls
    /// do instead of pretending the sentence asked for an LED.
    ButtonsScreen,
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
        (Load::Lamp, _) => Some(Tid::Lamp),
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

#[derive(Clone)]
pub struct Built {
    pub tid: Tid,
    pub title: String,
    pub circuit: Circuit,
    pub code: String,
    pub choice: usize,
    pub summary: String,
    pub notes: Vec<L>,
    /// Pins the sentence named that this circuit genuinely has no place for.
    /// A note explains each one, so the checklist does not call it missing.
    pub explained_pins: Vec<u8>,
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
    let door = ["доступ", "доспуп", "пропуск", "турникет", "замок", "скуд", "access", "домофон"]
        .iter()
        .any(|key| crate::words::mentions(&text, key));
    let cards = crate::words::mentions(&text, "карт") || text.contains("метк");
    let reader = text.contains("rfid") || text.contains("rc522") || text.contains("считывател");
    let screen = text.contains("диспле")
        || text.contains("экран")
        || text.contains("oled")
        || text.contains("lcd");
    let memory = crate::words::mentions(&text, "памят");
    // The exact kind of memory is what a card table needs. A bare "память"
    // also names an SD card or a text note, so on its own it counts only
    // together with a door.
    let store = text.contains("eeprom")
        || crate::words::mentions(&text, "энергонезависим")
        || crate::words::mentions(&text, "энергозависим")
        || text.contains("база карт")
        || text.contains("таблица карт");
    if store {
        return door || cards || reader;
    }
    (memory && door) || (memory && reader && screen)
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
        .filter(|n| *n > 0)
}

/// What the access scene must carry. The sentence decides the table size,
/// the screen and whether a refused card raises a siren; the scene then
/// builds exactly those parts instead of a fixed picture.
pub(crate) struct AccessSpec {
    pub slots: usize,
    pub asked: Option<usize>,
    pub screen: crate::parse::ScreenKind,
    pub siren: bool,
}

pub(crate) fn access_spec(raw: &str) -> AccessSpec {
    let asked = card_slots(raw);
    AccessSpec {
        // An Uno's EEPROM keeps 255 four-byte UIDs and COUNT is uint8_t.
        slots: asked.unwrap_or(49).min(255),
        asked,
        screen: crate::parse::screens_in(raw)
            .map(|s| s.kind)
            .unwrap_or(crate::parse::ScreenKind::Any),
        siren: crate::parse::named_devices(raw).buzzer > 0,
    }
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
                "Замок, карты в EEPROM, экран",
                "Lock, cards in EEPROM, a screen",
            ),
            L::new(
                "Кнопка пишет карту в таблицу EEPROM, знакомая карта открывает замок, экран показывает запись и доступ. Названный экран и сирена попадают в схему.",
                "A button saves a card into an EEPROM table, a known card opens the lock, and the screen shows the saving and the access. A named screen or siren is wired in.",
            ),
            "115 мА",
            "высокая",
            4,
        ),
    ];
    let text = crate::words::normalize(raw);
    // A named screen or relay must be on the recommended build, not only in a
    // choice the user has to find: the checklist would rightly report it
    // missing while the plan itself ignored the word.
    let screen = text.contains("диспле")
        || text.contains("экран")
        || text.contains("oled")
        || text.contains("lcd");
    let lock = text.contains("реле")
        || text.contains("замок")
        || text.contains("lock")
        || text.contains("турникет")
        || text.contains("домофон");
    let recommended = if wants_access(raw) {
        3
    } else if screen {
        1
    } else if lock {
        2
    } else {
        0
    };
    let title = match recommended {
        3 => L::new("RFID RC522: карты в EEPROM", "RFID RC522: cards in EEPROM"),
        2 => L::new("RFID RC522: замок на реле", "RFID RC522: relay lock"),
        1 => L::new("RFID RC522: UID на экран", "RFID RC522: UID on a screen"),
        _ => L::new("RFID RC522: чтение метки", "RFID RC522: reading a tag"),
    };
    (title, choices, recommended)
}

/// Why this plan, from the slots it closes. The number is not a share of
/// matched keywords: it is the evidence-weighted part of the typed task that
/// the circuit actually draws, and what stays open is named honestly.
fn explain(intent: &Intent, tid: Tid, choice: usize, lang: Lang) -> String {
    let ru = lang == Lang::Ru;
    let task = &intent.task;
    let mut cover = crate::task::template_cover(tid, choice);
    // `augment_built` adds every named output to the drawing, so a named
    // output that the template does not drive itself is still closed by the
    // built circuit. Sensors, controls and screens are not added that way,
    // so they stay honest in the coverage below.
    for slot in &task.outputs {
        if !cover.outputs.contains(&slot.kind) {
            cover.outputs.push(slot.kind);
        }
    }
    let coverage = task.coverage(&cover);
    let mut read: Vec<String> = Vec::new();
    for slot in &task.outputs {
        read.push(slot.kind.label().get(lang).to_string());
    }
    for slot in &task.controls {
        read.push(match slot.kind {
            Input::Button => tr(lang, "кнопка", "button").to_string(),
            Input::Pot => tr(lang, "потенциометр", "potentiometer").to_string(),
            Input::Ldr => tr(lang, "фоторезистор", "photoresistor").to_string(),
        });
    }
    for slot in &task.sensors {
        read.push(crate::task::SlotName::Sensor(slot.source).label().get(lang).to_string());
    }
    if !task.screens.is_empty() {
        read.push(tr(lang, "экран", "display").to_string());
    }
    if read.is_empty() {
        read.push(tr(lang, "не назван", "unnamed").to_string());
    }
    let mut out = format!("{}: {}", tr(lang, "Прочитано", "Read as"), read.join(" + "));
    let coverage_text = coverage.describe(lang);
    if !coverage_text.is_empty() {
        out.push_str("; ");
        out.push_str(&coverage_text);
    }
    let extra: Vec<String> = coverage
        .extra
        .iter()
        .map(|s| s.label().get(lang).to_string())
        .collect();
    if !extra.is_empty() {
        out.push_str(&format!(
            "; {}: {}",
            tr(lang, "план добавляет", "the plan adds"),
            extra.join(", ")
        ));
    }
    if let Some(ms) = crate::semantics::analyse(&intent.raw).period_ms {
        out.push_str(&format!(
            "; {} {} {}",
            tr(lang, "период", "period"),
            ms,
            tr(lang, "мс", "ms")
        ));
    }
    // A table that does not fit is clamped, not silently shrunk: an Uno keeps
    // 255 four-byte UIDs in 1024 bytes of EEPROM.
    if let Some(n) = card_slots(&intent.raw) {
        if wants_access(&intent.raw) && n > 255 {
            out.push_str(&format!(
                "; {}",
                if ru {
                    format!("в EEPROM Uno помещается 255 карт, запрошено {n}")
                } else {
                    format!("an Uno EEPROM holds 255 cards, {n} requested")
                }
            ));
        }
    }
    // A ready task is still worth showing, but it is a resemblance, not the
    // reading: the slots above are what decides the circuit.
    #[cfg(feature = "book-tasks")]
    if let Some(score) = crate::book_tasks::closest(&intent.raw).map(|(_, _, score)| score) {
        out = format!("{} {}%; {out}", tr(lang, "похожая задача книги, совпадение", "a similar book task, match"), score);
    }
    out
}

/// Why a printed lesson answers this sentence: the same coverage arithmetic
/// as the built-in templates, with the lesson's own parts as the plan.
fn lesson_why(i: usize, intent: &Intent, lang: Lang) -> String {
    let task = &intent.task;
    let mut cover = crate::task::lesson_cover(i);
    for slot in &task.outputs {
        if !cover.outputs.contains(&slot.kind) {
            cover.outputs.push(slot.kind);
        }
    }
    let coverage = task.coverage(&cover);
    let lesson = crate::lessons::ALL
        .get(i)
        .map(|l| l.title.get(lang))
        .unwrap_or(tr(lang, "урок", "lesson"));
    let mut out = format!("{}: {lesson}", tr(lang, "Урок", "Lesson"));
    let coverage_text = coverage.describe(lang);
    if !coverage_text.is_empty() {
        out.push_str("; ");
        out.push_str(&coverage_text);
    }
    out
}

/// A composition explains itself through its own recipe. The semantic roles
/// are a guess about the sentence; the recipe is what was actually built, so
/// the two must not disagree on screen.
fn recipe_why(recipe: crate::spec::Composition, lang: Lang) -> String {
    let words: Vec<String> = crate::compose::recipe_words(recipe)
        .iter()
        .map(|l| l.get(lang).to_string())
        .collect();
    format!(
        "{}: {}",
        tr(lang, "Прочитано", "Read as"),
        words.join(" → ")
    )
}

pub fn plan(intent: &Intent, lang: Lang) -> Option<Plan> {
    if let Some(screens) = intent.text_screens {
        return Some(text_screen_plan(&intent.raw, screens, lang));
    }
    if let Some(recipe) = intent.recipe {
        let mut p = crate::compose::plan(recipe, lang)?;
        p.why = Some(recipe_why(recipe, lang));
        return Some(p);
    }
    if let Some(i) = intent.lesson {
        let mut p = display_lesson_plan(i, intent.named.screens, lang);
        // A lesson is a real answer, so it explains itself like a template:
        // which slots it closes and what stays open. The old `None` left the
        // "why" panel empty precisely where the user needs it most.
        p.why = Some(lesson_why(i, intent, lang));
        return Some(p);
    }
    if intent.task.wants_buttons_screen() {
        let screens = intent.named.screens.unwrap_or(crate::parse::Screens {
            kind: ScreenKind::Any,
            count: 1,
            matrix: false,
            segments: false,
        });
        return Some(buttons_screen_plan(screens, lang));
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
        Tid::Lamp => {
            let mains = {
                let text = crate::words::normalize(&intent.raw);
                text.contains("220") || text.contains("сеть") || text.contains("симистор")
            };
            let mosfet = Choice {
                name: L::new("IRLZ44N, 12 В", "IRLZ44N, 12 V"),
                blurb: L::new(
                    "MOSFET в ключе низкой стороны: ШИМ на D9 плавно меняет яркость лампы 12 В.",
                    "A low-side MOSFET: PWM on D9 sets the brightness of a 12 V lamp.",
                ),
                pros: vec![
                    L::new("Плавная регулировка, ключ не греется", "Smooth dimming, the switch stays cool"),
                    L::new("Ток до 10 А — запас на лампу мощнее", "Up to 10 A, room for a bigger lamp"),
                ],
                cons: vec![
                    L::new("Нужен отдельный блок 12 В", "Needs a separate 12 V supply"),
                    L::new("Не годится для 220 В: ключ на 55 В", "Not for 220 V: the switch is rated 55 V"),
                ],
                specs: vec![
                    spec_line("12 В", VOLTAGE),
                    spec_line("0.5 А", CURRENT),
                    spec_line("средняя", PRICE),
                    spec_line(&level_word(2, lang), LEVEL),
                ],
                level: 2,
            };
            let relay = Choice {
                name: L::new("Реле SRD-05VDC, 220 В", "Relay SRD-05VDC, 220 V"),
                blurb: L::new(
                    "Реле развязывает лампу 220 В, но даёт только вкл/выкл: яркость им не регулируют.",
                    "A relay isolates a 220 V lamp but only switches it: it cannot dim.",
                ),
                pros: vec![L::new("Гальваническая развязка от сети", "Galvanic isolation from the mains")],
                cons: vec![
                    L::new("Яркость не регулируется", "Brightness is not adjustable"),
                    L::new("Для яркости нужен симисторный диммер", "Dimming needs a triac dimmer"),
                ],
                specs: vec![
                    spec_line("220 В AC", VOLTAGE),
                    spec_line("10 А", CURRENT),
                    spec_line("низкая", PRICE),
                    spec_line(&level_word(2, lang), LEVEL),
                ],
                level: 2,
            };
            // A mains lamp gets the relay only. Offering the 55 V MOSFET for
            // it would be unsafe, however the user picks.
            let choices = if mains {
                vec![relay]
            } else {
                vec![mosfet, relay]
            };
            let title = if mains {
                L::new(
                    "Лампа 220 В: включение и выключение",
                    "220 V lamp: on and off",
                )
            } else {
                L::new(
                    "Лампа накаливания: яркость",
                    "Incandescent lamp: brightness",
                )
            };
            (title, choices, 0)
        }
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
        Tid::Radio433 => radio433_plan(lang, &intent.raw),
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
        Tid::Lesson(_) | Tid::Composed | Tid::TextScreens | Tid::ButtonsScreen => return None,
    };
    Some(Plan {
        tid,
        title,
        headline: None,
        choices,
        recommended,
        why: Some(explain(intent, tid, recommended, lang)),
    })
}

/// A ready-made text display, described closely enough to emit a working
/// sketch: the module name for the bench, and the geometry the library needs.
#[derive(Clone, Copy)]
struct TextModule {
    name: &'static str,
    cols: u8,
    rows: u8,
    oled: bool,
}

const TEXT_LCD1602: TextModule = TextModule { name: "LCD1602", cols: 16, rows: 2, oled: false };
const TEXT_LCD2004: TextModule = TextModule { name: "LCD2004", cols: 20, rows: 4, oled: false };
const TEXT_OLED: TextModule = TextModule { name: "OLED 0.96", cols: 128, rows: 64, oled: true };

/// What the "text on displays" scene offers. An SSD1306 answers at only two
/// addresses, so three or more screens are built on character LCDs instead.
fn text_modules(screens: crate::parse::Screens) -> Vec<TextModule> {
    use crate::parse::ScreenKind;
    // The parser routes a matrix or a seven-segment board to its lesson;
    // this scene never receives one. The fallback keeps the table total.
    if screens.matrix || screens.segments {
        return vec![TEXT_LCD1602];
    }
    let mut out = match screens.kind {
        ScreenKind::Oled => vec![TEXT_OLED],
        ScreenKind::Lcd => vec![TEXT_LCD1602],
        ScreenKind::Lcd2004 => vec![TEXT_LCD2004],
        ScreenKind::Any => vec![TEXT_LCD1602, TEXT_LCD2004, TEXT_OLED],
    };
    if screens.count > 2 {
        out.retain(|m| !m.oled);
    }
    if out.is_empty() {
        out.push(TEXT_LCD1602);
    }
    out
}

/// A screen that shows what the buttons do. The sentence named controls and a
/// display, so the plan closes both slots instead of drawing an LED the user
/// never asked for.
fn buttons_screen_plan(screens: crate::parse::Screens, lang: Lang) -> Plan {
    let ru = lang == Lang::Ru;
    let modules = text_modules(screens);
    let title = if screens.count > 1 {
        L::new("Кнопки: счётчики на экранах", "Buttons: counters on the displays")
    } else {
        L::new("Кнопки: счётчик нажатий на экране", "Buttons: press counter on the display")
    };
    let mut choices = Vec::new();
    for (index, m) in modules.iter().enumerate() {
        let name = if m.oled {
            L::new("OLED 128×64 (I2C)", "OLED 128×64 (I2C)")
        } else if m.cols >= 20 {
            L::new("LCD2004 20×4 (I2C)", "LCD2004 20×4 (I2C)")
        } else {
            L::new("LCD1602 16×2 (I2C)", "LCD1602 16×2 (I2C)")
        };
        choices.push(Choice {
            name,
            blurb: if m.oled {
                L::new(
                    "Графический экран: две строки по 16 символов крупным шрифтом.",
                    "A graphic panel: two 16-character rows in a large font.",
                )
            } else {
                L::new(
                    "Символьный LCD: каждая кнопка занимает свою строку.",
                    "A character LCD: each button gets its own row.",
                )
            },
            pros: vec![L::new("Две линии питания и I2C", "Two supply pins and I2C")],
            cons: vec![L::new("Занимает A4/A5", "Uses A4/A5")],
            specs: vec![
                spec_line(if m.oled { "128×64" } else { "16×2" }, L::new("Знаков", "Characters")),
                spec_line("I2C", L::new("Шина", "Bus")),
                spec_line(&level_word(1, lang), LEVEL),
            ],
            level: 1,
        });
        if index + 1 >= modules.len().min(2) {
            break;
        }
    }
    Plan {
        tid: Tid::ButtonsScreen,
        title,
        headline: None,
        choices,
        recommended: 0,
        why: Some(if ru {
            "Задача называет кнопки и экран; план закрывает оба слота и показывает счётчики нажатий.".to_string()
        } else {
            "The task names buttons and a display; the plan closes both slots and shows the press counts.".to_string()
        }),
    }
}

/// The word "приветствие", "надпись" or "текст" from the sentence, so the
/// header repeats what the user asked to show rather than a generic title.
fn text_subject(raw: &str, lang: Lang) -> &'static str {
    let text = crate::words::normalize(raw);
    let ru = lang == Lang::Ru;
    if text.contains("приветств") || text.contains("привет") || text.contains("greeting") {
        if ru { "Приветствие" } else { "Greeting" }
    } else if text.contains("надпис") || text.contains("caption") {
        if ru { "Надпись" } else { "Caption" }
    } else {
        if ru { "Текст" } else { "Text" }
    }
}

fn text_screen_plan(raw: &str, screens: crate::parse::Screens, lang: Lang) -> Plan {
    let ru = lang == Lang::Ru;
    let subject = text_subject(raw, lang);
    let place = match (ru, screens.count) {
        (true, 1) => "одном экране".to_string(),
        (true, 2) => "двух экранах".to_string(),
        (true, 3) => "трёх экранах".to_string(),
        (true, 4) => "четырёх экранах".to_string(),
        (true, n) => format!("{n} экранах"),
        (false, 1) => "one display".to_string(),
        (false, 2) => "two displays".to_string(),
        (false, 3) => "three displays".to_string(),
        (false, 4) => "four displays".to_string(),
        (false, n) => format!("{n} displays"),
    };
    let choices = text_modules(screens)
        .into_iter()
        .map(|m| Choice {
            name: L::new(m.name, m.name),
            blurb: if m.oled {
                L::new(
                    "Графический OLED на шине I2C: две сигнальные линии, адрес 0x3C.",
                    "A graphic OLED on I2C: two signal wires at address 0x3C.",
                )
            } else {
                L::new(
                    "Символьный LCD через переходник I2C: SDA на A4, SCL на A5.",
                    "A character LCD through an I2C backpack: SDA on A4, SCL on A5.",
                )
            },
            pros: Vec::new(),
            cons: Vec::new(),
            specs: vec![
                spec_line(&screens.count.to_string(), L::new("Экранов", "Screens")),
                spec_line("I2C · A4/A5", L::new("Шина", "Bus")),
                spec_line(&level_word(2, lang), LEVEL),
            ],
            level: 2,
        })
        .collect();
    let why = if ru {
        format!(
            "Прочитано: {}; экранов {}; шина I2C: A4, A5",
            subject.to_lowercase(),
            screens.count
        )
    } else {
        format!(
            "Read as: {}; screens {}; I2C bus: A4, A5",
            subject.to_lowercase(),
            screens.count
        )
    };
    Plan {
        tid: Tid::TextScreens,
        title: L::new("Текст на экране", "Text on a display"),
        headline: Some(format!(
            "{} {} {}",
            subject,
            if ru { "на" } else { "on" },
            place
        )),
        choices,
        recommended: 0,
        why: Some(why),
    }
}

/// The display types offered when a sentence names a screen but not its
/// model. `choice` picks one; a second and third screen reuse it.
fn display_modules(screens: crate::parse::Screens) -> Vec<(&'static str, &'static str)> {
    use crate::parse::ScreenKind;
    if screens.segments {
        return vec![("Четырёхразрядный индикатор (I2C)", "7SEG-4")];
    }
    if screens.matrix {
        return vec![("Матрица 8×8 на MAX7219", "MAX7219")];
    }
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
    // The lesson already draws its screen: TV OUT, MAX7219, Nokia 5110 and
    // the rest must not gain a second I2C OLED that the sketch never talks
    // to. A matrix asked on a non-matrix lesson is not promised either: the
    // build has no driver for it, and the checklist will say so.
    if crate::lessons::display_count(i) > 0 || screens.matrix {
        return base;
    }
    // Seven segments belong to the clock lesson; another lesson would draw a
    // board its sketch does not use.
    if screens.segments && !crate::lessons::ALL[i].keys.contains(&"ds3231") {
        return base;
    }
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
    let headline = if lang == Lang::Ru {
        match screens.count {
            1 => "Экран на I2C".to_string(),
            2 => "Два экрана на I2C".to_string(),
            3 => "Три экрана на I2C".to_string(),
            n => format!("{n} экранов на I2C"),
        }
    } else {
        match screens.count {
            1 => "One I2C display".to_string(),
            2 => "Two I2C displays".to_string(),
            3 => "Three I2C displays".to_string(),
            n => format!("{n} I2C displays"),
        }
    };
    // A display lesson is about the screen; any other lesson (the clock, an
    // access counter) merely uses it as a readout, so its own title stays in
    // the header.
    let (title, headline) = if crate::lessons::is_text_display(i) {
        (
            L::new("Экран для урока", "A display for the lesson"),
            Some(headline),
        )
    } else {
        (
            base.title,
            Some(crate::lessons::ALL[i].title.get(lang).to_string()),
        )
    };
    Plan {
        tid: base.tid,
        title,
        headline,
        choices,
        recommended: 0,
        why: None,
    }
}

/// The left edge of a display row, the gap between boards, and the gap
/// between rows.
const SCREEN_LEFT: f32 = 2.0;
const SCREEN_GAP: f32 = 6.0;
const SCREEN_ROW_GAP: f32 = 8.0;

/// Where the displays of one scene go: below everything already on the bench,
/// never on top of each other or of the controller. A wide board such as the
/// 98 mm LCD2004 gets the room it needs, so a row may grow past the Uno
/// instead of hiding half of the next board.
pub(crate) fn screen_spots(count: usize, module: &str, top: f32) -> Vec<Pt> {
    let (w, h) = crate::modules::find(module)
        .map(|m| (m.w, m.h))
        .unwrap_or((60.0, 26.0));
    let right = (crate::model::UNO_X + crate::model::UNO_SIZE.0)
        .max(SCREEN_LEFT + 2.0 * (w + SCREEN_GAP));
    let mut out = Vec::with_capacity(count);
    let (mut x, mut y, mut row_h) = (SCREEN_LEFT, top, 0.0f32);
    for _ in 0..count {
        if x > SCREEN_LEFT && x + w > right {
            x = SCREEN_LEFT;
            y += row_h + SCREEN_ROW_GAP;
            row_h = 0.0;
        }
        out.push(Pt::new(x, y));
        x += w + SCREEN_GAP;
        row_h = row_h.max(h);
    }
    out
}

/// The whole circuit for "text on displays": the screens on the I2C bus, the
/// power rails, and a sketch that prints the same message on each board.
fn text_screen_build(
    lang: Lang,
    screens: crate::parse::Screens,
    choice: usize,
) -> (Circuit, String, L, Vec<L>) {
    let modules = text_modules(screens);
    let module = modules[choice.min(modules.len() - 1)];
    let mut b = B::new(lang);
    let mut label = 2u8;
    let top = b.c.bbox().bottom() + 8.0;
    for at in screen_spots(screens.count, module.name, top) {
        let name = format!("U{label}");
        label += 1;
        let disp = b.breakout(&name, module.name, at);
        if let Some(m) = crate::modules::find(module.name) {
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
    }
    feeds(&mut b);
    let code = text_display_code(lang, screens, module);
    // "Сообщение" covers a greeting, a caption and plain text alike, so the
    // summary stays true whichever word the task used.
    let summary = if screens.count == 1 {
        L::new(
            "Сообщение выводится на экран по шине I2C.",
            "The message is shown on the display over I2C.",
        )
    } else {
        L::new(
            "Сообщение выводится на каждый экран: у каждого свой адрес на общей шине I2C.",
            "The message goes to every display: each has its own address on the shared I2C bus.",
        )
    };
    let mut notes = Vec::new();
    if screens.count > 1 {
        notes.push(if module.oled {
            L::new(
                "SSD1306 отвечает на 0x3C и 0x3D: адрес задаёт перемычка SA0. Проверь оба адреса сканером I2C.",
                "An SSD1306 answers at 0x3C and 0x3D; the SA0 jumper sets the address. Check both with an I2C scanner.",
            )
        } else {
            L::new(
                "Первый LCD отвечает на 0x27, второй — на 0x26: у второго замкнут A0. Проверь адреса сканером I2C.",
                "The first LCD answers at 0x27 and the second at 0x26: the second has A0 bridged. Check both with an I2C scanner.",
            )
        });
    } else {
        notes.push(if module.oled {
            L::new(
                "Адрес SSD1306 — 0x3C или 0x3D; перемычка SA0 выбирает его.",
                "An SSD1306 answers at 0x3C or 0x3D; the SA0 jumper chooses.",
            )
        } else {
            L::new(
                "Адрес переходника LCD — 0x27 или 0x3F; его покажет сканер I2C.",
                "The LCD backpack answers at 0x27 or 0x3F; an I2C scanner tells which.",
            )
        });
    }
    notes.push(L::new(
        "В знакогенераторе HD44780 и в шрифте Adafruit нет кириллицы, поэтому в скетче «Privet!». Русские буквы требуют своей таблицы символов.",
        "Neither the HD44780 character ROM nor the Adafruit font has Cyrillic, so the sketch prints \"Privet!\". Russian letters need a custom character table.",
    ));
    (b.finish(), code, summary, notes)
}

/// A screen with only buttons: each press increments a counter and the LCD
/// shows the counts. The display and the buttons are the whole circuit, so
/// no LED or motor is invented to fill the gap.
fn buttons_screen_build(
    lang: Lang,
    screens: crate::parse::Screens,
    buttons: usize,
    choice: usize,
) -> (Circuit, String, L, Vec<L>) {
    let modules = text_modules(screens);
    let module = modules[choice.min(modules.len() - 1)];
    let count = buttons.clamp(1, 4);
    let pins = ["D2", "D3", "D4", "D5"];
    let mut b = B::new(lang);
    let gnd = b.gnd(52.0);
    let disp = b.breakout("DS1", module.name, Pt::new(2.0, 122.0));
    if let Some(m) = crate::modules::find(module.name) {
        for (pin, dx, _) in m.pins.iter().copied() {
            match pin {
                "GND" => b.w(b.mp(disp, pin), gnd, NetClass::Gnd),
                "VCC" | "+5V" | "3V3" | "+" => {
                    b.w(b.mp(disp, pin), b.vcc(2.0 + dx), NetClass::Vcc)
                }
                "SDA" => b.w(b.mp(disp, pin), b.uno_pin("A4"), NetClass::Signal),
                "SCL" => b.w(b.mp(disp, pin), b.uno_pin("A5"), NetClass::Signal),
                _ => {}
            }
        }
    }
    for n in 0..count {
        let label = format!("B{}", n + 1);
        let btn = b.button(&label, 14 + n as u8 * 7, Row::E);
        b.w(b.uno_pin(pins[n]), btn[3], NetClass::Signal);
        b.w(btn[0], gnd, NetClass::Gnd);
    }
    feeds(&mut b);
    let code = buttons_screen_code(lang, module, count);
    let summary = if count == 1 {
        L::new(
            "Каждое нажатие увеличивает счётчик, экран показывает его по I2C.",
            "Every press increments a counter, and the display shows it over I2C.",
        )
    } else {
        L::new(
            "У каждой кнопки свой счётчик; экран показывает все счётчики по I2C.",
            "Each button has its own counter; the display shows all of them over I2C.",
        )
    };
    let mut notes = vec![L::new(
        "Кнопки включены между пином и землёй: подтяжка INPUT_PULLUP внутри МК, дребезг гасится 30 мс.",
        "The buttons sit between a pin and ground: the MCU's INPUT_PULLUP does the pull-up, and 30 ms debounce removes chatter.",
    )];
    if module.oled {
        notes.push(L::new(
            "Адрес SSD1306 — 0x3C или 0x3D; перемычка SA0 выбирает его.",
            "An SSD1306 answers at 0x3C or 0x3D; the SA0 jumper chooses.",
        ));
    } else {
        notes.push(L::new(
            "Адрес переходника LCD — 0x27 или 0x3F; его покажет сканер I2C.",
            "The LCD backpack answers at 0x27 or 0x3F; an I2C scanner tells which.",
        ));
    }
    (b.finish(), code, summary, notes)
}

fn buttons_screen_code(lang: Lang, module: TextModule, count: usize) -> String {
    let ru = lang == Lang::Ru;
    let pins = &["2", "3", "4", "5"][..count];
    let pins = pins.join(", ");
    let cleared = (0..count)
        .map(|n| format!("  pressed[{n}] = false;\n  lastEdge[{n}] = 0;\n  count[{n}] = 0;\n"))
        .collect::<String>();
    let mut includes = String::from("#include <Wire.h>\n");
    let mut objects = String::new();
    let mut setup = String::from("  Wire.begin();\n");
    let mut show = String::new();
    if module.oled {
        includes.push_str("#include <Adafruit_GFX.h>\n#include <Adafruit_SSD1306.h>\n");
        objects.push_str("Adafruit_SSD1306 lcd(128, 64, &Wire, -1);\n");
        setup.push_str(
            "  lcd.begin(SSD1306_SWITCHCAPVCC, 0x3C);\n  lcd.clearDisplay();\n  lcd.setTextSize(2);\n  lcd.setTextColor(SSD1306_WHITE);\n",
        );
        for n in 0..count {
            show.push_str(&format!(
                "  lcd.setCursor(0, {});\n  lcd.print(F(\"B{}=\"));\n  lcd.print(count[{}]);\n",
                n * 24,
                n + 1,
                n
            ));
        }
        show.push_str("  lcd.display();\n");
    } else {
        includes.push_str("#include <LiquidCrystal_I2C.h>\n");
        objects.push_str(&format!(
            "LiquidCrystal_I2C lcd(0x27, {}, {});\n",
            module.cols, module.rows
        ));
        setup.push_str("  lcd.init();\n  lcd.backlight();\n");
        for n in 0..count {
            show.push_str(&format!(
                "  lcd.setCursor(0, {n});\n  lcd.print(F(\"B{}={{}}\"));\n  lcd.print(count[{n}]);\n",
                n + 1
            ));
        }
    }
    let mut loop_body = String::new();
    for n in 0..count {
        loop_body.push_str(&format!(
            "    bool now{n} = digitalRead(BUTTONS[{n}]) == LOW;\n    if (now{n} != pressed[{n}] && millis() - lastEdge[{n}] > DEBOUNCE_MS) {{\n      lastEdge[{n}] = millis();\n      pressed[{n}] = now{n};\n      if (now{n}) {{\n        count[{n}]++;\n        changed = true;\n      }}\n    }}\n"
        ));
    }
    let comment = if ru {
        "// Кнопки на D2..D5, экран по I2C: SDA — A4, SCL — A5."
    } else {
        "// Buttons on D2..D5, display over I2C: SDA on A4, SCL on A5."
    };
    format!(
        "{comment}\n{includes}{objects}\nconst uint8_t BUTTONS[] = {{{pins}}};\nconst uint8_t BUTTON_COUNT = {count};\nconst uint16_t DEBOUNCE_MS = 30;\nbool pressed[{count}] = {{false}};\nuint32_t lastEdge[{count}] = {{0}};\nuint32_t count[{count}] = {{0}};\n\nvoid setup() {{\n{setup}  for (uint8_t i = 0; i < BUTTON_COUNT; i++) pinMode(BUTTONS[i], INPUT_PULLUP);\n{cleared}}}\n\nvoid loop() {{\n  bool changed = false;\n{loop_body}  if (changed) {{\n{show}  }}\n  delay(20);\n}}\n"
    )
}

/// A sketch that prints one message on every display. The address of each
/// board is printed under the message on a character LCD, so a second board
/// that answers at another address is easy to spot.
fn text_display_code(
    lang: Lang,
    screens: crate::parse::Screens,
    module: TextModule,
) -> String {
    let ru = lang == Lang::Ru;
    let message = if ru { "Privet!" } else { "Hello!" };
    let mut includes = String::from("#include <Wire.h>\n");
    if module.oled {
        includes.push_str("#include <Adafruit_GFX.h>\n#include <Adafruit_SSD1306.h>\n");
    } else {
        includes.push_str("#include <LiquidCrystal_I2C.h>\n");
    }
    let mut objects = String::new();
    let mut setup = String::from("  Wire.begin();\n");
    let mut show = String::new();
    for n in 0..screens.count {
        let addr = if module.oled {
            0x3C + n as u8
        } else {
            0x27 - n as u8
        };
        if module.oled {
            objects.push_str(&format!("Adafruit_SSD1306 screen{n}(128, 64, &Wire, -1);\n"));
            setup.push_str(&format!(
                "  screen{n}.begin(SSD1306_SWITCHCAPVCC, 0x{addr:02X});\n  screen{n}.clearDisplay();\n  screen{n}.setTextSize(2);\n  screen{n}.setTextColor(SSD1306_WHITE);\n"
            ));
            show.push_str(&format!(
                "  screen{n}.setCursor(0, 24);\n  screen{n}.print(MESSAGE);\n  screen{n}.display();\n"
            ));
        } else {
            objects.push_str(&format!(
                "LiquidCrystal_I2C screen{n}(0x{addr:02X}, {}, {});\n",
                module.cols, module.rows
            ));
            setup.push_str(&format!(
                "  screen{n}.init();\n  screen{n}.backlight();\n"
            ));
            let second = if module.rows > 2 { 2 } else { 1 };
            show.push_str(&format!(
                "  screen{n}.setCursor(0, 0);\n  screen{n}.print(MESSAGE);\n  screen{n}.setCursor(0, {second});\n  screen{n}.print(F(\"0x{addr:02X}\"));\n"
            ));
        }
    }
    let comment = if ru {
        "// Текст на экранах по I2C: SDA — A4, SCL — A5."
    } else {
        "// Text on I2C displays: SDA on A4, SCL on A5."
    };
    format!(
        "{comment}\n{includes}{objects}\nconst char MESSAGE[] = \"{message}\";\n\nvoid setup() {{\n{setup}{show}}}\n\nvoid loop() {{\n}}\n"
    )
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
        L::new("Раздел, урок", "Section, lesson"),
    ));
    specs.push(spec_line(&level_word(l.level, lang), L::new("Сложность", "Effort")));
    Plan {
        tid: Tid::Lesson(i),
        title: L::new("Готовый урок", "A ready-made lesson"),
        // The header names the lesson, not the generic page title: a user
        // reading the panel should see which lesson answered the sentence.
        headline: Some(l.title.get(lang).to_string()),
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
    // A pin the sentence named is moved after the build, where every route
    // gets the same treatment.
    for (pin_name, target) in l.wires {
        let source = boards.iter().copied().find(|&p| {
            b.c.parts[p]
                .pins
                .iter()
                .any(|&q| b.c.pins[q].name == *pin_name)
        });
        let Some(part) = source else { continue };
        let a = b.mp(part, pin_name);
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
    let mut added_segments = false;
    if let Some(wish) = screens {
        // A display lesson already placed the screen it teaches: TV OUT,
        // MAX7219 and Nokia must not gain a second, unused OLED. Only a
        // lesson with no screen of its own gets one added.
        let present = crate::lessons::display_count(i);
        let want = if present > 0 {
            0
        } else if wish.segments {
            // The clock lesson is the only one whose sketch can drive four
            // digits: it shows time on them, not "Privet!".
            if i == 31 {
                added_segments = true;
                1
            } else {
                0
            }
        } else if wish.matrix {
            // No composition or lesson sketch drives a matrix here; drawing
            // it would be a part the code never uses. The checklist reports
            // the missing screen instead.
            0
        } else {
            wish.count
        };
        let modules = display_modules(wish);
        let module = modules[choice.min(modules.len() - 1)].1;
        let top = b.c.bbox().bottom() + 8.0;
        let h = crate::modules::find(module).map(|m| m.h).unwrap_or(30.0);
        for at in screen_spots(want, module, top) {
            let name = format!("U{}", label);
            label += 1;
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
            placed.push((at, h));
        }
    }

    feeds(&mut b);
    let mut notes = l.notes.to_vec();
    if let Some(m) = l.boards.first().and_then(|s| crate::modules::note(s.name)) {
        notes.push(m);
    }
    let base_code = l.code.render(l.title.get(lang));
    let code = if added_segments {
        clock_segments_code(lang)
    } else if i == 31
        && screens.is_some_and(|s| !s.matrix && !s.segments)
    {
        clock_display_code(lang, screens.unwrap(), choice.min(display_modules(screens.unwrap()).len() - 1))
    } else if screens.is_some_and(|s| !s.matrix && !s.segments) {
        format!("{}\n// Экран по I2C: SDA A4, SCL A5\n{}", base_code, display_note(screens.unwrap(), lang))
    } else {
        base_code
    };
    (b.finish(), code, l.summary, notes)
}

/// The clock lesson with four seven-segment digits on a TM1650, both devices
/// on the same I2C bus. The DS3231 keeps the time and the display shows it;
/// neither half is dropped.
fn clock_segments_code(lang: Lang) -> String {
    let ru = lang == Lang::Ru;
    let comment = if ru {
        "// DS3231 и четырёхразрядный индикатор TM1650 на общей шине I2C"
    } else {
        "// A DS3231 and a four-digit TM1650 display on the shared I2C bus"
    };
    format!(
        r#"{comment}
#include <Wire.h>
#include <RTClib.h>
#include <TM1650.h>
RTC_DS3231 rtc;
TM1650 display;
const uint32_t PERIOD_MS = 1000;
uint32_t last = 0;

void setup() {{
  Wire.begin();
  Serial.begin(9600);
  if (!rtc.begin()) Serial.println(F("no RTC"));
  display.init();
  display.setBrightness(5, true);
}}

void loop() {{
  if (millis() - last < PERIOD_MS) return;
  last = millis();
  DateTime now = rtc.now();
  char buf[5];
  snprintf(buf, sizeof(buf), "%02u%02u", now.hour(), now.minute());
  display.displayString(String(buf));
}}
"#
    )
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
        // An incandescent lamp has no polarity: one terminal is the supply,
        // the other is the switched side. Both leave the screw base, drawn
        // above the bulb so the wires do not cross the glass.
        Kind::Lamp => &[("-", 5.0, -3.0), ("+", 11.0, -3.0)],
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
        (Kind::Lamp, "+") => PinKind::Power,
        (Kind::Lamp, _) => PinKind::Passive,
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
const uint32_t HALF_MS = <<PERIOD>>;

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

fn lamp_pot_sketch(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// A0 — ручка, D9 — ШИМ на затворе MOSFET, лампа 12 В в стоке
const uint8_t KNOB = A0;
const uint8_t LAMP = 9;
const uint16_t SAMPLE_MS = 20;

uint32_t lastSample = 0;

void setup() {
  pinMode(LAMP, OUTPUT);
  Serial.begin(9600);
}

void loop() {
  if (millis() - lastSample < SAMPLE_MS) return;
  lastSample = millis();
  int raw = analogRead(KNOB);
  // Линейная шкала: 0…1023 с ручки в 0…255 скважности. Для лампы этого
  // достаточно, а понять и подправить map() проще, чем таблицу кривой.
  int duty = map(raw, 0, 1023, 0, 255);
  analogWrite(LAMP, duty);
  Serial.println(duty);
}
"#,
        &[("C0", tr(lang, "Яркость лампы 12 В от ручки", "12 V lamp brightness from a knob"))],
    )
}

fn lamp_buttons_sketch(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// D2 — ярче, D3 — тусклее; D9 — ШИМ на затворе MOSFET
const uint8_t UP = 2;
const uint8_t DOWN = 3;
const uint8_t LAMP = 9;
const uint8_t STEP = 25;
const uint16_t DEBOUNCE_MS = 30;

uint8_t duty = 0;
bool upWas = false;
bool downWas = false;
uint32_t lastEdge = 0;

void setup() {
  pinMode(UP, INPUT_PULLUP);
  pinMode(DOWN, INPUT_PULLUP);
  pinMode(LAMP, OUTPUT);
  analogWrite(LAMP, duty);
}

void loop() {
  bool up = digitalRead(UP) == LOW;
  bool down = digitalRead(DOWN) == LOW;
  if (millis() - lastEdge >= DEBOUNCE_MS) {
    if (up && !upWas) {
      lastEdge = millis();
      duty = (duty <= 255 - STEP) ? duty + STEP : 255;
    } else if (down && !downWas) {
      lastEdge = millis();
      duty = (duty >= STEP) ? duty - STEP : 0;
    }
    upWas = up;
    downWas = down;
    analogWrite(LAMP, duty);
  }
}
"#,
        &[("C0", tr(lang, "Две кнопки задают яркость лампы", "Two buttons set the lamp brightness"))],
    )
}

fn lamp_ramp_sketch(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// D9 — ШИМ на затворе MOSFET, лампа 12 В плавно разгорается и гаснет
const uint8_t LAMP = 9;
const uint8_t STEP = 1;
const uint16_t STEP_MS = 12;

uint8_t duty = 0;
int8_t dir = 1;
uint32_t last = 0;

void setup() {
  pinMode(LAMP, OUTPUT);
}

void loop() {
  if (millis() - last < STEP_MS) return;
  last = millis();
  if (dir > 0 && duty == 255) dir = -1;
  if (dir < 0 && duty == 0) dir = 1;
  duty += dir * STEP;
  analogWrite(LAMP, duty);
}
"#,
        &[("C0", tr(lang, "Плавное разжигание лампы 12 В", "A 12 V lamp fading up and down"))],
    )
}

fn lamp_relay_sketch(lang: Lang) -> String {
    fill(
        r#"// <<C0>>
// D2 — кнопка, D7 — модуль реле; лампа включается и выключается
const uint8_t BTN = 2;
const uint8_t RELAY = 7;
const uint16_t DEBOUNCE_MS = 30;

bool lampOn = false;
bool pressed = false;
uint32_t lastEdge = 0;

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  pinMode(RELAY, OUTPUT);
  digitalWrite(RELAY, LOW);
}

void loop() {
  bool now = digitalRead(BTN) == LOW;
  if (now != pressed && millis() - lastEdge >= DEBOUNCE_MS) {
    lastEdge = millis();
    pressed = now;
    if (now) {
      lampOn = !lampOn;
      digitalWrite(RELAY, lampOn ? HIGH : LOW);
    }
  }
}
"#,
        &[("C0", tr(lang, "Кнопка включает и выключает лампу через реле", "A button switches the lamp through a relay"))],
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

/// Parts the base circuit did not have, in the order they were added, with
/// the Uno pins they control. The code generator works from this list, so a
/// part a template already drew keeps its own, hand-written code instead of
/// getting a second copy.
pub(crate) struct AddedPart {
    pub part: usize,
    pub pins: Vec<u8>,
}

pub(crate) struct Added {
    pub parts: Vec<AddedPart>,
    /// How many parts the sentence asked for, before the pin budget was
    /// checked. When fewer are placed, the note says so instead of hiding it.
    pub requested: usize,
}

fn pin_num(name: &str) -> u8 {
    name.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(0)
}

/// A shelf below the finished circuit: extras never land on existing parts,
/// and the picture keeps a readable frame.
struct ExtraRow {
    x: f32,
    y: f32,
    h: f32,
}

impl ExtraRow {
    fn below(c: &Circuit) -> Self {
        let b = c.bbox();
        Self { x: b.x.max(2.0), y: b.bottom() + 12.0, h: 0.0 }
    }

    fn take(&mut self, w: f32, h: f32) -> Pt {
        if self.x + w > 158.0 {
            self.x = 2.0;
            self.y += self.h + 12.0;
            self.h = 0.0;
        }
        let at = Pt::new(self.x, self.y);
        self.x += w + 12.0;
        self.h = self.h.max(h);
        at
    }
}

fn free_label(c: &Circuit, prefix: &str) -> String {
    for n in 1u32.. {
        let label = format!("{prefix}{n}");
        if !c.parts().any(|p| p.label == label) {
            return label;
        }
    }
    unreachable!()
}

fn bench_part(c: &mut Circuit, kind: Kind, label: &str, value: &str, at: Pt) -> usize {
    let specs: Vec<PinSpec> = bench_pins(kind, value)
        .iter()
        .map(|(name, dx, dy)| PinSpec {
            name,
            pos: Pt::new(at.x + dx, at.y + dy),
            hole: Hole::OffBoard,
            kind: bench_pin_kind(kind, value, name),
        })
        .collect();
    // A tactile switch has two contacts, each made of two legs. Without the
    // internal links the mount check rightly calls the unused legs dangling.
    let internal = if kind == Kind::Button {
        vec![("1", "2"), ("3", "4")]
    } else {
        Vec::new()
    };
    c.add_part(kind, label, value, at, specs, internal)
}

fn count_kinds(c: &Circuit, kinds: &[Kind]) -> usize {
    c.parts().filter(|p| kinds.contains(&p.kind)).count()
}

/// A loose diode with the terminal names the checks know: `A` for the anode,
/// `K` for the cathode. `bench_pins` numbers its two legs 1 and 2 for the
/// generic axial parts, which is not enough to judge the flyback polarity.
fn bench_diode(c: &mut Circuit, label: &str, value: &str, at: Pt) -> (usize, usize, usize) {
    let a = at;
    let k = Pt::new(at.x + 10.0, at.y);
    let part = c.add_part(
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
    (part, c.pin(part, "A"), c.pin(part, "K"))
}

/// Add every device the sentence named but the plan did not include. This
/// runs on lessons, templates and composed circuits alike, so "пьезопищалка
/// + экран" and "RFID + светодиод" are honoured wherever the wording also
/// matches a ready-made task. A kind that is already on the bench is not
/// added twice: the template's own circuit is the better drawing of it.
fn augment_devices(
    c: &mut Circuit,
    named: &crate::parse::NamedDevices,
    ledger: &mut crate::resources::Resources,
) -> Added {
    let mut added = Added { parts: Vec::new(), requested: 0 };
    let Some(uno) = c.arduino else { return added };
    let gnd = c.pin(uno, "GND2");
    let vcc = c.pin(uno, "5V");
    let mut row = ExtraRow::below(c);
    added.requested = named
        .buzzer
        .saturating_sub(count_kinds(c, &[Kind::Buzzer, Kind::Speaker]))
        + named.relay.saturating_sub(count_kinds(c, &[Kind::Relay]))
        + named.lamp.saturating_sub(count_kinds(c, &[Kind::Lamp]))
        + named.led.saturating_sub(count_kinds(c, &[Kind::Led]))
        + named.rgb.saturating_sub(
            c.parts()
                .filter(|p| p.kind == Kind::Led && p.value.to_lowercase().contains("rgb"))
                .count(),
        )
        + named.servo.saturating_sub(count_kinds(c, &[Kind::Servo]))
        + named.motor.saturating_sub(count_kinds(c, &[Kind::Motor]))
        + named.button.saturating_sub(count_kinds(c, &[Kind::Button]));

    // Extras drive outputs, so they ask for digital pins only: `A0` is an
    // analog input number in a sketch, and pin 0 there would be D0.
    let signals = [
        "D8", "D7", "D6", "D5", "D4", "D3", "D2", "D9", "D10", "D11", "D12", "D13",
    ];
    // `analogWrite` only exists on the timer pins; the servo library and a
    // plain LED work on any digital pin.
    let pwm = ["D9", "D10", "D3", "D5", "D6", "D11"];
    let servo_pins = [
        "D9", "D10", "D3", "D5", "D6", "D11", "D2", "D4", "D7", "D8", "D12", "D13",
    ];

    for _ in 0..named.buzzer.saturating_sub(count_kinds(c, &[Kind::Buzzer, Kind::Speaker])) {
        let Some(pin) = ledger.claim_pin(&signals) else { break };
        let at = row.take(20.0, 18.0);
        let label = free_label(c, "BZ");
        let part = bench_part(c, Kind::Buzzer, &label, "пьезо", at);
        let signal = c.pin(part, "+");
        let ground = c.pin(part, "-");
        c.wire(End::Pin(signal), End::Pin(c.pin(uno, &pin)), NetClass::Signal);
        c.wire(End::Pin(ground), End::Pin(gnd), NetClass::Gnd);
        added.parts.push(AddedPart { part, pins: vec![pin_num(&pin)] });
    }

    // A relay is only useful when it switches something. Every added relay
    // gets its own small siren on COM/NO, so "два реле" has two working
    // contacts instead of a second relay whose terminals hang in the air.
    for _ in 0..named.relay.saturating_sub(count_kinds(c, &[Kind::Relay])) {
        let Some(pin) = ledger.claim_pin(&signals) else { break };
        let at = row.take(44.0, 30.0);
        let label = free_label(c, "K");
        let part = bench_part(c, Kind::Relay, &label, "SRD-05VDC", at);
        let inn = c.pin(part, "IN");
        let vcc_pin = c.pin(part, "VCC");
        let gnd_pin = c.pin(part, "GND");
        c.wire(End::Pin(inn), End::Pin(c.pin(uno, &pin)), NetClass::Signal);
        c.wire(End::Pin(vcc_pin), End::Pin(vcc), NetClass::Vcc);
        c.wire(End::Pin(gnd_pin), End::Pin(gnd), NetClass::Gnd);
        // The load on the contact side keeps COM and NO connected: a relay
        // with an open contact is a part that does nothing.
        let s_label = free_label(c, "HA");
        let siren = bench_part(
            c,
            Kind::Buzzer,
            &s_label,
            "сирена 5 В",
            row.take(22.0, 18.0),
        );
        let contact = c.pin(part, "COM");
        let normally_open = c.pin(part, "NO");
        let plus = c.pin(siren, "+");
        let minus = c.pin(siren, "-");
        c.wire(End::Pin(contact), End::Pin(vcc), NetClass::Vcc);
        c.wire(End::Pin(normally_open), End::Pin(plus), NetClass::Signal);
        c.wire(End::Pin(minus), End::Pin(gnd), NetClass::Gnd);
        added.parts.push(AddedPart { part, pins: vec![pin_num(&pin)] });
    }

    // An extra incandescent lamp is a heavy load, so it gets its own relay
    // and a load on the contact: another bare lamp would be an open circuit.
    for _ in 0..named.lamp.saturating_sub(count_kinds(c, &[Kind::Lamp])) {
        let Some(pin) = ledger.claim_pin(&signals) else { break };
        let mains = c
            .parts()
            .any(|p| p.kind == Kind::Lamp && p.value.contains("220"));
        let at = row.take(64.0, 36.0);
        let k_label = free_label(c, "K");
        let relay = bench_part(c, Kind::Relay, &k_label, "SRD-05VDC", at);
        let h_label = free_label(c, "H");
        let value = if mains { "220 В, 60 Вт" } else { "12 В, 21 Вт" };
        let lamp = bench_part(
            c,
            Kind::Lamp,
            &h_label,
            value,
            Pt::new(at.x + 20.0, at.y + 18.0),
        );
        let inn = c.pin(relay, "IN");
        let vcc_pin = c.pin(relay, "VCC");
        let gnd_pin = c.pin(relay, "GND");
        let contact = c.pin(relay, "COM");
        let normally_open = c.pin(relay, "NO");
        let plus = c.pin(lamp, "+");
        let minus = c.pin(lamp, "-");
        c.wire(End::Pin(inn), End::Pin(c.pin(uno, &pin)), NetClass::Signal);
        c.wire(End::Pin(vcc_pin), End::Pin(vcc), NetClass::Vcc);
        c.wire(End::Pin(gnd_pin), End::Pin(gnd), NetClass::Gnd);
        // The lamp sits between VIN and the normally open contact; the
        // contact closes the path to ground, exactly as in the lamp lesson.
        c.wire(End::Pin(contact), End::Pin(gnd), NetClass::Gnd);
        c.wire(End::Pin(normally_open), End::Pin(minus), NetClass::Signal);
        c.wire(End::Pin(plus), End::Pin(c.pin(uno, "VIN")), NetClass::Vcc);
        added.parts.push(AddedPart { part: relay, pins: vec![pin_num(&pin)] });
    }

    for _ in 0..named.led.saturating_sub(count_kinds(c, &[Kind::Led])) {
        let Some(pin) = ledger.claim_pin(&signals) else { break };
        let at = row.take(30.0, 16.0);
        let r_label = free_label(c, "R");
        let d_label = free_label(c, "D");
        let r = bench_part(c, Kind::Resistor, &r_label, "220 Ом", at);
        let d = bench_part(c, Kind::Led, &d_label, "красный 5 мм", Pt::new(at.x + 14.0, at.y));
        let ra = c.pin(r, "1");
        let rb = c.pin(r, "2");
        let a = c.pin(d, "1");
        let k = c.pin(d, "2");
        c.wire(End::Pin(c.pin(uno, &pin)), End::Pin(ra), NetClass::Signal);
        c.wire(End::Pin(rb), End::Pin(a), NetClass::Signal);
        c.wire(End::Pin(k), End::Pin(gnd), NetClass::Gnd);
        added.parts.push(AddedPart { part: d, pins: vec![pin_num(&pin)] });
    }

    let rgb_present = c
        .parts()
        .filter(|p| p.kind == Kind::Led && p.value.to_lowercase().contains("rgb"))
        .count();
    for _ in 0..named.rgb.saturating_sub(rgb_present) {
        let mut pins = Vec::new();
        for _ in 0..3 {
            let Some(pin) = ledger.claim_pin(&signals) else { break };
            pins.push(pin);
        }
        if pins.len() < 3 {
            break;
        }
        let at = row.take(48.0, 20.0);
        let mut ends = Vec::new();
        for i in 0..3 {
            let label = free_label(c, "R");
            let r = bench_part(
                c,
                Kind::Resistor,
                &label,
                "220 Ом",
                Pt::new(at.x + i as f32 * 13.0, at.y),
            );
            ends.push((c.pin(r, "1"), c.pin(r, "2")));
        }
        let d_label = free_label(c, "D");
        let d = bench_part(c, Kind::Led, &d_label, "RGB общий катод", Pt::new(at.x, at.y + 12.0));
        for (i, (ra, rb)) in ends.iter().enumerate() {
            let colour = ["R", "G", "B"][i];
            c.wire(End::Pin(c.pin(uno, &pins[i])), End::Pin(*ra), NetClass::Signal);
            c.wire(End::Pin(*rb), End::Pin(c.pin(d, colour)), NetClass::Signal);
        }
        c.wire(End::Pin(c.pin(d, "-")), End::Pin(gnd), NetClass::Gnd);
        added.parts.push(AddedPart {
            part: d,
            pins: pins.iter().map(|p| pin_num(p)).collect(),
        });
    }

    for _ in 0..named.servo.saturating_sub(count_kinds(c, &[Kind::Servo])) {
        let Some(pin) = ledger.claim_pin(&servo_pins) else { break };
        let at = row.take(28.0, 28.0);
        let label = free_label(c, "S");
        let part = bench_part(c, Kind::Servo, &label, "SG90", at);
        let s = c.pin(part, "S");
        let plus = c.pin(part, "+");
        let minus = c.pin(part, "-");
        c.wire(End::Pin(s), End::Pin(c.pin(uno, &pin)), NetClass::Signal);
        c.wire(End::Pin(plus), End::Pin(vcc), NetClass::Vcc);
        c.wire(End::Pin(minus), End::Pin(gnd), NetClass::Gnd);
        added.parts.push(AddedPart { part, pins: vec![pin_num(&pin)] });
    }

    for _ in 0..named.motor.saturating_sub(count_kinds(c, &[Kind::Motor])) {
        let Some(pin) = ledger.claim_pin(&pwm) else { break };
        let at = row.take(50.0, 44.0);
        let r1_label = free_label(c, "R");
        let r1 = bench_part(c, Kind::Resistor, &r1_label, "220 Ом", Pt::new(at.x, at.y + 18.0));
        let q_label = free_label(c, "Q");
        let q = bench_part(c, Kind::Nmos, &q_label, "IRLZ44N", Pt::new(at.x + 14.0, at.y + 18.0));
        let r2_label = free_label(c, "R");
        let r2 = bench_part(c, Kind::Resistor, &r2_label, "10 кОм", Pt::new(at.x, at.y + 32.0));
        let m_label = free_label(c, "M");
        let m = bench_part(c, Kind::Motor, &m_label, "5 В DC", Pt::new(at.x + 26.0, at.y));
        let d_label = free_label(c, "D");
        let (_d, da, dk) =
            bench_diode(c, &d_label, "1N4007", Pt::new(at.x + 26.0, at.y - 10.0));
        let (g, s, dr) = (c.pin(q, "G"), c.pin(q, "S"), c.pin(q, "D"));
        let (r1a, r1b) = (c.pin(r1, "1"), c.pin(r1, "2"));
        let (r2a, r2b) = (c.pin(r2, "1"), c.pin(r2, "2"));
        let (mp, mn) = (c.pin(m, "+"), c.pin(m, "-"));
        c.wire(End::Pin(c.pin(uno, &pin)), End::Pin(r1a), NetClass::Signal);
        c.wire(End::Pin(r1b), End::Pin(g), NetClass::Signal);
        c.wire(End::Pin(r2a), End::Pin(g), NetClass::Signal);
        c.wire(End::Pin(r2b), End::Pin(gnd), NetClass::Gnd);
        c.wire(End::Pin(s), End::Pin(gnd), NetClass::Gnd);
        c.wire(End::Pin(dr), End::Pin(mn), NetClass::Signal);
        c.wire(End::Pin(mp), End::Pin(vcc), NetClass::Vcc);
        // The flyback diode: anode on the switched side, cathode on +5 V.
        c.wire(End::Pin(da), End::Pin(mn), NetClass::Signal);
        c.wire(End::Pin(dk), End::Pin(mp), NetClass::Vcc);
        added.parts.push(AddedPart { part: m, pins: vec![pin_num(&pin)] });
    }

    for _ in 0..named.button.saturating_sub(count_kinds(c, &[Kind::Button])) {
        let Some(pin) = ledger.claim_pin(&signals) else { break };
        let at = row.take(22.0, 20.0);
        let label = free_label(c, "B");
        let part = bench_part(c, Kind::Button, &label, "такт 6×6", at);
        c.wire(End::Pin(c.pin(part, "4")), End::Pin(c.pin(uno, &pin)), NetClass::Signal);
        c.wire(End::Pin(c.pin(part, "1")), End::Pin(gnd), NetClass::Gnd);
        added.parts.push(AddedPart { part, pins: vec![pin_num(&pin)] });
    }

    added
}

/// Apply the numbers the user actually said to a generated sketch. Returns a
/// note when a spoken number had to be brought into the range the sketch can
/// safely use.
fn apply_spoken_numbers(code: &mut String, raw: &str) -> Option<L> {
    let sem = crate::semantics::analyse(raw);
    let mut note = None;
    if let Some(presses) = sem.after_presses {
        // The generated step code divides by `STEPS` and by `STEPS - 1`: a
        // spoken 0 or 1 would crash the board, and a byte counter cannot hold
        // more than 255. The plan keeps the spoken number visible, the sketch
        // gets the nearest safe one, and the note says so instead of hiding it.
        if code.contains("STEPS = 6") {
            let steps = presses.clamp(2, 255);
            if steps != presses {
                note = Some(L::new(
                    "Ступеней не может быть меньше 2 или больше 255: скетч делит на число ступеней и хранит счётчик в байте. В скетч записано ближайшее безопасное значение.",
                    "The number of steps must be between 2 and 255: the sketch divides by it and keeps the counter in one byte. The nearest safe value is written into the sketch.",
                ));
            }
            *code = code.replace("STEPS = 6", &format!("STEPS = {steps}"));
        }
    }
    if let Some(ms) = sem.period_ms {
        // Lessons name the same interval differently: the first sketch uses
        // HALF_MS, a sensor loop uses PERIOD_MS. Both must follow the spoken
        // number, or "каждые 200 мс" still blinks twice a second. The period
        // is a uint32_t, so a spoken 70000 ms is written as is.
        set_period_constants(code, ms);
    }
    note
}

fn apply_numbers(built: &mut Built, raw: &str) {
    if let Some(note) = apply_spoken_numbers(&mut built.code, raw) {
        built.notes.push(note);
    }
}

/// Replace the literal of `PERIOD_MS`/`HALF_MS` in generated code without
/// caring which default the lesson or composition used.
fn set_period_constants(code: &mut String, ms: u32) {
    let mut out = String::with_capacity(code.len());
    for (i, line) in code.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let starts = ["const uint32_t PERIOD_MS =", "const uint32_t HALF_MS ="];
        let is_period = starts.iter().any(|s| line.trim_start().starts_with(s));
        if is_period && let (Some(eq), Some(semi)) = (line.find('='), line.find(';')) {
            out.push_str(&line[..=eq]);
            out.push(' ');
            out.push_str(&ms.to_string());
            out.push_str(&line[semi..]);
            continue;
        }
        out.push_str(line);
    }
    if code.ends_with('\n') {
        out.push('\n');
    }
    *code = out;
}

/// Honour the parts the sentence named but the base circuit does not have.
/// Shared by lessons, templates and compositions, so every route adds them.
fn augment_built(built: &mut Built, named: &crate::parse::NamedDevices, lang: Lang) {
    let mut ledger = crate::resources::Resources::new(&built.circuit);
    let added = augment_devices(&mut built.circuit, named, &mut ledger);
    if added.parts.len() < added.requested {
        built.notes.push(L::new(
            "Не все названные детали поместились на плату: свободных цифровых пинов не хватило. Остальные детали оставлены за схемой, их число видно в задаче.",
            "Not every named part fit on the board: the free digital pins ran out. The rest are left out of the circuit; the required count stays visible in the task.",
        ));
    }
    if added.parts.is_empty() {
        return;
    }
    crate::codegen::augment_from_circuit(
        &built.circuit,
        &mut built.code,
        named.hourly,
        lang,
        &added.parts,
    );
    built.notes.push(L::new(
        "Явно названные в задаче детали добавлены к схеме.",
        "Parts named in the sentence were added to the circuit.",
    ));
}

/// Honour the pins the sentence named. A ready-made lesson draws its signal
/// on a default pin; when the sentence says "на пин 13", that signal moves
/// there and the sketch constant follows, so the drawing and the code agree.
///
/// A circuit with no digital signal at all (an I2C clock, for example) has
/// nowhere to put the pin. That is explained in a note instead of reported as
/// a missing part: the user asked for a clock, and an extra dangling wire
/// would be a different circuit.
fn honour_named_pins(built: &mut Built, intent: &Intent) {
    let Some(uno) = built.circuit.arduino else { return };
    for &pin in intent.named.pins.iter() {
        if crate::requirements::pin_wired(&built.circuit, pin) {
            continue;
        }
        let Some(target) = built.circuit.parts[uno]
            .pins
            .iter()
            .copied()
            .find(|&p| built.circuit.pins[p].name == format!("D{pin}"))
        else {
            continue;
        };
        // Prefer the signal that belongs to the part the sentence named:
        // "датчик температуры на пин 13" moves the sensor, not the screen's
        // RS line, and "порт 3 и кнопка" moves the button, not the LED.
        let mut old_pin: Option<(i32, usize, usize, u8)> = None;
        for (idx, wire) in built.circuit.wires.iter().enumerate() {
            if !wire.active || wire.class != NetClass::Signal {
                continue;
            }
            // `end` is the Uno signal pin; `other` is the part it feeds and
            // that part decides whether this is the signal the sentence meant.
            let (end, other) = match (wire.a, wire.b) {
                (End::Pin(a), b) if built.circuit.pins[a].part == uno => (a, b),
                (a, End::Pin(b)) if built.circuit.pins[b].part == uno => (b, a),
                _ => continue,
            };
            let name = built.circuit.pins[end].name;
            let Some(n) = name.strip_prefix('D').and_then(|s| s.parse::<u8>().ok()) else {
                continue;
            };
            // Never take a pin the sentence named for another part: that
            // would move the previous request off its own pin.
            if intent.named.pins.contains(&n) || !(2..=13).contains(&n) {
                continue;
            }
            let score = match other {
                End::Pin(p) => pin_move_score(&built.circuit, &intent.named, p),
                _ => 1,
            };
            if old_pin.as_ref().map_or(true, |best| score > best.0) {
                old_pin = Some((score, idx, end, n));
            }
        }
        // A named sensor that only works on an analog pin has no digital
        // signal to move. Do not borrow an unrelated LED just to satisfy the
        // pin number: explain the free pin instead.
        let named_any = intent.named.led
            + intent.named.lamp
            + intent.named.button
            + intent.named.relay
            + intent.named.buzzer
            + intent.named.motor
            + intent.named.servo
            + intent.named.temperature
            + intent.named.humidity
            + intent.named.soil
            + intent.named.pressure
            + intent.named.light
            + intent.named.range
            + intent.named.gas
            + intent.named.dht11
            + intent.named.dht22
            + usize::from(intent.named.clock)
            > 0;
        if named_any && old_pin.as_ref().is_some_and(|best| best.0 <= 1) {
            old_pin = None;
        }
        let Some((_, wire_idx, old_end, old_num)) = old_pin else {
            built.explained_pins.push(pin);
            built.notes.push(L::new(
                "Пин, названный в задаче, оставлен свободным: у её деталей нет цифрового вывода, который можно на него перенести.",
                "The pin named in the task stays free: its parts have no digital line that could move to it.",
            ));
            continue;
        };
        if built.circuit.wires[wire_idx].a == End::Pin(old_end) {
            built.circuit.wires[wire_idx].a = End::Pin(target);
        } else {
            built.circuit.wires[wire_idx].b = End::Pin(target);
        }
        built.circuit.touch();
        rewrite_pin(&mut built.code, old_num, pin);
        built.summary = built
            .summary
            .replace(&format!("D{old_num}"), &format!("D{pin}"));
        built.notes.push(L::new(
            "Вывод, названный в задаче, стоит в схеме: сигнал и константа скетча перенесены на него.",
            "The pin named in the task is in the circuit: the signal and the sketch constant moved to it.",
        ));
    }
}

/// How well a signal on `end` answers the part the sentence named. A button
/// wire beats an LED wire for "порт 3 и кнопка"; a sensor beats a screen for
/// "датчик температуры на пин 13 и дисплей". Every signal keeps a base
/// score, so a phrase that only names a pin still moves something real.
fn pin_move_score(
    c: &Circuit,
    named: &crate::parse::NamedDevices,
    end: usize,
) -> i32 {
    let part = &c.parts[c.pins[end].part];
    let kind = part.kind;
    let mut score = 1;
    if named.button > 0 && kind == Kind::Button {
        score = score.max(100);
    }
    if named.relay > 0 && kind == Kind::Relay {
        score = score.max(100);
    }
    if named.buzzer > 0 && matches!(kind, Kind::Buzzer | Kind::Speaker) {
        score = score.max(90);
    }
    if named.servo > 0 && kind == Kind::Servo {
        score = score.max(90);
    }
    // An LED sits on the breadboard strip after its series resistor, so the
    // Uno wire often reaches a resistor rather than the diode itself.
    let led_like = kind == Kind::Led || kind == Kind::Resistor;
    if named.led > 0 && led_like {
        score = score.max(80);
    }
    if named.lamp > 0 && matches!(kind, Kind::Lamp | Kind::Relay) {
        score = score.max(80);
    }
    if named.motor > 0 && leads_to_kind(c, c.pins[end].part, Kind::Motor) {
        score = score.max(80);
    }
    let sensors = named.temperature
        + named.humidity
        + named.soil
        + named.pressure
        + named.light
        + named.range
        + named.gas
        + named.dht11
        + named.dht22
        + usize::from(named.clock);
    if sensors > 0 && is_sensor_part(part) {
        score = score.max(90);
    }
    score
}

/// Does the signal chain from `start` reach a part of the wanted kind? A pin
/// reaches an LED through its series resistor, and a motor through a gate
/// resistor and a transistor, so the first part the Uno touches is not always
/// the part the sentence named. Ground and supply rails are not followed;
/// only signal wires join one stage to the next.
fn leads_to_kind(c: &Circuit, start: usize, want: Kind) -> bool {
    let mut seen = Vec::new();
    let mut queue = vec![(start, 0usize)];
    while let Some((part, depth)) = queue.pop() {
        if c.parts[part].kind == want {
            return true;
        }
        if depth >= 3 || seen.contains(&part) {
            continue;
        }
        seen.push(part);
        for wire in c.wires().filter(|w| w.class == NetClass::Signal) {
            if let (End::Pin(a), End::Pin(b)) = (wire.a, wire.b) {
                let (pa, pb) = (c.pins[a].part, c.pins[b].part);
                if pa == part && !seen.contains(&pb) {
                    queue.push((pb, depth + 1));
                }
                if pb == part && !seen.contains(&pa) {
                    queue.push((pa, depth + 1));
                }
            }
        }
    }
    false
}

/// Sensors are their own small boards, LDRs and thermistors are two-legged
/// parts; a display on the same bus is not the sensor the sentence named.
fn is_sensor_part(part: &Part) -> bool {
    if matches!(
        part.kind,
        Kind::Ldr | Kind::Thermistor | Kind::Tmp36
    ) {
        return true;
    }
    if part.kind != Kind::Module {
        return false;
    }
    !matches!(
        crate::modules::find(&part.value).map(|m| m.look),
        Some(
            crate::modules::Look::Lcd
                | crate::modules::Look::Display
                | crate::modules::Look::Segments
                | crate::modules::Look::Matrix
                | crate::modules::Look::Panel
        )
    )
}

/// Rename a pin in the generated sketch: a named constant such as `LED` or
/// `ONE_WIRE`, either flavour of `#define`, or a literal first argument of
/// `pinMode`/`digitalWrite`. Without this the picture and the sketch would
/// disagree about which pin carries the signal.
fn rewrite_pin(code: &mut String, old: u8, new: u8) -> bool {
    let snapshot = code.clone();
    let mut changed = false;
    let mut out = String::with_capacity(code.len());
    for (i, line) in code.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&rewrite_pin_line(line, old, new, &snapshot, &mut changed));
    }
    if changed {
        *code = out;
    }
    changed
}

fn rewrite_pin_line(line: &str, old: u8, new: u8, code: &str, changed: &mut bool) -> String {
    let mut out = line.to_string();
    let mut edits: Vec<(usize, usize)> = Vec::new();
    if let Some(name) = pin_constant(line)
        && (pin_like(&name) || uses_as_pin(code, &name))
        && let Some(span) = pin_value_span(line, old)
    {
        edits.push(span);
    }
    for call in [
        "pinMode",
        "digitalWrite",
        "digitalRead",
        "analogWrite",
        "analogRead",
        "attach",
    ] {
        let needle = format!("{call}(");
        let mut from = 0;
        while let Some(at) = line[from..].find(&needle) {
            let start = from + at + needle.len();
            let after = &line[start..];
            let trimmed = after.trim_start();
            let lead = after.len() - trimmed.len();
            let digits: String = trimmed
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if !digits.is_empty() && digits.parse::<u8>() == Ok(old) {
                let end = start + lead + digits.len();
                let next = line[end..].chars().next().unwrap_or(',');
                if next == ',' || next == ')' {
                    edits.push((start + lead, end));
                }
            }
            from = start + 1;
            if from >= line.len() {
                break;
            }
        }
    }
    edits.sort_unstable_by_key(|e| e.0);
    edits.dedup();
    for (start, end) in edits.into_iter().rev() {
        out.replace_range(start..end, &new.to_string());
        *changed = true;
    }
    out
}

/// The variable being declared on a `const` or `#define` line.
fn pin_constant(line: &str) -> Option<String> {
    let text = line.trim_start();
    if let Some(rest) = text.strip_prefix("#define") {
        let rest = rest.trim_start();
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        return (!name.is_empty()).then_some(name);
    }
    if text.starts_with("const ") {
        let eq = text.find('=')?;
        let left = text[..eq].trim_end();
        let name: String = left
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        return (!name.is_empty()).then_some(name);
    }
    None
}

/// The span of an integer literal equal to `old` in a declaration.
fn pin_value_span(line: &str, old: u8) -> Option<(usize, usize)> {
    let text = line.trim_start();
    let mut start = if text.starts_with("#define") {
        let mut parts = text.split_whitespace();
        parts.next();
        let name = parts.next()?;
        line.find(name)? + name.len()
    } else {
        line.find('=')? + 1
    };
    let bytes = line.as_bytes();
    while start < bytes.len() && !bytes[start].is_ascii_digit() {
        if bytes[start] == b';' || bytes[start] == b'\n' {
            return None;
        }
        start += 1;
    }
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == start || line[start..end].parse::<u8>().ok() != Some(old) {
        return None;
    }
    if end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        return None;
    }
    Some((start, end))
}

/// Names that hold a pin number even when the sketch never calls `pinMode`
/// directly (OneWire and some libraries take the number as an argument).
fn pin_like(name: &str) -> bool {
    name.contains("PIN")
        || name.ends_with("LED")
        || name.contains("WIRE")
        || matches!(
            name,
            "BTN"
                | "BUTTON"
                | "RELAY"
                | "BUZZER"
                | "SPEAKER"
                | "LAMP"
                | "MOTOR"
                | "TRIG"
                | "ECHO"
                | "SENSE"
                | "DATA"
                | "IR_LED"
        )
}

/// Is `name` the first argument of a pin call anywhere in the sketch?
fn uses_as_pin(code: &str, name: &str) -> bool {
    [
        "pinMode(",
        "digitalWrite(",
        "digitalRead(",
        "analogWrite(",
        "analogRead(",
        "attach(",
    ]
    .iter()
    .any(|call| {
        let mut rest = code;
        while let Some(at) = rest.find(call) {
            let arg = rest[at + call.len()..].trim_start();
            let ident: String = arg
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if ident == name {
                return true;
            }
            rest = &rest[at + call.len()..];
        }
        false
    })
}

// new build body, spliced into templates.rs
pub fn build(intent: &Intent, choice: usize, lang: Lang) -> Option<Built> {
    let mut built = build_route(intent, choice, lang)?;
    honour_named_pins(&mut built, intent);
    Some(built)
}

fn build_route(intent: &Intent, choice: usize, lang: Lang) -> Option<Built> {
    if let Some(screens) = intent.text_screens {
        let plan = text_screen_plan(&intent.raw, screens, lang);
        let choice = choice.min(plan.choices.len().saturating_sub(1));
        let (circuit, code, summary, notes) = text_screen_build(lang, screens, choice);
        let mut built = Built {
            tid: Tid::TextScreens,
            title: plan.title_text(lang),
            circuit,
            code,
            choice,
            summary: summary.get(lang).to_string(),
            notes,
            explained_pins: Vec::new(),
        };
        augment_built(&mut built, &intent.named, lang);
        apply_numbers(&mut built, &intent.raw);
        return Some(built);
    }
    if let Some(recipe) = intent.recipe {
        let mut built = crate::compose::build(recipe, choice, lang)?;
        augment_built(&mut built, &intent.named, lang);
        apply_numbers(&mut built, &intent.raw);
        return Some(built);
    }
    if let Some(i) = intent.lesson {
        let screens = intent.named.screens;
        let plan = display_lesson_plan(i, screens, lang);
        let choice = if plan.choices.is_empty() { 0 } else { choice.min(plan.choices.len().saturating_sub(1)) };
        let (circuit, code, summary, notes) =
            lesson_build(lang, i, screens, choice);
        let mut built = Built {
            tid: Tid::Lesson(i),
            title: plan.title_text(lang),
            circuit,
            code,
            choice: 0,
            summary: summary.get(lang).to_string(),
            notes,
            explained_pins: Vec::new(),
        };
        augment_built(&mut built, &intent.named, lang);
        apply_numbers(&mut built, &intent.raw);
        return Some(built);
    }
    // A screen with only buttons is its own honest circuit: count the presses
    // on the display. The old fallback drew an LED the sentence never named.
    if intent.task.wants_buttons_screen() {
        let screens = intent.named.screens.unwrap_or(crate::parse::Screens {
            kind: ScreenKind::Any,
            count: 1,
            matrix: false,
            segments: false,
        });
        let plan = buttons_screen_plan(screens, lang);
        let choice = choice.min(plan.choices.len().saturating_sub(1));
        let (circuit, code, summary, notes) =
            buttons_screen_build(lang, screens, intent.named.button, choice);
        let mut built = Built {
            tid: Tid::ButtonsScreen,
            title: plan.title_text(lang),
            circuit,
            code,
            choice,
            summary: summary.get(lang).to_string(),
            notes,
            explained_pins: Vec::new(),
        };
        augment_built(&mut built, &intent.named, lang);
        return Some(built);
    }
    // Most routes left here are LEDs, buzzers and buttons and cannot host a
    // screen: drawing one of those for "три экрана и кнопка" is the "similar
    // circuit" this checklist exists to catch. The access scene is the
    // exception: a screen named there is part of the lock, so it is drawn.
    let tid = route(intent.load, intent.input)?;
    if intent.named.screens.is_some()
        && !intent.named.phone
        && !matches!(tid, Tid::Rfid)
    {
        return None;
    }
    let plan = plan(intent, lang)?;
    let choice = choice.min(plan.choices.len().saturating_sub(1));
    let mut notes = Vec::new();
    if intent.presses {
        notes.push(L::new(
            "Нажатия считаются по фронту с антидребезгом 30 мс — удержание не добавляет ступеней.",
            "Presses are counted on the falling edge with 30 ms debounce; holding adds no steps.",
        ));
    }
    let (circuit, code, summary, extra) = match tid {
        Tid::MotorSteps => motor_steps(lang, choice),
        Tid::MotorPot => motor_pot(lang, choice),
        Tid::MotorTest => motor_test(lang, choice, &intent.raw),
        Tid::MotorDriver => motor_driver(lang, choice),
        Tid::Stepper => stepper(lang, choice),
        Tid::LedToggle => led_toggle(lang, choice),
        Tid::LedBlink => led_blink(lang, choice),
        Tid::NightLight => night_light(lang, choice),
        Tid::Lamp => lamp(lang, choice, &intent.raw),
        Tid::ServoSweep => servo_sweep(lang, choice),
        Tid::ServoPot => servo_sweep(lang, if choice == 1 { 1 } else { 2 }),
        Tid::BuzzerTone => buzzer_tone(lang, choice),
        Tid::Rfid => rfid(lang, choice, &intent.raw),
        Tid::Radio433 => radio433(lang, choice, &intent.raw),
        Tid::RadioNrf => radio_nrf(lang, choice),
        Tid::Bluetooth => bluetooth(lang, choice),
        Tid::Gsm => gsm(lang, choice),
        Tid::Ethernet => ethernet(lang, choice),
        Tid::Composed => return crate::compose::build(intent.recipe?, choice, lang),
        Tid::Lesson(_) | Tid::TextScreens | Tid::ButtonsScreen => return None,
    };
    let mut built = Built {
        tid,
        title: plan.title_text(lang),
        circuit,
        code,
        choice,
        summary: summary.get(lang).to_string(),
        notes,
        explained_pins: Vec::new(),
    };
    built.notes.extend(extra);
    // Named parts the template did not draw are added here, so a sentence
    // like "RFID + светодиод" keeps both halves.
    augment_built(&mut built, &intent.named, lang);
    apply_numbers(&mut built, &intent.raw);
    Some(built)
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

/// An incandescent lamp. It is a heavy load, so a pin never drives it
/// directly: choice 0 puts a logic-level MOSFET in the low side and dims with
/// PWM, choice 1 switches a relay for a lamp that only needs on and off.
fn lamp(lang: Lang, choice: usize, raw: &str) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let text = crate::words::normalize(raw);
    let knob = text.contains("потенциометр")
        || text.contains("регулятор")
        || text.contains("крутилк")
        || text.contains("ручк");
    let buttons = text.contains("кнопк") || text.contains("нажат");
    let mains = text.contains("220") || text.contains("сеть") || text.contains("симистор");
    // A mains lamp is switched by the relay only: the 55 V MOSFET is not
    // safe on 220 V, so any chosen index maps to the relay branch.
    let choice = if mains { 1 } else { choice };
    match choice {
        1 => {
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(6.0, 76.0));
            let lamp = b.bench(Kind::Lamp, "H1", "220 В, 60 Вт", Pt::new(46.0, 78.0));
            let btn = b.button("B1", 4, Row::E);
            let gnd = b.gnd(24.0);
            b.w(b.uno_pin("D2"), btn[3], NetClass::Signal);
            b.w(btn[0], gnd, NetClass::Gnd);
            b.w(b.uno_pin("D7"), rl[0], NetClass::Signal);
            b.w(rl[1], b.vcc(16.0), NetClass::Vcc);
            b.w(rl[2], b.gnd(22.0), NetClass::Gnd);
            b.w(lamp[1], b.uno_pin("VIN"), NetClass::Vcc);
            b.w(lamp[0], rl[4], NetClass::Signal);
            b.w(rl[3], b.gnd(30.0), NetClass::Gnd);
            feeds(&mut b);
            (
                b.finish(),
                lamp_relay_sketch(lang),
                L::new(
                    "Кнопка на D2 включает реле K1 на D7; контакт замыкает цепь лампы.",
                    "A button on D2 toggles the K1 relay on D7; its contact closes the lamp circuit.",
                ),
                vec![
                    L::new(
                        "Реле даёт только вкл/выкл: яркость им не меняют.",
                        "A relay only switches on and off: it cannot dim.",
                    ),
                    L::new(
                        "Для 220 В контакт реле ставят в разрыв фазного провода, а лампу питает сеть. Макетка и плата остаются под 5 В.",
                        "For 220 V the relay contact goes in series with the live wire and the lamp runs from the mains. The breadboard and the board stay at 5 V.",
                    ),
                ],
            )
        }
        _ => {
            let lamp = b.bench(Kind::Lamp, "H1", "12 В, 21 Вт", Pt::new(8.0, 58.0));
            let (r1a, r1b) = b.res("R1", "1 кОм", 21, Row::A, 3);
            let (r2a, r2b) = b.res("R2", "10 кОм", 24, Row::A, 3);
            let q = b.to92(Kind::Nmos, "Q1", "IRLZ44N", 21, Row::E, ["G", "S", "D"]);
            let gnd = b.gnd(32.0);
            b.w(b.uno_pin("D9"), r1a, NetClass::Signal);
            b.w(r1b, q[0], NetClass::Signal);
            b.w(q[0], r2a, NetClass::Signal);
            b.w(r2b, gnd, NetClass::Gnd);
            b.w(q[1], gnd, NetClass::Gnd);
            b.w(lamp[1], b.uno_pin("VIN"), NetClass::Vcc);
            b.w(lamp[0], q[2], NetClass::Signal);
            let (code, summary, notes, input) = if knob {
                let pot = b.bench(Kind::Pot, "RV1", "10 кОм", Pt::new(8.0, 84.0));
                let vcc = b.vcc(82.0);
                b.w(pot[0], vcc, NetClass::Vcc);
                b.w(pot[2], b.gnd(78.0), NetClass::Gnd);
                b.w(pot[1], b.uno_pin("A0"), NetClass::Analog);
                (
                    lamp_pot_sketch(lang),
                    L::new(
                        "Ручка задаёт ШИМ на затворе IRLZ44N, лампа 12 В светит ярче или тусклее.",
                        "The knob sets PWM at the IRLZ44N gate, so the 12 V lamp dims and brightens.",
                    ),
                    vec![L::new(
                        "Лампа — резистивная нагрузка, обратный диод ей не нужен.",
                        "A lamp is a resistive load, so it needs no flyback diode.",
                    )],
                    Some("A0"),
                )
            } else if buttons {
                let b1 = b.button("B1", 4, Row::E);
                let b2 = b.button("B2", 10, Row::E);
                let g = b.gnd(16.0);
                b.w(b.uno_pin("D2"), b1[3], NetClass::Signal);
                b.w(b1[0], g, NetClass::Gnd);
                b.w(b.uno_pin("D3"), b2[3], NetClass::Signal);
                b.w(b2[0], g, NetClass::Gnd);
                (
                    lamp_buttons_sketch(lang),
                    L::new(
                        "Кнопки D2 и D3 прибавляют и убавляют скважность ШИМ на затворе.",
                        "Buttons on D2 and D3 raise and lower the PWM duty at the gate.",
                    ),
                    vec![L::new(
                        "Шаг 25 из 255: двадцать нажатий от нуля до полной яркости.",
                        "A step of 25 out of 255: twenty presses from off to full brightness.",
                    )],
                    Some("D2, D3"),
                )
            } else {
                (
                    lamp_ramp_sketch(lang),
                    L::new(
                        "ШИМ на D9 через IRLZ44N плавно разжигает и гасит лампу 12 В.",
                        "PWM on D9 through the IRLZ44N fades the 12 V lamp up and down.",
                    ),
                    vec![L::new(
                        "12 В подаётся на VIN отдельным блоком: плата даёт только сигнал затвора.",
                        "The 12 V comes to VIN from a separate supply: the board only provides the gate signal.",
                    )],
                    None,
                )
            };
            feeds(&mut b);
            let mut summary = summary;
            if mains {
                summary = L::new(
                    "Лампа 220 В питается от сети: макетка коммутирует только затвор MOSFET.",
                    "A 220 V lamp runs from the mains: the breadboard only drives the MOSFET gate.",
                );
            }
            let mut notes = notes;
            notes.push(L::new(
                "IRLZ44N открывается логическим уровнем 5 В и держит ток до 10 А.",
                "The IRLZ44N turns on at a 5 V logic level and carries up to 10 A.",
            ));
            if input.is_some() {
                notes.push(L::new(
                    "Питание лампы — отдельный блок 12 В; земля блока соединена с землёй платы.",
                    "The lamp has its own 12 V supply; its ground is tied to the board ground.",
                ));
            }
            (b.finish(), code, summary, notes)
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

/// The same electrical value written two ways: "220 Ω" and "220 Ом", "10 кОм"
/// and "10 кΩ". The bill of materials groups by this key so one resistor does
/// not appear as two lines with different letters.
fn value_key(kind: Kind, value: &str) -> String {
    if !matches!(kind, Kind::Resistor | Kind::Pot | Kind::Thermistor) {
        return value.to_string();
    }
    value
        .replace(" кОм", " кΩ")
        .replace(" МОм", " МΩ")
        .replace(" Ом", " Ω")
}

/// Bill of materials, grouped by component kind and value.
pub fn bom(c: &Circuit, lang: Lang) -> Vec<(String, String, usize)> {
    let mut out: Vec<(String, String, usize)> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for p in &c.parts {
        let kind = kind_name(p.kind, lang).to_string();
        let key = format!("{}\u{1}{}", kind, value_key(p.kind, &p.value));
        if let Some(i) = keys.iter().position(|k| *k == key) {
            out[i].2 += 1;
        } else {
            keys.push(key);
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
        let value = value_key(p.kind, &p.value);
        let component = if value.is_empty() {
            kind.to_string()
        } else {
            format!("{kind} {value}")
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
        Kind::Lamp => tr(lang, "лампа накаливания", "incandescent lamp"),
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
fn motor_test(lang: Lang, choice: usize, raw: &str) -> (Circuit, String, L, Vec<L>) {
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
                motor_test_sketch(lang, 7, raw),
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
                motor_test_sketch(lang, 9, raw),
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
                motor_test_sketch(lang, 9, raw),
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

fn rfid(lang: Lang, choice: usize, raw: &str) -> (Circuit, String, L, Vec<L>) {
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
            let spec = access_spec(raw);
            let module = match spec.screen {
                ScreenKind::Lcd => "LCD1602",
                ScreenKind::Lcd2004 => "LCD2004",
                _ => "OLED 0.96",
            };
            let display = b.breakout("DS1", module, Pt::new(2.0, 122.0));
            b.w(b.uno_pin("5V"), b.mp(display, "VCC"), NetClass::Vcc);
            b.w(b.mp(display, "GND"), b.gnd(78.0), NetClass::Gnd);
            b.w(b.uno_pin("A4"), b.mp(display, "SDA"), NetClass::Signal);
            b.w(b.uno_pin("A5"), b.mp(display, "SCL"), NetClass::Signal);
            // A wide LCD2004 needs the relay and the siren to move right.
            let relay_x = match spec.screen {
                ScreenKind::Lcd2004 => 104.0,
                ScreenKind::Lcd => 66.0,
                _ => 44.0,
            };
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(relay_x, 122.0));
            b.w(b.uno_pin("D8"), rl[0], NetClass::Signal);
            b.w(rl[1], b.vcc(82.0), NetClass::Vcc);
            b.w(rl[2], gnd, NetClass::Gnd);
            let btn = b.button("B1", 16, Row::E);
            b.w(b.uno_pin("D2"), btn[3], NetClass::Signal);
            b.w(btn[0], gnd, NetClass::Gnd);
            if spec.siren {
                let bz = b.bench(Kind::Buzzer, "BZ1", "пьезо", Pt::new(relay_x + 30.0, 122.0));
                b.w(b.uno_pin("D7"), bz[1], NetClass::Signal);
                b.w(bz[0], gnd, NetClass::Gnd);
            }
            let mut notes = vec![
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
            ];
            if spec.siren {
                notes.push(L::new(
                    "Сирена на D7 звучит, когда карта не найдена в таблице.",
                    "The siren on D7 sounds when the card is not in the table.",
                ));
            }
            match spec.screen {
                ScreenKind::Lcd => notes.push(L::new(
                    "LCD1602 по I2C (A4/A5) показывает статус и число карт; считыватель остаётся на SPI.",
                    "The I2C LCD1602 (A4/A5) shows the status and the card count; the reader stays on SPI.",
                )),
                ScreenKind::Lcd2004 => notes.push(L::new(
                    "LCD2004 по I2C (A4/A5) показывает статус, число карт и UID; считыватель остаётся на SPI.",
                    "The I2C LCD2004 (A4/A5) shows the status, the card count and the UID; the reader stays on SPI.",
                )),
                _ => notes.push(L::new(
                    "OLED 128×64 по I2C (A4/A5) показывает статус, число карт и UID.",
                    "The I2C OLED 128×64 (A4/A5) shows the status, the card count and the UID.",
                )),
            }
            (
                b.finish(),
                access_sketch(lang, &spec),
                L::new(
                    "Таблица карт живёт в EEPROM, кнопка пишет карту, замок открывается знакомой картой.",
                    "The card table lives in EEPROM, a button saves a card, and a known card opens the lock.",
                ),
                notes,
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

/// Does the task ask for a sensor struct sent from one board to another?
/// "Структура" or a pair of temperature and humidity words names the DHT
/// link: it is not the one-board test pair that only sends '1' and '0'.
fn radio433_link(raw: &str) -> bool {
    let text = crate::words::normalize(raw);
    // A link that carries a reading needs the sensor on the board; a plain
    // pair of radios does not. "Передать температуру по радио" used to build
    // a pair with nothing to measure.
    let sensor = text.contains("температур")
        || text.contains("влажност")
        || text.contains("dht")
        || text.contains("ds18b20");
    let link = text.contains("радио")
        || text.contains("433")
        || text.contains("передатчик")
        || text.contains("приемник");
    text.contains("структур") || (sensor && link)
}

fn radio433_plan(lang: Lang, raw: &str) -> (L, Vec<Choice>, usize) {
    if radio433_link(raw) {
        return (
            L::new(
                "Радио 433 МГц: структура между двумя платами",
                "433 MHz radio: a struct between two boards",
            ),
            vec![quick(
                lang,
                L::new(
                    "DHT22 + FS1000A / XY-MK-5V",
                    "DHT22 + FS1000A / XY-MK-5V",
                ),
                L::new(
                    "Передатчик с DHT22 раз в 2 секунды шлёт номер, температуру и влажность структурой; вторая плата принимает и печатает.",
                    "A transmitter with a DHT22 sends its number, temperature and humidity as a struct every 2 seconds; the second board receives and prints them.",
                ),
                "30 мА",
                "низкая",
                3,
            )],
            0,
        );
    }
    (
        L::new(
            "Радио 433 МГц: передатчик и приёмник",
            "433 MHz radio: transmitter and receiver",
        ),
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
    )
}

fn radio433(lang: Lang, choice: usize, raw: &str) -> (Circuit, String, L, Vec<L>) {
    if radio433_link(raw) {
        return radio433_link_build(lang);
    }
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

/// One board of a two-board link: the transmitter carries the DHT22 and the
/// FS1000A, the receiver is the second board with an XY-MK-5V. The sketch
/// holds both roles, so the single drawing is the half with the sensor.
fn radio433_link_build(lang: Lang) -> (Circuit, String, L, Vec<L>) {
    let mut b = B::new(lang);
    let tx_gnd = b.gnd(16.0);
    let tx_vcc = b.vcc(20.0);
    let dht_gnd = b.gnd(52.0);
    let dht_vcc = b.vcc(56.0);
    feeds(&mut b);
    // Both modules are turned over: their header then faces the rails and the
    // board above, so a cable leaves the header instead of walking round a
    // thirty-millimetre body first. Each module keeps its own pair of rail
    // holes, so no cable runs past the other module.
    let tx = b.breakout("U2", "FS1000A", Pt::new(24.0, 94.0));
    b.c.rotate_part(tx, 2);
    // The sensor sits two millimetres lower, so its two cables run beside the
    // transmitter's instead of on top of them.
    let dht = b.breakout("U3", "DHT22", Pt::new(60.0, 98.0));
    b.c.rotate_part(dht, 2);
    b.w(b.uno_pin("A1"), b.mp(tx, "DATA"), NetClass::Signal);
    b.w(b.mp(tx, "VCC"), tx_vcc, NetClass::Vcc);
    b.w(b.mp(tx, "GND"), tx_gnd, NetClass::Gnd);
    b.w(b.uno_pin("A2"), b.mp(dht, "DATA"), NetClass::Signal);
    b.w(b.mp(dht, "VCC"), dht_vcc, NetClass::Vcc);
    b.w(b.mp(dht, "GND"), dht_gnd, NetClass::Gnd);
    (
        b.finish(),
        radio433_link_sketch(lang),
        L::new(
            "Плата передатчика: DHT22 на A2 и FS1000A на A1 (цифровые 16 и 15). Вторая плата — тот же скетч с SENDER 0 и XY-MK-5V на D2.",
            "The transmitter board: a DHT22 on A2 and an FS1000A on A1 (digital 16 and 15). The second board runs the same sketch with SENDER 0 and an XY-MK-5V on D2.",
        ),
        vec![
            L::new(
                "Обе платы прошиваются одним скетчем: у передатчика SENDER 1, у приёмника SENDER 0.",
                "Flash the same sketch into both boards: SENDER 1 on the transmitter, SENDER 0 on the receiver.",
            ),
            L::new(
                "Структура Reading уходит в эфир 32 битами: номер 8, температура 12 (со смещением 400), влажность 12.",
                "The Reading struct goes on air as 32 bits: an 8-bit number, 12-bit temperature (offset by 400) and 12-bit humidity.",
            ),
            L::new(
                "DHT22 опрашивают не чаще раза в 2 секунды, поэтому период уже 2000 мс; антенна 17 см нужна обоим модулям.",
                "A DHT22 is read at most every 2 seconds, so the period is already 2000 ms; both modules need a 17 cm antenna.",
            ),
        ],
    )
}

fn radio433_link_sketch(lang: Lang) -> String {
    format!(
        r#"// {C0}
// Прошей этот скетч в обе платы: у передатчика SENDER 1, у приёмника SENDER 0.
#include <RCSwitch.h>
#include <DHT.h>

#define SENDER 1              // 1 — плата с передатчиком, 0 — плата с приёмником
const uint32_t PERIOD_MS = 2000;

// Структура, которой обмениваются платы.
struct Reading {{
  uint8_t id;         // номер передатчика
  int16_t temp10;     // температура в десятых долях, °C
  uint16_t hum10;     // влажность в десятых долях, %
}};

RCSwitch radio = RCSwitch();
#if SENDER
const uint8_t SENDER_ID = 1;  // номер этой платы в эфире
const uint8_t TX_PIN = A1;    // FS1000A DATA, цифровой 15
const uint8_t DHT_PIN = A2;   // DHT22 DATA, цифровой 16
DHT dht(DHT_PIN, DHT22);
#else
const uint8_t RX_PIN = 2;     // XY-MK-5V DATA, прерывание 0
#endif

void setup() {{
  Serial.begin(9600);
#if SENDER
  radio.enableTransmit(TX_PIN);
  dht.begin();
#else
  radio.enableReceive(digitalPinToInterrupt(RX_PIN));
#endif
}}

#if SENDER
void loop() {{
  static uint32_t last = 0;
  if (millis() - last < PERIOD_MS) return;
  last = millis();
  float t = dht.readTemperature();
  float h = dht.readHumidity();
  if (isnan(t) || isnan(h)) return;
  Reading r;
  r.id = SENDER_ID;
  r.temp10 = (int16_t)(t * 10.0 + (t < 0 ? -0.5 : 0.5));
  r.hum10 = (uint16_t)(h * 10.0 + 0.5);
  // RCSwitch шлёт 32 бита: номер, температура со смещением 400, влажность.
  uint32_t payload = ((uint32_t)r.id << 24)
                   | ((uint32_t)(r.temp10 + 400) << 12)
                   | (r.hum10 & 0x0FFF);
  radio.send(payload, 32);
  Serial.print(F("send "));
  Serial.println(payload);
}}
#else
void loop() {{
  if (!radio.available()) return;
  Reading r;
  uint32_t payload = radio.getReceivedValue();
  r.id = payload >> 24;
  r.temp10 = (int16_t)((payload >> 12) & 0x0FFF) - 400;
  r.hum10 = payload & 0x0FFF;
  Serial.print(F("id "));
  Serial.print(r.id);
  Serial.print(F(" t "));
  Serial.print(r.temp10 / 10.0);
  Serial.print(F(" C h "));
  Serial.println(r.hum10 / 10.0);
  radio.resetAvailable();
}}
#endif
"#,
        C0 = tr(
            lang,
            "Передатчик с DHT22 шлёт структуру раз в 2 секунды, приёмник печатает её поля",
            "A DHT22 transmitter sends a struct every 2 seconds; the receiver prints its fields"
        ),
    )
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
    // The phone's '1' and '0' switch a visible LED, so the sketch's output
    // pin exists on the drawing instead of being a bare number.
    let (r3a, _r3b) = b.res("R3", "220 Ом", 24, Row::A, 3);
    let (_led_a, led_k) = b.led("D1", "красный 5 мм", 27, Row::A);
    b.w(b.uno_pin("D13"), r3a, NetClass::Signal);
    b.w(led_k, b.gnd(82.0), NetClass::Gnd);
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

fn motor_test_sketch(lang: Lang, pin: u8, raw: &str) -> String {
    // "Полив раз в полчаса" is not a switch test: the pump must run on a
    // schedule, or the plants are flooded once and the sketch lies about it.
    if let Some(ms) = crate::semantics::analyse(raw).period_ms {
        return fill(
            r#"// <<C0>>
const uint8_t MOTOR = <<PIN>>;
const uint32_t PERIOD_MS = <<PERIOD>>;   // как часто включать
const uint16_t RUN_MS = 5000;           // сколько держать включённым

uint32_t last = 0;
uint32_t started = 0;
bool running = false;

void setup() {
  pinMode(MOTOR, OUTPUT);
  digitalWrite(MOTOR, LOW);
}

void loop() {
  uint32_t now = millis();
  if (!running && now - last >= PERIOD_MS) {
    running = true;
    started = now;
    last = now;
    digitalWrite(MOTOR, HIGH);
  }
  if (running && now - started >= RUN_MS) {
    running = false;
    digitalWrite(MOTOR, LOW);
  }
}
"#,
            &[
                ("C0", tr(lang, "Полив по расписанию: насос включается на 5 секунд", "Scheduled watering: the pump runs for five seconds")),
                ("PIN", &pin.to_string()),
                ("PERIOD", &ms.to_string()),
            ],
        );
    }
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
    // Only the relay lock owns pin 8. The template used to drive it as an
    // output unconditionally, which turned an augmented button on D8 into an
    // output and made it unreadable.
    vars.push((
        "SETUP",
        if choice == 2 {
            "  pinMode(LOCK, OUTPUT);\n"
        } else {
            ""
        },
    ));
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
<<SETUP>>}

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

fn access_sketch(lang: Lang, spec: &AccessSpec) -> String {
    let (display_includes, display_object, display_setup, show_body) = match spec.screen {
        ScreenKind::Lcd => (
            "#include <LiquidCrystal_I2C.h>\n",
            "LiquidCrystal_I2C lcd(0x27, 16, 2);\n",
            "  Wire.begin();\n  lcd.init();\n  lcd.backlight();\n",
            concat!(
                "  lcd.clear();\n",
                "  lcd.setCursor(0, 0);\n",
                "  lcd.print(line);\n",
                "  lcd.setCursor(0, 1);\n",
                "  lcd.print(F(\"cards \"));\n",
                "  lcd.print(stored());\n",
                "  lcd.print('/');\n",
                "  lcd.print(MAX_CARDS);\n",
            ),
        ),
        ScreenKind::Lcd2004 => (
            "#include <LiquidCrystal_I2C.h>\n",
            "LiquidCrystal_I2C lcd(0x27, 20, 4);\n",
            "  Wire.begin();\n  lcd.init();\n  lcd.backlight();\n",
            concat!(
                "  lcd.clear();\n",
                "  lcd.setCursor(0, 0);\n",
                "  lcd.print(line);\n",
                "  lcd.setCursor(0, 1);\n",
                "  lcd.print(F(\"cards \"));\n",
                "  lcd.print(stored());\n",
                "  lcd.print('/');\n",
                "  lcd.print(MAX_CARDS);\n",
                "  if (uid) {\n",
                "    lcd.setCursor(0, 2);\n",
                "    for (uint8_t i = 0; i < UID_LEN; i++) {\n",
                "      if (uid[i] < 0x10) lcd.print('0');\n",
                "      lcd.print(uid[i], HEX);\n",
                "      lcd.print(' ');\n",
                "    }\n",
                "  }\n",
            ),
        ),
        _ => (
            "#include <Adafruit_GFX.h>\n#include <Adafruit_SSD1306.h>\n",
            "Adafruit_SSD1306 oled(128, 64, &Wire, -1);\n",
            "  Wire.begin();\n  oled.begin(SSD1306_SWITCHCAPVCC, 0x3C);\n  oled.setTextSize(1);\n  oled.setTextColor(SSD1306_WHITE);\n",
            concat!(
                "  oled.clearDisplay();\n",
                "  oled.setCursor(0, 0);\n",
                "  oled.println(line);\n",
                "  oled.print(F(\"cards \"));\n",
                "  oled.print(stored());\n",
                "  oled.print('/');\n",
                "  oled.println(MAX_CARDS);\n",
                "  if (uid) {\n",
                "    for (uint8_t i = 0; i < UID_LEN; i++) {\n",
                "      if (uid[i] < 0x10) oled.print('0');\n",
                "      oled.print(uid[i], HEX);\n",
                "      oled.print(' ');\n",
                "    }\n",
                "    oled.println();\n",
                "  }\n",
                "  oled.display();\n",
            ),
        ),
    };
    let siren_decl = if spec.siren { "const uint8_t BUZZ = 7;\n" } else { "" };
    let siren_setup = if spec.siren { "  pinMode(BUZZ, OUTPUT);\n" } else { "" };
    let siren_deny = if spec.siren { "    tone(BUZZ, 1200, 300);\n" } else { "" };
    let header = match spec.screen {
        ScreenKind::Lcd => "// D2 — кнопка записи, D8 — реле замка, SPI — RC522, I2C — LCD1602",
        ScreenKind::Lcd2004 => "// D2 — кнопка записи, D8 — реле замка, SPI — RC522, I2C — LCD2004",
        _ => "// D2 — кнопка записи, D8 — реле замка, SPI — RC522, I2C — OLED 128x64",
    };
    fill(
        r#"// <<C0>>
<<HEADER>>
#include <SPI.h>
#include <MFRC522.h>
#include <Wire.h>
<<DISPLAY_INCLUDES>>#include <EEPROM.h>

const uint8_t SS_PIN = 10;
const uint8_t RST_PIN = 9;
const uint8_t BTN = 2;
const uint8_t LOCK = 8;
<<SIREN_DECL>>const uint8_t MAX_CARDS = <<SLOTS>>;
const uint8_t MAGIC = 0xA5;
const uint8_t COUNT = 1;              // число карт в таблице
const uint8_t TABLE = 2;              // начало таблицы UID
const uint8_t UID_LEN = 4;
const uint16_t OPEN_MS = 5000;
const uint16_t GAP_MS = 300;

MFRC522 mfrc522(SS_PIN, RST_PIN);
<<DISPLAY_OBJECT>>uint32_t openedAt = 0;
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
<<SHOW>>}

void setup() {
  pinMode(LOCK, OUTPUT);
  digitalWrite(LOCK, HIGH);           // модуль реле активен низким
  pinMode(BTN, INPUT_PULLUP);
<<SIREN_SETUP>>  Serial.begin(9600);
  SPI.begin();
  mfrc522.PCD_Init();
<<DISPLAY_SETUP>>  if (EEPROM.read(0) != MAGIC) {
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
<<SIREN_DENY>>    show("ACCESS DENIED", mfrc522.uid.uidByte);
  }
  mfrc522.PICC_HaltA();
  mfrc522.PCD_StopCrypto1();
}
"#,
        &[
            ("C0", tr(lang, "Контроль доступа: таблица карт в EEPROM", "Access control: the card table in EEPROM")),
            ("HEADER", header),
            ("DISPLAY_INCLUDES", display_includes),
            ("DISPLAY_OBJECT", display_object),
            ("DISPLAY_SETUP", display_setup),
            ("SHOW", show_body),
            ("SIREN_DECL", siren_decl),
            ("SIREN_SETUP", siren_setup),
            ("SIREN_DENY", siren_deny),
            ("SLOTS", &spec.slots.to_string()),
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
    let setup = if choice == 2 {
        "  pinMode(RELAY, OUTPUT);\n"
    } else {
        "  pinMode(LED, OUTPUT);\n"
    };
    fill(
        r#"// <<C0>>
#include <SoftwareSerial.h>
SoftwareSerial bt(2, 3);          // TX модуля на D2, RX через делитель с D3
<<EXTRA>>
void setup() {
  Serial.begin(9600);
  bt.begin(9600);
<<SETUP>>}

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
            ("SETUP", setup),
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
    let mode = match choice {
        1 => "  pinMode(BTN, INPUT_PULLUP);\n",
        2 => "  pinMode(RELAY, OUTPUT);\n",
        _ => "",
    };
    fill(
        r#"// <<C0>>
#include <SoftwareSerial.h>
SoftwareSerial modem(2, 3);        // TX модема на D2, RX через делитель с D3
const uint8_t PWR = 4;
<<EXTRA>>
void setup() {
  Serial.begin(9600);
  modem.begin(9600);
  pinMode(PWR, OUTPUT);
<<MODE>>  // Импульс на PWR включает модем.
  digitalWrite(PWR, HIGH);
  delay(1200);
  digitalWrite(PWR, LOW);
  delay(3000);
  modem.println(F("AT"));
}

void loop() {
<<LOOP>>}
"#,
        &[
            ("C0", title),
            ("EXTRA", extra),
            ("MODE", mode),
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
        "  if (request.indexOf(\"GET /on\") >= 0) { digitalWrite(RELAY, HIGH); }\n  if (request.indexOf(\"GET /off\") >= 0) { digitalWrite(RELAY, LOW); }\n"
    } else {
        ""
    };
    let (decl, relay_setup) = if choice == 2 {
        ("const uint8_t RELAY = 7;\n", "  pinMode(RELAY, OUTPUT);\n")
    } else {
        ("", "")
    };
    fill(
        r#"// <<C0>>
#include <SPI.h>
#include <Ethernet.h>
<<DECL>>
byte mac[] = {0xDE, 0xAD, 0xBE, 0xEF, 0xFE, 0xED};
IPAddress ip(10, 0, 0, 42);
EthernetServer server(80);

void setup() {
  Serial.begin(9600);
  Ethernet.begin(mac, ip);
  server.begin();
<<RELAY_SETUP>>  Serial.print(F("server: "));
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
            ("DECL", decl),
            ("RELAY_SETUP", relay_setup),
            ("RELAY", relay),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{parse, Action, Input, Load};

    /// The percentage under the plan is the evidence-weighted share of the
    /// typed task the circuit closes, not a count of matched keywords. The
    /// open slot is named instead of hidden.
    #[test]
    fn the_explanation_counts_closed_slots() {
        use crate::text::Lang;
        let parsed = parse("светодиод и кнопка");
        let chosen = plan(&parsed.intent, Lang::Ru).expect("plans");
        let why = chosen.why.expect("an explanation");
        assert!(why.contains("покрытие слотов 100%"), "{why}");
        assert!(!why.contains("уверенность"), "{why}");

        let parsed = parse("светодиод и экран");
        let chosen = plan(&parsed.intent, Lang::Ru).expect("plans");
        let why = chosen.why.expect("an explanation");
        assert!(why.contains("не закрыто: экран"), "{why}");
    }

    /// A pin the sentence names moves the lesson's first digital signal
    /// there. "Датчик звука к пину D5" used to leave D5 unwired.
    #[test]
    fn a_spoken_pin_moves_the_lesson_signal() {
        use crate::text::Lang;
        let parsed = parse("датчик звука к пину D5");
        assert!(parsed.intent.lesson.is_some(), "{:?}", parsed.intent.lesson);
        let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
        let missing = crate::requirements::missing(&built, &parsed.intent);
        assert!(
            missing.iter().all(|m| m.name.ru != "пин"),
            "{:?}",
            missing.iter().map(|m| m.name.ru).collect::<Vec<_>>()
        );
    }

    /// A task that sends a sensor struct from one board to another is not the
    /// one-board test pair: it names the DHT and both roles in one sketch.
    #[test]
    fn a_radio_link_sends_a_struct_between_two_boards() {
        use crate::text::Lang;
        let task = "Составить программу для МК последовательно, через каждые 2 секунды отправлять значения номера передатчика, температуры и влажности на разных платах от 1 платы с передатчиком на 1 приёмник с по-мощью структуры.";
        let parsed = parse(task);
        let plan = plan(&parsed.intent, Lang::Ru).expect("plans");
        assert_eq!(
            plan.title_text(Lang::Ru),
            "Радио 433 МГц: структура между двумя платами"
        );
        assert_eq!(plan.choices.len(), 1);
        let built = build(&parsed.intent, plan.recommended, Lang::Ru).expect("builds");
        assert!(built.circuit.parts().any(|p| p.value == "DHT22"));
        assert!(built.circuit.parts().any(|p| p.value == "FS1000A"));
        assert!(
            !built.circuit.parts().any(|p| p.value == "XY-MK-5V"),
            "the receiver belongs to the second board"
        );
        for needle in [
            "struct Reading",
            "SENDER_ID",
            "enableTransmit(TX_PIN)",
            "digitalPinToInterrupt(RX_PIN)",
            "TX_PIN = A1",
            "DHT_PIN = A2",
            "readTemperature",
            "readHumidity",
            "PERIOD_MS = 2000",
            "radio.send(payload, 32)",
        ] {
            assert!(built.code.contains(needle), "{needle}:\n{}", built.code);
        }
        // The drawing has to carry the same two data pins as the sketch: on
        // A1 for the transmitter and A2 for the DHT.
        let uno = built
            .circuit
            .parts
            .iter()
            .position(|p| p.kind == Kind::Arduino)
            .expect("an Uno");
        let dht = built
            .circuit
            .parts
            .iter()
            .position(|p| p.value == "DHT22")
            .expect("a DHT22");
        let fs = built
            .circuit
            .parts
            .iter()
            .position(|p| p.value == "FS1000A")
            .expect("a transmitter");
        let wired = |pin: usize, other: usize| {
            built.circuit.wires.iter().any(|w| {
                (w.a == End::Pin(pin) && w.b == End::Pin(other))
                    || (w.b == End::Pin(pin) && w.a == End::Pin(other))
            })
        };
        assert!(wired(
            built.circuit.pin(uno, "A1"),
            built.circuit.pin(fs, "DATA")
        ));
        assert!(wired(
            built.circuit.pin(uno, "A2"),
            built.circuit.pin(dht, "DATA")
        ));
        // Both modules are turned over, so their header faces the board and a
        // cable leaves it without a lap around a thirty-millimetre body.
        for part in [fs, dht] {
            let data = built.circuit.pin(part, "DATA");
            assert!(
                built.circuit.pins[data].pos.y < built.circuit.parts[part].at.y,
                "{} header faces away from the board",
                built.circuit.parts[part].value
            );
        }
        let report = crate::checks::run(&built, Lang::Ru);
        assert_eq!(
            report.failed(),
            0,
            "{:?}",
            report
                .checks
                .iter()
                .filter(|c| c.sev == crate::checks::Sev::Fail)
                .map(|c| format!("{}: {}", c.name.ru, c.detail))
                .collect::<Vec<_>>()
        );
    }

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

    /// A spoken count must survive the whole route: the plan, the augmented
    /// circuit and the checklist. The screenshot case "две лампочки" drew one
    /// LED because the count table lacked the synonym; the same test covers
    /// every countable output so a new synonym cannot repeat the bug.
    #[test]
    fn every_named_output_is_drawn_as_many_times_as_asked() {
        use crate::model::Kind;
        use crate::text::Lang;
        let cases: &[(&str, &[Kind], usize)] = &[
            ("две лампочки", &[Kind::Led], 2),
            ("две лампы", &[Kind::Led], 2),
            ("пара лампочек", &[Kind::Led], 2),
            ("три светодиода", &[Kind::Led], 3),
            ("четыре светодиода", &[Kind::Led], 4),
            ("две лампы накаливания", &[Kind::Lamp], 2),
            ("два реле", &[Kind::Relay], 2),
            ("две пищалки", &[Kind::Buzzer, Kind::Speaker], 2),
            ("два динамика", &[Kind::Buzzer, Kind::Speaker], 2),
            ("два сервопривода", &[Kind::Servo], 2),
            ("два мотора", &[Kind::Motor], 2),
            ("две кнопки", &[Kind::Button], 2),
            ("две клавиши", &[Kind::Button], 2),
        ];
        for &(task, kinds, want) in cases {
            let parsed = parse(task);
            let plan = plan(&parsed.intent, Lang::Ru)
                .unwrap_or_else(|| panic!("{task}: no plan"));
            let built = build(&parsed.intent, plan.recommended, Lang::Ru)
                .unwrap_or_else(|| panic!("{task}: does not build"));
            let got = built
                .circuit
                .parts()
                .filter(|p| kinds.contains(&p.kind))
                .count();
            assert_eq!(got, want, "{task}: {got} parts, want {want}");
            let report = crate::checks::run_with(&built, Lang::Ru, &parsed.intent);
            assert_eq!(
                report.failed(),
                0,
                "{task} builds but fails: {:?}",
                report
                    .checks
                    .iter()
                    .filter(|c| c.sev == crate::checks::Sev::Fail)
                    .map(|c| c.detail.as_str())
                    .collect::<Vec<_>>()
            );
        }

        let parsed = parse("три кнопки и два светодиода");
        let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
        let count = |kind| built.circuit.parts().filter(|p| p.kind == kind).count();
        assert_eq!(count(Kind::Button), 3);
        assert_eq!(count(Kind::Led), 2);
    }

    #[test]
    fn every_added_relay_switches_a_load() {
        use crate::text::Lang;
        let parsed = parse("два реле");
        let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
        let report = crate::checks::run_with(&built, Lang::Ru, &parsed.intent);
        assert_eq!(report.failed(), 0);
        assert!(
            report
                .checks
                .iter()
                .all(|c| !c.detail.contains("COM") && !c.detail.contains("NO")),
            "an added relay has open contacts: {:?}",
            report
                .checks
                .iter()
                .map(|c| c.detail.as_str())
                .collect::<Vec<_>>()
        );
        assert!(
            built.circuit.parts().filter(|p| p.kind == crate::model::Kind::Relay).count() >= 2,
            "the second relay is gone"
        );
    }

    #[test]
    fn a_named_display_lesson_draws_one_screen() {
        use crate::text::Lang;
        for task in ["LCD1602", "OLED 128x64"] {
            let parsed = parse(task);
            let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
            let screens = built
                .circuit
                .parts()
                .filter(|p| p.value.starts_with("LCD") || p.value.starts_with("OLED"))
                .count();
            assert_eq!(screens, 1, "{task} draws {screens} screens");
        }
    }

    #[test]
    fn a_clock_on_lcd1602_keeps_the_clock_and_one_screen() {
        use crate::text::Lang;
        let parsed = parse("часы на LCD1602");
        let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
        let screens = built
            .circuit
            .parts()
            .filter(|p| p.value.starts_with("LCD") || p.value.starts_with("OLED"))
            .count();
        assert_eq!(screens, 1);
        assert!(
            built.circuit.parts().any(|p| p.value == "DS3231"),
            "the clock board is gone"
        );
        assert!(built.code.contains("rtc.now()"));
    }

    /// A text scene keeps both its message and the output named beside it.
    #[test]
    fn a_named_output_does_not_hide_the_text() {
        use crate::text::Lang;
        for task in ["текст и светодиод", "текст на экране и светодиод"] {
            let parsed = parse(task);
            assert!(parsed.intent.text_screens.is_some(), "{task}");
            let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
            let report = crate::checks::run(&built, Lang::Ru);
            assert_eq!(report.failed(), 0, "{task}: {:?}", report.checks.iter().map(|c| &c.detail).collect::<Vec<_>>());
            assert!(
                built.circuit.parts().any(|p| p.value == "LCD1602"),
                "{task}: the display is gone"
            );
            assert!(
                built.circuit.parts().any(|p| p.kind == crate::model::Kind::Led),
                "{task}: the LED is gone"
            );
            assert!(built.code.contains("MESSAGE"), "{task}:\n{}", built.code);
            assert!(built.code.contains("LED_PIN"), "{task}:\n{}", built.code);
        }
    }

    /// A clock and a buzzer are one alarm clock, not the buzzer lesson; the
    /// clock also keeps a named display and an LED beside it.
    #[test]
    fn a_clock_keeps_its_time_next_to_an_output() {
        use crate::text::Lang;
        let alarm = parse("часы и пьезо");
        let built = build(&alarm.intent, 0, Lang::Ru).expect("builds");
        assert!(built.circuit.parts().any(|p| p.value == "DS3231"));
        assert!(built.circuit.parts().any(|p| p.kind == crate::model::Kind::Buzzer));
        assert!(built.code.contains("rtc.now()"), "{}", built.code);

        let led = parse("часы на LCD и светодиод");
        let plan = plan(&led.intent, Lang::Ru).expect("plans");
        assert_eq!(plan.title_text(Lang::Ru), "Часы реального времени DS3231");
        let built = build(&led.intent, 0, Lang::Ru).expect("builds");
        assert!(built.circuit.parts().any(|p| p.value == "DS3231"));
        assert!(built.circuit.parts().any(|p| p.value == "LCD1602"));
        assert!(built.circuit.parts().any(|p| p.kind == crate::model::Kind::Led));
        assert!(built.code.contains("rtc.now()"), "{}", built.code);
    }

    /// A bare "текст" next to an output means a display; on its own it is not
    /// a screen ("переведи текст" stays refused).
    #[test]
    fn a_bare_text_word_is_a_screen_only_beside_an_output() {
        assert!(parse("текст и светодиод").intent.text_screens.is_some());
        assert!(parse("переведи текст").intent.text_screens.is_none());
    }

    /// The everyday names of a night light and of a dimmer must not fall
    /// through to "Не нашёл, чем управлять".
    #[test]
    fn everyday_loads_find_a_circuit() {
        use crate::text::Lang;
        let night = parse("ночник");
        let recipe = night.intent.recipe.expect("the night light composes");
        assert_eq!(recipe.behavior, crate::spec::Behavior::Solid);
        let built = build(&night.intent, 0, Lang::Ru).expect("builds");
        assert!(built.circuit.parts().any(|p| p.kind == crate::model::Kind::Ldr));
        assert!(built.circuit.parts().any(|p| p.kind == crate::model::Kind::Led));
        assert!(
            built.code.contains("digitalWrite(SINK_PIN, active ? HIGH : LOW)"),
            "{}",
            built.code
        );
        assert!(!built.code.contains("lit = !lit"), "{}", built.code);

        let night = parse("ночник и реле");
        let built = build(&night.intent, 0, Lang::Ru).expect("builds");
        assert!(built.circuit.parts().any(|p| p.kind == crate::model::Kind::Ldr));
        assert!(built.circuit.parts().any(|p| p.kind == crate::model::Kind::Relay));
        assert!(built.circuit.parts().any(|p| p.kind == crate::model::Kind::Led));

        let dimmer = parse("диммер на симисторе");
        let dimmer_plan = plan(&dimmer.intent, Lang::Ru).expect("plans");
        assert!(
            dimmer_plan.title_text(Lang::Ru).contains("220 В"),
            "{:?}",
            dimmer_plan.title_text(Lang::Ru)
        );

        for task in ["метеостанция", "умный дом"] {
            let parsed = parse(task);
            assert!(plan(&parsed.intent, Lang::Ru).is_some(), "{task}");
        }
    }

    /// "Часы на дисплее 1602" names the LCD model without the "LCD" word.
    #[test]
    fn a_1602_display_is_an_lcd() {
        use crate::text::Lang;
        let parsed = parse("часы на дисплее 1602");
        let screens = parsed.intent.text_screens.or_else(|| crate::parse::named_devices("часы на дисплее 1602").screens);
        let screens = screens.expect("a display is named");
        assert_eq!(screens.kind, crate::parse::ScreenKind::Lcd);
        let plan = plan(&parsed.intent, Lang::Ru).expect("plans");
        assert_eq!(plan.title_text(Lang::Ru), "Часы реального времени DS3231");
        let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
        assert!(built.circuit.parts().any(|p| p.value == "LCD1602"));
    }

    #[test]
    fn two_named_screens_are_drawn_in_a_composition() {
        use crate::text::Lang;
        let parsed = parse("датчик температуры и два экрана");
        let recipe = parsed.intent.recipe.expect("composes");
        assert_eq!(recipe.screens, 2);
        let plan = plan(&parsed.intent, Lang::Ru).expect("plans");
        assert!(
            plan.choices[0].specs.iter().any(|s| s.1.contains("×2")),
            "the plan hides the second screen"
        );
        let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
        let screens = built
            .circuit
            .parts()
            .filter(|p| p.value == "LCD1602")
            .count();
        assert_eq!(screens, 2);
        assert!(
            built.code.contains("lcd0") && built.code.contains("lcd1"),
            "the sketch prints on one screen only:\n{}",
            built.code
        );
    }

    /// "Текст приветствия на двух экранах" is two displays showing a message,
    /// not the motion sensor that a similar-looking word once matched.
    #[test]
    fn text_on_two_displays_builds_two_screens() {
        use crate::text::Lang;
        let parsed = crate::parse::parse("текст приветствия на двух экранах");
        assert_eq!(parsed.intent.lesson, None);
        let plan = plan(&parsed.intent, Lang::Ru).expect("plans");
        assert_eq!(plan.title_text(Lang::Ru), "Приветствие на двух экранах");
        let english = crate::templates::plan(&parsed.intent, Lang::En).expect("plans in English");
        assert_eq!(english.title_text(Lang::En), "Greeting on two displays");
        assert_eq!(plan.choices.len(), 3, "an unnamed display offers a choice");

        let built = build(&parsed.intent, 0, Lang::Ru).expect("builds");
        let screens = built.circuit.parts().filter(|p| p.value == "LCD1602").count();
        assert_eq!(screens, 2, "two screens means two boards");
        assert!(built.code.contains("LiquidCrystal_I2C screen0(0x27"));
        assert!(built.code.contains("LiquidCrystal_I2C screen1(0x26"));
        assert_eq!(built.code.matches("print(MESSAGE)").count(), 2);
        assert!(built.code.contains("void loop()"));
    }

    /// Every display model and every screen count must stay clear of the
    /// board, the controller and the other displays. A fixed 64 mm step used
    /// to put two 98 mm LCD2004 boards on top of each other.
    #[test]
    fn text_screens_never_overlap() {
        use crate::model::Kind;
        for text in [
            "текст на экране",
            "текст на двух экранах",
            "текст на трёх экранах",
            "текст на четырёх экранах",
            "текст на двух LCD1602",
            "текст на двух LCD2004",
            "две надписи на двух OLED",
        ] {
            let parsed = crate::parse::parse(text);
            let plan = plan(&parsed.intent, Lang::Ru)
                .unwrap_or_else(|| panic!("{text}: does not plan"));
            for choice in 0..plan.choices.len() {
                let built = build(&parsed.intent, choice, Lang::Ru)
                    .unwrap_or_else(|| panic!("{text} #{choice}: does not build"));
                let c = &built.circuit;
                let mut rects: Vec<(String, crate::model::Rect)> = c
                    .parts
                    .iter()
                    .filter(|p| matches!(p.kind, Kind::Module | Kind::Arduino))
                    .map(|p| (format!("{} {}", p.label, p.value), c.part_rect(p)))
                    .collect();
                rects.push(("breadboard".to_string(), c.board.rect()));
                for (a, (na, ra)) in rects.iter().enumerate() {
                    for (nb, rb) in &rects[a + 1..] {
                        assert!(
                            !ra.inflate(-0.5).intersects(rb.inflate(-0.5)),
                            "{text} #{choice}: {na} overlaps {nb}"
                        );
                    }
                }
            }
        }
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

    /// A step counter of 0 or 1 makes the generated code divide by zero, and
    /// a spoken period above 65535 used to be silently clamped. The sketch
    /// now keeps a safe value and says so; the period is a uint32_t.
    #[test]
    fn dangerous_spoken_numbers_are_made_safe_and_explained() {
        use crate::text::Lang;
        for (task, want) in [
            ("мотор и кнопка, после 0 нажатий", "STEPS = 2"),
            ("мотор и кнопка, после 1 нажатия", "STEPS = 2"),
            ("мотор и кнопка, после 300 нажатий", "STEPS = 255"),
        ] {
            let parsed = crate::parse::parse(task);
            let plan = plan(&parsed.intent, Lang::Ru)
                .unwrap_or_else(|| panic!("{task}: no plan"));
            let built = build(&parsed.intent, plan.recommended, Lang::Ru)
                .unwrap_or_else(|| panic!("{task}: does not build"));
            assert!(built.code.contains(want), "{task}: {}", built.code);
            assert!(
                built.notes.iter().any(|n| n.ru.contains("Ступеней")),
                "{task}: no note about the safe step count"
            );
            let report = crate::checks::run_with(&built, Lang::Ru, &parsed.intent);
            assert_eq!(report.failed(), 0, "{task}");
        }

        let parsed = crate::parse::parse("светодиод мигает каждые 70000 мс");
        let chosen = plan(&parsed.intent, Lang::Ru).expect("plans");
        let built = build(&parsed.intent, chosen.recommended, Lang::Ru).expect("builds");
        assert!(
            built.code.contains("HALF_MS = 70000"),
            "the spoken period is clamped: {}",
            built.code
        );
        assert!(!built.code.contains("65535"), "{}", built.code);
    }

    /// The 400 mA budget is advice, not a reason to throw away a part the
    /// sentence named. Pins can still run out; that is said in a note.
    #[test]
    fn named_parts_are_not_dropped_to_fit_the_power_budget() {
        use crate::model::Kind;
        use crate::text::Lang;
        let parsed = crate::parse::parse("три сервопривода");
        let chosen = plan(&parsed.intent, Lang::Ru).expect("plans");
        let built = build(&parsed.intent, chosen.recommended, Lang::Ru).expect("builds");
        let servos = built
            .circuit
            .parts()
            .filter(|p| p.kind == Kind::Servo)
            .count();
        assert_eq!(servos, 3, "the power budget still drops a named servo");
        let report = crate::checks::run_with(&built, Lang::Ru, &parsed.intent);
        assert_eq!(report.failed(), 0);

        let parsed = crate::parse::parse("двадцать светодиодов");
        let chosen = plan(&parsed.intent, Lang::Ru).expect("plans");
        let built = build(&parsed.intent, chosen.recommended, Lang::Ru).expect("builds");
        assert!(
            built.notes.iter().any(|n| n.ru.contains("Не все названные детали")),
            "the pin limit is silent: {:?}",
            built.notes
        );
    }

    /// One table of the phrases that used to build a wrong but confident
    /// circuit: a pin number swallowed as a count, "освещение" read as a
    /// photoresistor, a spoken period clamped, or a division by zero in the
    /// sketch. Every one must either be honestly refused or pass its checks.
    #[test]
    fn bug_table_phrases_never_end_in_a_false_failure() {
        use crate::text::Lang;
        for task in [
            "пин 13 и светодиод",
            "кнопка на пин 2 и светодиод",
            "порт 3 и кнопка",
            "контакт 4 и реле",
            "вывод 2 и датчик температуры",
            "датчик температуры на пин 13 и дисплей",
            "пин 13 и экран часов",
            "3 красных и 2 зелёных светодиода",
            "два светодиода на пины 12 и 13",
            "управление освещением",
            "регулировать яркость освещения",
            "включить освещение кнопкой",
            "светодиод мигает каждые 70000 мс",
            "мотор и кнопка, после 0 нажатий",
            "мотор и кнопка, после 1 нажатия",
            "мотор и кнопка, после 300 нажатий",
            "три сервопривода",
            "два датчика температуры",
            "два датчика света",
            "датчик температуры, влажности и давления",
        ] {
            let parsed = crate::parse::parse(task);
            if parsed.unsupported.is_some() {
                continue;
            }
            let chosen = plan(&parsed.intent, Lang::Ru)
                .unwrap_or_else(|| panic!("{task}: no plan"));
            let built = build(&parsed.intent, chosen.recommended, Lang::Ru)
                .unwrap_or_else(|| panic!("{task}: does not build"));
            let report = crate::checks::run_with(&built, Lang::Ru, &parsed.intent);
            let failures: Vec<_> = report
                .checks
                .iter()
                .filter(|c| c.sev == crate::checks::Sev::Fail)
                .collect();
            assert!(
                failures.is_empty(),
                "{task}: {}",
                failures
                    .iter()
                    .map(|c| format!("{}: {}", c.name.ru, c.detail))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
    }

    /// "на пины 12 и 13" names two pins, and both reach the drawing. The
    /// parser keeps the list, and the build moves one signal to each.
    #[test]
    fn every_named_pin_reaches_the_drawing() {
        use crate::text::Lang;
        let parsed = crate::parse::parse("два светодиода на пины 12 и 13");
        assert_eq!(parsed.intent.named.pins, vec![12, 13]);
        let plan = plan(&parsed.intent, Lang::Ru).expect("plans");
        let built = build(&parsed.intent, plan.recommended, Lang::Ru).expect("builds");
        for pin in [12, 13] {
            assert!(
                crate::requirements::pin_wired(&built.circuit, pin),
                "D{pin} is not in the circuit"
            );
        }
        assert!(
            built.code.contains("= 12;") && built.code.contains("= 13;"),
            "the sketch did not follow both pins:\n{}",
            built.code
        );
    }

    /// A clock and its display speak I2C, so a named digital pin has nothing
    /// to switch. The build explains the free pin instead of inventing an LED
    /// or reporting a part the user never asked for.
    #[test]
    fn a_pin_with_no_digital_part_is_explained() {
        use crate::text::Lang;
        let parsed = crate::parse::parse("пин 13 и экран часов");
        let plan = plan(&parsed.intent, Lang::Ru).expect("plans");
        let built = build(&parsed.intent, plan.recommended, Lang::Ru).expect("builds");
        assert!(built.explained_pins.contains(&13));
        assert!(
            built
                .notes
                .iter()
                .any(|n| n.ru.contains("оставлен свободным")),
            "no note explains the free pin"
        );
        assert!(
            crate::requirements::missing(&built, &parsed.intent)
                .iter()
                .all(|m| m.extra.as_deref() != Some("D13"))
        );
    }

    /// One resistor is one line in the bill, however its value is spelled.
    #[test]
    fn one_resistor_is_one_bill_line() {
        use crate::text::Lang;
        let parsed = crate::parse::parse("два светодиода");
        let plan = plan(&parsed.intent, Lang::Ru).expect("plans");
        let built = build(&parsed.intent, plan.recommended, Lang::Ru).expect("builds");
        let rows = component_rows(&built.circuit, Lang::Ru);
        let resistors: Vec<&ComponentRow> = rows
            .iter()
            .filter(|r| r.component.contains("220"))
            .collect();
        assert_eq!(
            resistors.len(),
            1,
            "one resistor is listed twice: {:?}",
            rows.iter().map(|r| &r.component).collect::<Vec<_>>()
        );
        assert!(resistors[0].quantity >= 2, "{:?}", resistors[0].quantity);
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
                    task: Default::default(),
                named: Default::default(),
                transistor: false,
                presses: false,
                lesson: None,
                action: None,
                raw: p.intent.raw,
                condition: None,
                recipe: None,
                text_screens: None,
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
                assert!(!built.summary.is_empty());
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

    /// An incandescent lamp gets a MOSFET dimmer or a relay, never a series
    /// resistor and a blink.
    #[test]
    fn a_lamp_is_switched_by_a_transistor_or_a_relay() {
        let p = parse("лампа накаливания 12 В");
        let plan = plan(&p.intent, Lang::Ru).expect("a lamp is routable");
        assert_eq!(plan.tid, Tid::Lamp);
        let mosfet = build(&p.intent, 0, Lang::Ru).expect("MOSFET choice");
        assert!(mosfet.circuit.parts.iter().any(|x| x.kind == Kind::Lamp));
        assert!(mosfet.circuit.parts.iter().any(|x| x.kind == Kind::Nmos));
        assert!(!mosfet.circuit.parts.iter().any(|x| x.kind == Kind::Led));
        assert!(mosfet.code.contains("analogWrite(LAMP"));

        let relay = build(&p.intent, 1, Lang::Ru).expect("relay choice");
        assert!(relay.circuit.parts.iter().any(|x| x.kind == Kind::Relay));
        assert!(relay.circuit.parts.iter().any(|x| x.kind == Kind::Lamp));
        assert!(relay.code.contains("digitalWrite(RELAY"));
    }

    /// A template may already declare `Servo arm;`. The servo added from the
    /// sentence must take a free name instead of declaring the object twice.
    #[test]
    fn added_servos_never_share_a_name() {
        for task in ["два сервопривода", "датчик движения и два сервопривода"] {
            let p = parse(task);
            let plan = plan(&p.intent, Lang::Ru).expect("routable");
            let built = build(&p.intent, plan.recommended, Lang::Ru).expect("builds");
            let names: Vec<&str> = built
                .code
                .lines()
                .filter_map(|line| line.trim().strip_prefix("Servo "))
                .filter_map(|rest| rest.strip_suffix(';'))
                .collect();
            assert!(names.len() >= 2, "{task}: {names:?}");
            let unique: std::collections::BTreeSet<&str> = names.iter().copied().collect();
            assert_eq!(unique.len(), names.len(), "{task}: {names:?}");
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

    /// The door scene is recognised through typos and through the reader
    /// alone, but a plain "EEPROM" task without a door stays a counter.
    #[test]
    fn the_access_scene_is_recognised_by_its_meaning() {
        for text in [
            "контроль доступа на 49 карт в EEPROM",
            "доспуп по картам в энергонезовисимую память",
            "пропуск на 20 карт, таблица карт в памяти",
            "турникет с базой карт в EEPROM",
            "RFID RC522 с экраном и базой карт в EEPROM",
            "замок на скуд, карты в EEPROM",
        ] {
            assert!(wants_access(text), "{text}");
        }
        for text in [
            "счётчик нажатий с записью в EEPROM",
            "выведи все данные из памяти EEPROM",
            "считыватель RFID RC522, UID в порт",
            "Карта памяти SD",
        ] {
            assert!(!wants_access(text), "{text}");
        }
    }

    /// A screen named by the task is on the recommended RFID build, not in a
    /// choice the user has to find first.
    #[test]
    fn a_named_screen_leads_the_rfid_plan() {
        let p = crate::parse::parse("контроль доступа по карте на 49 карт с экраном");
        let plan = plan(&p.intent, Lang::Ru).expect("plans");
        assert_eq!(plan.recommended, 1);
        let built = build(&p.intent, plan.recommended, Lang::Ru).expect("builds");
        assert!(crate::requirements::missing(&built, &p.intent).is_empty());
    }

    /// A screen named by the task replaces the default OLED, so the scene
    /// shows the part the user asked for instead of a fixed picture.
    #[test]
    fn a_named_screen_goes_into_the_access_scene() {
        let text = "контроль доступа на 50 карт в энергозависимую память EEPROM с LCD1602";
        let p = parse(text);
        assert_eq!(p.intent.load, Load::Rfid);
        assert!(p.intent.lesson.is_none(), "the display lesson must not win");
        let plan = plan(&p.intent, Lang::Ru).expect("plans");
        assert_eq!(plan.recommended, 3);
        let built = build(&p.intent, 3, Lang::Ru).expect("builds");
        assert!(built.code.contains("#include <LiquidCrystal_I2C.h>"), "no LCD");
        assert!(!built.code.contains("Adafruit_SSD1306"), "the OLED stayed");
        assert!(built.code.contains("MAX_CARDS = 50"), "wrong table size");
        let values: Vec<&str> = built.circuit.parts().map(|p| p.value.as_str()).collect();
        assert!(values.iter().any(|v| *v == "LCD1602"), "{values:?}");
        assert!(values.iter().any(|v| *v == "SRD-05VDC"), "{values:?}");
    }

    /// A siren named in the sentence is wired and sounds on a refused card.
    #[test]
    fn a_named_siren_sounds_on_a_refused_card() {
        let text = "контроль доступа на 49 карт в EEPROM с экраном, чужая карта сирена";
        let p = parse(text);
        let built = build(&p.intent, 3, Lang::Ru).expect("builds");
        assert!(built.code.contains("const uint8_t BUZZ = 7;"), "no siren pin");
        assert!(built.code.contains("tone(BUZZ, 1200, 300);"), "no siren call");
        let values: Vec<&str> = built.circuit.parts().map(|p| p.value.as_str()).collect();
        assert!(values.iter().any(|v| *v == "пьезо"), "{values:?}");
    }

    /// A pump asked to water every half hour runs on that schedule instead of
    /// staying on forever.
    #[test]
    fn a_watering_pump_follows_the_spoken_period() {
        let text = "полив раз в полчаса";
        let p = parse(text);
        let sandbox = plan(&p.intent, Lang::Ru).expect("plans");
        let built = build(&p.intent, sandbox.recommended, Lang::Ru).expect("builds");
        assert!(built.code.contains("PERIOD_MS = 1800000"), "{}", built.code);
        assert!(built.code.contains("RUN_MS"), "no run time");
        assert!(built.code.contains("millis()"), "the schedule is not timed");
        assert!(!built.code.contains("HIGH);   // тест"), "the pump still runs forever");
    }

    /// The EEPROM of an Uno holds 255 four-byte UIDs, so a bigger request is
    /// clamped and the plan says why instead of pretending to fit.
    #[test]
    fn a_table_bigger_than_the_eeprom_is_clamped_and_explained() {
        let text = "контроль доступа на 300 карт в EEPROM с экраном";
        let p = parse(text);
        let plan = plan(&p.intent, Lang::Ru).expect("plans");
        let why = plan.why.clone().expect("no explanation");
        assert!(why.contains("255") && why.contains("300"), "{why}");
        let built = build(&p.intent, 3, Lang::Ru).expect("builds");
        assert!(built.code.contains("MAX_CARDS = 255"), "not clamped");
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
                task: Default::default(),
                load: Load::Led,
                input: None,
                named: Default::default(),
                transistor: false,
                presses: false,
                lesson: Some(i),
                action: None,
                raw: String::new(),
                condition: None,
                recipe: None,
                text_screens: None,
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
                task: Default::default(),
                load: Load::Led,
                input: None,
                named: Default::default(),
                transistor: false,
                presses: false,
                lesson: Some(i),
                action: None,
                raw: String::new(),
                condition: None,
                recipe: None,
                text_screens: None,
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
            let report = crate::checks::run_with(&built, Lang::Ru, &parsed.intent);
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
            task: Default::default(),
            load: Load::Motor,
            input: Some(Input::Button),
            named: Default::default(),
            transistor: true,
            presses: true,
            lesson: None,
            action: None,
            raw: String::new(),
            condition: None,
            recipe: None,
            text_screens: None,
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

    /// The sentence in the bug report: a servo named in words, and the same
    /// servo named by its model. Both have to reach the schematic, and the
    /// sketch has to use the servo library on the wired pin.
    #[test]
    fn a_named_servo_is_on_the_schematic_and_in_the_sketch() {
        for text in [
            "когда на датчике движения движение, открываем сервопривод",
            "когда на датчике движения движение, открываем sg90",
            "когда на датчике движения движение, поверни сервопривод",
            "датчик движения и сервопривод",
        ] {
            let p = parse(text);
            assert!(p.unsupported.is_none(), "{text}: unsupported");
            let built = build(&p.intent, 0, Lang::Ru)
                .unwrap_or_else(|| panic!("{text}: does not build"));
            assert!(
                built.circuit.parts().any(|p| p.kind == Kind::Servo),
                "{text}: the servo is not on the schematic"
            );
            assert!(
                built.code.contains("Servo.h") && built.code.contains(".attach("),
                "{text}: the sketch does not drive a servo:\n{}",
                built.code
            );
            assert!(built.code.contains(".write("), "{text}: no servo position");
            let report = crate::checks::run(&built, Lang::Ru);
            let fails: Vec<String> = report
                .checks
                .iter()
                .filter(|c| c.sev == crate::checks::Sev::Fail)
                .map(|c| format!("{}: {}", c.name.ru, c.detail))
                .collect();
            assert!(fails.is_empty(), "{text}: {}", fails.join("; "));
        }
    }

    /// Every kind of part named in a sentence ends up in the circuit, even
    /// when the sentence also matches a ready-made lesson or template.
    #[test]
    fn named_extras_reach_the_schematic() {
        let cases: &[(&str, &[Kind])] = &[
            ("RFID и светодиод", &[Kind::Module, Kind::Led]),
            ("RFID и rgb светодиод", &[Kind::Module, Kind::Led]),
            ("датчик температуры и пьезо", &[Kind::Module, Kind::Buzzer]),
            ("датчик газа и rgb", &[Kind::Module, Kind::Led]),
            (
                "датчик газа и реле и мотор",
                &[Kind::Module, Kind::Relay, Kind::Motor],
            ),
            ("датчик газа и кнопка", &[Kind::Module, Kind::Button]),
            (
                "мотор и кнопка и светодиод",
                &[Kind::Motor, Kind::Button, Kind::Led],
            ),
            (
                "датчик движения, сервопривод и светодиод",
                &[Kind::Module, Kind::Servo, Kind::Led],
            ),
        ];
        for (text, kinds) in cases {
            let p = parse(text);
            assert!(p.unsupported.is_none(), "{text}: unsupported");
            let built = build(&p.intent, 0, Lang::Ru)
                .unwrap_or_else(|| panic!("{text}: does not build"));
            for kind in *kinds {
                assert!(
                    built.circuit.parts().any(|p| p.kind == *kind),
                    "{text}: {:?} is missing from [{}]",
                    kind,
                    built
                        .circuit
                        .parts()
                        .map(|p| format!("{} {}", p.label, p.value))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            assert!(!unfilled(&built.code), "{text}: unfilled token");
            let report = crate::checks::run(&built, Lang::Ru);
            let fails: Vec<String> = report
                .checks
                .iter()
                .filter(|c| c.sev == crate::checks::Sev::Fail)
                .map(|c| format!("{}: {}", c.name.ru, c.detail))
                .collect();
            assert!(fails.is_empty(), "{text}: {}", fails.join("; "));
        }
    }

    /// An extra output follows the lesson's own condition instead of blinking
    /// on its own clock: the PIR lesson already computes `bool active`.
    #[test]
    fn an_extra_led_follows_the_lesson_condition() {
        let p = parse("датчик движения, сервопривод и светодиод");
        let built = build(&p.intent, 0, Lang::Ru).expect("builds");
        assert!(built.code.contains("bool active"), "{}", built.code);
        assert!(
            built
                .code
                .contains("digitalWrite(LED_PIN, active ? HIGH : LOW)"),
            "the LED ignores the condition:\n{}",
            built.code
        );
        assert!(built.code.contains("arm.write(active ? 90 : 0)"), "{}", built.code);
    }

    /// Extras are laid out on a shelf below the finished circuit, so adding a
    /// part never draws it over the board or over another part.
    #[test]
    fn extra_parts_do_not_overlap_anything() {
        for text in [
            "RFID и светодиод",
            "RFID и rgb светодиод",
            "датчик газа и реле и мотор",
            "датчик газа и кнопка",
            "датчик движения, сервопривод и светодиод",
        ] {
            let p = parse(text);
            let built = build(&p.intent, 0, Lang::Ru)
                .unwrap_or_else(|| panic!("{text}: does not build"));
            let c = &built.circuit;
            let rects: Vec<(String, crate::model::Rect)> = c
                .parts()
                .map(|p| (format!("{} {}", p.label, p.value), c.part_rect(p)))
                .collect();
            for (a, (na, ra)) in rects.iter().enumerate() {
                for (nb, rb) in &rects[a + 1..] {
                    assert!(
                        !ra.inflate(-0.5).intersects(rb.inflate(-0.5)),
                        "{text}: {na} overlaps {nb}"
                    );
                }
            }
        }
    }
}

