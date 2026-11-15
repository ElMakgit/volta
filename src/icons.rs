//! Palette pictograms.
//!
//! Each part is drawn as a list of primitives in a 24 × 24 box, in the same
//! materials as the bench drawing: tan resistors, red LEDs, blue boards and
//! gold headers. No textures are involved, so the icons stay sharp at every
//! scale and cost a few kilobytes of code instead of megabytes of atlas.

use eframe::egui::{self, vec2, Color32, Painter, Rect, Stroke, StrokeKind};

use crate::manual::Place;

/// What the palette button stands for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Part(Place),
    Module(&'static str),
}

/// One shape of a pictogram, in a 24 × 24 grid with the origin at the top left.
pub enum IcoPrim {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rad: f32,
        fill: Color32,
        stroke: Option<(Color32, f32)>,
    },
    Circle {
        x: f32,
        y: f32,
        r: f32,
        fill: Color32,
        stroke: Option<(Color32, f32)>,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        w: f32,
        color: Color32,
    },
    Poly {
        pts: Vec<(f32, f32)>,
        fill: Color32,
    },
}

const BLACK: Color32 = Color32::from_rgb(0x24, 0x24, 0x28);
const DARK: Color32 = Color32::from_rgb(0x15, 0x15, 0x18);
const SILVER: Color32 = Color32::from_rgb(0xB9, 0xB9, 0xB4);
const STEEL: Color32 = Color32::from_rgb(0x8A, 0x8C, 0x8E);
const GOLD: Color32 = Color32::from_rgb(0xC9, 0xA2, 0x27);
const TAN: Color32 = Color32::from_rgb(0xCF, 0xB0, 0x74);
const LED_RED: Color32 = Color32::from_rgb(0xD2, 0x3A, 0x35);
const PCB_GREEN: Color32 = Color32::from_rgb(0x2E, 0x7D, 0x32);
const PCB_BLUE: Color32 = Color32::from_rgb(0x1D, 0x4F, 0xA8);
const PCB_RED: Color32 = Color32::from_rgb(0xA8, 0x32, 0x2A);
const PCB_TEAL: Color32 = Color32::from_rgb(0x0E, 0x6C, 0x8A);
const WHITE: Color32 = Color32::from_rgb(0xEC, 0xEC, 0xE6);

// --------------------------------------------------------------- tiny helpers

fn r(x: f32, y: f32, w: f32, h: f32, fill: Color32) -> IcoPrim {
    IcoPrim::Rect { x, y, w, h, rad: 0.0, fill, stroke: None }
}

fn rr(x: f32, y: f32, w: f32, h: f32, rad: f32, fill: Color32) -> IcoPrim {
    IcoPrim::Rect { x, y, w, h, rad, fill, stroke: None }
}

fn sr(x: f32, y: f32, w: f32, h: f32, fill: Color32, line: Color32, wgt: f32) -> IcoPrim {
    IcoPrim::Rect { x, y, w, h, rad: 0.0, fill, stroke: Some((line, wgt)) }
}

fn c(x: f32, y: f32, rad: f32, fill: Color32) -> IcoPrim {
    IcoPrim::Circle { x, y, r: rad, fill, stroke: None }
}

fn sc(x: f32, y: f32, rad: f32, fill: Color32, line: Color32, wgt: f32) -> IcoPrim {
    IcoPrim::Circle { x, y, r: rad, fill, stroke: Some((line, wgt)) }
}

fn l(x1: f32, y1: f32, x2: f32, y2: f32, color: Color32) -> IcoPrim {
    IcoPrim::Line { x1, y1, x2, y2, w: 1.3, color }
}

fn poly(pts: Vec<(f32, f32)>, fill: Color32) -> IcoPrim {
    IcoPrim::Poly { pts, fill }
}

/// A row of header pins: gold squares around dark sockets.
fn header(out: &mut Vec<IcoPrim>, x: f32, y: f32, w: f32, n: usize, along_x: bool) {
    let gap = w / n as f32;
    for i in 0..n {
        let (px, py) = if along_x {
            (x + gap * (i as f32 + 0.5), y)
        } else {
            (x, y + gap * (i as f32 + 0.5))
        };
        let s = 2.6;
        out.push(r(px - s / 2.0, py - s / 2.0, s, s, GOLD));
        out.push(r(px - 0.9, py - 0.9, 1.8, 1.8, DARK));
    }
}

/// A board with a header, the base of every module icon.
fn board(out: &mut Vec<IcoPrim>, colour: Color32, w: f32, h: f32, at: f32) {
    out.push(rr(at, 4.0, w, h, 1.2, colour));
    out.push(sr(at, 4.0, w, h, Color32::TRANSPARENT, colour.gamma_multiply(0.72), 0.6));
    out.push(c(at + 2.0, 6.0, 0.7, WHITE));
    out.push(c(at + w - 2.0, 6.0, 0.7, WHITE));
}

// ------------------------------------------------------------------ the parts

pub fn prims(icon: Icon) -> Vec<IcoPrim> {
    let mut out = Vec::new();
    match icon {
        Icon::Part(place) => part(&mut out, place),
        Icon::Module(name) => module(&mut out, name),
    }
    out
}

fn part(out: &mut Vec<IcoPrim>, place: Place) {
    match place {
        Place::Resistor => {
            out.push(l(1.0, 12.0, 5.0, 12.0, STEEL));
            out.push(l(19.0, 12.0, 23.0, 12.0, STEEL));
            out.push(rr(4.0, 8.5, 16.0, 7.0, 1.2, TAN));
            for (x, col) in [(7.0, BLACK), (9.5, LED_RED), (12.0, GOLD), (14.5, BLACK)] {
                out.push(r(x, 9.0, 1.3, 6.0, col));
            }
        }
        Place::Capacitor => {
            out.push(l(8.0, 15.0, 8.0, 22.0, STEEL));
            out.push(l(16.0, 15.0, 16.0, 22.0, STEEL));
            out.push(sc(12.0, 10.5, 6.2, Color32::from_rgb(0xE0, 0xB6, 0x3A), Color32::from_rgb(0x9A, 0x74, 0x1A), 0.6));
            out.push(l(8.0, 10.5, 16.0, 10.5, Color32::from_rgb(0x9A, 0x74, 0x1A)));
        }
        Place::Led => {
            out.push(l(9.0, 15.0, 8.0, 22.0, STEEL));
            out.push(l(15.0, 15.0, 16.0, 22.0, STEEL));
            out.push(r(8.0, 13.0, 8.0, 3.0, Color32::from_rgb(0xB0, 0x2A, 0x26)));
            out.push(sc(12.0, 9.0, 5.2, LED_RED, Color32::from_rgb(0x8E, 0x22, 0x1F), 0.6));
            out.push(c(10.0, 7.5, 1.3, Color32::from_rgb(0xF2, 0x9A, 0x95)));
        }
        Place::RgbLed => {
            for x in [7.0, 12.0, 17.0] {
                out.push(l(x, 16.0, x, 22.0, STEEL));
            }
            out.push(sc(12.0, 9.5, 6.4, Color32::from_rgb(0xD8, 0xDB, 0xDE), SILVER, 0.6));
            out.push(c(9.5, 9.5, 1.5, LED_RED));
            out.push(c(12.0, 8.6, 1.5, Color32::from_rgb(0x3F, 0xC8, 0x5A)));
            out.push(c(14.5, 9.5, 1.5, Color32::from_rgb(0x3D, 0x7B, 0xE8)));
        }
        Place::IrLed => {
            out.push(l(9.0, 15.0, 8.0, 22.0, STEEL));
            out.push(l(15.0, 15.0, 16.0, 22.0, STEEL));
            out.push(r(8.0, 13.0, 8.0, 3.0, Color32::from_rgb(0x44, 0x28, 0x5E)));
            out.push(sc(12.0, 9.0, 5.2, Color32::from_rgb(0x6E, 0x3A, 0x9A), Color32::from_rgb(0x3E, 0x1E, 0x5E), 0.6));
            out.push(c(10.5, 7.8, 1.1, Color32::from_rgb(0xB8, 0x8A, 0xE0)));
        }
        Place::Reed => {
            out.push(l(1.0, 12.0, 4.0, 12.0, STEEL));
            out.push(l(20.0, 12.0, 23.0, 12.0, STEEL));
            out.push(sr(4.0, 8.5, 16.0, 7.0, Color32::from_rgb(0xC8, 0xD2, 0xD8), Color32::from_rgb(0x8E, 0x9A, 0xA2), 0.6));
            out.push(l(6.0, 12.0, 11.5, 12.0, SILVER));
            out.push(l(12.5, 12.0, 18.0, 12.0, SILVER));
            out.push(c(12.0, 12.0, 0.8, GOLD));
        }
        Place::Thermistor => {
            out.push(l(9.0, 16.0, 8.0, 22.0, STEEL));
            out.push(l(15.0, 16.0, 16.0, 22.0, STEEL));
            out.push(sc(12.0, 10.0, 5.4, Color32::from_rgb(0x1E, 0x22, 0x28), Color32::from_rgb(0x08, 0x0A, 0x0E), 0.6));
            out.push(c(10.6, 8.8, 1.0, Color32::from_rgb(0x5A, 0x62, 0x6E)));
        }
        Place::Peltier => {
            out.push(l(2.0, 14.0, 2.0, 7.0, STEEL));
            out.push(l(2.0, 17.0, 2.0, 22.0, STEEL));
            out.push(sr(5.0, 4.0, 17.0, 16.0, Color32::from_rgb(0xE6, 0xE6, 0xDE), Color32::from_rgb(0xA8, 0xA8, 0xA0), 0.6));
            for (x, y) in [(9.0, 8.0), (12.5, 8.0), (16.0, 8.0), (9.0, 12.0), (12.5, 12.0), (16.0, 12.0), (9.0, 16.0), (12.5, 16.0), (16.0, 16.0)] {
                out.push(c(x, y, 1.0, Color32::from_rgb(0xB8, 0x8A, 0x4A)));
            }
        }
        Place::Diode => {
            out.push(l(1.0, 12.0, 4.0, 12.0, STEEL));
            out.push(l(20.0, 12.0, 23.0, 12.0, STEEL));
            out.push(rr(4.0, 9.0, 16.0, 6.0, 0.8, BLACK));
            out.push(r(16.0, 9.0, 2.0, 6.0, SILVER));
        }
        Place::Nmos => {
            out.push(r(7.0, 3.0, 10.0, 4.0, SILVER));
            out.push(c(12.0, 4.5, 1.0, STEEL));
            out.push(rr(8.0, 6.5, 8.0, 11.0, 0.6, BLACK));
            for x in [9.0, 12.0, 15.0] {
                out.push(l(x, 17.5, x, 22.0, STEEL));
            }
        }
        Place::Npn => {
            out.push(l(8.0, 16.0, 7.0, 22.0, STEEL));
            out.push(l(12.0, 16.0, 12.0, 22.0, STEEL));
            out.push(l(16.0, 16.0, 17.0, 22.0, STEEL));
            out.push(rr(6.0, 9.0, 12.0, 7.0, 2.4, BLACK));
            out.push(sc(12.0, 9.0, 3.0, BLACK, Color32::from_rgb(0x3A, 0x3A, 0x40), 0.5));
        }
        Place::Button => {
            out.push(l(5.0, 19.0, 5.0, 22.0, STEEL));
            out.push(l(19.0, 19.0, 19.0, 22.0, STEEL));
            out.push(sr(4.0, 6.0, 16.0, 13.0, Color32::from_rgb(0x3A, 0x3A, 0x40), SILVER, 0.6));
            out.push(sc(12.0, 11.5, 4.2, Color32::from_rgb(0x55, 0x55, 0x5C), SILVER.gamma_multiply(0.72), 0.5));
        }
        Place::Ldr => {
            out.push(l(9.0, 16.0, 8.0, 22.0, STEEL));
            out.push(l(15.0, 16.0, 16.0, 22.0, STEEL));
            out.push(sc(12.0, 10.0, 6.0, Color32::from_rgb(0xE6, 0xC6, 0x58), Color32::from_rgb(0x9A, 0x7A, 0x1E), 0.6));
            for i in 0..3 {
                let y = 7.5 + i as f32 * 2.2;
                out.push(l(9.0, y, 15.0, y + 1.2, Color32::from_rgb(0x7A, 0x5A, 0x12)));
            }
        }
        Place::Motor => {
            out.push(rr(3.0, 7.0, 14.0, 10.0, 4.0, Color32::from_rgb(0xB7, 0xB8, 0xB4)));
            out.push(sc(17.0, 11.0, 3.2, Color32::from_rgb(0x9A, 0x9B, 0x97), STEEL, 0.6));
            out.push(r(19.0, 10.0, 4.0, 2.0, STEEL));
            out.push(l(4.0, 17.0, 4.0, 21.0, STEEL));
            out.push(l(16.0, 17.0, 16.0, 21.0, STEEL));
        }
        Place::Servo => {
            out.push(l(7.0, 19.0, 6.0, 22.0, SILVER));
            out.push(l(12.0, 19.0, 12.0, 22.0, LED_RED));
            out.push(l(17.0, 19.0, 18.0, 22.0, STEEL));
            out.push(rr(4.0, 7.0, 13.0, 12.0, 1.4, PCB_BLUE));
            out.push(r(13.0, 9.0, 5.0, 4.0, WHITE));
            out.push(c(18.0, 11.0, 1.4, STEEL));
        }
        Place::Buzzer => {
            out.push(l(8.0, 17.0, 7.0, 22.0, STEEL));
            out.push(l(16.0, 17.0, 17.0, 22.0, STEEL));
            out.push(sc(12.0, 10.0, 6.5, BLACK, Color32::from_rgb(0x44, 0x44, 0x4A), 0.6));
            out.push(c(12.0, 10.0, 1.6, DARK));
        }
        Place::Speaker => {
            out.push(poly(vec![(4.0, 5.0), (6.0, 7.0), (6.0, 17.0), (4.0, 19.0)], BLACK));
            out.push(poly(vec![(6.0, 7.0), (17.0, 3.0), (17.0, 21.0), (6.0, 17.0)], Color32::from_rgb(0x50, 0x50, 0x56)));
            out.push(rr(16.0, 8.0, 5.0, 8.0, 1.0, STEEL));
            out.push(l(12.0, 21.0, 12.0, 23.0, STEEL));
        }
        Place::Pot => {
            out.push(l(8.0, 17.0, 7.0, 22.0, STEEL));
            out.push(l(12.0, 17.0, 12.0, 22.0, STEEL));
            out.push(l(16.0, 17.0, 17.0, 22.0, STEEL));
            out.push(rr(5.0, 6.0, 14.0, 11.0, 1.2, PCB_BLUE));
            out.push(sc(12.0, 11.0, 4.0, Color32::from_rgb(0xD8, 0xD8, 0xD0), STEEL, 0.6));
            out.push(l(10.0, 11.0, 14.0, 11.0, STEEL));
        }
        Place::Relay => {
            out.push(l(6.0, 18.0, 6.0, 22.0, STEEL));
            out.push(l(12.0, 18.0, 12.0, 22.0, STEEL));
            out.push(l(18.0, 18.0, 18.0, 22.0, STEEL));
            out.push(rr(3.0, 6.0, 18.0, 12.0, 1.0, PCB_BLUE));
            out.push(sr(5.0, 8.0, 14.0, 8.0, Color32::from_rgb(0x28, 0x28, 0x2E), BLACK, 0.5));
            out.push(l(9.0, 10.0, 12.0, 13.0, GOLD));
        }
        Place::Uno => {
            out.push(rr(2.0, 5.0, 20.0, 15.0, 1.2, PCB_TEAL));
            out.push(sr(2.0, 5.0, 20.0, 15.0, Color32::TRANSPARENT, Color32::from_rgb(0x08, 0x4A, 0x60), 0.6));
            out.push(r(4.0, 3.0, 16.0, 3.0, BLACK));
            out.push(r(4.0, 19.0, 16.0, 3.0, BLACK));
            header(out, 5.0, 4.5, 14.0, 7, true);
            header(out, 5.0, 20.5, 14.0, 7, true);
            out.push(rr(6.0, 9.0, 7.0, 6.0, 0.7, BLACK));
            out.push(r(15.0, 8.0, 5.0, 5.0, SILVER));
            out.push(c(19.0, 14.0, 1.6, WHITE));
        }
        // The palette never asks for a bare module here; the module buttons
        // carry their own names.
        Place::Module => {
            board(out, PCB_BLUE, 16.0, 14.0, 4.0);
            header(out, 5.0, 19.0, 14.0, 6, true);
        }
    }
}

fn module(out: &mut Vec<IcoPrim>, name: &str) {
    let info = crate::modules::find(name);
    let colour = info
        .map(|m| Color32::from_rgb(m.colour.0, m.colour.1, m.colour.2))
        .unwrap_or(PCB_BLUE);
    let look = info.map(|m| m.look).unwrap_or(crate::modules::Look::Board);
    match name {
        "L298N" => {
            board(out, PCB_RED, 20.0, 17.0, 2.0);
            out.push(rr(8.0, 5.0, 8.0, 6.0, 0.4, SILVER));
            for x in [9.5, 12.0, 14.5] {
                out.push(l(x, 5.5, x, 10.5, STEEL));
            }
            for y in [9.0, 13.0, 17.0] {
                out.push(sr(3.5, y - 1.4, 3.4, 2.8, PCB_BLUE, STEEL, 0.4));
            }
            header(out, 8.0, 22.0, 10.0, 6, true);
        }
        "L293D" => {
            board(out, PCB_RED, 20.0, 17.0, 2.0);
            out.push(rr(7.0, 9.0, 10.0, 7.0, 0.6, BLACK));
            header(out, 5.0, 22.0, 14.0, 8, true);
        }
        "ULN2003" => {
            board(out, PCB_BLUE, 20.0, 17.0, 2.0);
            out.push(rr(7.0, 8.0, 9.0, 8.0, 0.6, BLACK));
            header(out, 6.0, 22.0, 12.0, 6, true);
        }
        "28BYJ-48" => {
            out.push(sc(11.0, 11.0, 8.0, SILVER, STEEL, 0.7));
            out.push(c(11.0, 11.0, 3.0, Color32::from_rgb(0xA4, 0xA5, 0xA0)));
            out.push(c(11.0, 11.0, 1.0, STEEL));
            out.push(r(18.0, 10.0, 4.0, 2.0, STEEL));
            out.push(r(3.0, 20.0, 16.0, 2.0, PCB_BLUE));
            header(out, 5.0, 21.0, 12.0, 5, true);
        }
        "RC522" => {
            board(out, PCB_GREEN, 20.0, 17.0, 2.0);
            for i in 1..4 {
                let d = i as f32 * 1.6;
                out.push(sr(4.5 + d, 6.5 + d, 13.0 - 2.0 * d, 9.0 - 2.0 * d, Color32::TRANSPARENT, GOLD, 0.35));
            }
            out.push(rr(8.0, 15.0, 8.0, 4.0, 0.5, BLACK));
            header(out, 5.0, 22.0, 14.0, 6, true);
        }
        "FS1000A" | "XY-MK-5V" | "NRF24L01" => {
            board(out, colour, 18.0, 13.0, 3.0);
            out.push(rr(7.0, 9.0, 8.0, 5.0, 0.5, BLACK));
            out.push(l(18.0, 6.0, 23.0, 2.0, GOLD));
            header(out, 5.0, 17.0, 14.0, 6, true);
        }
        "HC-05" | "HC-06" => {
            board(out, PCB_BLUE, 20.0, 17.0, 2.0);
            out.push(rr(6.0, 7.0, 12.0, 8.0, 0.8, SILVER));
            out.push(r(17.0, 9.0, 4.0, 4.0, GOLD));
            header(out, 5.0, 22.0, 14.0, 6, true);
        }
        "NEOWAY M590" => {
            board(out, PCB_BLUE, 20.0, 17.0, 2.0);
            out.push(rr(6.0, 7.0, 12.0, 8.0, 0.6, SILVER));
            out.push(r(16.0, 6.0, 3.0, 3.0, GOLD));
            header(out, 4.0, 22.0, 16.0, 8, true);
        }
        "ENC28J60" | "W5100" => {
            board(out, PCB_BLUE, 20.0, 17.0, 2.0);
            out.push(rr(4.0, 7.0, 7.0, 8.0, 0.5, SILVER));
            out.push(rr(12.0, 8.0, 7.0, 6.0, 0.5, BLACK));
            out.push(r(9.0, 17.0, 5.0, 3.0, GOLD));
            header(out, 5.0, 22.0, 14.0, 6, true);
        }
        "LCD1602" => {
            board(out, PCB_GREEN, 21.0, 15.0, 1.5);
            out.push(rr(4.0, 7.0, 17.0, 8.0, 0.5, Color32::from_rgb(0x1F, 0x4A, 0x22)));
            for row in 0..2 {
                for col in 0..6 {
                    out.push(r(5.0 + col as f32 * 2.6, 8.0 + row as f32 * 3.0, 2.0, 2.0, Color32::from_rgb(0x8F, 0xC9, 0x7A)));
                }
            }
            header(out, 5.0, 20.0, 15.0, 8, true);
        }
        _ => match look {
            crate::modules::Look::Sensor => {
                board(out, colour, 18.0, 15.0, 3.0);
                out.push(sc(9.0, 10.0, 4.0, STEEL, Color32::from_rgb(0x5A, 0x5E, 0x66), 0.5));
                out.push(c(8.0, 9.0, 1.2, WHITE));
                out.push(rr(13.0, 13.0, 5.0, 4.0, 0.6, BLACK));
                header(out, 5.0, 21.0, 14.0, 4, true);
            }
            crate::modules::Look::Display => {
                board(out, colour, 20.0, 17.0, 2.0);
                out.push(rr(4.0, 6.0, 16.0, 10.0, 0.6, Color32::from_rgb(0x0B, 0x12, 0x1E)));
                out.push(r(6.0, 8.0, 5.0, 2.0, Color32::from_rgb(0x63, 0xD2, 0xE8)));
                out.push(r(12.0, 12.0, 5.0, 2.0, Color32::from_rgb(0xEB, 0xC2, 0x54)));
                header(out, 5.0, 22.0, 14.0, 5, true);
            }
            crate::modules::Look::Segments => {
                board(out, Color32::from_rgb(0x1B, 0x1D, 0x22), 20.0, 17.0, 2.0);
                for d in 0..1 {
                    let x = 5.0 + d as f32 * 8.0;
                    out.push(rr(x, 7.0, 6.0, 12.0, 0.4, Color32::from_rgb(0x14, 0x15, 0x19)));
                    out.push(r(x + 1.2, 8.0, 3.6, 1.2, LED_RED));
                    out.push(r(x + 4.8, 9.2, 1.2, 3.0, LED_RED));
                    out.push(r(x + 1.2, 12.2, 3.6, 1.2, LED_RED));
                    out.push(r(x + 1.2, 13.4, 3.6, 1.2, LED_RED));
                    out.push(r(x + 1.2, 14.6, 3.6, 1.2, LED_RED));
                }
                header(out, 6.0, 22.0, 12.0, 6, true);
            }
            crate::modules::Look::Matrix => {
                board(out, Color32::from_rgb(0x1B, 0x1D, 0x22), 20.0, 17.0, 2.0);
                for row in 0..4 {
                    for col in 0..4 {
                        out.push(c(6.0 + col as f32 * 3.2, 8.0 + row as f32 * 2.6, 0.9, LED_RED));
                    }
                }
                header(out, 5.0, 22.0, 14.0, 5, true);
            }
            crate::modules::Look::Panel => {
                board(out, Color32::from_rgb(0x1B, 0x1D, 0x22), 21.0, 15.0, 1.5);
                for row in 0..4 {
                    for col in 0..7 {
                        let on = (row + col) % 3 == 0;
                        out.push(c(
                            4.0 + col as f32 * 2.4,
                            7.0 + row as f32 * 2.4,
                            0.8,
                            if on { LED_RED } else { Color32::from_rgb(0x3A, 0x1A, 0x18) },
                        ));
                    }
                }
                header(out, 5.0, 21.0, 14.0, 5, true);
            }
            crate::modules::Look::Ring => {
                out.push(sc(11.0, 11.0, 9.0, Color32::from_rgb(0x14, 0x15, 0x19), Color32::from_rgb(0x33, 0x36, 0x3C), 0.5));
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::TAU / 8.0;
                    let col = match i % 3 {
                        0 => LED_RED,
                        1 => Color32::from_rgb(0x3F, 0xC8, 0x5A),
                        _ => Color32::from_rgb(0x3D, 0x7B, 0xE8),
                    };
                    out.push(c(11.0 + a.cos() * 6.2, 11.0 + a.sin() * 6.2, 1.3, col));
                }
                out.push(c(11.0, 11.0, 2.2, SILVER));
            }
            crate::modules::Look::Keypad => {
                board(out, colour, 20.0, 16.0, 2.0);
                out.push(sc(11.0, 10.0, 5.0, Color32::from_rgb(0x28, 0x2A, 0x30), SILVER, 0.6));
                out.push(c(11.0, 10.0, 3.0, Color32::from_rgb(0x18, 0x19, 0x1D)));
                for (dx, dy) in [(-1.6, -0.9), (1.6, -0.9), (0.0, 1.7)] {
                    out.push(c(11.0 + dx, 10.0 + dy, 0.6, GOLD));
                }
                header(out, 5.0, 21.0, 14.0, 4, true);
            }
            crate::modules::Look::Lcd => {
                board(out, PCB_GREEN, 21.0, 15.0, 1.5);
                out.push(rr(4.0, 6.0, 17.0, 9.0, 0.5, Color32::from_rgb(0x1F, 0x4A, 0x22)));
                for row in 0..2 {
                    for col in 0..6 {
                        out.push(r(
                            5.0 + col as f32 * 2.6,
                            7.0 + row as f32 * 3.2,
                            2.0,
                            2.0,
                            Color32::from_rgb(0x8F, 0xC9, 0x7A),
                        ));
                    }
                }
                header(out, 5.0, 20.0, 15.0, 6, true);
            }
            _ => {
                board(out, colour, 20.0, 16.0, 2.0);
                out.push(rr(7.0, 8.0, 9.0, 6.0, 0.5, BLACK));
                header(out, 6.0, 21.0, 13.0, 6, true);
            }
        },
    }
}

// ----------------------------------------------------------------- the paint

/// Draw an icon into `rect`, scaling the 24-unit grid to fit.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, accent: Color32) {
    let side = rect.width().min(rect.height());
    let at = rect.min + (rect.size() - vec2(side, side)) / 2.0;
    let s = side / 24.0;
    let p = |x: f32, y: f32| at + vec2(x * s, y * s);
    let boxed = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(p(x, y), vec2(w * s, h * s));
    for prim in prims(icon) {
        match prim {
            IcoPrim::Rect { x, y, w, h, rad, fill, stroke } => {
                let rr = boxed(x, y, w, h);
                if rad > 0.0 {
                    painter.rect_filled(rr, rad * s, fill);
                } else {
                    painter.rect_filled(rr, 0.0, fill);
                }
                if let Some((col, wgt)) = stroke {
                    painter.rect_stroke(
                        rr,
                        rad * s,
                        Stroke::new((wgt * s).clamp(0.5, 2.0), col),
                        StrokeKind::Inside,
                    );
                }
            }
            IcoPrim::Circle { x, y, r, fill, stroke } => {
                painter.circle_filled(p(x, y), r * s, fill);
                if let Some((col, wgt)) = stroke {
                    painter.circle_stroke(p(x, y), r * s, Stroke::new((wgt * s).clamp(0.5, 2.0), col));
                }
            }
            IcoPrim::Line { x1, y1, x2, y2, w, color } => {
                painter.line_segment(
                    [p(x1, y1), p(x2, y2)],
                    Stroke::new((w * s).max(1.0), color),
                );
            }
            IcoPrim::Poly { pts, fill } => {
                let pts: Vec<_> = pts.iter().map(|&(x, y)| p(x, y)).collect();
                painter.add(egui::Shape::convex_polygon(pts, fill, Stroke::NONE));
            }
        }
    }
    let _ = accent;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icons() -> Vec<Icon> {
        let mut out: Vec<Icon> = crate::manual::PALETTE.iter().copied().map(Icon::Part).collect();
        out.extend(crate::modules::ALL.iter().map(|m| Icon::Module(m.name)));
        out
    }

    /// Every palette entry has a picture, and every picture fits the 24-unit
    /// box so a button never has to clip it.
    #[test]
    fn every_palette_entry_has_an_icon_that_fits() {
        for icon in icons() {
            let prims = prims(icon);
            assert!(prims.len() >= 4, "icon with only {} primitives", prims.len());
            for p in prims {
                let (minx, miny, maxx, maxy) = match p {
                    IcoPrim::Rect { x, y, w, h, .. } => (x, y, x + w, y + h),
                    IcoPrim::Circle { x, y, r, .. } => (x - r, y - r, x + r, y + r),
                    IcoPrim::Line { x1, y1, x2, y2, .. } => {
                        (x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2))
                    }
                    IcoPrim::Poly { pts, .. } => pts.iter().fold(
                        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
                        |(a, b, c, d), &(x, y)| (a.min(x), b.min(y), c.max(x), d.max(y)),
                    ),
                };
                assert!(
                    minx >= -0.5 && miny >= -0.5 && maxx <= 24.5 && maxy <= 24.5,
                    "icon draws outside its box: {minx}..{maxx} {miny}..{maxy}"
                );
            }
        }
    }

    /// The Uno is a palette entry too, so a hand-drawn circuit can put the
    /// board back if it was removed.
    #[test]
    fn the_uno_is_in_the_palette() {
        assert!(crate::manual::PALETTE.contains(&Place::Uno));
    }
}
