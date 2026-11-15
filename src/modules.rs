//! The ready-made boards from the course books.
//!
//! Parts one to four of the ATmega328 manual are built around modules: sensors,
//! displays, radios, motor drivers and memory boards. They are all the same
//! kind of part to the circuit model — a rectangle with a header — so one
//! `Kind::Module` covers them, and this table supplies the size, the pin
//! header, the colour and what it draws.

use crate::text::L;

/// One header pin: name and position in millimetres from the module's
/// top-left corner. A tuple keeps the table terse and lets the painters reuse
/// the same slice as `templates::bench_pins`.
pub type Pin = (&'static str, f32, f32);

const fn p(name: &'static str, x: f32, y: f32) -> Pin {
    (name, x, y)
}

/// What the module looks like on the bench.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// A green/blue development board with a chip and a header.
    Board,
    /// A motor driver: big PCB, screw terminals, a heatsink.
    Driver,
    /// A round stepper motor.
    Stepper,
    /// An RFID reader: a PCB with a big coil.
    Rfid,
    /// A radio board: PCB and a thin antenna.
    Radio,
    /// A character display.
    Lcd,
    /// A graphic display: a glass panel on a small PCB.
    Display,
    /// One or four seven-segment digits.
    Segments,
    /// A dot matrix under a MAX7219.
    Matrix,
    /// A P10 LED panel with a HUB header.
    Panel,
    /// A WS2812 ring.
    Ring,
    /// A small sensor board with a dome, a grille or a trimmer.
    Sensor,
    /// A classic PS/2 socket on a breakout.
    Keypad,
}

pub struct Module {
    pub name: &'static str,
    pub w: f32,
    pub h: f32,
    pub colour: (u8, u8, u8),
    pub look: Look,
    pub pins: &'static [Pin],
    /// Typical current from the 5 V rail, in milliamps.
    pub current_ma: u16,
    /// The module wants the 3.3 V rail, not 5 V.
    pub three_three: bool,
}

impl Module {
    pub fn pin(&self, name: &str) -> Option<(f32, f32)> {
        self.pins.iter().find(|p| p.0 == name).map(|p| (p.1, p.2))
    }
}

// ---------------------------------------------------------------- the table

const L298N_PINS: &[Pin] = &[
    // The supply arrives on the board's own screw terminals, so these pads
    // sit on the screws rather than on a header outside the outline.
    p("+12V", 6.6, 9.0),
    p("GND", 6.6, 20.0),
    p("+5V", 6.6, 31.0),
    p("OUT1", 47.0, 8.0),
    p("OUT2", 47.0, 14.0),
    p("OUT3", 47.0, 30.0),
    p("OUT4", 47.0, 36.0),
    p("ENA", 5.0, 47.0),
    p("IN1", 10.0, 47.0),
    p("IN2", 15.0, 47.0),
    p("IN3", 20.0, 47.0),
    p("IN4", 25.0, 47.0),
    p("ENB", 30.0, 47.0),
];

const L293D_PINS: &[Pin] = &[
    p("OUT1", 6.0, -3.0),
    p("OUT2", 10.0, -3.0),
    p("OUT3", 17.0, -3.0),
    p("OUT4", 21.0, -3.0),
    p("EN1", 3.0, 27.0),
    p("IN1", 6.5, 27.0),
    p("IN2", 10.0, 27.0),
    p("EN2", 13.5, 27.0),
    p("IN3", 17.0, 27.0),
    p("IN4", 20.5, 27.0),
    p("VCC1", 24.0, 27.0),
    p("VCC2", 27.5, 27.0),
    p("GND", 13.5, -3.0),
];

const ULN2003_PINS: &[Pin] = &[
    p("IN1", 4.0, 33.0),
    p("IN2", 8.0, 33.0),
    p("IN3", 12.0, 33.0),
    p("IN4", 16.0, 33.0),
    p("GND", 22.0, 33.0),
    p("+5V", 28.0, 33.0),
    p("OUT1", 4.0, -3.0),
    p("OUT2", 8.0, -3.0),
    p("OUT3", 12.0, -3.0),
    p("OUT4", 16.0, -3.0),
    p("COM", 22.0, -3.0),
];

const STEPPER_PINS: &[Pin] = &[
    p("1", 5.0, 33.0),
    p("2", 10.0, 33.0),
    p("3", 15.0, 33.0),
    p("4", 20.0, 33.0),
    p("+", 25.0, 33.0),
];

const RC522_PINS: &[Pin] = &[
    p("SDA", 5.0, 63.0),
    p("SCK", 10.0, 63.0),
    p("MOSI", 15.0, 63.0),
    p("MISO", 20.0, 63.0),
    p("IRQ", 25.0, 63.0),
    p("GND", 30.0, 63.0),
    p("RST", 35.0, 63.0),
    p("3V3", 40.0, 63.0),
];

const TX433_PINS: &[Pin] = &[
    p("DATA", 5.0, 33.0),
    p("VCC", 11.0, 33.0),
    p("GND", 17.0, 33.0),
];

const RX433_PINS: &[Pin] = &[
    p("VCC", 5.0, 25.0),
    p("DATA", 12.0, 25.0),
    p("GND", 19.0, 25.0),
    p("DATA2", 26.0, 25.0),
];

const NRF24_PINS: &[Pin] = &[
    p("GND", 4.0, 19.0),
    p("VCC", 8.0, 19.0),
    p("CE", 12.0, 19.0),
    p("CSN", 16.0, 19.0),
    p("SCK", 20.0, 19.0),
    p("MOSI", 24.0, 19.0),
    p("MISO", 28.0, 19.0),
];

const BT_PINS: &[Pin] = &[
    p("VCC", 6.0, 29.0),
    p("GND", 12.0, 29.0),
    p("TXD", 18.0, 29.0),
    p("RXD", 24.0, 29.0),
];

const GSM_PINS: &[Pin] = &[
    p("VCC", 6.0, 37.0),
    p("GND", 12.0, 37.0),
    p("TXD", 18.0, 37.0),
    p("RXD", 24.0, 37.0),
    p("PWR", 30.0, 37.0),
];

const ENC28_PINS: &[Pin] = &[
    p("VCC", 5.0, 33.0),
    p("GND", 10.0, 33.0),
    p("CS", 15.0, 33.0),
    p("SCK", 20.0, 33.0),
    p("MOSI", 25.0, 33.0),
    p("MISO", 30.0, 33.0),
    p("INT", 35.0, 33.0),
];

const W5100_PINS: &[Pin] = &[
    p("VCC", 5.0, 39.0),
    p("GND", 10.0, 39.0),
    p("CS", 15.0, 39.0),
    p("SCK", 20.0, 39.0),
    p("MOSI", 25.0, 39.0),
    p("MISO", 30.0, 39.0),
];

const LCD_PINS: &[Pin] = &[
    p("GND", 5.0, 29.0),
    p("VCC", 11.0, 29.0),
    p("SDA", 17.0, 29.0),
    p("SCL", 23.0, 29.0),
];

const LCD2004_PINS: &[Pin] = &[
    p("GND", 10.0, 39.0),
    p("VCC", 16.0, 39.0),
    p("SDA", 22.0, 39.0),
    p("SCL", 28.0, 39.0),
];

const DS18B20_PINS: &[Pin] = &[
    p("GND", 4.0, 33.0),
    p("DATA", 7.0, 33.0),
    p("VCC", 10.0, 33.0),
];

const DHT11_PINS: &[Pin] = &[
    p("VCC", 5.0, 33.0),
    p("DATA", 12.0, 33.0),
    p("GND", 19.0, 33.0),
];

const DHT22_PINS: &[Pin] = &[
    p("VCC", 5.0, 35.0),
    p("DATA", 12.0, 35.0),
    p("GND", 19.0, 35.0),
];

const BMP_PINS: &[Pin] = &[
    p("VCC", 4.0, 17.0),
    p("GND", 7.0, 17.0),
    p("SDA", 10.0, 17.0),
    p("SCL", 13.0, 17.0),
];

const BME_PINS: &[Pin] = &[
    p("VCC", 5.0, 19.0),
    p("GND", 8.0, 19.0),
    p("SDA", 11.0, 19.0),
    p("SCL", 14.0, 19.0),
];

const MQ2_PINS: &[Pin] = &[
    p("VCC", 5.0, 35.0),
    p("GND", 10.0, 35.0),
    p("DO", 17.0, 35.0),
    p("AO", 24.0, 35.0),
];

const MQ3_PINS: &[Pin] = &[
    p("VCC", 5.0, 35.0),
    p("GND", 10.0, 35.0),
    p("DO", 17.0, 35.0),
    p("AO", 24.0, 35.0),
];

const PIR_PINS: &[Pin] = &[
    p("VCC", 5.0, 27.0),
    p("OUT", 14.0, 27.0),
    p("GND", 23.0, 27.0),
];

const SR04_PINS: &[Pin] = &[
    p("VCC", 6.0, 25.0),
    p("TRIG", 14.0, 25.0),
    p("ECHO", 22.0, 25.0),
    p("GND", 30.0, 25.0),
];

const TCRT_PINS: &[Pin] = &[
    p("VCC", 4.0, 15.0),
    p("GND", 7.0, 15.0),
    p("DO", 10.0, 15.0),
    p("AO", 13.0, 15.0),
];

const TSOP_PINS: &[Pin] = &[
    p("OUT", 5.0, 19.0),
    p("GND", 9.0, 19.0),
    p("VCC", 13.0, 19.0),
];

const SEG1_PINS: &[Pin] = &[
    p("A", 3.0, 35.0),
    p("B", 5.5, 35.0),
    p("C", 8.0, 35.0),
    p("D", 10.5, 35.0),
    p("E", 13.0, 35.0),
    p("F", 15.5, 35.0),
    p("G", 18.0, 35.0),
    p("DP", 20.5, 35.0),
    p("COM", 23.0, 35.0),
];

const SEG4_PINS: &[Pin] = &[
    p("VCC", 6.0, 27.0),
    p("GND", 12.0, 27.0),
    p("SDA", 18.0, 27.0),
    p("SCL", 24.0, 27.0),
];

const MAX7219_PINS: &[Pin] = &[
    p("VCC", 5.0, 37.0),
    p("GND", 10.0, 37.0),
    p("DIN", 15.0, 37.0),
    p("CS", 20.0, 37.0),
    p("CLK", 25.0, 37.0),
];

const NOKIA_PINS: &[Pin] = &[
    p("VCC", 5.0, 47.0),
    p("GND", 10.0, 47.0),
    p("SCE", 15.0, 47.0),
    p("RST", 20.0, 47.0),
    p("DC", 25.0, 47.0),
    p("DIN", 30.0, 47.0),
    p("CLK", 35.0, 47.0),
];

const OLED_PINS: &[Pin] = &[
    p("VCC", 5.0, 31.0),
    p("GND", 10.0, 31.0),
    p("SDA", 15.0, 31.0),
    p("SCL", 20.0, 31.0),
];

const TFT144_PINS: &[Pin] = &[
    p("VCC", 5.0, 45.0),
    p("GND", 10.0, 45.0),
    p("CS", 15.0, 45.0),
    p("RST", 20.0, 45.0),
    p("DC", 25.0, 45.0),
    p("MOSI", 30.0, 45.0),
    p("SCK", 35.0, 45.0),
];

const TFT24_PINS: &[Pin] = &[
    p("VCC", 5.0, 51.0),
    p("GND", 10.0, 51.0),
    p("CS", 15.0, 51.0),
    p("RST", 20.0, 51.0),
    p("DC", 25.0, 51.0),
    p("MOSI", 30.0, 51.0),
    p("SCK", 35.0, 51.0),
    p("T_CS", 40.0, 51.0),
    p("T_IRQ", 45.0, 51.0),
];

const TVOUT_PINS: &[Pin] = &[
    p("VCC", 5.0, 25.0),
    p("GND", 10.0, 25.0),
    p("VIDEO", 15.0, 25.0),
];

const P10_PINS: &[Pin] = &[
    p("VCC", 5.0, 35.0),
    p("GND", 10.0, 35.0),
    p("A", 15.0, 35.0),
    p("B", 20.0, 35.0),
    p("CLK", 25.0, 35.0),
    p("LAT", 30.0, 35.0),
    p("OE", 35.0, 35.0),
    p("DATA", 40.0, 35.0),
];

const P10RGB_PINS: &[Pin] = &[
    p("VCC", 5.0, 35.0),
    p("GND", 10.0, 35.0),
    p("R", 15.0, 35.0),
    p("G", 20.0, 35.0),
    p("B", 25.0, 35.0),
    p("CLK", 30.0, 35.0),
    p("LAT", 35.0, 35.0),
    p("OE", 40.0, 35.0),
    p("A", 45.0, 35.0),
    p("B", 50.0, 35.0),
];

const DS3231_PINS: &[Pin] = &[
    p("VCC", 5.0, 27.0),
    p("GND", 10.0, 27.0),
    p("SDA", 15.0, 27.0),
    p("SCL", 20.0, 27.0),
];

const WS2812_PINS: &[Pin] = &[
    p("VCC", 5.0, 37.0),
    p("GND", 10.0, 37.0),
    p("DIN", 15.0, 37.0),
];

const PS2_PINS: &[Pin] = &[
    p("VCC", 5.0, 23.0),
    p("GND", 10.0, 23.0),
    p("CLK", 15.0, 23.0),
    p("DATA", 20.0, 23.0),
];

const SD_PINS: &[Pin] = &[
    p("VCC", 5.0, 31.0),
    p("GND", 10.0, 31.0),
    p("CS", 15.0, 31.0),
    p("SCK", 20.0, 31.0),
    p("MOSI", 25.0, 31.0),
    p("MISO", 30.0, 31.0),
];

const SOUND_PINS: &[Pin] = &[
    p("VCC", 4.0, 23.0),
    p("GND", 8.0, 23.0),
    p("DO", 12.0, 23.0),
    p("AO", 16.0, 23.0),
];

const DFPLAYER_PINS: &[Pin] = &[
    p("VCC", 5.0, 31.0),
    p("GND", 10.0, 31.0),
    p("TX", 15.0, 31.0),
    p("RX", 20.0, 31.0),
    p("BUSY", 25.0, 31.0),
];

const MT8870_PINS: &[Pin] = &[
    p("VCC", 5.0, 27.0),
    p("GND", 10.0, 27.0),
    p("Q1", 15.0, 27.0),
    p("Q2", 20.0, 27.0),
    p("Q3", 25.0, 27.0),
    p("Q4", 30.0, 27.0),
    p("STQ", 35.0, 27.0),
];

const HALL_PINS: &[Pin] = &[
    p("VCC", 4.0, 17.0),
    p("GND", 8.0, 17.0),
    p("DO", 12.0, 17.0),
];

const RADIATION_PINS: &[Pin] = &[
    p("VCC", 5.0, 27.0),
    p("GND", 10.0, 27.0),
    p("OUT", 20.0, 27.0),
];

const ACS712_PINS: &[Pin] = &[
    p("VCC", 5.0, 25.0),
    p("GND", 10.0, 25.0),
    p("OUT", 15.0, 25.0),
];

const ENCODER_PINS: &[Pin] = &[
    p("VCC", 5.0, 27.0),
    p("GND", 10.0, 27.0),
    p("CLK", 15.0, 27.0),
    p("DT", 20.0, 27.0),
    p("SW", 25.0, 27.0),
];

const MPU6050_PINS: &[Pin] = &[
    p("VCC", 5.0, 21.0),
    p("GND", 9.0, 21.0),
    p("SDA", 13.0, 21.0),
    p("SCL", 17.0, 21.0),
    p("INT", 21.0, 21.0),
];

const TTP223_PINS: &[Pin] = &[
    p("VCC", 4.0, 17.0),
    p("GND", 8.0, 17.0),
    p("SIG", 12.0, 17.0),
];

const HX711_PINS: &[Pin] = &[
    p("VCC", 5.0, 23.0),
    p("GND", 10.0, 23.0),
    p("DT", 15.0, 23.0),
    p("SCK", 20.0, 23.0),
];

const FINGER_PINS: &[Pin] = &[
    p("VCC", 5.0, 33.0),
    p("GND", 10.0, 33.0),
    p("TX", 15.0, 33.0),
    p("RX", 20.0, 33.0),
];

const TCS3200_PINS: &[Pin] = &[
    p("VCC", 5.0, 27.0),
    p("GND", 10.0, 27.0),
    p("S0", 14.0, 27.0),
    p("S1", 17.0, 27.0),
    p("S2", 20.0, 27.0),
    p("S3", 23.0, 27.0),
    p("OUT", 26.0, 27.0),
];

/// Every module the palette offers, in the order it lists them.
pub const ALL: &[Module] = &[
    // ------------------------------------------------------------ part four
    Module {
        name: "L298N",
        w: 44.0,
        h: 44.0,
        colour: (0x9E, 0x2B, 0x22),
        look: Look::Driver,
        pins: L298N_PINS,
        current_ma: 20,
        three_three: false,
    },
    Module {
        name: "L293D",
        w: 31.0,
        h: 24.0,
        colour: (0x22, 0x2A, 0x30),
        look: Look::Driver,
        pins: L293D_PINS,
        current_ma: 20,
        three_three: false,
    },
    Module {
        name: "ULN2003",
        w: 38.0,
        h: 30.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Board,
        pins: ULN2003_PINS,
        current_ma: 20,
        three_three: false,
    },
    Module {
        name: "28BYJ-48",
        w: 30.0,
        h: 30.0,
        colour: (0xB9, 0xBC, 0xC0),
        look: Look::Stepper,
        pins: STEPPER_PINS,
        current_ma: 240,
        three_three: false,
    },
    Module {
        name: "RC522",
        w: 46.0,
        h: 60.0,
        colour: (0x1E, 0x6B, 0x3A),
        look: Look::Rfid,
        pins: RC522_PINS,
        current_ma: 30,
        three_three: true,
    },
    Module {
        name: "FS1000A",
        w: 22.0,
        h: 30.0,
        colour: (0x1B, 0x5E, 0x36),
        look: Look::Radio,
        pins: TX433_PINS,
        current_ma: 30,
        three_three: false,
    },
    Module {
        name: "XY-MK-5V",
        w: 32.0,
        h: 22.0,
        colour: (0x1B, 0x5E, 0x36),
        look: Look::Radio,
        pins: RX433_PINS,
        current_ma: 5,
        three_three: false,
    },
    Module {
        name: "NRF24L01",
        w: 32.0,
        h: 16.0,
        colour: (0x16, 0x3F, 0x7A),
        look: Look::Radio,
        pins: NRF24_PINS,
        current_ma: 15,
        three_three: true,
    },
    Module {
        name: "HC-05",
        w: 36.0,
        h: 26.0,
        colour: (0x1B, 0x4F, 0x8A),
        look: Look::Radio,
        pins: BT_PINS,
        current_ma: 40,
        three_three: false,
    },
    Module {
        name: "HC-06",
        w: 36.0,
        h: 26.0,
        colour: (0x2A, 0x5E, 0x3A),
        look: Look::Radio,
        pins: BT_PINS,
        current_ma: 40,
        three_three: false,
    },
    Module {
        name: "NEOWAY M590",
        w: 50.0,
        h: 34.0,
        colour: (0x2A, 0x35, 0x45),
        look: Look::Board,
        pins: GSM_PINS,
        current_ma: 500,
        three_three: false,
    },
    Module {
        name: "ENC28J60",
        w: 44.0,
        h: 30.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Board,
        pins: ENC28_PINS,
        current_ma: 180,
        three_three: false,
    },
    Module {
        name: "W5100",
        w: 50.0,
        h: 36.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Board,
        pins: W5100_PINS,
        current_ma: 200,
        three_three: false,
    },
    Module {
        name: "LCD1602",
        w: 60.0,
        h: 26.0,
        colour: (0x22, 0x5E, 0x3A),
        look: Look::Lcd,
        pins: LCD_PINS,
        current_ma: 25,
        three_three: false,
    },
    // ------------------------------------------------------------- part one
    Module {
        name: "DS18B20",
        w: 14.0,
        h: 30.0,
        colour: (0x2A, 0x2C, 0x30),
        look: Look::Sensor,
        pins: DS18B20_PINS,
        current_ma: 2,
        three_three: false,
    },
    Module {
        name: "DHT11",
        w: 24.0,
        h: 30.0,
        colour: (0x24, 0x6E, 0x9A),
        look: Look::Sensor,
        pins: DHT11_PINS,
        current_ma: 1,
        three_three: false,
    },
    Module {
        name: "DHT22",
        w: 24.0,
        h: 32.0,
        colour: (0xE8, 0xE8, 0xE2),
        look: Look::Sensor,
        pins: DHT22_PINS,
        current_ma: 2,
        three_three: false,
    },
    Module {
        name: "BMP180",
        w: 18.0,
        h: 14.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: BMP_PINS,
        current_ma: 2,
        three_three: true,
    },
    Module {
        name: "BMP280",
        w: 18.0,
        h: 14.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: BMP_PINS,
        current_ma: 2,
        three_three: true,
    },
    Module {
        name: "BME280",
        w: 20.0,
        h: 16.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: BME_PINS,
        current_ma: 2,
        three_three: true,
    },
    Module {
        name: "MQ-2",
        w: 30.0,
        h: 32.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: MQ2_PINS,
        current_ma: 150,
        three_three: false,
    },
    Module {
        name: "MQ-3",
        w: 30.0,
        h: 32.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: MQ3_PINS,
        current_ma: 150,
        three_three: false,
    },
    Module {
        name: "HC-SR501",
        w: 32.0,
        h: 24.0,
        colour: (0x1E, 0x6B, 0x3A),
        look: Look::Sensor,
        pins: PIR_PINS,
        current_ma: 50,
        three_three: false,
    },
    Module {
        name: "HC-SR04",
        w: 45.0,
        h: 22.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: SR04_PINS,
        current_ma: 15,
        three_three: false,
    },
    Module {
        name: "TCRT5000",
        w: 18.0,
        h: 12.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: TCRT_PINS,
        current_ma: 20,
        three_three: false,
    },
    Module {
        name: "TSOP1838",
        w: 18.0,
        h: 16.0,
        colour: (0x1A, 0x1C, 0x20),
        look: Look::Sensor,
        pins: TSOP_PINS,
        current_ma: 5,
        three_three: false,
    },
    // ------------------------------------------------------------- part two
    Module {
        name: "LCD2004",
        w: 98.0,
        h: 36.0,
        colour: (0x22, 0x5E, 0x3A),
        look: Look::Lcd,
        pins: LCD2004_PINS,
        current_ma: 30,
        three_three: false,
    },
    Module {
        name: "7SEG-1",
        w: 26.0,
        h: 32.0,
        colour: (0x1B, 0x1D, 0x22),
        look: Look::Segments,
        pins: SEG1_PINS,
        current_ma: 60,
        three_three: false,
    },
    Module {
        name: "7SEG-4",
        w: 42.0,
        h: 24.0,
        colour: (0x1B, 0x1D, 0x22),
        look: Look::Segments,
        pins: SEG4_PINS,
        current_ma: 80,
        three_three: false,
    },
    Module {
        name: "MAX7219",
        w: 34.0,
        h: 34.0,
        colour: (0x1B, 0x1D, 0x22),
        look: Look::Matrix,
        pins: MAX7219_PINS,
        current_ma: 100,
        three_three: false,
    },
    Module {
        name: "NOKIA 5110",
        w: 44.0,
        h: 44.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Display,
        pins: NOKIA_PINS,
        current_ma: 10,
        three_three: false,
    },
    Module {
        name: "OLED 0.96",
        w: 28.0,
        h: 28.0,
        colour: (0x1B, 0x1D, 0x22),
        look: Look::Display,
        pins: OLED_PINS,
        current_ma: 15,
        three_three: false,
    },
    Module {
        name: "TFT 1.44",
        w: 40.0,
        h: 42.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Display,
        pins: TFT144_PINS,
        current_ma: 30,
        three_three: false,
    },
    Module {
        name: "TFT 2.4",
        w: 64.0,
        h: 48.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Display,
        pins: TFT24_PINS,
        current_ma: 60,
        three_three: false,
    },
    Module {
        name: "TV OUT",
        w: 30.0,
        h: 22.0,
        colour: (0x1B, 0x5E, 0x36),
        look: Look::Display,
        pins: TVOUT_PINS,
        current_ma: 25,
        three_three: false,
    },
    Module {
        name: "P10",
        w: 64.0,
        h: 32.0,
        colour: (0x1B, 0x1D, 0x22),
        look: Look::Panel,
        pins: P10_PINS,
        current_ma: 500,
        three_three: false,
    },
    Module {
        name: "P10 RGB",
        w: 64.0,
        h: 32.0,
        colour: (0x1B, 0x1D, 0x22),
        look: Look::Panel,
        pins: P10RGB_PINS,
        current_ma: 900,
        three_three: false,
    },
    // ----------------------------------------------------------- part three
    Module {
        name: "DS3231",
        w: 28.0,
        h: 24.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: DS3231_PINS,
        current_ma: 5,
        three_three: false,
    },
    Module {
        name: "WS2812",
        w: 34.0,
        h: 34.0,
        colour: (0x1B, 0x1D, 0x22),
        look: Look::Ring,
        pins: WS2812_PINS,
        current_ma: 300,
        three_three: false,
    },
    Module {
        name: "PS/2",
        w: 24.0,
        h: 20.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Keypad,
        pins: PS2_PINS,
        current_ma: 30,
        three_three: false,
    },
    Module {
        name: "SD",
        w: 34.0,
        h: 28.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Board,
        pins: SD_PINS,
        current_ma: 30,
        three_three: false,
    },
    Module {
        name: "SOUND",
        w: 20.0,
        h: 20.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: SOUND_PINS,
        current_ma: 5,
        three_three: false,
    },
    Module {
        name: "DFPLAYER",
        w: 34.0,
        h: 28.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Board,
        pins: DFPLAYER_PINS,
        current_ma: 30,
        three_three: false,
    },
    Module {
        name: "MT8870",
        w: 34.0,
        h: 24.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Board,
        pins: MT8870_PINS,
        current_ma: 10,
        three_three: false,
    },
    Module {
        name: "HALL",
        w: 16.0,
        h: 14.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: HALL_PINS,
        current_ma: 5,
        three_three: false,
    },
    Module {
        name: "RADIATION",
        w: 28.0,
        h: 24.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: RADIATION_PINS,
        current_ma: 10,
        three_three: false,
    },
    Module {
        name: "ACS712",
        w: 26.0,
        h: 22.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: ACS712_PINS,
        current_ma: 10,
        three_three: false,
    },
    Module {
        name: "ENCODER",
        w: 26.0,
        h: 24.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: ENCODER_PINS,
        current_ma: 10,
        three_three: false,
    },
    Module {
        name: "MPU6050",
        w: 24.0,
        h: 18.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: MPU6050_PINS,
        current_ma: 5,
        three_three: false,
    },
    Module {
        name: "TTP223",
        w: 16.0,
        h: 14.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: TTP223_PINS,
        current_ma: 2,
        three_three: false,
    },
    Module {
        name: "HX711",
        w: 30.0,
        h: 20.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Board,
        pins: HX711_PINS,
        current_ma: 15,
        three_three: false,
    },
    Module {
        name: "FINGERPRINT",
        w: 34.0,
        h: 30.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: FINGER_PINS,
        current_ma: 60,
        three_three: false,
    },
    Module {
        name: "TCS3200",
        w: 28.0,
        h: 24.0,
        colour: (0x1D, 0x4F, 0xA8),
        look: Look::Sensor,
        pins: TCS3200_PINS,
        current_ma: 20,
        three_three: false,
    },
];

/// The board the palette offers first.
pub const DEFAULT: &str = "L298N";

pub fn find(name: &str) -> Option<&'static Module> {
    ALL.iter().find(|m| m.name == name)
}

/// A short hint about the module's quirks, shown as a note beside the build.
pub fn note(name: &str) -> Option<L> {
    let m = find(name)?;
    Some(match m.name {
        "RC522" => L::new(
            "RC522 питается только от 3.3 В: от 5 В модуль выходит из строя.",
            "The RC522 runs on 3.3 V only: 5 V kills it.",
        ),
        "NRF24L01" => L::new(
            "NRF24L01 питается от 3.3 В и боится просадок: 10 мкФ по питанию не помешает.",
            "The NRF24L01 wants 3.3 V and hates brownouts: a 10 µF cap on VCC helps.",
        ),
        "HC-05" | "HC-06" => L::new(
            "У HC-05 линия RXD рассчитана на 3.3 В: с 5 В её ставят через делитель.",
            "The HC-05 RXD pin is a 3.3 V logic input: a divider is used with a 5 V board.",
        ),
        "NEOWAY M590" => L::new(
            "M590 в импульсе берёт до 2 А: питание только от отдельного блока, не от USB.",
            "The M590 draws up to 2 A in bursts: power it from its own supply, never USB.",
        ),
        "L298N" => L::new(
            "L298N питается от внешнего 12 В; для двигателей 5 В удобно взять 5 В с самого модуля.",
            "The L298N takes an external 12 V supply and gives out its own 5 V for the logic.",
        ),
        "ULN2003" => L::new(
            "ULN2003 — сборка Дарлингтонов с диодами: обмотки шаговика можно включать без защиты.",
            "The ULN2003 is a Darlington array with built-in diodes, so the coils need no extra protection.",
        ),
        "ENC28J60" => L::new(
            "ENC28J60 питается от 3.3 В или от 5 В через свой стабилизатор — смотри перемычку платы.",
            "The ENC28J60 board runs from 3.3 V or 5 V through its own regulator — check its jumper.",
        ),
        "MQ-2" | "MQ-3" => L::new(
            "Нагреватель MQ берёт около 150 мА: после включения датчику нужно 20 секунд на прогрев.",
            "The MQ heater draws about 150 mA and needs 20 seconds to warm up after power-up.",
        ),
        "HC-SR04" => L::new(
            "HC-SR04 питается от 5 В, ECHO выдаёт 5 В — делитель не нужен. Между импульсами не меньше 60 мс.",
            "The HC-SR04 runs on 5 V and its ECHO is 5 V, so no divider is needed. Leave 60 ms between bursts.",
        ),
        "DHT11" | "DHT22" => L::new(
            "DHT опрашивают не чаще раза в 2 секунды; линию DATA подтягивают к 5 В резистором 10 кОм.",
            "Poll a DHT at most every two seconds; pull its DATA line to 5 V with a 10 kΩ resistor.",
        ),
        "TFT 2.4" => L::new(
            "Логика TFT 2.4 рассчитана на 3.3 В, а шилд ставится прямо на Uno: на плате стоит преобразователь уровней.",
            "The TFT 2.4 works at 3.3 V logic, but the shield plugs straight into the Uno, which carries level shifters.",
        ),
        "P10" | "P10 RGB" => L::new(
            "Панель P10 берёт до 2 А на цвет: питание отдельное, земля общая с платой.",
            "A P10 panel draws up to 2 A in colour: give it its own supply and a common ground with the board.",
        ),
        "WS2812" => L::new(
            "WS2812 питается от 5 В, но данные любит 3.3–5 В; последовательный резистор 330 Ом на DIN бережёт первую микросхему.",
            "The WS2812 runs on 5 V but accepts 3.3–5 V data; a 330 Ω series resistor on DIN protects the first chip.",
        ),
        "HX711" => L::new(
            "HX711 питается от 5 В и опрашивается двумя линиями; тензодатчик подключают по цветам моста.",
            "The HX711 runs on 5 V and is read over two wires; the load cell follows the bridge colour code.",
        ),
        "FINGERPRINT" => L::new(
            "У отпечатка пальцев TX/RX — 3.3 В: с 5 В платой соединяют через делитель на RX модуля.",
            "The fingerprint module uses 3.3 V UART: with a 5 V board its RX goes through a divider.",
        ),
        "MT8870" => L::new(
            "MT8870 выдаёт двоичный код нажатой цифры и импульс STQ: читать код нужно по спаду STQ.",
            "The MT8870 puts the dialled digit on Q1–Q4 and pulses STQ: read the code on the falling edge of STQ.",
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Kind;

    #[test]
    fn every_module_has_a_header_inside_its_outline() {
        for m in ALL {
            assert!(m.w > 0.0 && m.h > 0.0, "{}", m.name);
            assert!(!m.pins.is_empty(), "{} has no header", m.name);
            assert_eq!(
                crate::templates::bench_pins(Kind::Module, m.name).len(),
                m.pins.len(),
                "{} is not reachable from bench_pins",
                m.name
            );
            for (name, x, y) in m.pins {
                assert!(
                    *x >= -4.0 && *x <= m.w + 4.0 && *y >= -4.0 && *y <= m.h + 4.0,
                    "{} pin {name} sits outside the board",
                    m.name
                );
            }
        }
    }

    #[test]
    fn names_are_unique_and_lookup_works() {
        for m in ALL {
            assert!(find(m.name).is_some(), "{} is missing", m.name);
            assert!(find("no such board").is_none());
            assert_eq!(
                ALL.iter().filter(|q| q.name == m.name).count(),
                1,
                "{} is listed twice",
                m.name
            );
        }
    }

    #[test]
    fn every_book_is_represented() {
        assert!(ALL.len() >= 45, "only {} boards", ALL.len());
        for name in ["DHT11", "HC-SR04", "OLED 0.96", "DS3231", "MPU6050", "TCS3200"] {
            assert!(find(name).is_some(), "{name} is missing");
        }
    }
}
