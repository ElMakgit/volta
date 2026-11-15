//! The practical lessons of the four ATmega328 books.
//!
//! Each entry knows its board, its wiring, a sketch and a short explanation.
//! The builder in `templates` turns an entry into the same `Circuit` the rest
//! of the program draws, checks and exports, so a lesson is not a picture: it
//! is a real netlist with a real sketch.

use crate::text::L;

/// A ready-made board and where it lies on the bench.
#[derive(Clone, Copy)]
pub struct BoardSpec {
    pub name: &'static str,
    pub at: (f32, f32),
}

impl BoardSpec {
    const fn new(name: &'static str, x: f32, y: f32) -> Self {
        Self { name, at: (x, y) }
    }
}

/// An extra component beside the board. The builder places it, wires it and
/// leaves the sketch to name the Uno pin.
#[derive(Clone, Copy)]
pub enum Extra {
    /// A LED with its own series resistor, anode on `pin`.
    Led { pin: &'static str, colour: &'static str },
    /// A four-leg RGB LED, one resistor per channel.
    Rgb { r: &'static str, g: &'static str, b: &'static str },
    /// A tactile switch between `pin` and ground, pull-up in the sketch.
    Button { pin: &'static str },
    /// A photoresistor divider: 10 kΩ to 5 V, LDR to ground, node on `pin`.
    Ldr { pin: &'static str },
    /// A knob across the rails, wiper on `pin`.
    Pot { pin: &'static str },
    /// A piezo or a small speaker on `pin`.
    Buzzer { pin: &'static str },
    /// An IR LED with a series resistor on `pin`.
    IrLed { pin: &'static str },
    /// A reed switch between `pin` and ground, pull-up in the sketch.
    Reed { pin: &'static str },
    /// A logic-level MOSFET switching `load` from `gate`.
    Mosfet { gate: &'static str, load: LoadKind, value: &'static str },
    /// A relay module whose contact switches `load`.
    Relay { pin: &'static str, load: LoadKind, value: &'static str },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoadKind {
    Motor,
    Peltier,
}

/// How the sketch is produced. Simple sensors share one honest shape; the
/// displays and libraries carry their own code.
pub enum Code {
    Digital { pin: &'static str, active_low: bool },
    /// A switch or sensor that also drives a LED, as most book lessons do.
    Sensor {
        pin: &'static str,
        active_low: bool,
        led: &'static str,
    },
    Analog { pin: &'static str },
    Raw(&'static str),
}

impl Code {
    pub fn render(&self, title: &str) -> String {
        match self {
            Code::Digital { pin, active_low } => {
                let mode = if *active_low { "INPUT_PULLUP" } else { "INPUT" };
                let level = if *active_low { "LOW" } else { "HIGH" };
                format!(
                    "// {title}\nconst uint8_t SENSOR = {pin};\nconst uint16_t PERIOD_MS = 250;\nuint32_t last = 0;\n\nvoid setup() {{\n  pinMode(SENSOR, {mode});\n  Serial.begin(9600);\n}}\n\nvoid loop() {{\n  if (millis() - last < PERIOD_MS) return;\n  last = millis();\n  bool active = digitalRead(SENSOR) == {level};\n  Serial.println(active ? 1 : 0);\n}}\n"
                )
            }
            Code::Sensor { pin, active_low, led } => {
                let mode = if *active_low { "INPUT_PULLUP" } else { "INPUT" };
                let level = if *active_low { "LOW" } else { "HIGH" };
                format!(
                    "// {title}\nconst uint8_t SENSOR = {pin};\nconst uint8_t LED = {led};\n\nvoid setup() {{\n  pinMode(SENSOR, {mode});\n  pinMode(LED, OUTPUT);\n}}\n\nvoid loop() {{\n  bool active = digitalRead(SENSOR) == {level};\n  digitalWrite(LED, active ? HIGH : LOW);\n}}\n"
                )
            }
            Code::Analog { pin } => format!(
                "// {title}\nconst uint8_t SENSE = {pin};\nconst uint16_t PERIOD_MS = 200;\nuint32_t last = 0;\n\nvoid setup() {{\n  Serial.begin(9600);\n}}\n\nvoid loop() {{\n  if (millis() - last < PERIOD_MS) return;\n  last = millis();\n  int value = analogRead(SENSE);\n  Serial.println(value);\n}}\n"
            ),
            Code::Raw(code) => (*code).to_string(),
        }
    }
}

pub struct Lesson {
    pub book: u8,
    pub number: u8,
    pub title: L,
    /// Words that pick this lesson out of a sentence.
    pub keys: &'static [&'static str],
    pub boards: &'static [BoardSpec],
    /// Module pin to Uno pin (or `GND`, `5V`, `3V3`). Supply pins are wired
    /// automatically from the module table.
    pub wires: &'static [(&'static str, &'static str)],
    pub extras: &'static [Extra],
    pub code: Code,
    pub summary: L,
    pub notes: &'static [L],
    pub level: u8,
}

const NO_BOARDS: &[BoardSpec] = &[];
const NO_WIRES: &[(&str, &str)] = &[];
const NO_EXTRA: &[Extra] = &[];
const NO_NOTES: &[L] = &[];

const fn board(name: &'static str, x: f32, y: f32) -> BoardSpec {
    BoardSpec::new(name, x, y)
}

pub const ALL: &[Lesson] = &[
    // ---------------------------------------------------------------- part I
    Lesson {
        book: 1,
        number: 6,
        title: L::new("Светодиод: первая программа", "An LED: the first sketch"),
        keys: &["занятие 6", "первый светодиод", "первая программа", "мигает"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Led { pin: "D9", colour: "красный 5 мм" }],
        code: Code::Raw(
            r#"// Светодиод мигает без delay(): плата остаётся свободной
const uint8_t LED = 9;
const uint16_t HALF_MS = 500;
bool on = false;
uint32_t last = 0;

void setup() {
  pinMode(LED, OUTPUT);
}

void loop() {
  if (millis() - last >= HALF_MS) {
    last = millis();
    on = !on;
    digitalWrite(LED, on ? HIGH : LOW);
  }
}
"#,
        ),
        summary: L::new(
            "Светодиод с резистором 220 Ω на D9: мигание по millis().",
            "An LED with a 220 Ω resistor on D9, blinking on millis().",
        ),
        notes: NO_NOTES,
        level: 1,
    },
    Lesson {
        book: 1,
        number: 7,
        title: L::new("Плавное свечение: ШИМ", "Fading: PWM"),
        keys: &["занятие 7", "плавн", "свечен"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Led { pin: "D9", colour: "красный 5 мм" }],
        code: Code::Raw(
            r#"// Плавное свечение на аппаратном ШИМ
const uint8_t LED = 9;
const uint16_t STEP_MS = 12;
uint8_t duty = 0;
int8_t dir = 1;
uint32_t last = 0;

void setup() {
  pinMode(LED, OUTPUT);
}

void loop() {
  if (millis() - last >= STEP_MS) {
    last = millis();
    if (dir > 0 && duty == 255) dir = -1;
    if (dir < 0 && duty == 0) dir = 1;
    duty += dir;
    analogWrite(LED, duty);
  }
}
"#,
        ),
        summary: L::new(
            "ШИМ на D9: скважность меняется ступеньками, свет плавный.",
            "PWM on D9: the duty ramps in steps, so the light looks smooth.",
        ),
        notes: &[L::new(
            "ШИМ есть только на D3, D5, D6, D9, D10 и D11 — там стоят таймеры.",
            "PWM exists only on D3, D5, D6, D9, D10 and D11, where the timers are.",
        )],
        level: 1,
    },
    Lesson {
        book: 1,
        number: 8,
        title: L::new("Цветной светодиод RGB", "An RGB LED"),
        keys: &["rgb", "цветн светодиод", "трехцветн"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Rgb { r: "D9", g: "D10", b: "D11" }],
        code: Code::Raw(
            r#"// Три канала RGB, у каждого свой резистор 220 Ω
const uint8_t R = 9, G = 10, B = 11;
const uint16_t STEP_MS = 400;
uint8_t phase = 0;
uint32_t last = 0;

void setColour(uint8_t c) {
  digitalWrite(R, (c & 0b001) ? HIGH : LOW);
  digitalWrite(G, (c & 0b010) ? HIGH : LOW);
  digitalWrite(B, (c & 0b100) ? HIGH : LOW);
}

void setup() {
  pinMode(R, OUTPUT);
  pinMode(G, OUTPUT);
  pinMode(B, OUTPUT);
}

void loop() {
  if (millis() - last >= STEP_MS) {
    last = millis();
    setColour(phase++ % 8);
  }
}
"#,
        ),
        summary: L::new(
            "Восемь цветов перебором трёх каналов: D9, D10, D11 и общий катод.",
            "Eight colours by mixing three channels on D9, D10 and D11.",
        ),
        notes: &[L::new(
            "Общий вывод RGB — катод: он идёт на землю, каналы зажигаются высоким уровнем.",
            "The shared RGB leg is the cathode: it goes to ground, and the channels light on HIGH.",
        )],
        level: 1,
    },
    Lesson {
        book: 1,
        number: 9,
        title: L::new("Мощная нагрузка: MOSFET", "A heavy load: the MOSFET"),
        keys: &["мощн нагрузк", "мощная нагрузка", "полев транзистор", "irlz44n"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[
            Extra::Button { pin: "D2" },
            Extra::Mosfet { gate: "D9", load: LoadKind::Motor, value: "5 В DC" },
        ],
        code: Code::Raw(
            r#"// Кнопка на D2 включает мотор через IRLZ44N на D9
const uint8_t BTN = 2;
const uint8_t PWM_PIN = 9;
const uint16_t DEBOUNCE_MS = 30;
bool on = false;
bool pressed = false;
uint32_t lastEdge = 0;

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  pinMode(PWM_PIN, OUTPUT);
  analogWrite(PWM_PIN, 0);
}

void loop() {
  bool now = digitalRead(BTN) == LOW;
  if (now != pressed && millis() - lastEdge > DEBOUNCE_MS) {
    lastEdge = millis();
    pressed = now;
    if (now) {
      on = !on;
      analogWrite(PWM_PIN, on ? 255 : 0);
    }
  }
}
"#,
        ),
        summary: L::new(
            "N-канальный MOSFET в ключе низкой стороны: затвор на D9 через 1 кОм, исток на земле.",
            "A logic-level N-MOSFET as a low-side switch: gate on D9 through 1 kΩ, source to ground.",
        ),
        notes: &[L::new(
            "Мотор защищён диодом 1N4007 катодом к +5 В, анодом к стоку.",
            "The motor has a 1N4007 across it, cathode to +5 V and anode to the drain.",
        )],
        level: 2,
    },
    Lesson {
        book: 1,
        number: 10,
        title: L::new("Геркон", "A reed switch"),
        keys: &["геркон", "reed"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Reed { pin: "D2" }, Extra::Led { pin: "D9", colour: "зелёный 5 мм" }],
        code: Code::Sensor { pin: "2", active_low: true, led: "9" },
        summary: L::new(
            "Геркон замыкает D2 на землю, когда рядом магнит; светодиод на D9 повторяет состояние.",
            "The reed pulls D2 to ground near a magnet; the LED on D9 follows it.",
        ),
        notes: &[L::new(
            "Геркон — это контакт в стеклянной трубке: дребезг есть, поэтому в скетче INPUT_PULLUP.",
            "A reed is a contact in a glass tube, so it bounces; the sketch uses INPUT_PULLUP.",
        )],
        level: 1,
    },
    Lesson {
        book: 1,
        number: 11,
        title: L::new("Кнопка", "A push button"),
        keys: &["занятие 11"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Button { pin: "D2" }, Extra::Led { pin: "D9", colour: "красный 5 мм" }],
        code: Code::Raw(
            r#"// Кнопка на D2 переключает светодиод на D9
const uint8_t BTN = 2;
const uint8_t LED = 9;
const uint16_t DEBOUNCE_MS = 30;
bool on = false;
bool pressed = false;
uint32_t lastEdge = 0;

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  pinMode(LED, OUTPUT);
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
        ),
        summary: L::new(
            "Кнопка замыкает D2 на землю, подтяжка включена внутри МК.",
            "The button pulls D2 to ground; the pull-up is inside the chip.",
        ),
        notes: &[L::new(
            "Без INPUT_PULLUP линия висела бы в воздухе и ловила наводки.",
            "Without INPUT_PULLUP the line would float and pick up noise.",
        )],
        level: 1,
    },
    Lesson {
        book: 1,
        number: 12,
        title: L::new("Реле", "A relay"),
        keys: &["реле", "relay"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Relay { pin: "D7", load: LoadKind::Motor, value: "5 В DC" }],
        code: Code::Raw(
            r#"// Реле на D7 замыкает цепь мотора
const uint8_t RELAY = 7;
const uint16_t PERIOD_MS = 1500;
bool on = false;
uint32_t last = 0;

void setup() {
  pinMode(RELAY, OUTPUT);
  digitalWrite(RELAY, LOW);
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    on = !on;
    digitalWrite(RELAY, on ? HIGH : LOW);
  }
}
"#,
        ),
        summary: L::new(
            "Модуль реле на D7 разрывает цепь мотора: контакт развязывает нагрузки.",
            "A relay module on D7 switches the motor circuit, so the contact isolates the load.",
        ),
        notes: &[L::new(
            "Катушка реле — индуктивность; диод на модуле уже стоит, отдельный не нужен.",
            "The relay coil is inductive; the module already carries its diode.",
        )],
        level: 2,
    },
    Lesson {
        book: 1,
        number: 13,
        title: L::new("Монитор порта", "The serial monitor"),
        keys: &["занятие 13", "монитор порта"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Button { pin: "D2" }],
        code: Code::Raw(
            r#"// Состояние кнопки уходит в порт на 9600 бод
const uint8_t BTN = 2;
const uint16_t PERIOD_MS = 200;
bool was = false;
uint32_t last = 0;

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  Serial.begin(9600);
  Serial.println(F("ready"));
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    bool now = digitalRead(BTN) == LOW;
    if (now != was) {
      was = now;
      Serial.println(now ? F("pressed") : F("released"));
    }
  }
}
"#,
        ),
        summary: L::new(
            "Кнопка на D2, события печатаются в последовательный порт.",
            "A button on D2; its edges are printed to the serial port.",
        ),
        notes: &[L::new(
            "Serial.begin(9600) должно совпадать со скоростью в мониторе порта.",
            "Serial.begin(9600) has to match the speed chosen in the serial monitor.",
        )],
        level: 1,
    },
    Lesson {
        book: 1,
        number: 14,
        title: L::new("Пьезоизлучатель", "A piezo buzzer"),
        keys: &["занятие 14", "пьезоизлучат"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Button { pin: "D2" }, Extra::Buzzer { pin: "D8" }],
        code: Code::Raw(
            r#"// Пьезо на D8 пищит, пока держат кнопку на D2
const uint8_t BTN = 2;
const uint8_t BUZZER = 8;

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  pinMode(BUZZER, OUTPUT);
}

void loop() {
  if (digitalRead(BTN) == LOW) {
    tone(BUZZER, 880);
  } else {
    noTone(BUZZER);
  }
}
"#,
        ),
        summary: L::new(
            "Пьезо на D8: тон 880 Гц по нажатой кнопке.",
            "The piezo on D8 sounds at 880 Hz while the button is held.",
        ),
        notes: NO_NOTES,
        level: 1,
    },
    Lesson {
        book: 1,
        number: 15,
        title: L::new("Фоторезистор", "A photoresistor"),
        keys: &["занятие 15", "фоторезистор"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Ldr { pin: "A0" }, Extra::Led { pin: "D9", colour: "жёлтый 5 мм" }],
        code: Code::Raw(
            r#"// Ночник: делитель с фоторезистором на A0
const uint8_t SENSE = A0;
const uint8_t LED = 9;
const int THRESHOLD = 400;
const uint16_t PERIOD_MS = 100;
uint32_t last = 0;

void setup() {
  pinMode(LED, OUTPUT);
  Serial.begin(9600);
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    int light = analogRead(SENSE);
    digitalWrite(LED, light < THRESHOLD ? HIGH : LOW);
    Serial.println(light);
  }
}
"#,
        ),
        summary: L::new(
            "Делитель 10 кОм + фоторезистор на A0; светодиод на D9 включается в темноте.",
            "A 10 kΩ and photoresistor divider on A0; the LED on D9 turns on in the dark.",
        ),
        notes: &[L::new(
            "Порог 400 — отправная точка: у каждого фоторезистора своя кривая.",
            "The 400 threshold is a starting point; every photoresistor differs.",
        )],
        level: 2,
    },
    Lesson {
        book: 1,
        number: 16,
        title: L::new("Термистор и DS18B20", "A thermistor and the DS18B20"),
        keys: &["термистор", "ds18b20", "18b20"],
        boards: &[board("DS18B20", 2.0, 54.0)],
        wires: &[("DATA", "D2")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// DS18B20 на D2, питание 5 В, DATA подтянут к питанию
#include <OneWire.h>
#include <DallasTemperature.h>
const uint8_t ONE_WIRE = 2;
const uint16_t PERIOD_MS = 1000;
OneWire wire(ONE_WIRE);
DallasTemperature sensors(&wire);
uint32_t last = 0;

void setup() {
  Serial.begin(9600);
  sensors.begin();
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    sensors.requestTemperatures();
    Serial.println(sensors.getTempCByIndex(0));
  }
}
"#,
        ),
        summary: L::new(
            "DS18B20 на D2: цифровая температура по шине 1-Wire.",
            "A DS18B20 on D2: digital temperature over the 1-Wire bus.",
        ),
        notes: &[L::new(
            "Линию DATA подтягивают к 5 В резистором 4.7 кОм — на модуле он часто уже стоит.",
            "Pull DATA to 5 V with a 4.7 kΩ resistor; the breakout often has one.",
        )],
        level: 2,
    },
    Lesson {
        book: 1,
        number: 17,
        title: L::new("DHT11 и DHT22", "The DHT11 and DHT22"),
        keys: &["dht11", "dht22", "влажност"],
        boards: &[board("DHT11", 2.0, 54.0)],
        wires: &[("DATA", "D2")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// DHT11/DHT22 на D2: температура и влажность
#include <DHT.h>
const uint8_t DATA_PIN = 2;
const uint16_t PERIOD_MS = 2500;
DHT dht(DATA_PIN, DHT11);
uint32_t last = 0;

void setup() {
  Serial.begin(9600);
  dht.begin();
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    float t = dht.readTemperature();
    float h = dht.readHumidity();
    Serial.print(t);
    Serial.print(F(" C "));
    Serial.println(h);
  }
}
"#,
        ),
        summary: L::new(
            "DHT11 на D2: оба значения читаются одной линией не чаще раза в 2 секунды.",
            "A DHT11 on D2: both readings come over one wire, at most every two seconds.",
        ),
        notes: &[L::new(
            "Для DHT22 в конструкторе DHT меняют DHT11 на DHT22.",
            "For a DHT22, change DHT11 to DHT22 in the DHT constructor.",
        )],
        level: 2,
    },
    Lesson {
        book: 1,
        number: 18,
        title: L::new("Давление: BMP180, BMP280, BME280", "Pressure: BMP180, BMP280, BME280"),
        keys: &["bmp085", "bmp180", "bmp280", "bme280", "давлен"],
        boards: &[board("BMP280", 2.0, 54.0)],
        wires: &[("SDA", "A4"), ("SCL", "A5")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// BMP280 на I2C: SDA — A4, SCL — A5
#include <Wire.h>
#include <Adafruit_BMP280.h>
Adafruit_BMP280 bmp;
const uint16_t PERIOD_MS = 1000;
uint32_t last = 0;

void setup() {
  Serial.begin(9600);
  Wire.begin();
  if (!bmp.begin(0x76)) {
    Serial.println(F("no BMP280"));
  }
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    Serial.print(bmp.readPressure() / 100.0);
    Serial.print(F(" hPa "));
    Serial.println(bmp.readTemperature());
  }
}
"#,
        ),
        summary: L::new(
            "Датчик давления на I2C: адрес 0x76, SDA на A4, SCL на A5.",
            "The pressure sensor on I2C: address 0x76, SDA on A4 and SCL on A5.",
        ),
        notes: &[L::new(
            "BMP180 отвечает по адресу 0x77 и требует библиотеки Adafruit BMP085.",
            "The BMP180 answers at 0x77 and uses the Adafruit BMP085 library.",
        )],
        level: 2,
    },
    Lesson {
        book: 1,
        number: 19,
        title: L::new("Датчик газа MQ-2", "The MQ-2 gas sensor"),
        keys: &["mq-2", "mq2", "газ", "дым"],
        boards: &[board("MQ-2", 2.0, 54.0)],
        wires: &[("AO", "A0"), ("DO", "D2")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// MQ-2: аналог на A0, порог на D2
const uint8_t SENSE = A0;
const uint8_t ALARM = 2;
const uint16_t PERIOD_MS = 250;
uint32_t last = 0;

void setup() {
  pinMode(ALARM, INPUT);
  Serial.begin(9600);
  Serial.println(F("warm up 20 s"));
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    Serial.print(analogRead(SENSE));
    Serial.print(' ');
    Serial.println(digitalRead(ALARM));
  }
}
"#,
        ),
        summary: L::new(
            "MQ-2: аналоговый выход на A0, компараторный — на D2.",
            "The MQ-2: analogue output on A0, comparator output on D2.",
        ),
        notes: &[L::new(
            "Нагревателю нужно 20 секунд на прогрев, а всему датчику — до суток на калибровку.",
            "The heater needs 20 seconds to warm up and the sensor up to a day to settle.",
        )],
        level: 2,
    },
    Lesson {
        book: 1,
        number: 20,
        title: L::new("Датчик движения HC-SR501", "The HC-SR501 motion sensor"),
        keys: &["pir", "hc-sr501", "движени"],
        boards: &[board("HC-SR501", 2.0, 54.0)],
        wires: &[("OUT", "D2")],
        extras: NO_EXTRA,
        code: Code::Digital { pin: "2", active_low: false },
        summary: L::new(
            "PIR выдаёт высокий уровень на D2, пока видит движение.",
            "The PIR holds D2 high while it sees movement.",
        ),
        notes: &[L::new(
            "После включения PIR молчит около минуты: так работает его схема.",
            "The PIR stays quiet for about a minute after power-up; that is its settling time.",
        )],
        level: 1,
    },
    Lesson {
        book: 1,
        number: 21,
        title: L::new("Ультразвуковой дальномер HC-SR04", "The HC-SR04 ultrasonic ranger"),
        keys: &["hc-sr04", "ультразвук", "дальномер"],
        boards: &[board("HC-SR04", 2.0, 54.0)],
        wires: &[("TRIG", "D9"), ("ECHO", "D10")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// HC-SR04: импульс на TRIG, длительность на ECHO
const uint8_t TRIG = 9;
const uint8_t ECHO = 10;
const uint16_t PERIOD_MS = 100;
uint32_t last = 0;

void setup() {
  pinMode(TRIG, OUTPUT);
  pinMode(ECHO, INPUT);
  Serial.begin(9600);
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    digitalWrite(TRIG, LOW);
    digitalWrite(TRIG, HIGH);
    unsigned long us = pulseIn(ECHO, HIGH, 30000UL);
    Serial.print(us / 58);
    Serial.println(F(" cm"));
  }
}
"#,
        ),
        summary: L::new(
            "TRIG на D9, ECHO на D10: расстояние = длительность / 58.",
            "TRIG on D9, ECHO on D10: distance is the pulse length divided by 58.",
        ),
        notes: &[L::new(
            "Между импульсами нужно не меньше 60 мс, иначе ловится эхо предыдущего.",
            "Leave at least 60 ms between bursts, or the previous echo is caught.",
        )],
        level: 2,
    },
    Lesson {
        book: 1,
        number: 22,
        title: L::new("ИК-датчик препятствия TCRT5000", "The TCRT5000 obstacle sensor"),
        keys: &["tcrt5000", "препятств", "инфракрасн датчик"],
        boards: &[board("TCRT5000", 2.0, 54.0)],
        wires: &[("DO", "D2"), ("AO", "A0")],
        extras: NO_EXTRA,
        code: Code::Analog { pin: "A0" },
        summary: L::new(
            "TCRT5000: отражение меняет аналог на A0, порог доступен на D2.",
            "The TCRT5000: reflected light changes A0, and its comparator is on D2.",
        ),
        notes: NO_NOTES,
        level: 1,
    },
    Lesson {
        book: 1,
        number: 23,
        title: L::new("ИК-пульт: TSOP1838", "An IR remote: the TSOP1838"),
        keys: &["tsop", "ик-пульт", "пульт"],
        boards: &[board("TSOP1838", 2.0, 54.0)],
        wires: &[("OUT", "D2")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// TSOP1838 на D2: коды кнопок пульта
#include <IRremote.h>
const uint8_t RECV = 2;

void setup() {
  Serial.begin(9600);
  IrReceiver.begin(RECV, false);
}

void loop() {
  if (IrReceiver.decode()) {
    Serial.println(IrReceiver.decodedIRData.command);
    IrReceiver.resume();
  }
}
"#,
        ),
        summary: L::new(
            "Приёмник TSOP1838 на D2 отдаёт код нажатой кнопки пульта.",
            "The TSOP1838 on D2 hands over the code of the pressed remote button.",
        ),
        notes: &[L::new(
            "У приёмника три вывода: OUT, GND и VCC; перепутать питание — сжечь его.",
            "The receiver has OUT, GND and VCC; swapping power kills it.",
        )],
        level: 2,
    },
    Lesson {
        book: 1,
        number: 24,
        title: L::new("Энергонезависимая память EEPROM", "EEPROM memory"),
        keys: &["eeprom", "энергонезависим"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Button { pin: "D2" }],
        code: Code::Raw(
            r#"// Счётчик нажатий переживает выключение: EEPROM
#include <EEPROM.h>
const uint8_t BTN = 2;
const uint16_t DEBOUNCE_MS = 30;
const uint16_t ADDR = 0;
bool pressed = false;
uint32_t lastEdge = 0;

void setup() {
  pinMode(BTN, INPUT_PULLUP);
  Serial.begin(9600);
  Serial.println(EEPROM.read(ADDR));
}

void loop() {
  bool now = digitalRead(BTN) == LOW;
  if (now != pressed && millis() - lastEdge > DEBOUNCE_MS) {
    lastEdge = millis();
    pressed = now;
    if (now) {
      uint8_t n = EEPROM.read(ADDR) + 1;
      EEPROM.update(ADDR, n);
      Serial.println(n);
    }
  }
}
"#,
        ),
        summary: L::new(
            "Счётчик нажатий пишется в EEPROM и переживает сброс платы.",
            "A press counter goes to EEPROM and survives a board reset.",
        ),
        notes: &[L::new(
            "У ячейки EEPROM около 100 000 циклов записи: update() пишет только при изменении.",
            "An EEPROM cell lasts about 100 000 writes; update() only writes when the byte changed.",
        )],
        level: 2,
    },
    // --------------------------------------------------------------- part II
    Lesson {
        book: 2,
        number: 1,
        title: L::new("Текстовый дисплей LCD1602 по I2C", "An LCD1602 over I2C"),
        keys: &["lcd1602", "16x2", "16*2"],
        boards: &[board("LCD1602", 2.0, 54.0)],
        wires: &[("SDA", "A4"), ("SCL", "A5")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// LCD1602 с модулем I2C: SDA — A4, SCL — A5
#include <Wire.h>
#include <LiquidCrystal_I2C.h>
LiquidCrystal_I2C lcd(0x27, 16, 2);

void setup() {
  Wire.begin();
  lcd.init();
  lcd.backlight();
  lcd.print(F("volta"));
}

void loop() {
  lcd.setCursor(0, 1);
  lcd.print(millis() / 1000);
}
"#,
        ),
        summary: L::new(
            "Символьный дисплей 16×2 через переходник I2C: всего две сигнальные линии.",
            "A 16×2 character display through an I2C backpack: only two signal wires.",
        ),
        notes: &[L::new(
            "Адрес переходника бывает 0x27 или 0x3F — его находят сканером I2C.",
            "The backpack answers at 0x27 or 0x3F; an I2C scanner tells which.",
        )],
        level: 2,
    },
    Lesson {
        book: 2,
        number: 1,
        title: L::new("Текстовый дисплей LCD2004 по I2C", "An LCD2004 over I2C"),
        keys: &["lcd2004", "20x4", "20*4", "lcd 20"],
        boards: &[board("LCD2004", 2.0, 54.0)],
        wires: &[("SDA", "A4"), ("SCL", "A5")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// LCD2004: четыре строки по двадцать знаков
#include <Wire.h>
#include <LiquidCrystal_I2C.h>
LiquidCrystal_I2C lcd(0x27, 20, 4);

void setup() {
  Wire.begin();
  lcd.init();
  lcd.backlight();
  lcd.setCursor(0, 0);
  lcd.print(F("volta"));
}

void loop() {
  lcd.setCursor(0, 3);
  lcd.print(millis() / 1000);
}
"#,
        ),
        summary: L::new(
            "LCD 20×4 на том же переходнике I2C: в конструкторе меняются только размеры.",
            "A 20×4 LCD on the same I2C backpack; only the size changes in the constructor.",
        ),
        notes: NO_NOTES,
        level: 2,
    },
    Lesson {
        book: 2,
        number: 2,
        title: L::new("Одноразрядный семисегментный индикатор", "A single seven-segment digit"),
        keys: &["7seg-1", "семисегмент", "одноразрядн"],
        boards: &[board("7SEG-1", 2.0, 54.0)],
        wires: &[
            ("A", "D2"),
            ("B", "D3"),
            ("C", "D4"),
            ("D", "D5"),
            ("E", "D6"),
            ("F", "D7"),
            ("G", "D8"),
            ("DP", "D9"),
            ("COM", "GND"),
        ],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Сегменты A..G и DP на D2..D9, общий вывод на землю
const uint8_t SEG[8] = {2, 3, 4, 5, 6, 7, 8, 9};
// Цифра 5: A, C, D, F, G
const uint8_t MASK = 0b0110_1101;

void setup() {
  for (uint8_t i = 0; i < 8; i++) pinMode(SEG[i], OUTPUT);
}

void loop() {
  for (uint8_t i = 0; i < 8; i++) {
    digitalWrite(SEG[i], (MASK >> i) & 1);
  }
}
"#,
        ),
        summary: L::new(
            "Семь сегментов и точка идут на D2–D9, общий катод — на землю.",
            "Seven segments and the dot go to D2–D9, with the common cathode on ground.",
        ),
        notes: &[L::new(
            "В реальной сборке каждому сегменту нужен свой резистор 220 Ω; в схеме они не показаны, чтобы не прятать проводку.",
            "A real build gives every segment its own 220 Ω resistor; the drawing omits them so the wiring stays readable.",
        )],
        level: 2,
    },
    Lesson {
        book: 2,
        number: 3,
        title: L::new("Четырёхразрядный индикатор по I2C", "A four-digit display over I2C"),
        keys: &["7seg-4", "четырехразрядн", "tm1650"],
        boards: &[board("7SEG-4", 2.0, 54.0)],
        wires: &[("SDA", "A4"), ("SCL", "A5")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Четырёхразрядный индикатор с модулем I2C
#include <Wire.h>
#include <TM1650.h>
TM1650 display;
const uint16_t PERIOD_MS = 1000;
uint32_t last = 0;

void setup() {
  Wire.begin();
  display.init();
  display.setBrightness(5, true);
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    display.displayString(String(millis() / 1000));
  }
}
"#,
        ),
        summary: L::new(
            "Четыре разряда на TM1650: динамическая индикация спрятана в модуле.",
            "Four digits on a TM1650: the multiplexing lives inside the module.",
        ),
        notes: NO_NOTES,
        level: 3,
    },
    Lesson {
        book: 2,
        number: 4,
        title: L::new("Матрица 8×8 на MAX7219", "An 8×8 matrix on a MAX7219"),
        keys: &["max7219", "матриц", "matrix", "бегущ строк"],
        boards: &[board("MAX7219", 2.0, 54.0)],
        wires: &[("DIN", "D11"), ("CS", "D10"), ("CLK", "D13")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Матрица 8x8: DIN — D11, CS — D10, CLK — D13
#include <LedControl.h>
const uint8_t DIN = 11;
const uint8_t CS = 10;
const uint8_t CLK = 13;
LedControl matrix(DIN, CLK, CS, 1);
const uint8_t HEART[8] = {0x00, 0x66, 0xFF, 0xFF, 0x7E, 0x3C, 0x18, 0x00};

void setup() {
  matrix.shutdown(0, false);
  matrix.setIntensity(0, 8);
  for (uint8_t row = 0; row < 8; row++) {
    matrix.setRow(0, row, HEART[row]);
  }
}

void loop() {
}
"#,
        ),
        summary: L::new(
            "MAX7219 держит на себе всю матрицу: от МК идут только DIN, CS и CLK.",
            "The MAX7219 drives the whole matrix; only DIN, CS and CLK come from the board.",
        ),
        notes: &[L::new(
            "MAX7219 питается от 5 В и берёт до 100 мА на полной яркости.",
            "The MAX7219 runs on 5 V and draws up to 100 mA at full brightness.",
        )],
        level: 3,
    },
    Lesson {
        book: 2,
        number: 5,
        title: L::new("Графический дисплей Nokia 5110", "A Nokia 5110 graphic display"),
        keys: &["nokia", "5110"],
        boards: &[board("NOKIA 5110", 2.0, 54.0)],
        wires: &[
            ("SCE", "D7"),
            ("RST", "D6"),
            ("DC", "D5"),
            ("DIN", "D11"),
            ("CLK", "D13"),
        ],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Nokia 5110: аппаратный SPI и четыре управляющие линии
#include <SPI.h>
#include <Adafruit_GFX.h>
#include <Adafruit_PCD8544.h>
const uint8_t SCE = 7;
const uint8_t RST = 6;
const uint8_t DC = 5;
Adafruit_PCD8544 lcd(SCE, DC, RST);

void setup() {
  lcd.begin();
  lcd.setContrast(55);
  lcd.setTextSize(1);
  lcd.println(F("volta"));
}

void loop() {
}
"#,
        ),
        summary: L::new(
            "Монохромный 84×48 на PCD8544: SPI плюс SCE, RST и DC.",
            "A monochrome 84×48 PCD8544: SPI plus SCE, RST and DC.",
        ),
        notes: NO_NOTES,
        level: 3,
    },
    Lesson {
        book: 2,
        number: 6,
        title: L::new("OLED 0.96 на SSD1306", "A 0.96 OLED on an SSD1306"),
        keys: &["oled", "0.96", "ssd1306"],
        boards: &[board("OLED 0.96", 2.0, 54.0)],
        wires: &[("SDA", "A4"), ("SCL", "A5")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// OLED 128x64 на I2C
#include <Wire.h>
#include <Adafruit_GFX.h>
#include <Adafruit_SSD1306.h>
Adafruit_SSD1306 oled(128, 64, &Wire, -1);

void setup() {
  Wire.begin();
  oled.begin(SSD1306_SWITCHCAPVCC, 0x3C);
  oled.clearDisplay();
  oled.setTextColor(SSD1306_WHITE);
  oled.setCursor(0, 0);
  oled.println(F("volta"));
  oled.display();
}

void loop() {
}
"#,
        ),
        summary: L::new(
            "OLED 128×64 на SSD1306: две линии I2C, адрес 0x3C.",
            "A 128×64 SSD1306 OLED: two I2C wires at address 0x3C.",
        ),
        notes: NO_NOTES,
        level: 2,
    },
    Lesson {
        book: 2,
        number: 7,
        title: L::new("Цветной TFT 1.44", "A 1.44 colour TFT"),
        keys: &["tft 1.44", "st7735", "1.44"],
        boards: &[board("TFT 1.44", 2.0, 54.0)],
        wires: &[
            ("CS", "D10"),
            ("RST", "D9"),
            ("DC", "D8"),
            ("MOSI", "D11"),
            ("SCK", "D13"),
        ],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// TFT 1.44 на ST7735: аппаратный SPI
#include <SPI.h>
#include <Adafruit_GFX.h>
#include <Adafruit_ST7735.h>
const uint8_t CS = 10;
const uint8_t RST = 9;
const uint8_t DC = 8;
Adafruit_ST7735 tft(CS, DC, RST);

void setup() {
  tft.initR(INITR_BLACKTAB);
  tft.fillScreen(ST77XX_BLACK);
  tft.setCursor(4, 4);
  tft.setTextColor(ST77XX_GREEN);
  tft.println(F("volta"));
}

void loop() {
}
"#,
        ),
        summary: L::new(
            "Цветной TFT 128×128: SPI и три управляющие линии CS, DC, RST.",
            "A 128×128 colour TFT: SPI plus the CS, DC and RST control lines.",
        ),
        notes: NO_NOTES,
        level: 3,
    },
    Lesson {
        book: 2,
        number: 8,
        title: L::new("Сенсорный TFT 2.4", "A 2.4 touch TFT"),
        keys: &["tft 2.4", "2.4", "сенсорн"],
        boards: &[board("TFT 2.4", 2.0, 54.0)],
        wires: &[
            ("CS", "D10"),
            ("RST", "D9"),
            ("DC", "D8"),
            ("MOSI", "D11"),
            ("SCK", "D13"),
            ("T_CS", "A1"),
            ("T_IRQ", "A2"),
        ],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Сенсорный TFT 2.4 на шилде MCUFRIEND
#include <MCUFRIEND_kbv.h>
MCUFRIEND_kbv tft;

void setup() {
  uint16_t id = tft.readID();
  tft.begin(id);
  tft.setRotation(1);
  tft.fillScreen(TFT_BLACK);
  tft.setTextColor(TFT_WHITE);
  tft.println(F("volta"));
}

void loop() {
}
"#,
        ),
        summary: L::new(
            "TFT 320×240 с сенсором: шилд ставится на Uno, контакты CS/DC/RST идут на D10/D8/D9.",
            "A 320×240 touch TFT: the shield sits on the Uno, with CS/DC/RST on D10/D8/D9.",
        ),
        notes: &[L::new(
            "Шилд занимает почти все цифровые пины; A1 и A2 остаются под сенсор.",
            "The shield claims most digital pins; A1 and A2 are left for the touch screen.",
        )],
        level: 3,
    },
    Lesson {
        book: 2,
        number: 9,
        title: L::new("Вывод на телевизор TV OUT", "TV output"),
        keys: &["tv out", "телевизор", "тв выход"],
        boards: &[board("TV OUT", 2.0, 54.0)],
        wires: &[("VIDEO", "D9")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// TVout: видеосигнал с D9 на тюльпан
#include <TVout.h>
const uint8_t VIDEO = 9;
TVout tv;

void setup() {
  tv.begin(PAL, 120, 96);
  tv.select_font(font6x8);
  tv.println(F("volta"));
}

void loop() {
}
"#,
        ),
        summary: L::new(
            "Библиотека TVout формирует видеосигнал на D9 и синхронизацию на D8.",
            "The TVout library makes the video signal on D9 and sync on D8.",
        ),
        notes: &[L::new(
            "Телевизор должен понимать композитный вход; современный HDMI такой сигнал не примет.",
            "The TV needs a composite input; a modern HDMI-only set will not take it.",
        )],
        level: 3,
    },
    Lesson {
        book: 2,
        number: 10,
        title: L::new("Монохромная панель P10", "A monochrome P10 panel"),
        keys: &["p10", "led панел", "панель p10"],
        boards: &[board("P10", 2.0, 54.0)],
        wires: &[
            ("DATA", "D11"),
            ("CLK", "D13"),
            ("LAT", "A0"),
            ("OE", "A1"),
            ("A", "D2"),
            ("B", "D3"),
        ],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Панель P10 32x16 на библиотеке DMD
#include <SPI.h>
#include <DMD.h>
#include <TimerOne.h>
#include <SystemFont5x7.h>
const uint8_t PANELS = 1;
DMD dmd(PANELS, 1);

void setup() {
  dmd.clearScreen(true);
  dmd.selectFont(SystemFont5x7);
  dmd.drawString(0, 0, F("volta"), 5, GRAPHICS_NORMAL);
  Timer1.initialize(5000);
  Timer1.attachInterrupt(scan);
}

void scan() {
  dmd.scanDisplayBySPI();
}

void loop() {
}
"#,
        ),
        summary: L::new(
            "P10 32×16: HUB12, данные на D11, такт на D13, строка на A0/A1.",
            "A 32×16 P10: HUB12, data on D11, clock on D13, row on A0/A1.",
        ),
        notes: &[L::new(
            "Панель берёт до 500 мА: питание отдельное, земля общая с платой.",
            "The panel draws up to 500 mA: give it its own supply and a common ground.",
        )],
        level: 3,
    },
    Lesson {
        book: 2,
        number: 11,
        title: L::new("Цветная панель P10 RGB", "A P10 RGB panel"),
        keys: &["p10 rgb", "цветная панель", "rgb панел", "цветная панель p10"],
        boards: &[board("P10 RGB", 2.0, 54.0)],
        wires: &[
            ("R", "D2"),
            ("G", "D3"),
            ("B", "D4"),
            ("CLK", "D13"),
            ("LAT", "A0"),
            ("OE", "A1"),
            ("A", "A2"),
            ("B", "A3"),
        ],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Цветная панель P10: три канала и общая развёртка
#include <RGBmatrixPanel.h>
const uint8_t CLK = 13;
const uint8_t LAT = 10;
const uint8_t OE = 9;
const uint8_t A = 8;
const uint8_t B = 7;
const uint8_t C = 6;
RGBmatrixPanel panel(A, B, C, CLK, LAT, OE, false, 32);

void setup() {
  panel.begin();
  panel.fillScreen(panel.Color333(0, 0, 0));
  panel.setCursor(1, 1);
  panel.setTextColor(panel.Color333(7, 0, 0));
  panel.print(F("volta"));
}

void loop() {
}
"#,
        ),
        summary: L::new(
            "P10 RGB: три канала яркости плюс те же CLK, LAT и OE.",
            "A P10 RGB: three colour channels plus the same CLK, LAT and OE.",
        ),
        notes: &[L::new(
            "Адресные линии A и B выбирают половину панели; у больших панелей их четыре.",
            "The address lines A and B choose a half of the panel; larger panels use four.",
        )],
        level: 3,
    },
    // -------------------------------------------------------------- part III
    Lesson {
        book: 3,
        number: 1,
        title: L::new("Часы реального времени DS3231", "A DS3231 real-time clock"),
        keys: &["ds3231", "ds1307", "часы реальн", "rtc"],
        boards: &[board("DS3231", 2.0, 54.0)],
        wires: &[("SDA", "A4"), ("SCL", "A5")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// DS3231 на I2C: время и температура кристалла
#include <Wire.h>
#include <RTClib.h>
RTC_DS3231 rtc;
const uint16_t PERIOD_MS = 1000;
uint32_t last = 0;

void setup() {
  Wire.begin();
  Serial.begin(9600);
  if (!rtc.begin()) Serial.println(F("no RTC"));
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    DateTime now = rtc.now();
    Serial.print(now.hour());
    Serial.print(':');
    Serial.println(now.minute());
  }
}
"#,
        ),
        summary: L::new(
            "DS3231 держит время сам: у него батарейка и термокомпенсированный кварц.",
            "The DS3231 keeps time on its own, with a coin cell and a temperature-compensated crystal.",
        ),
        notes: NO_NOTES,
        level: 2,
    },
    Lesson {
        book: 3,
        number: 2,
        title: L::new("RGB-кольцо WS2812", "A WS2812 RGB ring"),
        keys: &["ws2812", "rgb-кольц", "rgb кольц", "neopixel"],
        boards: &[board("WS2812", 2.0, 54.0)],
        wires: &[("DIN", "D6")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Кольцо WS2812: одна линия данных на D6
#include <FastLED.h>
const uint8_t DATA_PIN = 6;
const uint8_t LEDS = 12;
CRGB ring[LEDS];
uint8_t phase = 0;
uint32_t last = 0;

void setup() {
  FastLED.addLeds<WS2812B, DATA_PIN, GRB>(ring, LEDS);
  FastLED.setBrightness(40);
}

void loop() {
  if (millis() - last >= 40) {
    last = millis();
    fadeToBlackBy(ring, LEDS, 24);
    ring[phase % LEDS] = CRGB::Red;
    phase++;
    FastLED.show();
  }
}
"#,
        ),
        summary: L::new(
            "Адресная лента: все светодиоды висят на одной линии D6.",
            "An addressable strip: every LED hangs on the single data line D6.",
        ),
        notes: &[L::new(
            "На полной яркости 12 светодиодов берут около 700 мА — от USB лучше не питать.",
            "At full brightness 12 LEDs draw about 700 mA, which is too much for USB.",
        )],
        level: 2,
    },
    Lesson {
        book: 3,
        number: 3,
        title: L::new("ИК-излучатель", "An IR emitter"),
        keys: &["ик-излучат", "ик передатчик", "ir led"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::IrLed { pin: "D3" }],
        code: Code::Raw(
            r#"// ИК-светодиод на D3: посылка кода пульта
#include <IRremote.h>
const uint8_t IR_LED = 3;

void setup() {
  IrSender.begin(IR_LED);
}

void loop() {
  IrSender.sendNEC(0x00FFA25D, 0x01, 0);
  tone(4, 0, 0);
}
"#,
        ),
        summary: L::new(
            "ИК-светодиод на D3 повторяет код кнопки пульта NEC.",
            "An IR LED on D3 repeats a NEC remote button code.",
        ),
        notes: &[L::new(
            "Мощность ИК-светодиода ограничивают резистором 100–220 Ω, иначе он быстро деградирует.",
            "Limit the IR LED with a 100–220 Ω resistor, or it degrades quickly.",
        )],
        level: 2,
    },
    Lesson {
        book: 3,
        number: 4,
        title: L::new("Клавиатура PS/2", "A PS/2 keyboard"),
        keys: &["ps/2", "ps2", "клавиатур"],
        boards: &[board("PS/2", 2.0, 54.0)],
        wires: &[("CLK", "D2"), ("DATA", "D3")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// PS/2: такт на D2, данные на D3
#include <PS2Keyboard.h>
const uint8_t CLK = 2;
const uint8_t DATA = 3;
PS2Keyboard keyboard;

void setup() {
  Serial.begin(9600);
  keyboard.begin(DATA, CLK);
}

void loop() {
  if (keyboard.available()) {
    char key = keyboard.read();
    Serial.print(key);
  }
}
"#,
        ),
        summary: L::new(
            "PS/2-клавиатура отдаёт скан-коды по двум линиям: CLK и DATA.",
            "A PS/2 keyboard sends scan codes over two wires: CLK and DATA.",
        ),
        notes: NO_NOTES,
        level: 3,
    },
    Lesson {
        book: 3,
        number: 5,
        title: L::new("Карта памяти SD", "An SD card"),
        keys: &["sd-карт", "microsd", "sd card", "карта памяти"],
        boards: &[board("SD", 2.0, 54.0)],
        wires: &[("CS", "D10"), ("SCK", "D13"), ("MOSI", "D11"), ("MISO", "D12")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// SD-карта на SPI: CS — D10
#include <SPI.h>
#include <SD.h>
const uint8_t CS = 10;
const uint16_t PERIOD_MS = 5000;
uint32_t last = 0;

void setup() {
  Serial.begin(9600);
  if (!SD.begin(CS)) {
    Serial.println(F("no card"));
    return;
  }
  File log = SD.open("volta.txt", FILE_WRITE);
  if (log) {
    log.println(F("start"));
    log.close();
  }
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    File log = SD.open("volta.txt", FILE_WRITE);
    if (log) {
      log.println(millis() / 1000);
      log.close();
    }
  }
}
"#,
        ),
        summary: L::new(
            "SD-карта на аппаратном SPI: карта держится за CS на D10.",
            "An SD card on hardware SPI: the card is selected by CS on D10.",
        ),
        notes: &[L::new(
            "Карту форматируют в FAT16 или FAT32; exFAT библиотека не читает.",
            "Format the card as FAT16 or FAT32; the library does not read exFAT.",
        )],
        level: 3,
    },
    Lesson {
        book: 3,
        number: 6,
        title: L::new("Датчик звука", "A sound sensor"),
        keys: &["датчик звука", "микрофон", "sound sensor"],
        boards: &[board("SOUND", 2.0, 54.0)],
        wires: &[("AO", "A0"), ("DO", "D2")],
        extras: NO_EXTRA,
        code: Code::Analog { pin: "A0" },
        summary: L::new(
            "Микрофонный модуль: огибающая на A0, компаратор на D2.",
            "A microphone module: the envelope on A0 and a comparator on D2.",
        ),
        notes: NO_NOTES,
        level: 1,
    },
    Lesson {
        book: 3,
        number: 7,
        title: L::new("MP3-плеер DFPlayer", "The DFPlayer MP3 player"),
        keys: &["dfplayer", "mp3", "плеер"],
        boards: &[board("DFPLAYER", 2.0, 54.0)],
        wires: &[("TX", "D2"), ("RX", "D3"), ("BUSY", "D4")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// DFPlayer: программный UART, TX на D2, RX на D3
#include <SoftwareSerial.h>
#include <DFRobotDFPlayerMini.h>
const uint8_t TX = 2;
const uint8_t RX = 3;
SoftwareSerial serial(RX, TX);
DFRobotDFPlayerMini player;

void setup() {
  Serial.begin(9600);
  serial.begin(9600);
  if (!player.begin(serial)) {
    Serial.println(F("no DFPlayer"));
    return;
  }
  player.volume(20);
  player.play(1);
}

void loop() {
  if (player.available()) {
    Serial.println(player.readType());
  }
}
"#,
        ),
        summary: L::new(
            "DFPlayer получает команды по UART: TX модуля на D2, RX на D3.",
            "The DFPlayer takes commands over UART: its TX on D2, its RX on D3.",
        ),
        notes: &[L::new(
            "У модуля свой усилитель: динамик подключают к SPK1 и SPK2, не к плате.",
            "The module has its own amplifier; the speaker goes to SPK1 and SPK2, not to the board.",
        )],
        level: 3,
    },
    Lesson {
        book: 3,
        number: 8,
        title: L::new("Управление DTMF: MT8870", "DTMF control: the MT8870"),
        keys: &["mt8870", "dtmf", "тоновый набор"],
        boards: &[board("MT8870", 2.0, 54.0)],
        wires: &[
            ("Q1", "D2"),
            ("Q2", "D3"),
            ("Q3", "D4"),
            ("Q4", "D5"),
            ("STQ", "D6"),
        ],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// MT8870: код цифры на Q1..Q4, импульс на STQ
const uint8_t Q[4] = {2, 3, 4, 5};
const uint8_t STQ = 6;

void setup() {
  for (uint8_t i = 0; i < 4; i++) pinMode(Q[i], INPUT);
  pinMode(STQ, INPUT);
  Serial.begin(9600);
}

void loop() {
  if (digitalRead(STQ) == HIGH) {
    uint8_t code = 0;
    for (uint8_t i = 0; i < 4; i++) {
      if (digitalRead(Q[i])) code |= 1 << i;
    }
    Serial.println(code);
    while (digitalRead(STQ) == HIGH) { }
  }
}
"#,
        ),
        summary: L::new(
            "MT8870 превращает тон телефона в четыре бита и импульс готовности.",
            "The MT8870 turns a phone tone into four bits and a ready pulse.",
        ),
        notes: NO_NOTES,
        level: 3,
    },
    Lesson {
        book: 3,
        number: 9,
        title: L::new("Датчик магнитного поля", "A magnetic field sensor"),
        keys: &["датчик холла", "hall", "a3144", "магнитн"],
        boards: &[board("HALL", 2.0, 54.0)],
        wires: &[("DO", "D2")],
        extras: NO_EXTRA,
        code: Code::Digital { pin: "2", active_low: true },
        summary: L::new(
            "Датчик Холла A3144 притягивает выход к земле рядом с магнитом.",
            "An A3144 Hall sensor pulls its output low near a magnet.",
        ),
        notes: NO_NOTES,
        level: 1,
    },
    Lesson {
        book: 3,
        number: 10,
        title: L::new("Датчик радиации", "A radiation sensor"),
        keys: &["радиац", "дозиметр", "счетчик гейгера"],
        boards: &[board("RADIATION", 2.0, 54.0)],
        wires: &[("OUT", "D2")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Счётчик импульсов радиации на D2
const uint8_t PULSE = 2;
volatile uint32_t count = 0;
uint32_t last = 0;

void onPulse() {
  count++;
}

void setup() {
  pinMode(PULSE, INPUT_PULLUP);
  attachInterrupt(digitalPinToInterrupt(PULSE), onPulse, FALLING);
  Serial.begin(9600);
}

void loop() {
  if (millis() - last >= 1000) {
    last = millis();
    Serial.println(count);
  }
}
"#,
        ),
        summary: L::new(
            "Каждый импульс трубки считается прерыванием на D2.",
            "Every tube pulse is counted by an interrupt on D2.",
        ),
        notes: &[L::new(
            "Счётчик показывает импульсы, а не зиверты: коэффициент зависит от трубки.",
            "The counter reports pulses, not sieverts: the conversion factor belongs to the tube.",
        )],
        level: 3,
    },
    Lesson {
        book: 3,
        number: 11,
        title: L::new("Датчик тока ACS712", "An ACS712 current sensor"),
        keys: &["acs712", "датчик тока", "ток и напряж"],
        boards: &[board("ACS712", 2.0, 54.0)],
        wires: &[("OUT", "A0")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// ACS712: середина шкалы — ноль ампер
const uint8_t SENSE = A0;
const float ZERO = 512.0;
const float VOLTS_PER_AMP = 0.185;
const uint16_t PERIOD_MS = 250;
uint32_t last = 0;

void setup() {
  Serial.begin(9600);
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    float amps = (analogRead(SENSE) - ZERO) * 5.0 / 1023.0 / VOLTS_PER_AMP;
    Serial.println(amps, 2);
  }
}
"#,
        ),
        summary: L::new(
            "ACS712 выдаёт напряжение, пропорциональное току; ноль — половина питания.",
            "The ACS712 outputs a voltage proportional to current, centred at half the supply.",
        ),
        notes: &[L::new(
            "Модуль на 5 А имеет крутизну 185 мВ/А, на 20 А — 100 мВ/А, на 30 А — 66 мВ/А.",
            "The 5 A module is 185 mV/A, the 20 A one 100 mV/A and the 30 A one 66 mV/A.",
        )],
        level: 2,
    },
    Lesson {
        book: 3,
        number: 12,
        title: L::new("Энкодер", "A rotary encoder"),
        keys: &["энкодер", "encoder", "ky-040"],
        boards: &[board("ENCODER", 2.0, 54.0)],
        wires: &[("CLK", "D2"), ("DT", "D3"), ("SW", "D4")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Энкодер: CLK на D2, DT на D3, кнопка на D4
const uint8_t CLK = 2;
const uint8_t DT = 3;
const uint8_t SW = 4;
int32_t position = 0;
int lastClk = HIGH;

void setup() {
  pinMode(CLK, INPUT_PULLUP);
  pinMode(DT, INPUT_PULLUP);
  pinMode(SW, INPUT_PULLUP);
  Serial.begin(9600);
}

void loop() {
  int clk = digitalRead(CLK);
  if (clk != lastClk && clk == LOW) {
    position += digitalRead(DT) == HIGH ? 1 : -1;
    Serial.println(position);
  }
  lastClk = clk;
}
"#,
        ),
        summary: L::new(
            "Механический энкодер: два квадратурных сигнала и кнопка вала.",
            "A mechanical encoder: two quadrature signals and the shaft button.",
        ),
        notes: NO_NOTES,
        level: 2,
    },
    Lesson {
        book: 3,
        number: 13,
        title: L::new("Датчик линии", "A line sensor"),
        keys: &["датчик линии", "line sensor", "линию"],
        boards: &[board("TCRT5000", 2.0, 54.0)],
        wires: &[("DO", "D2"), ("AO", "A0")],
        extras: NO_EXTRA,
        code: Code::Digital { pin: "2", active_low: true },
        summary: L::new(
            "TCRT5000 над тёмной линией отдаёт низкий уровень на D2.",
            "A TCRT5000 over a dark line pulls D2 low.",
        ),
        notes: NO_NOTES,
        level: 1,
    },
    Lesson {
        book: 3,
        number: 14,
        title: L::new("Гироскоп и акселерометр MPU6050", "The MPU6050 gyro and accelerometer"),
        keys: &["mpu6050", "gy-521", "гироскоп", "акселерометр"],
        boards: &[board("MPU6050", 2.0, 54.0)],
        wires: &[("SDA", "A4"), ("SCL", "A5"), ("INT", "D2")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// MPU6050 на I2C: ускорение по трём осям
#include <Wire.h>
#include <Adafruit_MPU6050.h>
Adafruit_MPU6050 mpu;
const uint16_t PERIOD_MS = 200;
uint32_t last = 0;

void setup() {
  Serial.begin(9600);
  Wire.begin();
  if (!mpu.begin()) {
    Serial.println(F("no MPU6050"));
  }
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    sensors_event_t a, g, temp;
    mpu.getEvent(&a, &g, &temp);
    Serial.println(a.acceleration.x);
  }
}
"#,
        ),
        summary: L::new(
            "MPU6050 отдаёт ускорение и угловую скорость по I2C.",
            "The MPU6050 gives acceleration and angular rate over I2C.",
        ),
        notes: NO_NOTES,
        level: 3,
    },
    Lesson {
        book: 3,
        number: 15,
        title: L::new("Ёмкостная кнопка TTP223", "A TTP223 capacitive button"),
        keys: &["ttp223", "сенсорн кнопк", "касани"],
        boards: &[board("TTP223", 2.0, 54.0)],
        wires: &[("SIG", "D2")],
        extras: NO_EXTRA,
        code: Code::Digital { pin: "2", active_low: false },
        summary: L::new(
            "TTP223 замыкает SIG высоким уровнем при касании площадки.",
            "The TTP223 drives SIG high when its pad is touched.",
        ),
        notes: NO_NOTES,
        level: 1,
    },
    Lesson {
        book: 3,
        number: 16,
        title: L::new("Весы на HX711", "Scales on an HX711"),
        keys: &["hx711", "весы", "тензодатчик"],
        boards: &[board("HX711", 2.0, 54.0)],
        wires: &[("DT", "D2"), ("SCK", "D3")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// HX711: данные на D2, такт на D3
#include <HX711.h>
const uint8_t DT = 2;
const uint8_t SCK = 3;
HX711 scale;
const uint16_t PERIOD_MS = 500;
uint32_t last = 0;

void setup() {
  Serial.begin(9600);
  scale.begin(DT, SCK);
  scale.tare();
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    Serial.println(scale.get_units(1), 1);
  }
}
"#,
        ),
        summary: L::new(
            "HX711 — 24-битный АЦП для тензодатчика: две линии, DT и SCK.",
            "The HX711 is a 24-bit ADC for a load cell, read over DT and SCK.",
        ),
        notes: &[L::new(
            "Показания зависят от калибровки: scale.set_scale() подбирают по эталонной гире.",
            "Readings depend on calibration: set_scale() is found with a known weight.",
        )],
        level: 3,
    },
    Lesson {
        book: 3,
        number: 17,
        title: L::new("Модуль отпечатка пальцев", "A fingerprint module"),
        keys: &["отпечат", "fingerprint", "r307"],
        boards: &[board("FINGERPRINT", 2.0, 54.0)],
        wires: &[("TX", "D2"), ("RX", "D3")],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// Отпечаток пальцев: UART, TX модуля на D2, RX на D3
#include <SoftwareSerial.h>
#include <Adafruit_Fingerprint.h>
const uint8_t TX = 2;
const uint8_t RX = 3;
SoftwareSerial serial(RX, TX);
Adafruit_Fingerprint finger(&serial);

void setup() {
  Serial.begin(9600);
  serial.begin(57600);
  finger.begin(57600);
  if (finger.verifyPassword()) Serial.println(F("sensor ok"));
}

void loop() {
  uint8_t found = finger.getImage();
  if (found == FINGERPRINT_OK) {
    Serial.println(F("image"));
    finger.image2Tz();
  }
}
"#,
        ),
        summary: L::new(
            "Сканер отпечатка общается по UART на 57600 бод.",
            "The fingerprint scanner talks over UART at 57600 baud.",
        ),
        notes: &[L::new(
            "Линия RX модуля рассчитана на 3.3 В: с платой 5 В её соединяют через делитель.",
            "The module RX expects 3.3 V, so a 5 V board needs a divider.",
        )],
        level: 3,
    },
    Lesson {
        book: 3,
        number: 18,
        title: L::new("Элемент Пельтье", "A Peltier element"),
        keys: &["пельтье", "peltier", "охлажд"],
        boards: NO_BOARDS,
        wires: NO_WIRES,
        extras: &[Extra::Mosfet {
            gate: "D9",
            load: LoadKind::Peltier,
            value: "TEC1-12706",
        }],
        code: Code::Raw(
            r#"// Пельтье на D9 через MOSFET: полярность задаёт сторону холода
const uint8_t GATE = 9;
const uint16_t PERIOD_MS = 5000;
bool cold = false;
uint32_t last = 0;

void setup() {
  pinMode(GATE, OUTPUT);
  digitalWrite(GATE, LOW);
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    cold = !cold;
    digitalWrite(GATE, cold ? HIGH : LOW);
  }
}
"#,
        ),
        summary: L::new(
            "Элемент Пельтье на 5 В через MOSFET: ток задаёт сторону холода.",
            "A Peltier element on 5 V through a MOSFET: the current sets which side gets cold.",
        ),
        notes: &[L::new(
            "Пельтье берёт амперы: от USB его не питают, нужен отдельный блок и радиатор на горячей стороне.",
            "A Peltier draws amps: never power it from USB, use a separate supply and a heatsink on the hot side.",
        )],
        level: 3,
    },
    Lesson {
        book: 3,
        number: 19,
        title: L::new("Датчик этилового спирта MQ-3", "The MQ-3 alcohol sensor"),
        keys: &["mq-3", "mq3", "алкогол", "спирт"],
        boards: &[board("MQ-3", 2.0, 54.0)],
        wires: &[("AO", "A0"), ("DO", "D2")],
        extras: NO_EXTRA,
        code: Code::Analog { pin: "A0" },
        summary: L::new(
            "MQ-3: аналоговый выход на A0, порог компаратора на D2.",
            "The MQ-3: analogue output on A0, comparator threshold on D2.",
        ),
        notes: &[L::new(
            "MQ-3 чувствителен и к другим парам: показания калибруют по известной концентрации.",
            "The MQ-3 reacts to other vapours too, so readings are calibrated against a known concentration.",
        )],
        level: 2,
    },
    Lesson {
        book: 3,
        number: 20,
        title: L::new("Датчик цвета TCS3200", "The TCS3200 colour sensor"),
        keys: &["tcs3200", "tcs230", "датчик цвета"],
        boards: &[board("TCS3200", 2.0, 54.0)],
        wires: &[
            ("OUT", "D2"),
            ("S0", "D3"),
            ("S1", "D4"),
            ("S2", "D5"),
            ("S3", "D6"),
        ],
        extras: NO_EXTRA,
        code: Code::Raw(
            r#"// TCS3200: частота на OUT, фильтры выбираются S2/S3
const uint8_t OUT = 2;
const uint8_t S0 = 3, S1 = 4, S2 = 5, S3 = 6;
const uint16_t PERIOD_MS = 500;
uint32_t last = 0;

uint16_t readChannel(bool s2, bool s3) {
  digitalWrite(S2, s2);
  digitalWrite(S3, s3);
  return pulseIn(OUT, HIGH, 20000UL);
}

void setup() {
  pinMode(OUT, INPUT);
  pinMode(S0, OUTPUT);
  pinMode(S1, OUTPUT);
  pinMode(S2, OUTPUT);
  pinMode(S3, OUTPUT);
  digitalWrite(S0, HIGH);
  digitalWrite(S1, LOW);
  Serial.begin(9600);
}

void loop() {
  if (millis() - last >= PERIOD_MS) {
    last = millis();
    Serial.print(readChannel(LOW, LOW));
    Serial.print(' ');
    Serial.print(readChannel(HIGH, HIGH));
    Serial.print(' ');
    Serial.println(readChannel(LOW, HIGH));
  }
}
"#,
        ),
        summary: L::new(
            "TCS3200 выдаёт частоту, пропорциональную выбранному цвету.",
            "The TCS3200 outputs a frequency proportional to the selected colour.",
        ),
        notes: &[L::new(
            "Красный, зелёный и синий фильтры выбирают уровнями S2 и S3.",
            "The red, green and blue filters are selected by S2 and S3.",
        )],
        level: 3,
    },
];

/// Everyday words that name a quantity rather than a part: a user asking for
/// a thermometer does not have to know the DS18B20 by name. These are only
/// consulted when the sentence names no part and no load at all, so
/// "датчик температуры DHT22" still goes to the DHT lesson and "шуметь
/// динамиком" is not read as a sound sensor.
const ALIASES: &[(&str, usize)] = &[
    ("термометр", THERMISTOR),
    ("температур", THERMISTOR),
    ("термодатчик", THERMISTOR),
    ("градус", THERMISTOR),
    ("temp", THERMISTOR),
    ("освещ", PHOTORESISTOR),
    ("люкс", PHOTORESISTOR),
    ("датчик света", PHOTORESISTOR),
    ("расстояни", RANGE),
    ("дальномер", RANGE),
    ("дальност", RANGE),
    ("уровень шума", SOUND),
    ("громкост", SOUND),
    ("барометр", 12),
    ("дым", 13),
    ("утечк", 13),
    ("присутств", 14),
    ("пульт", 17),
    ("часы", 31),
    ("кольцо", 32),
    ("клавиатур", 34),
    ("карта памяти", 35),
    ("плеер", 37),
    ("наклон", 44),
    ("акселерометр", 44),
    ("гироскоп", 44),
    ("отпечат", 47),
    ("спирт", 49),
    ("емкостн", 45),
    ("люксметр", PHOTORESISTOR),
    ("распознать цвет", 50),
    ("определить цвет", 50),
    ("temperature", THERMISTOR),
    ("thermometer", THERMISTOR),
    ("humidity", 11),
    ("pressure", 12),
    ("distance", RANGE),
    ("brightness", PHOTORESISTOR),
    ("light sensor", PHOTORESISTOR),
    ("noise", SOUND),
    ("sound level", SOUND),
    ("weight", 46),
    ("scales", 46),
    ("fingerprint", 47),
    ("clock", 31),
    ("smoke", 13),
    ("gas", 13),
    ("color sensor", 50),
    ("colour sensor", 50),
];

/// Indices of lessons that everyday words may point at.
pub const PHOTORESISTOR: usize = 9;
/// The PWM fade lesson in [`ALL`].
pub const PWM: usize = 1;
/// The MQ-2 gas sensor lesson.
pub const GAS: usize = 13;

/// The thermistor/DS18B20 lesson in [`ALL`].
pub const THERMISTOR: usize = 10;
/// The HC-SR04 ranger lesson.
pub const RANGE: usize = 15;
/// The digital sound sensor lesson.
pub const SOUND: usize = 36;

/// Pick the lesson a sentence is about. The most keyword hits wins; ties go
/// to the longer keyword, which keeps a specific board ahead of a generic
/// sensor word. Only when nothing matches at all do the everyday aliases
/// get a vote.
#[cfg(test)]
pub fn find(text: &str) -> Option<usize> {
    find_with(text, true)
}

/// The same search, with the everyday words allowed or not. The parser only
/// enables them for a sentence that names no part, so a known load always
/// wins over an alias.
pub fn find_with(text: &str, aliases: bool) -> Option<usize> {
    let text = crate::words::normalize(text);
    let exact = find_pass(&text, false);
    // A first pass may have matched only an action word ("мигает" in the
    // blink lesson). Before settling for that, let the everyday names vote:
    // "датчик света, который мигает" is about the photoresistor, and a
    // generic verb must not hide the part the sentence actually names.
    if exact.map(|(score, _)| score.0 > 0).unwrap_or(false) {
        return exact.map(|(_, i)| i);
    }
    if aliases {
        if let Some((score, i)) = find_pass(&text, true) {
            if score.0 > 0 || exact.is_none() {
                return Some(i);
            }
        }
    }
    exact.map(|(_, i)| i)
}

/// Keys that name an action rather than a part. A lesson matched only by one
/// of these must not beat a lesson whose own sensor was named: "датчик газа,
/// который мигает" is about the gas sensor, not about blinking in general.
const GENERIC_KEYS: &[&str] = &["мигает", "плавн", "свечен"];

fn find_pass(text: &str, aliases: bool) -> Option<((usize, usize, usize), usize)> {
    // Score: a part name first, then the number of hits, then the longest
    // word. Without the first component an action word ("мигает" in the LED
    // lesson) would outrank "газ" from the gas lesson on length alone.
    let mut best: Option<((usize, usize, usize), usize)> = None;
    for (i, lesson) in ALL.iter().enumerate() {
        let mut specific = 0;
        let mut hits = 0;
        let mut longest = 0;
        for key in lesson.keys {
            if crate::words::mentions(text, key) {
                hits += 1;
                longest = longest.max(key.len());
                if !GENERIC_KEYS.contains(key) {
                    specific += 1;
                }
            }
        }
        if aliases {
            for (word, target) in ALIASES {
                if *target == i && crate::words::mentions(text, word) {
                    hits += 1;
                    specific += 1;
                }
            }
        }
        if hits == 0 {
            continue;
        }
        if best
            .map(|(score, _)| (specific, hits, longest) > score)
            .unwrap_or(true)
        {
            best = Some(((specific, hits, longest), i));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lesson_has_a_title_wires_and_code() {
        for (i, l) in ALL.iter().enumerate() {
            assert!(!l.title.ru.is_empty() && !l.title.en.is_empty(), "{i}");
            assert!(!l.summary.ru.is_empty() && !l.summary.en.is_empty(), "{i}");
            assert!((1..=4).contains(&l.book), "{i}");
            assert!(!l.keys.is_empty(), "{i} has no keywords");
            for b in l.boards {
                assert!(crate::modules::find(b.name).is_some(), "{} is unknown", b.name);
            }
            let code = l.code.render(l.title.ru);
            assert!(code.contains("void setup()"), "{i} has no setup");
            assert!(code.contains("void loop()"), "{i} has no loop");
        }
    }

    /// Every distinctive key is a part number or a board name (`dht22`,
    /// `16*2`, `ps/2`). A user typing it must land on that lesson, never on a
    /// neighbour.
    #[test]
    fn a_board_name_routes_to_its_lesson() {
        for (i, lesson) in ALL.iter().enumerate() {
            for key in lesson.keys {
                if key.chars().any(|c| c.is_ascii_alphanumeric()) {
                    assert_eq!(find(key), Some(i), "lesson {i} key {key}");
                }
            }
        }
    }

    #[test]
    fn a_sentence_finds_its_lesson() {
        for (text, book) in [
            ("подключить dht11 к ардуино", 1),
            ("геркон и светодиод", 1),
            ("часы реального времени ds3231", 3),
            ("датчик цвета tcs3200", 3),
            ("матрица max7219", 2),
        ] {
            let i = find(text).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(ALL[i].book, book, "{text}");
        }
    }

    /// A user asking for a thermometer should not have to name the chip, but
    /// a sentence that names a different sensor must keep that sensor.
    #[test]
    fn everyday_words_route_only_when_no_part_is_named() {
        for text in ["термометр", "измерить температуру", "датчик температуры", "градусы"] {
            assert_eq!(find(text), Some(THERMISTOR), "{text}");
        }
        assert_eq!(find("датчик температуры dht22"), Some(THERMISTOR + 1));
        assert_eq!(find("термометр на ds18b20"), Some(THERMISTOR));
        assert_eq!(find("погода"), None);
        let dht = find("датчик температуры dht22").unwrap();
        assert!(ALL[dht].title.ru.contains("DHT"), "{}", ALL[dht].title.ru);
    }
}
