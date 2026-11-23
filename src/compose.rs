//! Turn a [`Composition`] into a plan, a circuit and a sketch.
//!
//! The builder does not know any particular sensor or output: it places the
//! source from its table entry, picks a free pin for the sink, and joins the
//! fragments. Adding a sensor is a data edit in `spec.rs`.

use crate::model::{Circuit, Kind, NetClass, Pt, Row};
use crate::parse::{Action, Load};
use crate::spec::*;
use crate::templates::{self, B, Built, Choice, Plan, Tid};
use crate::text::{tr, Lang, L};

/// A sentence with a sensor and a verb becomes a composition. The sink comes
/// from the sentence's load word, or from an explicit "реле"/"пищалка".
pub fn detect(
    raw: &str,
    load: Load,
    named_load: bool,
    lesson: Option<usize>,
    action: Option<Action>,
    condition: Option<Condition>,
) -> Option<Composition> {
    let text = crate::words::normalize(raw);
    // Only a sentence that asks the circuit to do something is composed:
    // "Измерь температуру" stays the lesson that teaches the sensor.
    // A screen named outright is a sink on its own: "температура на дисплее"
    // has no verb the action table catches, yet it asks for a readout. The
    // serial port is not a screen, so "вывести в порт" stays a lesson.
    let names_screen = !text.contains("порт")
        && (text.contains("диспле")
            || text.contains("экран")
            || text.contains("табло")
            || text.contains("lcd")
            || text.contains("oled")
            || text.contains("display"));
    // A named relay, buzzer, screen, servo, motor or LED is itself a request
    // to act: "влажность выше 60, реле", "датчик движения, сервопривод".
    let explicit_sink = text.contains("реле")
        || text.contains("relay")
        || text.contains("пищал")
        || text.contains("пьезо")
        || text.contains("сирен")
        || text.contains("зуммер")
        || text.contains("buzzer")
        || text.contains("siren")
        || text.contains("gong")
        || text.contains("серво")
        || text.contains("servo")
        || text.contains("sg90")
        || text.contains("sg-90")
        || text.contains("mg90")
        || text.contains("mg995")
        || text.contains("mg996")
        || text.contains("fs90")
        || text.contains("мотор")
        || text.contains("motor")
        || text.contains("двигател")
        || text.contains("вентилятор")
        || text.contains("насос")
        || text.contains("кулер")
        || text.contains("светодиод")
        || text.contains("led")
        || text.contains("ламп")
        || text.contains("подсвет")
        || text.contains("ночник")
        || text.contains("фонар")
        || text.contains("гирлянд")
        || text.contains("rgb")
        || text.contains("цветной светодиод")
        || text.contains("светом")
        || text.contains("полив")
        || text.contains("орошен")
        || names_screen;
    let asks_action = matches!(
        action,
        Some(Action::Blink | Action::Alarm | Action::Toggle)
    );
    // A named output plus a spoken threshold is a request too: "цвет выше
    // 300, светодиод" has no verb, but it is plainly a lamp on a condition.
    let asks_condition = named_load && condition.is_some();
    // A sensor named on its own is a request to read it: the serial monitor
    // is the natural output and occupies no pin. Without this, "термометр на
    // термисторе" had no lesson that fit the named part and fell back to
    // "не нашёл, чем управлять". A lesson the book already answers keeps the
    // book's own build, unless that build draws a different part: lesson 10
    // pairs a thermistor with the DS18B20 but its drawing is the DS18B20
    // alone, so a thermistor sentence is composed from the NTC instead.
    let source_text = source_scope(&text);
    let sensor_only = source_in(&source_text)
        .is_some_and(|s| lesson.is_none() || s.id == SourceId::Thermistor);
    if !asks_action && !explicit_sink && !asks_condition && !sensor_only {
        return None;
    }
    // A sensor named with an everyday word ("температура", "давление") is
    // found through the alias table; a load word in the same sentence must
    // not hide the sensor that the sentence is about.
    // The book lesson is a strong hint, but it can be about the sink: in
    // "газ выше 400, реле" the relay lesson wins the vote. So fall back to
    // the source whose own keywords match the sentence best.
    // The book's "двигатель постоянного тока" describes the motor; it is not
    // a request for a current sensor. Likewise "по алгоритму движения" is a
    // motion program, not the HC-SR501. Both readings come from word stems,
    // so the words are dropped before the source table looks at them.
    // The words the sentence uses name the part; the lesson hint fills in
    // only when they say nothing. "Термометр на термисторе" used to take the
    // DS18B20 because lesson 10 was consulted first, even though the source
    // table has an NTC that matches the word exactly.
    let source = source_in(&source_text)
        .or_else(|| lesson.and_then(source_for_lesson))?;
    // A lesson that teaches a screen is about the screen itself: "цветной
    // дисплей 1.44" is a screen, not a colour sensor with an OLED readout.
    // "Давление на OLED" stays a recipe: its sensor is not the word "цвет".
    if lesson.is_some_and(screen_lesson) && source.id == SourceId::Color {
        return None;
    }
    // A window belongs to one reading. If the sentence names a second
    // sensor, its threshold is not this sensor's: refuse rather than show a
    // range that mixes two boards.
    if matches!(condition, Some(Condition::Between { .. })) {
        let names_other = SOURCES.iter().any(|s| {
            s.id != source.id
                && s.keys.iter().any(|k| crate::words::mentions(&source_text, k))
        });
        if names_other {
            return None;
        }
    }
    // A matrix or a seven-segment board is not an I2C text screen. The
    // composition table has no sink for it, so it must not quietly choose an
    // LCD instead; the lesson route owns those boards.
    if crate::parse::screens_in(raw).is_some_and(|s| s.matrix || s.segments) {
        return None;
    }
    let sink = if !explicit_sink && !asks_action && !asks_condition {
        SinkId::Serial
    } else if text.contains("реле") || text.contains("relay") {
        SinkId::Relay
    } else if text.contains("пищал")
        || text.contains("пьезо")
        || text.contains("сирен")
        || text.contains("зуммер")
        || text.contains("buzzer")
        || text.contains("siren")
        || text.contains("gong")
    {
        SinkId::Buzzer
    } else if text.contains("серво")
        || text.contains("servo")
        || text.contains("sg90")
        || text.contains("sg-90")
        || text.contains("mg90")
        || text.contains("mg995")
        || text.contains("mg996")
        || text.contains("fs90")
    {
        SinkId::Servo
    } else if text.contains("мотор")
        || text.contains("motor")
        || text.contains("двигател")
        || text.contains("вентилятор")
        || text.contains("насос")
        || text.contains("кулер")
        || text.contains("полив")
        || text.contains("орошен")
    {
        SinkId::Motor
    } else if text.contains("rgb")
        || text.contains("трехцвет")
        || text.contains("цветной светодиод")
    {
        SinkId::Rgb
    } else if names_screen {
        // A character display is the plainer readout: it is what "дисплей"
        // means by default, and the one the lessons put first. OLED has to be
        // asked for by name.
        if text.contains("2004") {
            SinkId::Lcd2004
        } else if text.contains("oled") || text.contains("графическ") {
            SinkId::Oled
        } else {
            SinkId::Lcd
        }
    } else {
        match load {
            Load::Led => SinkId::Led,
            Load::Buzzer => SinkId::Buzzer,
            _ => return None,
        }
    };
    // A night light is a steady lamp, not a flasher: when it gets dark the
    // LED simply stays on. Blinking is only the default for an alarm.
    let night_light = sink == SinkId::Led
        && (text.contains("ночник") || text.contains("night light"));
    // A clap light stays on while the sound is there; blinking would turn a
    // single clap into a strobe. A sound *level* alarm still blinks.
    let clap_light = sink == SinkId::Led
        && source.id == SourceId::Sound
        && (text.contains("хлоп") || text.contains("clap"));
    let behavior = if night_light || clap_light {
        Behavior::Solid
    } else {
        behavior_for(sink, action)
    };
    Some(Composition {
        source: source.id,
        sink,
        behavior,
        condition,
        screens: crate::parse::screens_in(raw).map(|s| s.count).unwrap_or(1),
    })
}

/// Drop the stem collisions that are not about a sensor: "постоянного тока"
/// names the motor's supply, and "по алгоритму движения" names a motion
/// program. Both words still reach the rest of the parser.
fn source_scope(text: &str) -> String {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let algorithmic = tokens.iter().any(|t| crate::words::mentions(t, "алгоритм"));
    let mut out = String::new();
    for (i, token) in tokens.iter().enumerate() {
        let motion_program = algorithmic && crate::words::mentions(token, "движени");
        let dc_supply = i > 0
            && crate::words::mentions(token, "ток")
            && (crate::words::mentions(tokens[i - 1], "постоянн")
                || crate::words::mentions(tokens[i - 1], "переменн"));
        if motion_program || dc_supply {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(token);
    }
    out
}

/// The lesson's own board is a display: the sentence is about showing
/// something, not about a sensor that happens to end in a screen.
fn screen_lesson(i: usize) -> bool {
    use crate::modules::Look;
    crate::lessons::ALL[i].boards.iter().any(|b| {
        crate::modules::find(b.name)
            .map(|m| matches!(m.look, Look::Lcd | Look::Display | Look::Segments | Look::Matrix | Look::Panel))
            .unwrap_or(false)
    })
}

/// The verb a sink does when it is switched by a condition. The action only
/// changes one case: a lamp asked to "toggle" is held on, not blinked.
pub fn behavior_for(sink: SinkId, action: Option<Action>) -> Behavior {
    match sink {
        SinkId::Led => match action {
            Some(Action::Toggle) => Behavior::Solid,
            _ => Behavior::Blink,
        },
        SinkId::Buzzer => Behavior::Beep,
        SinkId::Relay => Behavior::Switch,
        SinkId::Lcd | SinkId::Lcd2004 | SinkId::Oled => Behavior::Show,
        SinkId::Servo => Behavior::Turn,
        SinkId::Motor => Behavior::Spin,
        SinkId::Rgb => Behavior::Color,
        SinkId::Serial => Behavior::Show,
    }
}

/// The words "Понял так" shows for a composition: source · condition · action.
pub fn recipe_words(c: Composition) -> Vec<L> {
    let source = source(c.source);
    // A screen shows the reading itself, so a threshold would be a lie: the
    // recipe names the board instead of a condition.
    if c.behavior == Behavior::Show {
        return vec![source.short, sink(c.sink).title];
    }
    let condition = c.condition.unwrap_or_else(|| {
        source
            .variants
            .first()
            .map(|v| v.condition)
            .unwrap_or(Condition::Active)
    });
    vec![source.short, condition.label(), c.behavior.label()]
}

/// The sensor the sentence itself names, ignoring any lesson hint. The parser
/// uses it to reject a book lesson that teaches a different sensor than the
/// words do: "влажность почвы" is not the DHT lesson.
pub fn source_in_text(raw: &str) -> Option<&'static SourceSpec> {
    source_in(&source_scope(&crate::words::normalize(raw)))
}

/// The declared source whose keywords best match the sentence.
fn source_in(text: &str) -> Option<&'static SourceSpec> {
    // "Влажность почвы" is a resistive probe, not the DHT humidity sensor.
    // The words share "влажность", so the soil probe has to win by name.
    // "Грунт" and "земля" mean the same probe and used to fall through to the
    // DHT22 branch.
    if crate::words::mentions(text, "почв")
        || crate::words::mentions(text, "грунт")
        || crate::words::mentions(text, "soil")
        || (crate::words::mentions(text, "влажност") && crate::words::mentions(text, "земл"))
    {
        if let Some(s) = SOURCES.iter().find(|s| s.id == SourceId::Soil) {
            return Some(s);
        }
    }
    // "Термометр" and "термистор" are both nine letters, so the generic
    // longest-key tie-break kept the DS18B20 for a thermistor sentence. The
    // part named by its own word wins over the word that describes it.
    if crate::words::mentions(text, "термистор")
        || crate::words::mentions(text, "терморезистор")
        || crate::words::mentions(text, "thermistor")
    {
        if let Some(s) = SOURCES.iter().find(|s| s.id == SourceId::Thermistor) {
            return Some(s);
        }
    }
    let mut best: Option<((usize, usize), &'static SourceSpec)> = None;
    for s in SOURCES {
        let mut hits = 0;
        let mut longest = 0;
        for key in s.keys {
            if crate::words::mentions(text, key) {
                hits += 1;
                longest = longest.max(key.len());
            }
        }
        if hits == 0 {
            continue;
        }
        if best.map(|(score, _)| (hits, longest) > score).unwrap_or(true) {
            best = Some(((hits, longest), s));
        }
    }
    best.map(|(_, s)| s)
}

struct Resolved<'a> {
    source: &'a SourceSpec,
    variant: &'a Variant,
    condition: Condition,
    sink: SinkId,
    /// The Uno pins the sink lands on. A single free pin for a part, or the
    /// two wires of the I2C bus, which a source may already be on.
    sink_pins: Vec<&'static str>,
    behavior: Behavior,
    /// The number the sentence gave, if it gave one: a comparator variant has
    /// to say that the number does not reach its sketch.
    spoken: Option<Condition>,
    /// How many screens the sentence asked for.
    screens: usize,
}

fn resolve(c: &Composition, variant: usize) -> Option<Resolved<'static>> {
    let source = source(c.source);
    let v = source.variants.get(variant)?;
    let condition = match v.logic {
        Logic::Flag => Condition::Active,
        Logic::Number => c.condition.unwrap_or(v.condition),
    };
    let sink = sink(c.sink);
    let sink_pins = match sink.wiring {
        // A part cannot land on a pin the source already uses. The first free
        // candidate wins, which keeps the drawing stable between runs.
        SinkWiring::Pin => {
            let free: Vec<&'static str> = sink
                .pins
                .iter()
                .copied()
                .filter(|p| !v.pins.contains(p))
                .take(sink.take)
                .collect();
            if free.len() < sink.take {
                return None;
            }
            free
        }
        // The bus is shared on purpose: an I2C source and an I2C screen are
        // two devices on the same two wires.
        SinkWiring::I2c => sink.pins.to_vec(),
    };
    Some(Resolved {
        source,
        variant: v,
        condition,
        sink: c.sink,
        sink_pins,
        behavior: c.behavior,
        spoken: c.condition,
        screens: c.screens,
    })
}

/// Whether any of the source's ways of being read leaves a free pin for the
/// sink. A false here is a real refusal, not something to paper over.
pub fn fits(c: Composition) -> bool {
    (0..source(c.source).variants.len()).any(|i| resolve(&c, i).is_some())
}

fn action_phrase(sink: SinkId, behavior: Behavior, lang: Lang) -> &'static str {
    match (sink, behavior) {
        (SinkId::Led, Behavior::Blink) => {
            tr(lang, "мигающий светодиод", "a blinking LED")
        }
        (SinkId::Led, _) => tr(lang, "светодиод по условию", "an LED on condition"),
        (SinkId::Buzzer, _) => tr(lang, "пищалка по порогу", "a buzzer on threshold"),
        (SinkId::Relay, _) => tr(lang, "реле с нагрузкой", "a relay with a load"),
        (SinkId::Lcd, _) => tr(lang, "вывод на LCD1602", "an LCD1602 readout"),
        (SinkId::Lcd2004, _) => tr(lang, "вывод на LCD2004", "an LCD2004 readout"),
        (SinkId::Oled, _) => tr(lang, "вывод на OLED", "an OLED readout"),
        (SinkId::Servo, _) => tr(lang, "серво по условию", "a servo on condition"),
        (SinkId::Motor, _) => tr(lang, "мотор по условию", "a motor on condition"),
        (SinkId::Rgb, _) => tr(lang, "цвет по условию", "a colour on condition"),
        (SinkId::Serial, _) => tr(lang, "монитор порта", "the serial monitor"),
    }
}

fn level_name(n: u8, lang: Lang) -> String {
    match (n, lang) {
        (1, Lang::Ru) => "низкая".to_string(),
        (1, Lang::En) => "low".to_string(),
        (2, Lang::Ru) => "средняя".to_string(),
        (2, Lang::En) => "medium".to_string(),
        (_, Lang::Ru) => "высокая".to_string(),
        (_, Lang::En) => "high".to_string(),
    }
}

fn number(v: f32, decimals: u8) -> String {
    if decimals == 0 {
        format!("{:.0}", v)
    } else {
        format!("{:.*}", decimals as usize, v)
    }
}

fn choice(r: &Resolved, lang: Lang) -> Choice {
    let mut cons = Vec::new();
    if r.behavior != Behavior::Show && r.spoken.is_some() && r.variant.logic == Logic::Flag {
        cons.push(L::new(
            "Число из фразы не попадает в скетч: этот порог задаёт подстроечник на модуле",
            "The number from the sentence does not reach the sketch: the trimmer on the module sets this threshold",
        ));
    }
    let current = format!(
        "{} мА",
        r.variant_current() + sink(r.sink).current_ma
    );
    let effort = templates::spec_line(
        &level_name(r.variant.level, lang),
        L::new("Сложность", "Effort"),
    );
    let (pros, specs) = if r.sink == SinkId::Serial {
        (
            vec![L::new(
                "Значение видно в мониторе порта: ни одного пина не занято",
                "The value goes to the serial monitor, using no pins",
            )],
            vec![
                templates::spec_line(&r.variant.name.get(lang), L::new("Чтение", "Reading")),
                templates::spec_line("9600", L::new("Порт", "Port")),
                effort,
            ],
        )
    } else if r.behavior == Behavior::Show {
        let screen = sink(r.sink).title.get(lang);
        let screen = if r.screens > 1 {
            format!("{screen} ×{}", r.screens)
        } else {
            screen.to_string()
        };
        (
            vec![L::new(
                "Значение видно на экране, порт остаётся свободным",
                "The value shows on the screen and the port stays free",
            )],
            vec![
                templates::spec_line("I2C: A4, A5", L::new("Шина", "Bus")),
                templates::spec_line(&screen, L::new("Экран", "Screen")),
                templates::spec_line(&current, L::new("Ток", "Current")),
                effort,
            ],
        )
    } else {
        let decimals = r.variant.decimals;
        let unit = r.variant.unit.get(lang);
        let threshold = match r.condition {
            Condition::Above(v) | Condition::Below(v) => {
                format!("{} {}", number(v, decimals), unit)
            }
            Condition::AboveBand { on, off } | Condition::BelowBand { on, off } => {
                format!(
                    "{} → {} {}",
                    number(on, decimals),
                    number(off, decimals),
                    unit
                )
            }
            Condition::Between { low, high } => format!(
                "{}…{} {}",
                number(low, decimals),
                number(high, decimals),
                unit
            ),
            Condition::Active => unit.to_string(),
        };
        (
            vec![
                L::new(
                    "Значение видно в мониторе порта",
                    "The value goes to the serial monitor",
                ),
                r.condition.label(),
            ],
            vec![
                templates::spec_line(&threshold, L::new("Порог", "Threshold")),
                templates::spec_line(&r.sink_pins.join(", "), L::new("Выход", "Output")),
                templates::spec_line(&current, L::new("Ток", "Current")),
                effort,
            ],
        )
    };
    Choice {
        name: r.variant.name,
        blurb: r.variant.blurb,
        pros,
        cons,
        specs,
        level: r.variant.level,
    }
}

impl Resolved<'_> {
    fn variant_current(&self) -> u16 {
        match self.source.place {
            Place::Board { name, .. } => crate::modules::find(name).map(|m| m.current_ma).unwrap_or(0),
            _ => 0,
        }
    }
}

pub fn plan(c: Composition, lang: Lang) -> Option<Plan> {
    let source = source(c.source);
    let mut choices = Vec::new();
    for i in 0..source.variants.len() {
        if let Some(r) = resolve(&c, i) {
            choices.push(choice(&r, lang));
        }
    }
    if choices.is_empty() {
        return None;
    }
    let headline = format!(
        "{}: {}",
        source.title.get(lang),
        action_phrase(c.sink, c.behavior, lang)
    );
    Some(Plan {
        tid: Tid::Composed,
        title: L::new("Схема по описанию", "A composed circuit"),
        headline: Some(headline),
        choices,
        recommended: 0,
        why: None,
    })
}

pub fn build(c: Composition, choice: usize, lang: Lang) -> Option<Built> {
    let plan = plan(c, lang)?;
    let choice = choice.min(plan.choices.len().saturating_sub(1));
    let r = resolve(&c, choice)?;
    let circuit = circuit(&r, lang)?;
    let code = sketch(&r, lang);
    let mut notes = vec![r.variant.note];
    notes.push(match r.sink {
        SinkId::Led => L::new(
            "Светодиод через 220 Ом: ток канала около 13 мА.",
            "The LED has a 220 Ω resistor: about 13 mA per channel.",
        ),
        SinkId::Buzzer => L::new(
            "Пьезо без генератора: частоту задаёт tone().",
            "The piezo has no oscillator: tone() sets the frequency.",
        ),
        SinkId::Relay => L::new(
            "Модуль реле активен низким уровнем; контакты COM и NO развязаны.",
            "The relay module is active low; COM and NO are isolated contacts.",
        ),
        SinkId::Lcd => L::new(
            "Экран на шине I2C: SDA на A4, SCL на A5, адрес 0x27 проверь сканером шины.",
            "The screen sits on I2C: SDA on A4, SCL on A5; check the 0x27 address with a bus scanner.",
        ),
        SinkId::Lcd2004 => L::new(
            "Экран 20x4 на шине I2C: SDA на A4, SCL на A5, адрес 0x27.",
            "The 20x4 screen sits on I2C: SDA on A4, SCL on A5, address 0x27.",
        ),
        SinkId::Oled => L::new(
            "OLED на шине I2C: SDA на A4, SCL на A5; адрес бывает 0x3C или 0x3D.",
            "The OLED sits on I2C: SDA on A4, SCL on A5; its address is 0x3C or 0x3D.",
        ),
        SinkId::Servo => L::new(
            "Серво поворачивается в 90° по условию и возвращается в 0. SG90 питается от 5 В.",
            "The servo turns to 90° while the condition holds and back to 0. An SG90 runs on 5 V.",
        ),
        SinkId::Motor => L::new(
            "Мотор включает MOSFET низкой стороны; диод гасит выброс обмотки.",
            "A low-side MOSFET switches the motor; the diode absorbs the coil's kickback.",
        ),
        SinkId::Serial => L::new(
            "Скетч печатает значение в монитор порта на 9600 бод; ни один пин не занят.",
            "The sketch prints the value to the serial monitor at 9600 baud; no pin is used.",
        ),
        SinkId::Rgb => L::new(
            "RGB-светодиод: красный и зелёный с общим катодом, по резистору на канал.",
            "The RGB LED has red and green channels and a common cathode, one resistor each.",
        ),
    });
    // A screen has no threshold to tune: the reading goes out as it is.
    if r.behavior == Behavior::Show && r.screens > 1 {
        notes.push(L::new(
            "Несколько экранов на одной шине I2C: задай им разные адреса перемычками, иначе они ответят одновременно.",
            "Several screens on one I2C bus need different addresses set by their jumpers, or they answer together.",
        ));
    }
    if r.behavior != Behavior::Show {
        notes.push(match r.condition {
            Condition::Above(_) | Condition::Below(_) => L::new(
                "Порог можно подобрать по выводу в мониторе порта.",
                "Tune the threshold from the serial monitor.",
            ),
            Condition::AboveBand { .. } | Condition::BelowBand { .. } => L::new(
                "Гистерезис не даёт дребезжать у порога: вход на одном значении, выход на другом.",
                "Hysteresis stops chatter at the threshold: it comes on at one value and off at another.",
            ),
            Condition::Between { .. } => L::new(
                "Оба условия проверяются на одном чтении: срабатывание только внутри диапазона.",
                "Both conditions read the same sample: it fires only inside the range.",
            ),
            Condition::Active => L::new(
                "Условие — уровень с модуля: подстроечник задаёт его на месте.",
                "The condition is the module's level; its trimmer sets it on the spot.",
            ),
        });
    }
    Some(Built {
        tid: Tid::Composed,
        title: plan.title_text(lang),
        circuit,
        code,
        choice,
        summary: r.source.summary.get(lang).to_string(),
        notes,
        explained_pins: Vec::new(),
    })
}

fn circuit(r: &Resolved, lang: Lang) -> Option<Circuit> {
    let mut b = B::new(lang);
    place_source(&mut b, r.source, r.variant)?;
    place_sink(&mut b, r)?;
    templates::feeds(&mut b);
    Some(b.finish())
}

/// Drop a ready-made board on the bench and wire its supply pins. Its signal
/// pins are left to the caller: a source and a screen bring their own.
fn place_board(b: &mut B, label: &str, name: &str, at: (f32, f32)) -> usize {
    let part = b.breakout(label, name, Pt::new(at.0, at.1));
    if let Some(m) = crate::modules::find(name) {
        for (pin, dx, _) in m.pins.iter().copied() {
            match pin {
                "GND" => {
                    let g = b.gnd(at.0 + dx);
                    b.w(b.mp(part, pin), g, NetClass::Gnd);
                }
                "VCC" | "+5V" | "+12V" | "+" => {
                    if m.three_three {
                        b.w(b.uno_pin("3V3"), b.mp(part, pin), NetClass::Vcc);
                    } else {
                        let p = b.vcc(at.0 + dx);
                        b.w(b.mp(part, pin), p, NetClass::Vcc);
                    }
                }
                "3V3" => {
                    b.w(b.uno_pin("3V3"), b.mp(part, pin), NetClass::Vcc);
                }
                _ => {}
            }
        }
    }
    part
}

fn place_source(b: &mut B, source: &SourceSpec, v: &Variant) -> Option<()> {
    match source.place {
        Place::Board { name, at } => {
            let part = place_board(b, "U2", name, at);
            for (pin, target) in v.signals.iter().copied() {
                if b.c.parts[part]
                    .pins
                    .iter()
                    .any(|&p| b.c.pins[p].name == pin)
                {
                    let class = if target.starts_with('A') {
                        NetClass::Analog
                    } else {
                        NetClass::Signal
                    };
                    b.w(b.mp(part, pin), b.uno_pin(target), class);
                }
            }
        }
        Place::Ldr => {
            let (a, z) = b.axial(Kind::Ldr, "LDR1", "GL5528", 10, Row::C, 2);
            let (ra, rb) = b.res("R1", "10 кОм", 10, Row::E, 3);
            let vcc = b.vcc(8.0);
            let gnd = b.gnd(16.0);
            b.w(vcc, a, NetClass::Vcc);
            b.w(z, ra, NetClass::Analog);
            b.w(rb, gnd, NetClass::Gnd);
            b.w(z, b.uno_pin("A0"), NetClass::Analog);
        }
        Place::Thermistor => {
            let (a, z) = b.axial(Kind::Thermistor, "TH1", "NTC 10 кОм", 10, Row::C, 2);
            let (ra, rb) = b.res("R1", "10 кОм", 10, Row::E, 3);
            let vcc = b.vcc(8.0);
            let gnd = b.gnd(16.0);
            b.w(vcc, a, NetClass::Vcc);
            b.w(z, ra, NetClass::Analog);
            b.w(rb, gnd, NetClass::Gnd);
            b.w(z, b.uno_pin("A0"), NetClass::Analog);
        }
    }
    Some(())
}

fn place_sink(b: &mut B, r: &Resolved) -> Option<()> {
    // The serial monitor is an output inside the board: it has no part and
    // no pin to place.
    if r.sink == SinkId::Serial {
        return Some(());
    }
    let pin = r.sink_pins.first().copied()?;
    match r.sink {
        SinkId::Led => {
            let (ra, _rb) = b.res("R1", "220 Ом", 20, Row::A, 3);
            let (_a, k) = b.led("D1", "красный 5 мм", 23, Row::E);
            b.w(b.uno_pin(pin), ra, NetClass::Signal);
            b.w(k, b.gnd(52.0), NetClass::Gnd);
        }
        SinkId::Buzzer => {
            // A bench part talks to the Uno headers directly, so its power
            // does not travel across the breadboard.
            let z = b.bench(Kind::Buzzer, "HA1", "пьезо 5 В", Pt::new(50.0, 54.0));
            b.w(b.uno_pin(pin), z[1], NetClass::Signal);
            b.w(z[0], b.uno_pin("GND1"), NetClass::Gnd);
        }
        SinkId::Relay => {
            let rl = b.bench(Kind::Relay, "K1", "SRD-05VDC", Pt::new(44.0, 54.0));
            let siren = b.bench(Kind::Buzzer, "HA1", "сирена 5 В", Pt::new(84.0, 80.0));
            b.w(b.uno_pin(pin), rl[0], NetClass::Signal);
            b.w(rl[1], b.uno_pin("5V"), NetClass::Vcc);
            b.w(rl[2], b.uno_pin("GND1"), NetClass::Gnd);
            b.w(rl[3], b.uno_pin("5V"), NetClass::Vcc);
            b.w(rl[4], siren[1], NetClass::Signal);
            b.w(siren[0], b.uno_pin("GND1"), NetClass::Gnd);
        }
        SinkId::Servo => {
            let s = b.bench(Kind::Servo, "S1", "SG90", Pt::new(50.0, 54.0));
            b.w(b.uno_pin(pin), s[2], NetClass::Signal);
            b.w(s[1], b.uno_pin("5V"), NetClass::Vcc);
            b.w(s[0], b.uno_pin("GND1"), NetClass::Gnd);
        }
        SinkId::Motor => {
            // The lesson's low-side switch, moved to the free half of the
            // bench so the source board keeps its corner.
            let (ra, rb) = b.res("R3", "220 Ом", 21, Row::A, 3);
            let q = b.to92(Kind::Nmos, "Q1", "IRLZ44N", 25, Row::E, ["G", "S", "D"]);
            let (_r4a, r4b) = b.res("R4", "10 кОм", 25, Row::A, 3);
            let m = b.bench(Kind::Motor, "M1", "5 В DC", Pt::new(52.0, 54.0));
            let d = b.diode("D1", "1N4007", 52.0, 82.0);
            let vcc = b.vcc(82.0);
            b.w(b.uno_pin(pin), ra, NetClass::Signal);
            b.w(rb, q[0], NetClass::Signal);
            b.w(r4b, b.gnd(74.0), NetClass::Gnd);
            b.w(q[1], b.gnd(70.0), NetClass::Gnd);
            b.w(q[2], m[1], NetClass::Signal);
            b.w(m[0], vcc, NetClass::Vcc);
            b.w(d.1, m[0], NetClass::Vcc);
            b.w(d.0, m[1], NetClass::Signal);
        }
        SinkId::Rgb => {
            // Three channels, three resistors: the strip joins each resistor
            // to its own leg of the LED.
            let (ra, rb) = b.res("R3", "220 Ом", 20, Row::A, 3);
            let (ga, gb) = b.res("R4", "220 Ом", 21, Row::B, 3);
            let (ba, bb) = b.res("R5", "220 Ом", 22, Row::C, 3);
            let led = b.rgb("D1", "RGB общий катод", 26, Row::E);
            for (target, start) in r.sink_pins.iter().zip([ra, ga, ba]) {
                b.w(b.uno_pin(target), start, NetClass::Signal);
            }
            for (end, leg) in [(rb, led[0]), (gb, led[1]), (bb, led[2])] {
                b.w(end, leg, NetClass::Signal);
            }
            b.w(led[3], b.gnd(52.0), NetClass::Gnd);
        }
        SinkId::Serial => {}
        SinkId::Lcd | SinkId::Lcd2004 | SinkId::Oled => {
            let spec = sink(r.sink);
            let name = spec.module?;
            // The sentence may ask for two screens: every board joins the
            // same A4/A5 bus, and the drawing must show as many as the plan
            // promised. One screen keeps its usual place on the bench.
            let spots = if r.screens <= 1 {
                vec![Pt::new(spec.at.0, spec.at.1)]
            } else {
                let top = b.c.bbox().bottom() + 8.0;
                templates::screen_spots(r.screens, name, top)
            };
            let sda = *r.sink_pins.first()?;
            let scl = *r.sink_pins.get(1)?;
            for (n, at) in spots.iter().enumerate() {
                let part = place_board(b, &format!("DS{}", n + 1), name, (at.x, at.y));
                // The supply is already on the rails; the bus lines go to the
                // shared A4/A5 pair, where the source may already sit.
                for (pin_name, target) in [("SDA", sda), ("SCL", scl)] {
                    if b.c.parts[part]
                        .pins
                        .iter()
                        .any(|&p| b.c.pins[p].name == pin_name)
                    {
                        b.w(b.mp(part, pin_name), b.uno_pin(target), NetClass::Signal);
                    }
                }
            }
        }
    }
    Some(())
}

/// Join two sketch fragments, keeping a line only once: an I2C source and an
/// I2C screen both ask for `#include <Wire.h>`.
fn join_unique(a: &str, b: &str) -> String {
    let mut out = String::new();
    for line in a.lines().chain(b.lines()) {
        if line.is_empty() || out.lines().any(|seen| seen == line) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// One object, one initialisation and one show block per screen, each on its
/// own I2C address. A composition that names two screens must print on both,
/// not only on the first board in the drawing.
fn screen_fragments(spec: &SinkSpec, count: usize) -> (String, String, String) {
    let oled = spec.id == SinkId::Oled;
    let base = if oled { "display" } else { "lcd" };
    let mut globals = String::new();
    let mut setup = String::new();
    let mut show = String::new();
    for n in 0..count {
        let name = format!("{base}{n}");
        let addr = if oled {
            0x3C + n as u8
        } else {
            0x27u8.wrapping_sub(n as u8)
        };
        let addr = format!("0x{addr:02X}");
        globals.push_str(&spec.globals.replace(base, &name).replace("0x27", &addr));
        let mut init = spec.setup.replace(base, &name);
        if oled {
            init = init.replace("0x3C", &addr);
        }
        if n > 0 {
            init = init.replace("  Wire.begin();\n", "");
        }
        setup.push_str(&init);
        show.push_str(&spec.show.replace(base, &name));
    }
    (globals, setup, show)
}

fn sketch(r: &Resolved, lang: Lang) -> String {
    let spec = sink(r.sink);
    let show = r.behavior == Behavior::Show;
    let (extra, active, action) = if show {
        // A screen takes the reading as it is: there is nothing to compare
        // and no output pin to drive.
        (String::new(), String::new(), spec.show.to_string())
    } else {
        let decimals = r.variant.decimals;
        let (persist, active) = match r.condition {
            Condition::Above(v) => (
                String::new(),
                format!("  bool active = value > {};\n", number(v, decimals)),
            ),
            Condition::Below(v) => (
                String::new(),
                format!("  bool active = value < {};\n", number(v, decimals)),
            ),
            Condition::Active => (String::new(), "  bool active = value != 0;\n".to_string()),
            // A band keeps its state between polls, so `active` lives next to
            // `last` instead of being recomputed from one reading.
            Condition::AboveBand { on, off } => (
                "bool active = false;\n".to_string(),
                format!(
                    "  if (!active && value > {}) active = true;\n  if (active && value < {}) active = false;\n",
                    number(on, decimals),
                    number(off, decimals)
                ),
            ),
            Condition::BelowBand { on, off } => (
                "bool active = false;\n".to_string(),
                format!(
                    "  if (!active && value < {}) active = true;\n  if (active && value > {}) active = false;\n",
                    number(on, decimals),
                    number(off, decimals)
                ),
            ),
            Condition::Between { low, high } => (
                String::new(),
                format!(
                    "  bool active = value > {} && value < {};\n",
                    number(low, decimals),
                    number(high, decimals)
                ),
            ),
        };
        let (extra, action) = match (r.sink, r.behavior) {
            (SinkId::Led, Behavior::Blink) => (
                "bool lit = false;\n",
                "  if (active) {\n    lit = !lit;\n    digitalWrite(SINK_PIN, lit ? HIGH : LOW);\n  } else {\n    lit = false;\n    digitalWrite(SINK_PIN, LOW);\n  }\n",
            ),
            (SinkId::Led, _) => (
                "",
                "  digitalWrite(SINK_PIN, active ? HIGH : LOW);\n",
            ),
            (SinkId::Buzzer, _) => (
                "",
                "  if (active) {\n    tone(SINK_PIN, 1200);\n  } else {\n    noTone(SINK_PIN);\n  }\n",
            ),
            (SinkId::Relay, _) => (
                "",
                "  digitalWrite(SINK_PIN, active ? LOW : HIGH);\n",
            ),
            (SinkId::Servo, _) => ("", "  arm.write(active ? 90 : 0);\n"),
            (SinkId::Motor, _) => ("", "  analogWrite(SINK_PIN, active ? 255 : 0);\n"),
            (SinkId::Rgb, _) => (
                "",
                "  digitalWrite(R_PIN, active ? HIGH : LOW);\n  digitalWrite(G_PIN, active ? LOW : HIGH);\n  digitalWrite(B_PIN, active ? LOW : HIGH);\n",
            ),
            // Screens are handled by the branch above; this keeps the match
            // exhaustive without pretending a display has a threshold.
            (SinkId::Lcd | SinkId::Lcd2004 | SinkId::Oled, _) => ("", ""),
            (SinkId::Serial, _) => ("", ""),
        };
        (format!("{persist}{extra}"), active, action.to_string())
    };
    let (pins, output) = if show {
        (String::new(), String::new())
    } else if r.sink == SinkId::Rgb {
        let numbers: Vec<&str> = r
            .sink_pins
            .iter()
            .map(|p| p.trim_start_matches('D'))
            .collect();
        (
            format!(
                "const uint8_t R_PIN = {};\nconst uint8_t G_PIN = {};\nconst uint8_t B_PIN = {};\n",
                numbers[0], numbers[1], numbers[2]
            ),
            "  pinMode(R_PIN, OUTPUT);\n  pinMode(G_PIN, OUTPUT);\n  pinMode(B_PIN, OUTPUT);\n"
                .to_string(),
        )
    } else {
        // The servo library claims its pin in attach(), so a pinMode here
        // would only fight it.
        let output = if r.sink == SinkId::Servo {
            String::new()
        } else {
            "  pinMode(SINK_PIN, OUTPUT);\n".to_string()
        };
        (
            format!(
                "const uint8_t SINK_PIN = {};\n",
                r.sink_pins[0].trim_start_matches('D')
            ),
            output,
        )
    };
    let includes = join_unique(r.variant.includes, spec.includes);
    // Two screens must both receive the reading: each gets its own object and
    // its own address on the bus.
    let (globals, setup, action) = if show && r.screens > 1 {
        let (g, s, a) = screen_fragments(spec, r.screens);
        (
            join_unique(r.variant.globals, &g),
            format!("{}{}", r.variant.setup, s),
            a,
        )
    } else {
        (
            join_unique(r.variant.globals, spec.globals),
            format!("{}{}", r.variant.setup, spec.setup),
            action,
        )
    };
    let title = format!(
        "{}: {}",
        r.source.title.get(lang),
        action_phrase(r.sink, r.behavior, lang)
    );
    let mut s = String::from(
        r#"// <<C0>>
<<INCLUDES>><<GLOBALS>><<PINS>>const uint32_t PERIOD_MS = <<PERIOD>>;
uint32_t last = 0;
<<EXTRA>>
void setup() {
<<SETUP>><<SETUP_PIN>>  Serial.begin(9600);
}

void loop() {
  if (millis() - last < PERIOD_MS) return;
  last = millis();
<<SAMPLE>>
<<ACTIVE>>
<<ACTION>>
<<PRINT>>
}
"#,
    );
    for (k, v) in [
        ("C0", title.as_str()),
        ("INCLUDES", includes.as_str()),
        ("GLOBALS", globals.as_str()),
        ("PINS", pins.as_str()),
        ("EXTRA", extra.as_str()),
        ("SETUP", setup.as_str()),
        ("SETUP_PIN", output.as_str()),
        ("SAMPLE", r.variant.sample),
        ("ACTIVE", active.as_str()),
        ("ACTION", action.as_str()),
        ("PRINT", r.variant.print),
    ] {
        s = s.replace(&format!("<<{k}>>"), v);
    }
    s = s.replace("<<PERIOD>>", &r.source.period_ms.to_string());
    s = s.replace("<<NAME>>", r.source.short.get(lang));
    s = s.replace("<<DECIMALS>>", &r.variant.decimals.to_string());
    s
}

// ------------------------------------------------------- several sensors

/// One sensor on the bench: the table entry and the Uno pins this build gave
/// it. Several instances of the same source are separate slots, so a drawing
/// can show two DHT22 boards and a sketch can read both.
struct MultiSlot {
    source: &'static SourceSpec,
    /// Uno pins in the order of the variant's `signals`; for an LDR the one
    /// analogue input of its divider.
    pins: Vec<&'static str>,
    /// A board the sentence named instead of the source's default: the DHT
    /// source covers both DHT11 and DHT22, and the drawing must show the one
    /// whose name is in the requirement list.
    model: Option<&'static str>,
}

/// Sources whose several instances this build can both wire and read. A
/// source outside the list keeps the honest refusal: drawing it twice while
/// the sketch reads one board would be a false answer.
fn multi_supported(id: SourceId) -> bool {
    matches!(
        id,
        SourceId::Temperature
            | SourceId::Humidity
            | SourceId::Pressure
            | SourceId::Gas
            | SourceId::Soil
            | SourceId::Light
            | SourceId::Range
            | SourceId::Motion
    )
}

/// A DHT22 reads temperature and humidity from one board: that pair is one
/// sensor, not a multi-sensor build.
fn multi_needed(sensors: &[(SourceId, usize)]) -> bool {
    let total: usize = sensors.iter().map(|(_, n)| *n).sum();
    if total < 2 {
        return false;
    }
    let dht_pair = sensors.len() == 2
        && sensors
            .iter()
            .all(|(id, n)| *n == 1 && matches!(id, SourceId::Temperature | SourceId::Humidity));
    !dht_pair
}

fn take_pin(pool: &[&'static str], used: &mut Vec<&'static str>) -> Option<&'static str> {
    let pin = pool.iter().copied().find(|p| !used.contains(p))?;
    used.push(pin);
    Some(pin)
}

/// Give every sensor instance its own input. Analogue sensors share nothing,
/// two DS18B20 share the OneWire bus and BMP280 boards share I2C but answer
/// on different addresses.
fn multi_pins(sensors: &[(SourceId, usize)]) -> Option<Vec<MultiSlot>> {
    const DIGITAL: &[&str] = &[
        "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D10", "D11", "D12", "D13",
    ];
    const ANALOG: &[&str] = &["A0", "A1", "A2", "A3"];
    let mut used_digital: Vec<&'static str> = Vec::new();
    let mut used_analog: Vec<&'static str> = Vec::new();
    // Reserve the OneWire bus before anything else can take D2.
    if sensors.iter().any(|(id, _)| *id == SourceId::Temperature) {
        used_digital.push("D2");
    }
    let mut out = Vec::new();
    for &(id, count) in sensors {
        if count == 0 || !multi_supported(id) {
            return None;
        }
        let source = source(id);
        for _ in 0..count {
            let pins = match id {
                SourceId::Temperature => vec!["D2"],
                SourceId::Pressure => vec!["A4", "A5"],
                SourceId::Humidity | SourceId::Motion => {
                    vec![take_pin(DIGITAL, &mut used_digital)?]
                }
                SourceId::Range => vec![
                    take_pin(DIGITAL, &mut used_digital)?,
                    take_pin(DIGITAL, &mut used_digital)?,
                ],
                SourceId::Gas | SourceId::Soil | SourceId::Light => {
                    vec![take_pin(ANALOG, &mut used_analog)?]
                }
                _ => return None,
            };
            out.push(MultiSlot {
                source,
                pins,
                model: None,
            });
        }
    }
    Some(out)
}

/// The parser asks before it refuses a multi-sensor sentence, and the builder
/// asks the same question. The predicate therefore lives in one place: a
/// sentence is a plain measurement of the sensors it named, not a threshold
/// or an output that would need its own circuit.
pub fn multi_request(task: &crate::task::Task) -> Option<Vec<(SourceId, usize)>> {
    if !task.outputs.is_empty()
        || !task.screens.is_empty()
        || !task.conditions.is_empty()
        || !task.behaviors.is_empty()
        || task.text_screens.is_some()
        || task.transistor
        || task.presses
    {
        return None;
    }
    if task
        .controls
        .iter()
        .any(|c| c.kind != crate::parse::Input::Ldr)
    {
        return None;
    }
    let n = &task.named;
    if n.screens.is_some()
        || n.led > 0
        || n.lamp > 0
        || n.rgb > 0
        || n.buzzer > 0
        || n.relay > 0
        || n.motor > 0
        || n.servo > 0
        || n.button > 0
        || n.pin.is_some()
        || !n.pins.is_empty()
    {
        return None;
    }
    let sensors: Vec<(SourceId, usize)> = task
        .sensors
        .iter()
        .map(|s| (s.source, s.count))
        .collect();
    if !multi_needed(&sensors) {
        return None;
    }
    // Two BMP280 boards are the most the I2C bus can address by SDO.
    if sensors
        .iter()
        .any(|(id, count)| *count > 4 || (*id == SourceId::Pressure && *count > 2))
    {
        return None;
    }
    multi_pins(&sensors)?;
    Some(sensors)
}

fn multi_choice(sensors: &[(SourceId, usize)], lang: Lang) -> Choice {
    let total: usize = sensors.iter().map(|(_, n)| *n).sum();
    let lines: Vec<String> = sensors
        .iter()
        .map(|(id, n)| format!("{} ×{}", source(*id).title.get(lang), n))
        .collect();
    Choice {
        name: L::new("Несколько датчиков", "Several sensors"),
        blurb: L::new(
            "Каждый датчик на своём входе; значения по очереди идут в монитор порта.",
            "Every sensor gets its own input; the readings go to the serial monitor.",
        ),
        pros: vec![
            L::new(
                "Все названные датчики есть в схеме и в скетче",
                "Every named sensor is in the drawing and in the sketch",
            ),
            L::new(
                "Числа видно в мониторе порта на 9600 бод",
                "The numbers show in the serial monitor at 9600 baud",
            ),
        ],
        cons: Vec::new(),
        specs: vec![
            crate::templates::spec_line(&total.to_string(), L::new("Датчиков", "Sensors")),
            crate::templates::spec_line(&lines.join(", "), L::new("Состав", "Parts")),
        ],
        level: 2,
    }
}

pub fn multi_plan(
    sensors: &[(SourceId, usize)],
    humidity_model: Option<&'static str>,
    lang: Lang,
) -> Option<Plan> {
    multi_pins(sensors)?;
    let lines: Vec<String> = sensors
        .iter()
        .map(|(id, n)| {
            let name = if *id == SourceId::Humidity {
                humidity_model.unwrap_or("DHT22")
            } else {
                source(*id).title.get(lang)
            };
            format!("{name} ×{n}")
        })
        .collect();
    Some(Plan {
        tid: Tid::Composed,
        title: L::new("Схема по описанию", "A composed circuit"),
        headline: Some(format!(
            "{}: {}",
            tr(lang, "несколько датчиков", "several sensors"),
            lines.join(", ")
        )),
        choices: vec![multi_choice(sensors, lang)],
        recommended: 0,
        why: None,
    })
}

pub fn multi_build(
    sensors: &[(SourceId, usize)],
    humidity_model: Option<&'static str>,
    lang: Lang,
) -> Option<Built> {
    let mut slots = multi_pins(sensors)?;
    for slot in &mut slots {
        if slot.source.id == SourceId::Humidity {
            slot.model = humidity_model;
        }
    }
    let circuit = multi_circuit(&slots, lang);
    let (code, notes) = multi_sketch(&slots, lang);
    let lines: Vec<String> = sensors
        .iter()
        .map(|(id, n)| {
            let name = if *id == SourceId::Humidity {
                humidity_model.unwrap_or("DHT22")
            } else {
                source(*id).title.get(lang)
            };
            format!("{name} ×{n}")
        })
        .collect();
    Some(Built {
        tid: Tid::Composed,
        title: format!(
            "{}: {}",
            tr(lang, "несколько датчиков", "several sensors"),
            lines.join(", ")
        ),
        circuit,
        code,
        choice: 0,
        summary: format!(
            "{}: {}.",
            tr(lang, "Датчики на схеме", "Sensors on the bench"),
            lines.join(", ")
        ),
        notes,
        explained_pins: Vec::new(),
    })
}

fn multi_circuit(slots: &[MultiSlot], lang: Lang) -> Circuit {
    let mut b = B::new(lang);
    let mut label = 2u8;
    let (mut x, mut y, mut row_h) = (2.0f32, 54.0f32, 0.0f32);
    let mut ldr = 0u8;
    for slot in slots {
        if matches!(slot.source.place, Place::Ldr) {
            // Each photoresistor gets its own divider: LDR to 5 V, a 10 kΩ
            // resistor to ground, and the middle to its analogue input.
            // Seven columns per divider: the next LDR must not land on the
            // column where the previous resistor meets ground.
            let c = 2 + ldr * 7;
            let (la, lz) = b.axial(
                Kind::Ldr,
                &format!("LDR{}", ldr + 1),
                "GL5528",
                c,
                Row::C,
                1,
            );
            let (ra, rb) = b.res(&format!("R{}", ldr + 1), "10 кОм", c + 3, Row::E, 3);
            let vcc = b.vcc(8.0 + f32::from(ldr) * 12.0);
            let gnd = b.gnd(16.0 + f32::from(ldr) * 12.0);
            b.w(vcc, la, NetClass::Vcc);
            b.w(lz, ra, NetClass::Analog);
            b.w(rb, gnd, NetClass::Gnd);
            b.w(lz, b.uno_pin(slot.pins[0]), NetClass::Analog);
            ldr += 1;
            continue;
        }
        let Place::Board { name, .. } = slot.source.place else {
            continue;
        };
        let name = slot.model.unwrap_or(name);
        let (w, h) = crate::modules::find(name)
            .map(|m| (m.w, m.h))
            .unwrap_or((30.0, 24.0));
        if x > 2.0 && x + w > 78.0 {
            x = 2.0;
            y += row_h + 6.0;
            row_h = 0.0;
        }
        let part = place_board(&mut b, &format!("U{label}"), name, (x, y));
        label += 1;
        for ((pin, _), target) in slot.source.variants[0]
            .signals
            .iter()
            .zip(slot.pins.iter())
        {
            if b.c.parts[part]
                .pins
                .iter()
                .any(|&p| b.c.pins[p].name == *pin)
            {
                let class = if target.starts_with('A') {
                    NetClass::Analog
                } else {
                    NetClass::Signal
                };
                b.w(b.mp(part, pin), b.uno_pin(target), class);
            }
        }
        x += w + 6.0;
        row_h = row_h.max(h);
    }
    templates::feeds(&mut b);
    b.finish()
}

fn slots_of<'a>(slots: &'a [MultiSlot], id: SourceId) -> Vec<&'a MultiSlot> {
    slots.iter().filter(|s| s.source.id == id).collect()
}

fn add_include(out: &mut String, line: &str) {
    if !out.lines().any(|seen| seen == line) {
        out.push_str(line);
        out.push('\n');
    }
}

fn multi_sketch(slots: &[MultiSlot], lang: Lang) -> (String, Vec<L>) {
    let mut includes = String::new();
    let mut globals = String::new();
    let mut setup = String::new();
    let mut sample = String::new();
    let mut print = String::new();
    let mut notes = vec![L::new(
        "Каждый датчик занял свой вход; числа по очереди идут в монитор порта на 9600 бод.",
        "Every sensor has its own input; the readings go to the serial monitor at 9600 baud.",
    )];
    let mut period = 100u32;
    let mut note = |l: L| {
        if !notes.iter().any(|seen| seen.ru == l.ru) {
            notes.push(l);
        }
    };

    let temperatures = slots_of(slots, SourceId::Temperature);
    if !temperatures.is_empty() {
        add_include(&mut includes, "#include <OneWire.h>");
        add_include(&mut includes, "#include <DallasTemperature.h>");
        let bus = temperatures[0].pins[0].trim_start_matches('D');
        globals.push_str(&format!(
            "const uint8_t ONE_WIRE = {bus};\nOneWire wire(ONE_WIRE);\nDallasTemperature sensors(&wire);\n"
        ));
        setup.push_str("  sensors.begin();\n");
        sample.push_str("  sensors.requestTemperatures();\n");
        for (i, _) in temperatures.iter().enumerate() {
            sample.push_str(&format!(
                "  float temp{} = sensors.getTempCByIndex({i});\n",
                i + 1
            ));
            print.push_str(&format!(
                "  Serial.print(F(\"temp{} \"));\n  Serial.println(temp{}, 1);\n",
                i + 1,
                i + 1
            ));
        }
        period = period.max(1000);
        if temperatures.len() > 1 {
            note(L::new(
                "Несколько DS18B20 висят на одной шине OneWire: у каждого свой адрес, скетч читает их по индексу.",
                "Several DS18B20 share one OneWire bus: each has its own address and the sketch reads them by index.",
            ));
        }
    }

    let humidities = slots_of(slots, SourceId::Humidity);
    if !humidities.is_empty() {
        add_include(&mut includes, "#include <DHT.h>");
        let model = humidities[0].model.unwrap_or("DHT22");
        for (i, slot) in humidities.iter().enumerate() {
            let n = i + 1;
            let pin = slot.pins[0].trim_start_matches('D');
            globals.push_str(&format!(
                "const uint8_t DHT{n}_PIN = {pin};\nDHT dht{n}(DHT{n}_PIN, {model});\n"
            ));
            setup.push_str(&format!("  dht{n}.begin();\n"));
            sample.push_str(&format!("  float hum{n} = dht{n}.readHumidity();\n"));
            print.push_str(&format!(
                "  Serial.print(F(\"hum{n} \"));\n  Serial.println(hum{n}, 1);\n"
            ));
        }
        period = period.max(2000);
        if humidities.len() > 1 {
            note(L::new(
                "У каждого датчика влажности свой DATA-пин; опрос не чаще раза в две секунды.",
                "Every humidity sensor has its own DATA pin; poll no more than once every two seconds.",
            ));
        }
    }

    let pressures = slots_of(slots, SourceId::Pressure);
    if !pressures.is_empty() {
        add_include(&mut includes, "#include <Wire.h>");
        add_include(&mut includes, "#include <Adafruit_BMP280.h>");
        for (i, _) in pressures.iter().enumerate() {
            globals.push_str(&format!("Adafruit_BMP280 bmp{};\n", i + 1));
        }
        setup.push_str("  Wire.begin();\n");
        for (i, _) in pressures.iter().enumerate() {
            let n = i + 1;
            let addr = if i == 0 { 0x76 } else { 0x77 };
            setup.push_str(&format!(
                "  if (!bmp{n}.begin(0x{addr:02X})) Serial.println(F(\"no bmp{n}\"));\n"
            ));
            sample.push_str(&format!(
                "  float press{n} = bmp{n}.readPressure() / 100.0;\n"
            ));
            print.push_str(&format!(
                "  Serial.print(F(\"press{n} \"));\n  Serial.println(press{n}, 2);\n"
            ));
        }
        period = period.max(1000);
        if pressures.len() > 1 {
            note(L::new(
                "Два BMP280 на одной шине I2C: адреса 0x76 и 0x77 задаёт перемычка SDO на плате.",
                "Two BMP280 boards share I2C: the SDO jumper on each sets 0x76 and 0x77.",
            ));
        }
    }

    let gases = slots_of(slots, SourceId::Gas);
    if !gases.is_empty() {
        for (i, slot) in gases.iter().enumerate() {
            let n = i + 1;
            globals.push_str(&format!(
                "const uint8_t GAS{n}_PIN = {};\n",
                slot.pins[0]
            ));
            sample.push_str(&format!("  float gas{n} = analogRead(GAS{n}_PIN);\n"));
            print.push_str(&format!(
                "  Serial.print(F(\"gas{n} \"));\n  Serial.println(gas{n}, 0);\n"
            ));
            note(slot.source.variants[0].note);
        }
        period = period.max(u32::from(source(SourceId::Gas).period_ms));
    }

    let soils = slots_of(slots, SourceId::Soil);
    if !soils.is_empty() {
        for (i, slot) in soils.iter().enumerate() {
            let n = i + 1;
            globals.push_str(&format!(
                "const uint8_t SOIL{n}_PIN = {};\n",
                slot.pins[0]
            ));
            sample.push_str(&format!(
                "  float soil{n} = map(analogRead(SOIL{n}_PIN), 1023, 0, 0, 100);\n"
            ));
            print.push_str(&format!(
                "  Serial.print(F(\"soil{n} \"));\n  Serial.println(soil{n}, 0);\n"
            ));
            note(slot.source.variants[0].note);
        }
        period = period.max(u32::from(source(SourceId::Soil).period_ms));
    }

    let lights = slots_of(slots, SourceId::Light);
    if !lights.is_empty() {
        for (i, slot) in lights.iter().enumerate() {
            let n = i + 1;
            globals.push_str(&format!(
                "const uint8_t LDR{n}_PIN = {};\n",
                slot.pins[0]
            ));
            sample.push_str(&format!("  float ldr{n} = analogRead(LDR{n}_PIN);\n"));
            print.push_str(&format!(
                "  Serial.print(F(\"ldr{n} \"));\n  Serial.println(ldr{n}, 0);\n"
            ));
            note(slot.source.variants[0].note);
        }
        period = period.max(u32::from(source(SourceId::Light).period_ms));
    }

    let ranges = slots_of(slots, SourceId::Range);
    if !ranges.is_empty() {
        for (i, slot) in ranges.iter().enumerate() {
            let n = i + 1;
            let trig = slot.pins[0].trim_start_matches('D');
            let echo = slot.pins[1].trim_start_matches('D');
            globals.push_str(&format!(
                "const uint8_t TRIG{n} = {trig};\nconst uint8_t ECHO{n} = {echo};\n"
            ));
            setup.push_str(&format!(
                "  pinMode(TRIG{n}, OUTPUT);\n  pinMode(ECHO{n}, INPUT);\n"
            ));
            sample.push_str(&format!(
                "  digitalWrite(TRIG{n}, LOW);\n  delayMicroseconds(2);\n  digitalWrite(TRIG{n}, HIGH);\n  delayMicroseconds(10);\n  digitalWrite(TRIG{n}, LOW);\n  unsigned long us{n} = pulseIn(ECHO{n}, HIGH, 30000UL);\n  float dist{n} = us{n} / 58.0;\n"
            ));
            print.push_str(&format!(
                "  Serial.print(F(\"dist{n} \"));\n  Serial.println(dist{n}, 1);\n"
            ));
            note(slot.source.variants[0].note);
        }
        period = period.max(u32::from(source(SourceId::Range).period_ms));
    }

    let motions = slots_of(slots, SourceId::Motion);
    if !motions.is_empty() {
        for (i, slot) in motions.iter().enumerate() {
            let n = i + 1;
            let pin = slot.pins[0].trim_start_matches('D');
            globals.push_str(&format!(
                "const uint8_t MOTION{n}_PIN = {pin};\n"
            ));
            setup.push_str(&format!("  pinMode(MOTION{n}_PIN, INPUT);\n"));
            sample.push_str(&format!(
                "  float motion{n} = digitalRead(MOTION{n}_PIN) ? 1.0 : 0.0;\n"
            ));
            print.push_str(&format!(
                "  Serial.print(F(\"motion{n} \"));\n  Serial.println(motion{n}, 0);\n"
            ));
            note(slot.source.variants[0].note);
        }
        period = period.max(u32::from(source(SourceId::Motion).period_ms));
    }

    let mut code = String::from(
        r#"// <<C0>>
<<INCLUDES>><<GLOBALS>>const uint32_t PERIOD_MS = <<PERIOD>>;
uint32_t last = 0;

void setup() {
<<SETUP>>  Serial.begin(9600);
}

void loop() {
  if (millis() - last < PERIOD_MS) return;
  last = millis();
<<SAMPLE>><<PRINT>>}
"#,
    );
    let title = format!(
        "{}: {}",
        tr(lang, "несколько датчиков", "several sensors"),
        slots
            .iter()
            .map(|s| s.source.short.get(lang))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for (k, v) in [
        ("C0", title.as_str()),
        ("INCLUDES", includes.as_str()),
        ("GLOBALS", globals.as_str()),
        ("SETUP", setup.as_str()),
        ("SAMPLE", sample.as_str()),
        ("PRINT", print.as_str()),
    ] {
        code = code.replace(&format!("<<{k}>>"), v);
    }
    code = code.replace("<<PERIOD>>", &period.to_string());
    (code, notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

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

    /// The sentence from the brief is now an ordinary composition: MQ-2, a
    /// threshold and a blinking LED. Both ways of setting the threshold build
    /// and pass every check.
    #[test]
    fn the_gas_alarm_is_a_composition() {
        let p = parse("датчик газа который мигает когда много газа");
        let recipe = p.intent.recipe.expect("the sentence composes");
        assert_eq!(recipe.source, SourceId::Gas);
        assert_eq!(recipe.sink, SinkId::Led);
        assert_eq!(recipe.behavior, Behavior::Blink);
        let plan = plan(recipe, Lang::Ru).expect("the alarm plans");
        assert_eq!(plan.tid, Tid::Composed);
        assert_eq!(plan.choices.len(), 2);
        assert!(plan.title_text(Lang::Ru).contains("MQ-2"));
        assert!(plan.title_text(Lang::Ru).contains("мигающ"));
        for choice in 0..plan.choices.len() {
            for lang in [Lang::Ru, Lang::En] {
                let built = build(recipe, choice, lang).expect("builds");
                assert!(!unfilled(&built.code), "choice {choice}: unfilled token");
                assert!(built.circuit.parts().any(|q| q.value == "MQ-2"));
                assert!(built.circuit.parts().any(|q| q.kind == Kind::Led));
                let report = crate::checks::run(&built, lang);
                assert_eq!(
                    report.failed(),
                    0,
                    "choice {choice} ({}): {:?}",
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
        let analog = build(recipe, 0, Lang::Ru).unwrap();
        let digital = build(recipe, 1, Lang::Ru).unwrap();
        assert!(analog.code.contains("analogRead"));
        assert!(digital.code.contains("digitalRead"));
    }

    /// The matrix is the contract: every declared source, read its way, with
    /// every sink, must build and pass the checks in both languages.
    #[test]
    fn every_declared_combination_builds_and_passes() {
        let mut checked = 0;
        for source in SOURCES {
            for _variant in 0..source.variants.len() {
                for (sink, behavior) in [
                    (SinkId::Led, Behavior::Blink),
                    (SinkId::Led, Behavior::Solid),
                    (SinkId::Buzzer, Behavior::Beep),
                    (SinkId::Relay, Behavior::Switch),
                    (SinkId::Lcd, Behavior::Show),
                    (SinkId::Lcd2004, Behavior::Show),
                    (SinkId::Oled, Behavior::Show),
                    (SinkId::Servo, Behavior::Turn),
                    (SinkId::Motor, Behavior::Spin),
                    (SinkId::Rgb, Behavior::Color),
                ] {
                    let c = Composition {
                        source: source.id,
                        sink,
                        behavior,
                        condition: None,
                        screens: 1,
                    };
                    let plan = plan(c, Lang::Ru)
                        .unwrap_or_else(|| panic!("{}: {sink:?} plans", source.title.ru));
                    for choice in 0..plan.choices.len() {
                        for lang in [Lang::Ru, Lang::En] {
                            let built = build(c, choice, lang).unwrap_or_else(|| {
                                panic!("{}: {sink:?} builds", source.title.ru)
                            });
                            assert!(
                                !unfilled(&built.code),
                                "{}: unfilled token in {}",
                                source.title.ru,
                                lang.code()
                            );
                            assert!(
                                !built.circuit.parts.is_empty(),
                                "{}: empty circuit",
                                source.title.ru
                            );
                            let report = crate::checks::run(&built, lang);
                            let fails: Vec<String> = report
                                .checks
                                .iter()
                                .filter(|c| c.sev == crate::checks::Sev::Fail)
                                .map(|c| format!("{}: {}", c.name.get(lang), c.detail))
                                .collect();
                            assert!(
                                fails.is_empty(),
                                "{} / {sink:?} / {}: {}",
                                source.title.ru,
                                lang.code(),
                                fails.join("; ")
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked >= 80, "the matrix shrank to {checked} builds");
    }

    /// A source pin is never reused by the sink: the ranger's D9/D10 forces
    /// the LED onto another pin.
    #[test]
    fn the_sink_avoids_the_source_pins() {
        let c = Composition {
            source: SourceId::Range,
            sink: SinkId::Led,
            behavior: Behavior::Blink,
            condition: None,
            screens: 1,
        };
        let built = build(c, 0, Lang::Ru).unwrap();
        let uses = |pin: &str| {
            built
                .circuit
                .parts
                .iter()
                .find(|p| p.kind == Kind::Arduino)
                .map(|p| {
                    p.pins
                        .iter()
                        .any(|&i| built.circuit.pins[i].name == pin)
                })
                .unwrap_or(false)
        };
        assert!(uses("D9"));
        assert!(!built.code.contains("const uint8_t SINK_PIN = 9;"));
    }

    /// The sentence threshold is used, not the default.
    #[test]
    fn a_spoken_threshold_wins_over_the_default() {
        let p = parse("датчик газа мигает когда газ выше 700");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(recipe.condition, Some(Condition::Above(700.0)));
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("value > 700"));
        assert!(!built.code.contains("value > 400"));
    }

    /// A slow sensor keeps its own sampling period: the DS18B20 conversion
    /// takes 750 ms and the DHT dislikes faster than 2 s polls.
    #[test]
    fn each_source_keeps_its_sampling_period() {
        for source in SOURCES {
            let c = Composition {
                source: source.id,
                sink: SinkId::Led,
                behavior: Behavior::Solid,
                condition: None,
                screens: 1,
            };
            let built = build(c, 0, Lang::Ru).unwrap();
            assert!(
                built
                    .code
                    .contains(&format!("const uint32_t PERIOD_MS = {};", source.period_ms)),
                "{}: period not {} ms\n{}",
                source.title.ru,
                source.period_ms,
                built.code
            );
        }
    }

    /// A decimal threshold written with a comma still reads as a number.
    #[test]
    fn a_comma_threshold_is_a_number() {
        let p = parse("ток выше 1,5 ампер реле");
        let recipe = p.intent.recipe.expect("the current sensor composes");
        assert_eq!(recipe.source, SourceId::Current);
        assert_eq!(recipe.sink, SinkId::Relay);
        assert_eq!(recipe.condition, Some(Condition::Above(1.5)));
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("value > 1.50"), "{}", built.code);
    }

    /// A screen is a sink too: "температура на дисплее" composes an LCD, the
    /// sketch prints the reading, and there is no output pin to drive.
    #[test]
    fn a_screen_shows_the_reading() {
        let p = parse("температура на дисплее");
        let recipe = p.intent.recipe.expect("the sentence composes");
        assert_eq!(recipe.sink, SinkId::Lcd);
        assert_eq!(recipe.behavior, Behavior::Show);
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("LiquidCrystal_I2C lcd"));
        assert!(built.code.contains("lcd.print(value, 1)"));
        assert!(!built.code.contains("SINK_PIN"));
        assert!(!built.code.contains("bool active"));
        assert!(built.circuit.parts().any(|q| q.value == "LCD1602"));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// An OLED has to be named: a bare "дисплей" means the plain LCD.
    #[test]
    fn an_oled_is_named() {
        let p = parse("давление покажи на OLED");
        let recipe = p.intent.recipe.expect("the sentence composes");
        assert_eq!(recipe.source, SourceId::Pressure);
        assert_eq!(recipe.sink, SinkId::Oled);
        assert_eq!(recipe.behavior, Behavior::Show);
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("Adafruit_SSD1306 display"));
        assert!(!built.code.contains("SINK_PIN"));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// Two I2C boards share A4/A5: the gyro and the screen are devices on one
    /// bus, so the builder must not refuse the second one.
    #[test]
    fn an_i2c_screen_shares_the_bus_with_an_i2c_source() {
        let c = Composition {
            source: SourceId::Tilt,
            sink: SinkId::Oled,
            behavior: Behavior::Show,
            condition: None,
            screens: 1,
        };
        assert!(fits(c));
        let built = build(c, 0, Lang::Ru).unwrap();
        let uses = |pin: &str| {
            built
                .circuit
                .parts
                .iter()
                .find(|p| p.kind == Kind::Arduino)
                .map(|p| {
                    p.pins
                        .iter()
                        .any(|&i| built.circuit.pins[i].name == pin)
                })
                .unwrap_or(false)
        };
        assert!(uses("A4") && uses("A5"));
        assert_eq!(built.code.matches("#include <Wire.h>").count(), 1);
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// A servo turns to its working angle while the condition holds and
    /// returns to zero when it clears.
    #[test]
    fn a_servo_turns_on_the_condition() {
        let p = parse("расстояние меньше 20 серво");
        let recipe = p.intent.recipe.expect("the ranger composes");
        assert_eq!(recipe.sink, SinkId::Servo);
        assert_eq!(recipe.behavior, Behavior::Turn);
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("#include <Servo.h>"));
        assert!(built.code.contains("arm.attach(SINK_PIN)"));
        assert!(built.code.contains("arm.write(active ? 90 : 0)"));
        assert!(!built.code.contains("pinMode(SINK_PIN"));
        assert!(built.circuit.parts().any(|q| q.kind == Kind::Servo));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// A motor is switched by a MOSFET with a gate resistor, a pulldown and a
    /// flyback diode — the checks insist on all three.
    #[test]
    fn a_motor_gets_a_switch_and_a_diode() {
        let p = parse("температура выше 30 мотор");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(recipe.sink, SinkId::Motor);
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("analogWrite(SINK_PIN, active ? 255 : 0)"));
        for kind in [Kind::Motor, Kind::Nmos, Kind::Diode, Kind::Resistor] {
            assert!(
                built.circuit.parts().any(|q| q.kind == kind),
                "{kind:?} missing"
            );
        }
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// An RGB LED takes three free pins and shows one colour for the
    /// condition, another without it.
    #[test]
    fn an_rgb_led_takes_three_pins() {
        let p = parse("газ выше 400 rgb");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(recipe.sink, SinkId::Rgb);
        let built = build(recipe, 0, Lang::Ru).unwrap();
        for pin in ["R_PIN", "G_PIN", "B_PIN"] {
            assert!(built.code.contains(&format!("const uint8_t {pin}")), "{pin}");
        }
        assert!(built.code.contains("digitalWrite(R_PIN, active ? HIGH : LOW)"));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// A spoken hysteresis turns one threshold into two: the relay comes on
    /// at 400 and lets go only below 350, so it does not chatter.
    #[test]
    fn a_hysteresis_band_keeps_its_state() {
        let p = parse("газ выше 400 реле с гистерезисом 50");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(
            recipe.condition,
            Some(Condition::AboveBand { on: 400.0, off: 350.0 })
        );
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("bool active = false;"));
        assert!(built.code.contains("value > 400"));
        assert!(built.code.contains("value < 350"));
        assert!(!built.code.contains("bool active = value >"));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// The band works from the low side too: "ниже 20" comes on under 20 and
    /// clears only above 23.
    #[test]
    fn a_low_band_returns_above_the_threshold() {
        let p = parse("температура ниже 20 серво с гистерезисом 3");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(
            recipe.condition,
            Some(Condition::BelowBand { on: 20.0, off: 23.0 })
        );
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("value < 20"));
        assert!(built.code.contains("value > 23"));
    }

    /// Two thresholds joined by "и" are two conditions on one reading: the
    /// relay has to sit inside the window.
    #[test]
    fn two_conditions_make_a_window() {
        let p = parse("температура выше 20 и ниже 30 реле");
        let recipe = p.intent.recipe.expect("composes");
        assert_eq!(
            recipe.condition,
            Some(Condition::Between { low: 20.0, high: 30.0 })
        );
        let built = build(recipe, 0, Lang::Ru).unwrap();
        assert!(built.code.contains("value > 20.0 && value < 30.0"));
        assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
    }

    /// A window belongs to one reading: two sensors in one sentence are not
    /// squeezed into one range, the sentence stays a lesson.
    #[test]
    fn a_window_with_a_second_sensor_is_refused() {
        let p = parse("температура выше 20 и влажность ниже 60 реле");
        assert!(p.intent.recipe.is_none());
        assert!(crate::templates::plan(&p.intent, Lang::Ru).is_some());
    }

    /// A request to print in the serial port is not a request for a screen.
    #[test]
    fn a_serial_port_is_not_a_display() {
        let p = parse("вывести температуру в порт");
        assert!(p.intent.recipe.is_none());
        assert!(crate::templates::plan(&p.intent, Lang::Ru).is_some());
    }

    #[test]
    fn a_measure_verb_stays_a_lesson() {
        let p = parse("измерить температуру");
        assert!(p.intent.recipe.is_none());
        assert!(crate::templates::plan(&p.intent, Lang::Ru).is_some());
    }

    /// A named thermistor is its own part: an NTC divider. The DS18B20 reads
    /// the same quantity, but it is a different circuit, so it must not stand
    /// in for the part the sentence named.
    #[test]
    fn a_named_thermistor_keeps_its_own_part() {
        let p = parse("термометр на термисторе с выводом на экран");
        assert!(
            p.unsupported.is_none(),
            "{:?}",
            p.unsupported.map(|u| u.ru)
        );
        let built = crate::templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        assert!(
            built.circuit.parts().any(|q| q.value == "NTC 10 кОм"),
            "the thermistor must be drawn"
        );
        assert!(
            !built
                .circuit
                .parts()
                .any(|q| q.value.starts_with("DS18B20")),
            "a thermistor is not a DS18B20"
        );
        assert!(built.circuit.parts().any(|q| q.value.starts_with("LCD")));
        assert_eq!(
            crate::checks::run_with(&built, Lang::Ru, &p.intent).failed(),
            0
        );
    }

    /// A sensor named with no lesson behind it is still a request to read it.
    /// The serial monitor takes no pin, so the answer is a real circuit rather
    /// than "не нашёл, чем управлять".
    #[test]
    fn a_bare_sensor_gets_a_serial_readout() {
        for task in [
            "термометр на термисторе",
            "Составить программу термометра на термисторе.",
        ] {
            let p = parse(task);
            let recipe = p.intent.recipe.unwrap_or_else(|| panic!("{task} composes"));
            assert_eq!(recipe.source, SourceId::Thermistor, "{task}");
            assert_eq!(recipe.sink, SinkId::Serial, "{task}");
            let built = build(recipe, 0, Lang::Ru).unwrap();
            assert!(built.code.contains("Serial.println"), "{task}");
            assert_eq!(crate::checks::run(&built, Lang::Ru).failed(), 0);
        }
    }

    /// The screen the sentence names is the screen that is drawn. A matrix
    /// display used to become an LCD plus an OLED that the sketch never
    /// touched.
    #[test]
    fn a_matrix_display_is_not_an_lcd() {
        let p = parse("матричный дисплей без книги");
        let built = crate::templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        assert!(
            built.circuit.parts().any(|q| q.value == "MAX7219"),
            "the matrix must be drawn"
        );
        assert!(
            !built
                .circuit
                .parts()
                .any(|q| q.value.starts_with("LCD") || q.value.starts_with("OLED")),
            "a matrix is not an LCD or an OLED"
        );
        assert_eq!(
            crate::checks::run_with(&built, Lang::Ru, &p.intent).failed(),
            0
        );
    }

    /// A screen sentence that no lesson, recipe or text scene can build is
    /// refused. It used to be answered with a button and an LED: a plausible
    /// circuit for a different question.
    #[test]
    fn three_screens_are_not_a_button_and_led() {
        let p = parse("три экрана и кнопка");
        assert!(
            crate::templates::build(&p.intent, 0, Lang::Ru).is_none(),
            "a screen task must not fall through to the button template"
        );
    }

    /// A period spoken in the task belongs in the sketch: "каждые 200 мс"
    /// used to keep the half-second of the first-program lesson.
    #[test]
    fn a_spoken_blink_period_reaches_the_sketch() {
        let p = parse("мигать каждые 200 мс");
        let built = crate::templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        assert!(
            built.code.contains("const uint32_t HALF_MS = 200;"),
            "{}",
            built.code
        );
    }

    /// A pin named by the sentence is a requirement: the build must move the
    /// signal there, and the sketch must use the same number.
    #[test]
    fn a_named_pin_is_honoured() {
        let p = parse("светодиод к пину 7");
        let built = crate::templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        assert!(
            crate::requirements::pin_wired(&built.circuit, 7),
            "the LED is not wired to D7"
        );
        assert!(
            built.code.contains("const uint8_t LED = 7;"),
            "the sketch still points at another pin:\n{}",
            built.code
        );
        assert!(crate::requirements::missing(&built, &p.intent).is_empty());

        // The checker itself still notices a pin the drawing does not have.
        assert!(!crate::requirements::pin_wired(&built.circuit, 8));

        let p = parse("светодиод к пину 9");
        let built = crate::templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        assert!(crate::requirements::missing(&built, &p.intent).is_empty());
    }

    /// Every sensor the bug table names must be in the drawing as many times
    /// as it was asked for, and the sketch must read each instance. A single
    /// board for "два датчика" used to pass the picture and fail the checklist.
    #[test]
    fn several_sensors_are_all_drawn_and_read() {
        use crate::model::Kind;

        fn boards(built: &Built, value: &str) -> usize {
            built
                .circuit
                .parts
                .iter()
                .filter(|p| p.kind == Kind::Module && p.value == value)
                .count()
        }
        fn ldrs(built: &Built) -> usize {
            built.circuit.parts.iter().filter(|p| p.kind == Kind::Ldr).count()
        }

        let cases: &[(&str, &str, usize)] = &[
            ("два датчика температуры", "DS18B20", 2),
            ("два DS18B20", "DS18B20", 2),
            ("два датчика влажности", "DHT22", 2),
            ("два DHT22", "DHT22", 2),
            ("два DHT11", "DHT11", 2),
            ("два датчика влажности DHT11", "DHT11", 2),
            ("два датчика газа", "MQ-2", 2),
            ("два датчика давления", "BMP280", 2),
            ("два датчика почвы", "SOIL", 2),
            ("два датчика расстояния", "HC-SR04", 2),
        ];
        for &(phrase, value, want) in cases {
            let p = parse(phrase);
            assert!(
                p.unsupported.is_none(),
                "{phrase}: unexpectedly refused: {:?}",
                p.unsupported
            );
            let built = crate::templates::build(&p.intent, 0, Lang::Ru)
                .unwrap_or_else(|| panic!("{phrase}: does not build"));
            assert_eq!(boards(&built, value), want, "{phrase}: {value} count");
            assert!(!unfilled(&built.code), "{phrase}: unfilled token");
            let report = crate::checks::run_with(&built, Lang::Ru, &p.intent);
            assert_eq!(
                report.failed(),
                0,
                "{phrase}: {:?}",
                report
                    .checks
                    .iter()
                    .filter(|c| c.sev == crate::checks::Sev::Fail)
                    .map(|c| format!("{}: {}", c.name.get(Lang::Ru), c.detail))
                    .collect::<Vec<_>>()
            );
        }

        let p = parse("два датчика света");
        let light = crate::templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        assert_eq!(ldrs(&light), 2, "two photoresistors");
        assert!(light.code.contains("ldr1") && light.code.contains("ldr2"));
        assert_eq!(
            crate::checks::run_with(&light, Lang::Ru, &p.intent).failed(),
            0
        );

        let p = parse("датчик температуры, влажности и давления");
        let mixed = crate::templates::build(&p.intent, 0, Lang::Ru).expect("builds");
        assert_eq!(boards(&mixed, "DS18B20"), 1);
        assert_eq!(boards(&mixed, "DHT22"), 1);
        assert_eq!(boards(&mixed, "BMP280"), 1);
        assert!(
            mixed.code.contains("temp1")
                && mixed.code.contains("hum1")
                && mixed.code.contains("press1")
        );
        assert_eq!(
            crate::checks::run_with(&mixed, Lang::Ru, &p.intent).failed(),
            0
        );

        // A DHT reads temperature and humidity from one board: the pair is
        // not a multi-sensor build, but the sentence may still name DHT22.
        // The lesson used to draw DHT11 and fail the checklist for DHT22.
        for (phrase, model) in [
            ("датчик температуры DHT22", "DHT22"),
            ("датчик температуры DHT11", "DHT11"),
        ] {
            let p = parse(phrase);
            let dht = crate::templates::build(&p.intent, 0, Lang::Ru)
                .unwrap_or_else(|| panic!("{phrase}: does not build"));
            assert_eq!(boards(&dht, model), 1, "{phrase}: the {model} board");
            assert!(
                dht.code.contains(&format!("DHT dht(DATA_PIN, {model});")),
                "{phrase}: the sketch uses another model:\n{}",
                dht.code
            );
            assert_eq!(
                crate::checks::run_with(&dht, Lang::Ru, &p.intent).failed(),
                0,
                "{phrase}: the checklist fails the named model"
            );
        }
    }
}
