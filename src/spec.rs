//! What a sentence can ask for: sensors, sinks, conditions and behaviours.
//!
//! The tables here are data, not branches. A new sensor is a new entry: how it
//! is wired, what it reads, what the number means, and when it is considered
//! triggered. The composer in `compose.rs` turns one source, one condition and
//! one sink into a verified circuit and a sketch.

use crate::text::L;

/// A sensor the composer knows how to read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceId {
    Gas,
    Alcohol,
    Light,
    Motion,
    Range,
    Sound,
    Line,
    Hall,
    Current,
    Radiation,
    Temperature,
    Humidity,
    Pressure,
    Tilt,
    Weight,
    Color,
    /// Soil moisture: a resistive probe, not the DHT humidity sensor.
    Soil,
}

/// How the source is put on the bench. Boards come from the module table;
/// the two loose parts are a divider on the breadboard.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Place {
    Board { name: &'static str, at: (f32, f32) },
    Ldr,
    /// An NTC divider. The table reads temperature through the DS18B20
    /// today; the NTC path stays here for the cheap variant.
    #[allow(dead_code)]
    Thermistor,
}

/// What a reading means.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Logic {
    /// A number compared with a threshold.
    Number,
    /// A flag: the condition is "the signal is present".
    Flag,
}

/// The condition a sentence can ask for. `Active` is the only one that makes
/// sense for a digital sensor.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Condition {
    Above(f32),
    Below(f32),
    Active,
    /// Turns on above `on` and off only after falling back to `off`: the
    /// relay does not chatter around one noisy threshold.
    AboveBand { on: f32, off: f32 },
    /// The same from the other side: comes on below `on`, clears above `off`.
    BelowBand { on: f32, off: f32 },
    /// Two conditions on one reading: the value has to sit inside the window.
    Between { low: f32, high: f32 },
}

impl Condition {
    pub fn label(self) -> L {
        match self {
            Condition::Above(_) => L::new("выше порога", "above the threshold"),
            Condition::Below(_) => L::new("ниже порога", "below the threshold"),
            Condition::Active => L::new("есть сигнал", "signal present"),
            Condition::AboveBand { .. } => {
                L::new("выше порога с возвратом", "above the threshold, with a return")
            }
            Condition::BelowBand { .. } => {
                L::new("ниже порога с возвратом", "below the threshold, with a return")
            }
            Condition::Between { .. } => L::new("в диапазоне", "inside the range"),
        }
    }
}

/// What the sink does while the condition holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Behavior {
    Blink,
    Solid,
    Beep,
    Switch,
    /// The reading goes to a screen instead of driving a threshold.
    Show,
    /// A servo moves to a working angle and returns when the condition clears.
    Turn,
    /// A motor runs while the condition holds.
    Spin,
    /// An RGB LED shows one colour for the condition and another without it.
    Color,
}

impl Behavior {
    /// The verb the plan and "understood as" line show.
    pub fn label(self) -> L {
        match self {
            Behavior::Blink => L::new("мигать", "blink"),
            Behavior::Solid => L::new("гореть", "light up"),
            Behavior::Beep => L::new("пищать", "beep"),
            Behavior::Switch => L::new("щёлкать", "switch"),
            Behavior::Show => L::new("показывать", "show"),
            Behavior::Turn => L::new("поворачивать", "turn"),
            Behavior::Spin => L::new("вращаться", "spin"),
            Behavior::Color => L::new("подсвечивать", "light up in colour"),
        }
    }
}

/// The three outputs of the first matrix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SinkId {
    Led,
    Buzzer,
    Relay,
    Lcd,
    Lcd2004,
    Oled,
    Servo,
    Motor,
    Rgb,
}

/// How a sink reaches the Uno. A single pin is allocated from the free list;
/// an I2C board uses the bus, which a source may already be on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SinkWiring {
    Pin,
    I2c,
}

/// One source, one sink, one condition. This is what the parser stores in an
/// [`crate::parse::Intent`] once a sentence names both a sensor and a verb.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Composition {
    pub source: SourceId,
    pub sink: SinkId,
    pub behavior: Behavior,
    /// An explicit threshold from the sentence, if it gave one.
    pub condition: Option<Condition>,
    /// How many screens the sentence asked for. One unless it says "два
    /// экрана"; other sinks ignore it.
    pub screens: usize,
}

/// One way to read a sensor: the analogue output, the module comparator, or a
/// library call. The plan shows these as cards, exactly like part choices.
pub struct Variant {
    pub name: L,
    pub blurb: L,
    pub logic: Logic,
    /// Uno pins used by this variant. The sink allocator avoids them.
    pub pins: &'static [&'static str],
    /// Board header pin to Uno pin. Supply pins are wired automatically.
    pub signals: &'static [(&'static str, &'static str)],
    pub includes: &'static str,
    pub globals: &'static str,
    pub setup: &'static str,
    /// Must assign `float value`.
    pub sample: &'static str,
    pub print: &'static str,
    pub unit: L,
    /// Digits for `Serial.println(value, n)`.
    pub decimals: u8,
    pub condition: Condition,
    pub level: u8,
    pub note: L,
}

/// A sensor and all the ways it can be read.
pub struct SourceSpec {
    pub id: SourceId,
    pub title: L,
    pub short: L,
    /// The book lesson this part belongs to.
    pub lesson: usize,
    pub keys: &'static [&'static str],
    /// How often the sketch samples the sensor.
    pub period_ms: u16,
    pub place: Place,
    pub variants: &'static [Variant],
    pub summary: L,
}

pub struct SinkSpec {
    /// Kept even when only the tests read it: the id ties the table to the
    /// enum, and the title is what the plan card shows.
    #[allow(dead_code)]
    pub id: SinkId,
    pub title: L,
    /// Candidate Uno pins, best first. For an I2C board: SDA and SCL.
    pub pins: &'static [&'static str],
    /// How many of those pins the sink takes: one for a lamp, three for an
    /// RGB LED. The I2C bus ignores it.
    pub take: usize,
    pub current_ma: u16,
    pub wiring: SinkWiring,
    /// The ready-made board dropped on the bench, for display sinks.
    pub module: Option<&'static str>,
    /// Where that board sits.
    pub at: (f32, f32),
    /// Sketch fragments the board brings.
    pub includes: &'static str,
    pub globals: &'static str,
    pub setup: &'static str,
    /// What goes into the loop when the sink is a screen.
    pub show: &'static str,
}

const WARMUP: L = L::new(
    "Нагревателю нужно 20 секунд на прогрев, а калибровка занимает до суток.",
    "The heater needs 20 seconds to warm up and the sensor up to a day to settle.",
);
const TRIMMER: L = L::new(
    "Порог задаётся подстроечником на модуле: после включения дай 20 секунд прогрева.",
    "The trimmer on the module sets the threshold; give it 20 seconds after power-up.",
);

pub const SOURCES: &[SourceSpec] = &[
    SourceSpec {
        id: SourceId::Gas,
        title: L::new("Датчик газа MQ-2", "The MQ-2 gas sensor"),
        short: L::new("газ", "gas"),
        lesson: 13,
        keys: &["газ", "дым", "mq-2", "mq2", "утечк", "gas", "smoke", "leak"],
        period_ms: 250,
        place: Place::Board { name: "MQ-2", at: (2.0, 54.0) },
        summary: L::new(
            "MQ-2: аналоговый выход на A0, компараторный — на D2.",
            "The MQ-2: analogue output on A0, comparator output on D2.",
        ),
        variants: &[
            Variant {
                name: L::new("Порог по аналогу", "Threshold in the sketch"),
                blurb: L::new(
                    "AO на A0: порог задан в скетче, значение видно в порту.",
                    "AO to A0: the threshold lives in the sketch and the value goes to the port.",
                ),
                logic: Logic::Number,
                pins: &["A0"],
                signals: &[("AO", "A0")],
                includes: "",
                globals: "const uint8_t SENSE = A0;\n",
                setup: "",
                sample: "  float value = analogRead(SENSE);\n",
                print: "  Serial.println(value, 0);\n",
                unit: L::new("отсчётов", "counts"),
                decimals: 0,
                condition: Condition::Above(400.0),
                level: 2,
                note: WARMUP,
            },
            Variant {
                name: L::new("Порог на модуле", "Threshold on the module"),
                blurb: L::new(
                    "DO на D2: порог крутится подстроечником, скетч читает готовый сигнал.",
                    "DO to D2: the module's trimmer sets the threshold, the sketch reads a ready signal.",
                ),
                logic: Logic::Flag,
                pins: &["D2"],
                signals: &[("DO", "D2")],
                includes: "",
                globals: "const uint8_t SENSE = 2;\n",
                setup: "  pinMode(SENSE, INPUT);\n",
                sample: "  float value = digitalRead(SENSE) ? 1.0 : 0.0;\n",
                print: "  Serial.println(value, 0);\n",
                unit: L::new("сигнал", "signal"),
                decimals: 0,
                condition: Condition::Active,
                level: 1,
                note: TRIMMER,
            },
        ],
    },
    SourceSpec {
        id: SourceId::Alcohol,
        title: L::new("Датчик спирта MQ-3", "The MQ-3 alcohol sensor"),
        short: L::new("спирт", "alcohol"),
        lesson: 49,
        keys: &["спирт", "алкогол", "mq-3", "mq3", "alcohol", "breath"],
        period_ms: 250,
        place: Place::Board { name: "MQ-3", at: (2.0, 54.0) },
        summary: L::new(
            "MQ-3: аналоговый выход на A0, компараторный — на D2.",
            "The MQ-3: analogue output on A0, comparator output on D2.",
        ),
        variants: &[
            Variant {
                name: L::new("Порог по аналогу", "Threshold in the sketch"),
                blurb: L::new(
                    "AO на A0: значение видно в порту, порог задан в скетче.",
                    "AO to A0: the value goes to the port and the threshold lives in the sketch.",
                ),
                logic: Logic::Number,
                pins: &["A0"],
                signals: &[("AO", "A0")],
                includes: "",
                globals: "const uint8_t SENSE = A0;\n",
                setup: "",
                sample: "  float value = analogRead(SENSE);\n",
                print: "  Serial.println(value, 0);\n",
                unit: L::new("отсчётов", "counts"),
                decimals: 0,
                condition: Condition::Above(400.0),
                level: 2,
                note: WARMUP,
            },
            Variant {
                name: L::new("Порог на модуле", "Threshold on the module"),
                blurb: L::new(
                    "DO на D2: порог задаёт подстроечник на модуле.",
                    "DO to D2: the trimmer on the module sets the threshold.",
                ),
                logic: Logic::Flag,
                pins: &["D2"],
                signals: &[("DO", "D2")],
                includes: "",
                globals: "const uint8_t SENSE = 2;\n",
                setup: "  pinMode(SENSE, INPUT);\n",
                sample: "  float value = digitalRead(SENSE) ? 1.0 : 0.0;\n",
                print: "  Serial.println(value, 0);\n",
                unit: L::new("сигнал", "signal"),
                decimals: 0,
                condition: Condition::Active,
                level: 1,
                note: TRIMMER,
            },
        ],
    },
    SourceSpec {
        id: SourceId::Light,
        title: L::new("Фоторезистор GL5528", "The GL5528 photoresistor"),
        short: L::new("свет", "light"),
        lesson: 9,
        keys: &["фоторезистор", "ldr", "люкс", "освещ", "датчик света", "темн", "light", "photoresistor", "brightness", "dark", "ночник", "night light"],
        period_ms: 100,
        place: Place::Ldr,
        summary: L::new(
            "Фоторезистор в делителе с 10 кОм: чем темнее, тем выше напряжение на A0.",
            "A photoresistor and a 10 kΩ divider: the darker it is, the higher the voltage on A0.",
        ),
        variants: &[Variant {
            name: L::new("Делитель 10 кОм", "10 kΩ divider"),
            blurb: L::new(
                "LDR и 10 кОм на A0: широкий диапазон, мягкий порог.",
                "An LDR and 10 kΩ on A0: a wide range and a soft threshold.",
            ),
            logic: Logic::Number,
            pins: &["A0"],
            signals: &[],
            includes: "",
            globals: "const uint8_t SENSE = A0;\n",
            setup: "",
            sample: "  float value = analogRead(SENSE);\n",
            print: "  Serial.println(value, 0);\n",
            unit: L::new("отсчётов", "counts"),
            decimals: 0,
            condition: Condition::Below(400.0),
            level: 1,
            note: L::new(
                "Порог 400 — начало сумерек: подбери под свою комнату.",
                "400 is early dusk: tune it to your room.",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Motion,
        title: L::new("Датчик движения HC-SR501", "The HC-SR501 motion sensor"),
        short: L::new("движение", "motion"),
        lesson: 14,
        keys: &["движени", "pir", "hc-sr501", "присутств", "motion", "presence"],
        period_ms: 250,
        place: Place::Board { name: "HC-SR501", at: (2.0, 54.0) },
        summary: L::new(
            "HC-SR501: выход OUT на D2, высокий уровень — движение.",
            "The HC-SR501: OUT on D2, a high level means motion.",
        ),
        variants: &[Variant {
            name: L::new("Выход OUT", "OUT output"),
            blurb: L::new(
                "OUT на D2: высокий уровень, пока датчик видит движение.",
                "OUT to D2: a high level while the sensor sees motion.",
            ),
            logic: Logic::Flag,
            pins: &["D2"],
            signals: &[("OUT", "D2")],
            includes: "",
            globals: "const uint8_t SENSE = 2;\n",
            setup: "  pinMode(SENSE, INPUT);\n",
            sample: "  float value = digitalRead(SENSE) ? 1.0 : 0.0;\n",
            print: "  Serial.println(value, 0);\n",
            unit: L::new("сигнал", "signal"),
            decimals: 0,
            condition: Condition::Active,
            level: 1,
            note: L::new(
                "Датчику нужно до минуты на прогрев, а повторное срабатывание держится 2–3 секунды.",
                "The sensor takes up to a minute to settle, and a trigger holds for 2–3 seconds.",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Range,
        title: L::new("Дальномер HC-SR04", "The HC-SR04 ranger"),
        short: L::new("расстояние", "distance"),
        lesson: 15,
        keys: &["расстояни", "дальномер", "ультразвук", "hc-sr04", "distance", "ranger", "ultrasonic"],
        period_ms: 100,
        place: Place::Board { name: "HC-SR04", at: (2.0, 54.0) },
        summary: L::new(
            "HC-SR04: импульс на TRIG, длительность на ECHO; сантиметры — микросекунды / 58.",
            "The HC-SR04: a pulse on TRIG, its length on ECHO; centimetres are microseconds / 58.",
        ),
        variants: &[Variant {
            name: L::new("TRIG D9, ECHO D10", "TRIG D9, ECHO D10"),
            blurb: L::new(
                "Импульс 10 мкс на TRIG и замер длины ответа на ECHO.",
                "A 10 µs pulse on TRIG and a measurement of the echo on ECHO.",
            ),
            logic: Logic::Number,
            pins: &["D9", "D10"],
            signals: &[("TRIG", "D9"), ("ECHO", "D10")],
            includes: "",
            globals: "const uint8_t TRIG = 9;\nconst uint8_t ECHO = 10;\n",
            setup: "  pinMode(TRIG, OUTPUT);\n  pinMode(ECHO, INPUT);\n",
            sample: "  digitalWrite(TRIG, LOW);\n  delayMicroseconds(2);\n  digitalWrite(TRIG, HIGH);\n  delayMicroseconds(10);\n  digitalWrite(TRIG, LOW);\n  unsigned long us = pulseIn(ECHO, HIGH, 30000UL);\n  float value = us / 58.0;\n",
            print: "  Serial.println(value, 1);\n",
            unit: L::new("см", "cm"),
            decimals: 1,
            condition: Condition::Below(20.0),
            level: 2,
            note: L::new(
                "Дальномер видит от 2 см до 4 м; на мягкой ткани и под углом эхо пропадает.",
                "The ranger sees 2 cm to 4 m; soft cloth and steep angles swallow the echo.",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Sound,
        title: L::new("Датчик звука", "The sound sensor"),
        short: L::new("звук", "sound"),
        lesson: 36,
        keys: &[
            "звук", "шум", "микрофон", "громкост", "хлоп", "хлопк", "sound",
            "noise", "microphone", "clap",
        ],
        period_ms: 100,
        place: Place::Board { name: "SOUND", at: (2.0, 54.0) },
        summary: L::new(
            "Датчик звука: аналоговый уровень на A0, компараторный сигнал на D2.",
            "The sound sensor: an analogue level on A0 and a comparator signal on D2.",
        ),
        variants: &[
            Variant {
                name: L::new("Уровень с AO", "Level from AO"),
                blurb: L::new(
                    "AO на A0: видно, насколько громко, порог задан в скетче.",
                    "AO to A0: the loudness is visible, and the threshold lives in the sketch.",
                ),
                logic: Logic::Number,
                pins: &["A0"],
                signals: &[("AO", "A0")],
                includes: "",
                globals: "const uint8_t SENSE = A0;\n",
                setup: "",
                sample: "  float value = analogRead(SENSE);\n",
                print: "  Serial.println(value, 0);\n",
                unit: L::new("отсчётов", "counts"),
                decimals: 0,
                condition: Condition::Above(500.0),
                level: 1,
                note: L::new(
                    "У дешёвого модуля нет АРУ: хлопок и музыка дают разные числа.",
                    "A cheap module has no AGC: a clap and music give different numbers.",
                ),
            },
            Variant {
                name: L::new("Сигнал с DO", "Signal from DO"),
                blurb: L::new(
                    "DO на D2: компаратор модуля уже решил, что звук громкий.",
                    "DO to D2: the module's comparator has already decided it is loud.",
                ),
                logic: Logic::Flag,
                pins: &["D2"],
                signals: &[("DO", "D2")],
                includes: "",
                globals: "const uint8_t SENSE = 2;\n",
                setup: "  pinMode(SENSE, INPUT);\n",
                sample: "  float value = digitalRead(SENSE) ? 1.0 : 0.0;\n",
                print: "  Serial.println(value, 0);\n",
                unit: L::new("сигнал", "signal"),
                decimals: 0,
                condition: Condition::Active,
                level: 1,
                note: L::new(
                    "Чувствительность крутится подстроечником на модуле.",
                    "The trimmer on the module sets the sensitivity.",
                ),
            },
        ],
    },
    SourceSpec {
        id: SourceId::Line,
        title: L::new("Датчик линии TCRT5000", "The TCRT5000 line sensor"),
        short: L::new("линия", "line"),
        lesson: 43,
        keys: &["лини", "tcrt5000", "трасс", "line sensor", "track"],
        period_ms: 200,
        place: Place::Board { name: "TCRT5000", at: (2.0, 54.0) },
        summary: L::new(
            "TCRT5000: выход DO на D2, низкий уровень — тёмная линия под датчиком.",
            "The TCRT5000: DO on D2, a low level means a dark line under the sensor.",
        ),
        variants: &[Variant {
            name: L::new("Выход DO", "DO output"),
            blurb: L::new(
                "DO на D2: компаратор модуля различает светлое и тёмное.",
                "DO to D2: the module's comparator tells light from dark.",
            ),
            logic: Logic::Flag,
            pins: &["D2"],
            signals: &[("DO", "D2")],
            includes: "",
            globals: "const uint8_t SENSE = 2;\n",
            setup: "  pinMode(SENSE, INPUT);\n",
            sample: "  float value = digitalRead(SENSE) == LOW ? 1.0 : 0.0;\n",
            print: "  Serial.println(value, 0);\n",
            unit: L::new("линия", "line"),
            decimals: 0,
            condition: Condition::Active,
            level: 1,
            note: L::new(
                "Высоту над полом подбирают 5–15 мм: ближе датчик слепнет от своего ИК.",
                "Set the height above the floor to 5–15 mm: closer and the sensor is blinded by its own IR.",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Hall,
        title: L::new("Датчик Холла A3144", "The A3144 Hall sensor"),
        short: L::new("поле", "magnetic field"),
        lesson: 39,
        keys: &["холл", "магнитн", "a3144", "hall", "magnet"],
        period_ms: 200,
        place: Place::Board { name: "HALL", at: (2.0, 54.0) },
        summary: L::new(
            "A3144: открытый коллектор на D2, низкий уровень у магнита.",
            "The A3144: an open collector on D2, a low level near a magnet.",
        ),
        variants: &[Variant {
            name: L::new("Выход DO", "DO output"),
            blurb: L::new(
                "DO на D2 с подтяжкой: магнит притягивает выход к земле.",
                "DO to D2 with a pull-up: a magnet pulls the output to ground.",
            ),
            logic: Logic::Flag,
            pins: &["D2"],
            signals: &[("DO", "D2")],
            includes: "",
            globals: "const uint8_t SENSE = 2;\n",
            setup: "  pinMode(SENSE, INPUT_PULLUP);\n",
            sample: "  float value = digitalRead(SENSE) == LOW ? 1.0 : 0.0;\n",
            print: "  Serial.println(value, 0);\n",
            unit: L::new("магнит", "magnet"),
            decimals: 0,
            condition: Condition::Active,
            level: 1,
            note: L::new(
                "A3144 срабатывает на один полюс: переверни магнит, если реакции нет.",
                "The A3144 responds to one pole: turn the magnet over if nothing happens.",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Current,
        title: L::new("Датчик тока ACS712", "The ACS712 current sensor"),
        short: L::new("ток", "current"),
        lesson: 41,
        keys: &["ток", "acs712", "ампер", "current", "amp"],
        period_ms: 250,
        place: Place::Board { name: "ACS712", at: (2.0, 54.0) },
        summary: L::new(
            "ACS712: середина шкалы — ноль ампер, 185 мВ на ампер у модуля 5 А.",
            "The ACS712: mid-scale is zero amps, 185 mV per amp on the 5 A board.",
        ),
        variants: &[Variant {
            name: L::new("Выход OUT", "OUT output"),
            blurb: L::new(
                "OUT на A0: амперы считаются из середины шкалы 512.",
                "OUT to A0: amps are measured from mid-scale 512.",
            ),
            logic: Logic::Number,
            pins: &["A0"],
            signals: &[("OUT", "A0")],
            includes: "",
            globals: "const uint8_t SENSE = A0;\nconst float ZERO = 512.0;\nconst float VOLTS_PER_AMP = 0.185;\n",
            setup: "",
            sample: "  float value = (analogRead(SENSE) - ZERO) * 5.0 / 1023.0 / VOLTS_PER_AMP;\n",
            print: "  Serial.println(value, 2);\n",
            unit: L::new("А", "A"),
            decimals: 2,
            condition: Condition::Above(1.0),
            level: 2,
            note: L::new(
                "Нулю нужна калибровка: при выключенной нагрузке подстрой ZERO по порту.",
                "Zero needs calibration: with the load off, trim ZERO by the port reading.",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Radiation,
        title: L::new("Счётчик Гейгера", "The Geiger counter"),
        short: L::new("радиация", "radiation"),
        lesson: 40,
        keys: &["радиац", "гейгер", "дозиметр", "radiation", "geiger"],
        period_ms: 1000,
        place: Place::Board { name: "RADIATION", at: (2.0, 54.0) },
        summary: L::new(
            "Счётчик Гейгера: импульсы на D2, за секунду считаем вспышки.",
            "A Geiger counter: pulses on D2, counting flashes per second.",
        ),
        variants: &[Variant {
            name: L::new("Импульсы на D2", "Pulses on D2"),
            blurb: L::new(
                "Прерывание на D2 считает импульсы, раз в период счёт сбрасывается.",
                "An interrupt on D2 counts pulses and the count resets every period.",
            ),
            logic: Logic::Number,
            pins: &["D2"],
            signals: &[("OUT", "D2")],
            includes: "",
            globals: "const uint8_t PULSE = 2;\nvolatile uint32_t pulses = 0;\nvoid onPulse() { pulses++; }\n",
            setup: "  pinMode(PULSE, INPUT_PULLUP);\n  attachInterrupt(digitalPinToInterrupt(PULSE), onPulse, FALLING);\n",
            sample: "  float value = pulses;\n  pulses = 0;\n",
            print: "  Serial.println(value, 0);\n",
            unit: L::new("имп/с", "cps"),
            decimals: 0,
            condition: Condition::Above(10.0),
            level: 3,
            note: L::new(
                "Это счётчик импульсов, а не дозиметр: для Зв нужен коэффициент трубки.",
                "This counts pulses, it is not a dosimeter: converting to sieverts needs the tube's factor.",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Temperature,
        title: L::new("Термометр DS18B20", "The DS18B20 thermometer"),
        short: L::new("температура", "temperature"),
        lesson: 10,
        keys: &["температур", "термометр", "ds18b20", "18b20", "градус", "temperature", "thermometer", "heat"],
        period_ms: 1000,
        place: Place::Board { name: "DS18B20", at: (2.0, 54.0) },
        summary: L::new(
            "DS18B20 на D2: цифровая температура по OneWire, точность 0.5 °C.",
            "The DS18B20 on D2: a digital OneWire temperature, accurate to 0.5 °C.",
        ),
        variants: &[Variant {
            name: L::new("OneWire на D2", "OneWire on D2"),
            blurb: L::new(
                "DATA на D2 с подтяжкой 4.7 кОм: до 12 бит, чтение раз в секунду.",
                "DATA to D2 with a 4.7 kΩ pull-up: up to 12 bits, read once a second.",
            ),
            logic: Logic::Number,
            pins: &["D2"],
            signals: &[("DATA", "D2")],
            includes: "#include <OneWire.h>\n#include <DallasTemperature.h>\n",
            globals: "const uint8_t ONE_WIRE = 2;\nOneWire wire(ONE_WIRE);\nDallasTemperature sensors(&wire);\n",
            setup: "  sensors.begin();\n",
            sample: "  sensors.requestTemperatures();\n  float value = sensors.getTempCByIndex(0);\n",
            print: "  Serial.println(value, 1);\n",
            unit: L::new("°C", "°C"),
            decimals: 1,
            condition: Condition::Above(30.0),
            level: 2,
            note: L::new(
                "Преобразование занимает до 750 мс: опрос раз в секунду, не чаще.",
                "A conversion takes up to 750 ms: poll once a second, no faster.",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Humidity,
        title: L::new("Влажность DHT22", "DHT22 humidity"),
        short: L::new("влажность", "humidity"),
        lesson: 11,
        keys: &["влажност", "dht11", "dht22", "dht", "humidity"],
        period_ms: 2000,
        place: Place::Board { name: "DHT22", at: (2.0, 54.0) },
        summary: L::new(
            "DHT22 на D2: температура и влажность, обновление раз в две секунды.",
            "The DHT22 on D2: temperature and humidity, refreshed every two seconds.",
        ),
        variants: &[
            Variant {
                name: L::new("Влажность", "Humidity"),
                blurb: L::new(
                    "DATA на D2: влажность в процентах, шаг 0.1 %.",
                    "DATA to D2: relative humidity in percent, 0.1 % steps.",
                ),
                logic: Logic::Number,
                pins: &["D2"],
                signals: &[("DATA", "D2")],
                includes: "#include <DHT.h>\n",
                globals: "const uint8_t DATA_PIN = 2;\nDHT dht(DATA_PIN, DHT22);\n",
                setup: "  dht.begin();\n",
                sample: "  float value = dht.readHumidity();\n",
                print: "  Serial.println(value, 1);\n",
                unit: L::new("%", "%"),
                decimals: 1,
                condition: Condition::Above(60.0),
                level: 1,
                note: L::new(
                    "DHT не чаще раза в 2 секунды, иначе датчик врёт и греется.",
                    "Do not read the DHT more than once every 2 seconds, or it drifts and warms up.",
                ),
            },
            Variant {
                name: L::new("Температура DHT22", "DHT22 temperature"),
                blurb: L::new(
                    "DATA на D2: температура в градусах, шаг 0.1 °C.",
                    "DATA to D2: temperature in degrees, 0.1 °C steps.",
                ),
                logic: Logic::Number,
                pins: &["D2"],
                signals: &[("DATA", "D2")],
                includes: "#include <DHT.h>\n",
                globals: "const uint8_t DATA_PIN = 2;\nDHT dht(DATA_PIN, DHT22);\n",
                setup: "  dht.begin();\n",
                sample: "  float value = dht.readTemperature();\n",
                print: "  Serial.println(value, 1);\n",
                unit: L::new("°C", "°C"),
                decimals: 1,
                condition: Condition::Above(30.0),
                level: 1,
                note: L::new(
                    "DHT не чаще раза в 2 секунды, иначе датчик врёт и греется.",
                    "Do not read the DHT more than once every 2 seconds, or it drifts and warms up.",
                ),
            },
        ],
    },
    SourceSpec {
        id: SourceId::Pressure,
        title: L::new("Барометр BMP280", "The BMP280 barometer"),
        short: L::new("давление", "pressure"),
        lesson: 12,
        keys: &["давлен", "барометр", "bmp180", "bmp280", "bme280", "pressure", "barometer"],
        period_ms: 1000,
        place: Place::Board { name: "BMP280", at: (2.0, 54.0) },
        summary: L::new(
            "BMP280 по I2C: SDA на A4, SCL на A5, давление в гектопаскалях.",
            "The BMP280 over I2C: SDA on A4, SCL on A5, pressure in hectopascals.",
        ),
        variants: &[
            Variant {
                name: L::new("Давление", "Pressure"),
                blurb: L::new(
                    "I2C на A4/A5: давление в гПа, шаг 0.01.",
                    "I2C on A4/A5: pressure in hPa, 0.01 steps.",
                ),
                logic: Logic::Number,
                pins: &["A4", "A5"],
                signals: &[("SDA", "A4"), ("SCL", "A5")],
                includes: "#include <Wire.h>\n#include <Adafruit_BMP280.h>\n",
                globals: "Adafruit_BMP280 bmp;\n",
                setup: "  Wire.begin();\n  if (!bmp.begin(0x76)) Serial.println(F(\"no BMP280\"));\n",
                sample: "  float value = bmp.readPressure() / 100.0;\n",
                print: "  Serial.println(value, 2);\n",
                unit: L::new("гПа", "hPa"),
                decimals: 2,
                condition: Condition::Above(1010.0),
                level: 2,
                note: L::new(
                    "Адрес 0x76 или 0x77: если порт молчит, поменяй адрес в begin().",
                    "Address 0x76 or 0x77: if the port stays silent, change the address in begin().",
                ),
            },
            Variant {
                name: L::new("Температура BMP280", "BMP280 temperature"),
                blurb: L::new(
                    "I2C на A4/A5: температура в градусах, шаг 0.01.",
                    "I2C on A4/A5: temperature in degrees, 0.01 steps.",
                ),
                logic: Logic::Number,
                pins: &["A4", "A5"],
                signals: &[("SDA", "A4"), ("SCL", "A5")],
                includes: "#include <Wire.h>\n#include <Adafruit_BMP280.h>\n",
                globals: "Adafruit_BMP280 bmp;\n",
                setup: "  Wire.begin();\n  if (!bmp.begin(0x76)) Serial.println(F(\"no BMP280\"));\n",
                sample: "  float value = bmp.readTemperature();\n",
                print: "  Serial.println(value, 2);\n",
                unit: L::new("°C", "°C"),
                decimals: 2,
                condition: Condition::Above(30.0),
                level: 2,
                note: L::new(
                    "Температура BMP280 греется от собственного чипа на пару градусов.",
                    "The BMP280's own chip warms it by a couple of degrees.",
                ),
            },
        ],
    },
    SourceSpec {
        id: SourceId::Tilt,
        title: L::new("Наклон MPU6050", "The MPU6050 tilt sensor"),
        short: L::new("наклон", "tilt"),
        lesson: 44,
        keys: &[
            "наклон", "mpu6050", "gy-521", "гироскоп", "акселерометр", "tilt",
            "accelerometer",
        ],
        period_ms: 200,
        place: Place::Board { name: "MPU6050", at: (2.0, 54.0) },
        summary: L::new(
            "MPU6050 по I2C: SDA на A4, SCL на A5, прерывание на D2.",
            "The MPU6050 over I2C: SDA on A4, SCL on A5, the interrupt on D2.",
        ),
        variants: &[Variant {
            name: L::new("Наклон по оси X", "Tilt along X"),
            blurb: L::new(
                "Ускорение по X: наклон в градусах считается из проекции силы тяжести.",
                "Acceleration along X: the tilt follows from the projection of gravity.",
            ),
            logic: Logic::Number,
            pins: &["A4", "A5", "D2"],
            signals: &[("SDA", "A4"), ("SCL", "A5"), ("INT", "D2")],
            includes: "#include <Wire.h>\n#include <Adafruit_MPU6050.h>\n",
            globals: "Adafruit_MPU6050 mpu;\n",
            setup: "  Wire.begin();\n  if (!mpu.begin()) Serial.println(F(\"no MPU6050\"));\n",
            sample: "  sensors_event_t a, g, temp;\n  mpu.getEvent(&a, &g, &temp);\n  float value = a.acceleration.x;\n",
            print: "  Serial.println(value, 2);\n",
            unit: L::new("g", "g"),
            decimals: 2,
            condition: Condition::Above(0.5),
            level: 3,
            note: L::new(
                "Ось и знак зависят от того, как закреплена плата: проверь по порту и поменяй a.acceleration.x на .y или .z.",
                "The axis and its sign depend on how the board is mounted: check the port and switch a.acceleration.x to .y or .z.",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Weight,
        title: L::new("Весы HX711", "Scales on an HX711"),
        short: L::new("вес", "weight"),
        lesson: 46,
        keys: &["вес", "весы", "hx711", "тензодатчик", "weight", "scale"],
        period_ms: 500,
        place: Place::Board { name: "HX711", at: (2.0, 54.0) },
        summary: L::new(
            "HX711: данные на D2, такт на D3, мост тензодатчика на винтовых клеммах.",
            "The HX711: data on D2, the clock on D3, the load cell on the screw terminals.",
        ),
        variants: &[Variant {
            name: L::new("Данные D2, такт D3", "Data D2, clock D3"),
            blurb: L::new(
                "Библиотека HX711 читает мост и отдаёт усреднённое значение.",
                "The HX711 library reads the bridge and returns an averaged value.",
            ),
            logic: Logic::Number,
            pins: &["D2", "D3"],
            signals: &[("DT", "D2"), ("SCK", "D3")],
            includes: "#include <HX711.h>\n",
            globals: "const uint8_t DT = 2;\nconst uint8_t SCK = 3;\nHX711 scale;\n",
            setup: "  scale.begin(DT, SCK);\n  scale.tare();\n",
            sample: "  float value = scale.get_units(1);\n",
            print: "  Serial.println(value, 1);\n",
            unit: L::new("единицы", "units"),
            decimals: 1,
            condition: Condition::Above(100.0),
            level: 3,
            note: L::new(
                "Пока нет калибровки, число условное: положи известную гирю и подбери scale.set_scale().",
                "Without calibration the number is arbitrary: put a known weight on and tune scale.set_scale().",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Color,
        title: L::new("Датчик цвета TCS3200", "The TCS3200 colour sensor"),
        short: L::new("цвет", "colour"),
        lesson: 50,
        keys: &["цвет", "tcs3200", "tcs230", "color", "colour"],
        period_ms: 500,
        place: Place::Board { name: "TCS3200", at: (2.0, 54.0) },
        summary: L::new(
            "TCS3200: частота на OUT, красный канал выбирают S2 и S3, масштаб — S0 и S1.",
            "The TCS3200: a frequency on OUT, S2 and S3 pick the red channel, S0 and S1 set the scale.",
        ),
        variants: &[Variant {
            name: L::new("Красный канал", "Red channel"),
            blurb: L::new(
                "OUT на D2, S0–S3 на D3–D6: ширина импульса красного канала.",
                "OUT on D2, S0–S3 on D3–D6: the pulse width of the red channel.",
            ),
            logic: Logic::Number,
            pins: &["D2", "D3", "D4", "D5", "D6"],
            signals: &[
                ("OUT", "D2"),
                ("S0", "D3"),
                ("S1", "D4"),
                ("S2", "D5"),
                ("S3", "D6"),
            ],
            includes: "",
            globals: "const uint8_t OUT = 2;\nconst uint8_t S0 = 3, S1 = 4, S2 = 5, S3 = 6;\nuint16_t readChannel(bool s2, bool s3) {\n  digitalWrite(S2, s2);\n  digitalWrite(S3, s3);\n  return pulseIn(OUT, HIGH, 20000UL);\n}\n",
            setup: "  pinMode(OUT, INPUT);\n  pinMode(S0, OUTPUT);\n  pinMode(S1, OUTPUT);\n  pinMode(S2, OUTPUT);\n  pinMode(S3, OUTPUT);\n  digitalWrite(S0, HIGH);\n  digitalWrite(S1, LOW);\n",
            sample: "  float value = readChannel(LOW, LOW);\n",
            print: "  Serial.println(value, 0);\n",
            unit: L::new("мкс", "µs"),
            decimals: 0,
            condition: Condition::Above(300.0),
            level: 3,
            note: L::new(
                "Один канал — грубое приближение: для настоящего цвета нужны все три и баланс белого.",
                "One channel is a rough guess: real colour needs all three and a white balance.",
            ),
        }],
    },
    SourceSpec {
        id: SourceId::Soil,
        title: L::new("Влажность почвы FC-28", "FC-28 soil moisture"),
        short: L::new("влажность почвы", "soil moisture"),
        // No book lesson teaches this probe yet: the sensor is a bench part,
        // not a page of the course.
        lesson: usize::MAX,
        keys: &["почв", "soil", "fc-28", "yl-69", "yl-38", "датчик влажности почвы"],
        period_ms: 1000,
        place: Place::Board { name: "SOIL", at: (2.0, 54.0) },
        summary: L::new(
            "FC-28: аналоговый выход на A0, компараторный — на D2. Сухая почва даёт большее число.",
            "The FC-28: an analogue output on A0 and a comparator on D2. Dry soil reads higher.",
        ),
        variants: &[Variant {
            name: L::new("Аналоговый AO", "Analogue AO"),
            blurb: L::new(
                "AO на A0: число переводится в проценты, порог задан в скетче.",
                "AO to A0: the reading is mapped to percent, and the threshold lives in the sketch.",
            ),
            logic: Logic::Number,
            pins: &["A0"],
            signals: &[("AO", "A0")],
            includes: "",
            globals: "",
            setup: "",
            sample: "  float value = map(analogRead(A0), 1023, 0, 0, 100);\n",
            print: "  Serial.println(value, 0);\n",
            unit: L::new("%", "%"),
            decimals: 0,
            condition: Condition::Below(40.0),
            level: 1,
            note: L::new(
                "Сухой воздух и сухая почва дают разные числа: откалибруй порог на своей земле, полив и насухо.",
                "Dry air and dry soil read differently: calibrate the threshold on your own soil, wet and dry.",
            ),
        }],
    },
];

pub const SINKS: &[SinkSpec] = &[
    SinkSpec {
        id: SinkId::Led,
        title: L::new("светодиод", "LED"),
        pins: &["D9", "D10", "D11", "D6", "D5"],
        take: 1,
        current_ma: 20,
        wiring: SinkWiring::Pin,
        module: None,
        at: (0.0, 0.0),
        includes: "",
        globals: "",
        setup: "",
        show: "",
    },
    SinkSpec {
        id: SinkId::Buzzer,
        title: L::new("пищалка", "buzzer"),
        pins: &["D8", "D7", "D6", "D5"],
        take: 1,
        current_ma: 30,
        wiring: SinkWiring::Pin,
        module: None,
        at: (0.0, 0.0),
        includes: "",
        globals: "",
        setup: "",
        show: "",
    },
    SinkSpec {
        id: SinkId::Relay,
        title: L::new("реле", "relay"),
        pins: &["D7", "D6", "D5"],
        take: 1,
        current_ma: 70,
        wiring: SinkWiring::Pin,
        module: None,
        at: (0.0, 0.0),
        includes: "",
        globals: "",
        setup: "",
        show: "",
    },
    SinkSpec {
        id: SinkId::Lcd,
        title: L::new("дисплей LCD1602", "an LCD1602 display"),
        pins: &["A4", "A5"],
        take: 2,
        current_ma: 25,
        wiring: SinkWiring::I2c,
        module: Some("LCD1602"),
        at: (44.0, 64.0),
        includes: "#include <Wire.h>\n#include <LiquidCrystal_I2C.h>\n",
        globals: "LiquidCrystal_I2C lcd(0x27, 16, 2);\n",
        setup: "  lcd.init();\n  lcd.backlight();\n",
        show: "  lcd.clear();\n  lcd.setCursor(0, 0);\n  lcd.print(F(\"<<NAME>>\"));\n  lcd.setCursor(0, 1);\n  lcd.print(value, <<DECIMALS>>);\n",
    },
    SinkSpec {
        id: SinkId::Lcd2004,
        title: L::new("дисплей LCD2004", "an LCD2004 display"),
        pins: &["A4", "A5"],
        take: 2,
        current_ma: 30,
        wiring: SinkWiring::I2c,
        module: Some("LCD2004"),
        at: (36.0, 64.0),
        includes: "#include <Wire.h>\n#include <LiquidCrystal_I2C.h>\n",
        globals: "LiquidCrystal_I2C lcd(0x27, 20, 4);\n",
        setup: "  lcd.init();\n  lcd.backlight();\n",
        show: "  lcd.clear();\n  lcd.setCursor(0, 0);\n  lcd.print(F(\"<<NAME>>\"));\n  lcd.setCursor(0, 1);\n  lcd.print(value, <<DECIMALS>>);\n",
    },
    SinkSpec {
        id: SinkId::Oled,
        title: L::new("дисплей OLED 0.96", "an OLED 0.96 display"),
        pins: &["A4", "A5"],
        take: 2,
        current_ma: 15,
        wiring: SinkWiring::I2c,
        module: Some("OLED 0.96"),
        at: (44.0, 64.0),
        includes: "#include <Wire.h>\n#include <Adafruit_GFX.h>\n#include <Adafruit_SSD1306.h>\n",
        globals: "Adafruit_SSD1306 display(128, 64, &Wire, -1);\n",
        setup: "  Wire.begin();\n  if (!display.begin(SSD1306_SWITCHCAPVCC, 0x3C)) Serial.println(F(\"no SSD1306\"));\n  display.clearDisplay();\n  display.setTextColor(SSD1306_WHITE);\n",
        show: "  display.clearDisplay();\n  display.setTextSize(1);\n  display.setCursor(0, 0);\n  display.print(F(\"<<NAME>>\"));\n  display.setTextSize(2);\n  display.setCursor(0, 20);\n  display.print(value, <<DECIMALS>>);\n  display.display();\n",
    },
    SinkSpec {
        id: SinkId::Servo,
        title: L::new("сервопривод", "a servo"),
        pins: &["D9", "D10", "D11", "D6", "D5"],
        take: 1,
        current_ma: 250,
        wiring: SinkWiring::Pin,
        module: None,
        at: (0.0, 0.0),
        includes: "#include <Servo.h>\n",
        globals: "Servo arm;\n",
        setup: "  arm.attach(SINK_PIN);\n",
        show: "",
    },
    SinkSpec {
        id: SinkId::Motor,
        title: L::new("мотор", "a motor"),
        // PWM pins only: the sketch drives the switch with analogWrite.
        pins: &["D9", "D10", "D11", "D6", "D5"],
        take: 1,
        current_ma: 250,
        wiring: SinkWiring::Pin,
        module: None,
        at: (0.0, 0.0),
        includes: "",
        globals: "",
        setup: "",
        show: "",
    },
    SinkSpec {
        id: SinkId::Rgb,
        title: L::new("RGB-светодиод", "an RGB LED"),
        pins: &["D9", "D10", "D11", "D6", "D5"],
        take: 3,
        current_ma: 60,
        wiring: SinkWiring::Pin,
        module: None,
        at: (0.0, 0.0),
        includes: "",
        globals: "",
        setup: "",
        show: "",
    },
];

pub fn source(id: SourceId) -> &'static SourceSpec {
    &SOURCES[id as usize]
}

pub fn sink(id: SinkId) -> &'static SinkSpec {
    &SINKS[id as usize]
}

/// A source that can answer a sentence about this lesson.
pub fn source_for_lesson(lesson: usize) -> Option<&'static SourceSpec> {
    SOURCES.iter().find(|s| s.lesson == lesson)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The enum order and the table order have to stay in step: `source()`
    /// indexes the slice with the discriminant.
    #[test]
    fn ids_match_the_table() {
        for (i, s) in SOURCES.iter().enumerate() {
            assert_eq!(s.id as usize, i, "{} is out of order", s.title.ru);
            assert_eq!(source(s.id).id, s.id);
            assert!(!s.variants.is_empty(), "{}", s.title.ru);
        }
        for (i, s) in SINKS.iter().enumerate() {
            assert_eq!(s.id as usize, i, "{} is out of order", s.title.ru);
            assert_eq!(sink(s.id).id, s.id);
            assert!(!s.pins.is_empty());
        }
    }

    #[test]
    fn every_variant_declares_its_pins() {
        for s in SOURCES {
            for v in s.variants {
                assert!(!v.sample.contains("<<"), "{}: unfilled sample", s.title.ru);
                if v.logic == Logic::Flag {
                    assert_eq!(v.condition, Condition::Active, "{}", s.title.ru);
                }
                for (pin, uno) in v.signals {
                    assert!(!pin.is_empty(), "{}: empty header pin", s.title.ru);
                    assert!(
                        v.pins.contains(uno),
                        "{}: {} is wired to {uno} but not declared",
                        s.title.ru,
                        pin
                    );
                }
            }
        }
    }
}
