//! Scene building: the drawing, independent of who rasterises it.
//!
//! One list of primitives is produced here and used twice — the egui canvas
//! paints it on screen, the SVG writer paints it into a file. That is the only
//! way the screenshot can look like what the user sees.

use crate::model::*;
use crate::modules::Look;
use crate::templates::bench_pins;
use crate::theme::Theme;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(r, g, b, 255)
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(r, g, b, a)
    }
    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.trim().trim_start_matches('#');
        let pick = |a: usize, b: usize| u8::from_str_radix(&h[a..b], 16).ok();
        match h.len() {
            6 => Some(Self(pick(0, 2)?, pick(2, 4)?, pick(4, 6)?, 255)),
            8 => Some(Self(pick(0, 2)?, pick(2, 4)?, pick(4, 6)?, pick(6, 8)?)),
            3 => {
                let d = |i: usize| {
                    u8::from_str_radix(&h[i..i + 1], 16)
                        .ok()
                        .map(|v| v * 17)
                };
                Some(Self(d(0)?, d(1)?, d(2)?, 255))
            }
            _ => None,
        }
    }
    pub fn lighten(self, t: f32) -> Self {
        let f = |c: u8| (c as f32 + (255.0 - c as f32) * t.clamp(0.0, 1.0)) as u8;
        Self(f(self.0), f(self.1), f(self.2), self.3)
    }
    pub fn darken(self, t: f32) -> Self {
        let f = |c: u8| (c as f32 * (1.0 - t.clamp(0.0, 1.0))) as u8;
        Self(f(self.0), f(self.1), f(self.2), self.3)
    }
    pub fn mix(self, o: Rgba, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        Self(f(self.0, o.0), f(self.1, o.1), f(self.2, o.2), f(self.3, o.3))
    }
    pub fn with_a(self, a: u8) -> Self {
        Self(self.0, self.1, self.2, a)
    }
    pub fn alpha(self, t: f32) -> Self {
        Self(self.0, self.1, self.2, (self.3 as f32 * t.clamp(0.0, 1.0)) as u8)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Anchor {
    Start,
    Center,
    End,
}

impl Prim {
    /// Rotate a primitive by quarter turns about a pivot. Quarter turns keep
    /// rectangles axis-aligned, which is why parts can be turned without a
    /// general transform pipeline.
    pub fn rotated(&self, pivot: Pt, quarter: i32) -> Prim {
        if quarter.rem_euclid(4) == 0 {
            return self.clone();
        }
        match self {
            Prim::Rect { r, radius, fill, stroke } => {
                let corners = [
                    Pt::new(r.x, r.y),
                    Pt::new(r.right(), r.y),
                    Pt::new(r.right(), r.bottom()),
                    Pt::new(r.x, r.bottom()),
                ];
                let mut min = Pt::new(f32::MAX, f32::MAX);
                let mut max = Pt::new(f32::MIN, f32::MIN);
                for c in corners {
                    let q = c.rotate(pivot, quarter);
                    min = Pt::new(min.x.min(q.x), min.y.min(q.y));
                    max = Pt::new(max.x.max(q.x), max.y.max(q.y));
                }
                Prim::Rect {
                    r: Rect::new(min.x, min.y, max.x - min.x, max.y - min.y),
                    radius: *radius,
                    fill: *fill,
                    stroke: *stroke,
                }
            }
            Prim::Circle { c, r, fill, stroke } => Prim::Circle {
                c: c.rotate(pivot, quarter),
                r: *r,
                fill: *fill,
                stroke: *stroke,
            },
            Prim::Poly { pts, w, color, closed } => Prim::Poly {
                pts: pts.iter().map(|p| p.rotate(pivot, quarter)).collect(),
                w: *w,
                color: *color,
                closed: *closed,
            },
            Prim::Text { p, size, text, color, anchor, bold, angle } => Prim::Text {
                p: p.rotate(pivot, quarter),
                size: *size,
                text: text.clone(),
                color: *color,
                anchor: *anchor,
                bold: *bold,
                angle: angle + 90.0 * quarter.rem_euclid(4) as f32,
            },
        }
    }

    /// The rectangle a primitive paints into, used to keep the frame honest.
    pub fn bbox(&self) -> Rect {
        match self {
            Prim::Rect { r, stroke, .. } => match stroke {
                Some((_, w)) => r.inflate(*w / 2.0),
                None => *r,
            },
            Prim::Circle { c, r, stroke, .. } => {
                let r = match stroke {
                    Some((_, w)) => r + w / 2.0,
                    None => *r,
                };
                Rect::new(c.x - r, c.y - r, 2.0 * r, 2.0 * r)
            }
            Prim::Poly { pts, w, .. } => {
                let (mut minx, mut miny) = (f32::MAX, f32::MAX);
                let (mut maxx, mut maxy) = (f32::MIN, f32::MIN);
                for p in pts {
                    minx = minx.min(p.x);
                    miny = miny.min(p.y);
                    maxx = maxx.max(p.x);
                    maxy = maxy.max(p.y);
                }
                Rect::new(minx - w, miny - w, maxx - minx + 2.0 * w, maxy - miny + 2.0 * w)
            }
            Prim::Text { p, size, text, anchor, angle, .. } => {
                // Painting goes through the font, so this is a readable
                // approximation: one em tall, 0.62 em per glyph.
                let w = text_width(*size, text);
                let h = size * 1.15;
                // A label turned on its side trades width for height. This is
                // only used to keep the export frame honest, so a centred box
                // is close enough and never crops.
                if angle.rem_euclid(180.0) > 45.0 {
                    return Rect::new(p.x - h / 2.0, p.y - w / 2.0, h, w);
                }
                let x = match anchor {
                    Anchor::Start => p.x,
                    Anchor::Center => p.x - w / 2.0,
                    Anchor::End => p.x - w,
                };
                Rect::new(x, p.y - h / 2.0, w, h)
            }
        }
    }
}

/// A rough advance width for a string. The painter uses the real font, but a
/// label has to be placed before it is painted, so layout and hit-testing rely
/// on this estimate.
fn text_width(size: f32, text: &str) -> f32 {
    size * 0.62 * text.chars().count().max(1) as f32
}

/// Empty bench around the circuit, in millimetres. This also keeps the
/// selection frame from being cropped.
pub const PAD: f32 = 2.2;

#[derive(Clone, Debug)]
pub enum Prim {
    Rect {
        r: Rect,
        radius: f32,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    },
    Circle {
        c: Pt,
        r: f32,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    },
    Poly {
        pts: Vec<Pt>,
        w: f32,
        color: Rgba,
        closed: bool,
    },
    Text {
        p: Pt,
        size: f32,
        text: String,
        color: Rgba,
        anchor: Anchor,
        bold: bool,
        /// Degrees clockwise, for the rotated pin labels on a board header.
        angle: f32,
    },
}

/// Which object painted a primitive. The flat primitive list keeps this as a
/// parallel array, so hit-testing can use the real drawn shape instead of a
/// second, approximate bounding box.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Owner {
    Part(usize),
    Wire(usize),
}

#[derive(Clone)]
pub struct Scene {
    pub prims: Vec<Prim>,
    pub owners: Vec<Option<Owner>>,
    pub bg: Rgba,
    pub w: f32,
    pub h: f32,
    /// The bench coordinate that sits at the scene's own (0, 0). Drawing is
    /// shifted to the origin so nothing is cropped, so anyone mapping between
    /// the screen and the circuit has to pass through this point.
    pub origin: Pt,
}
impl Scene {
    /// The topmost painted object under a point in scene coordinates. Unlike
    /// a bounding-box test on the model, this follows what is actually drawn.
    pub fn hit_owner(&self, p: Pt) -> Option<Owner> {
        self.prims
            .iter()
            .zip(&self.owners)
            .rev()
            .find(|(prim, _)| prim.bbox().contains(p))
            .and_then(|(_, owner)| *owner)
    }
}



impl Scene {
    /// Bench millimetres to scene millimetres.
    pub fn to_scene(&self, p: Pt) -> Pt {
        Pt::new(p.x - self.origin.x, p.y - self.origin.y)
    }

    /// Scene millimetres back to the circuit's own coordinates.
    pub fn to_bench(&self, p: Pt) -> Pt {
        Pt::new(p.x + self.origin.x, p.y + self.origin.y)
    }

    /// The union of everything the scene paints.
    pub fn bounds(&self) -> Rect {
        let mut r = self.prims.first().map(|p| p.bbox()).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        for p in self.prims.iter().skip(1) {
            let b = p.bbox();
            let x0 = r.x.min(b.x);
            let y0 = r.y.min(b.y);
            let x1 = r.right().max(b.right());
            let y1 = r.bottom().max(b.bottom());
            r = Rect::new(x0, y0, x1 - x0, y1 - y0);
        }
        r
    }

    /// Turn the primitives added since `from`, which is how a whole part or
    /// the breadboard is rotated after being painted upright.
    pub fn rotate_prims(&mut self, from: usize, pivot: Pt, quarter: i32) {
        if quarter.rem_euclid(4) == 0 || from >= self.prims.len() {
            return;
        }
        for prim in &mut self.prims[from..] {
            *prim = prim.rotated(pivot, quarter);
        }
    }

    /// Move the whole drawing to the origin. The bounds and the painted
    /// primitives then agree, so a part left of the breadboard is not cropped
    /// and the margins stay even.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let p = |q: &Pt| Pt::new(q.x + dx, q.y + dy);
        for prim in &mut self.prims {
            match prim {
                Prim::Rect { r, .. } => *r = r.shift(dx, dy),
                Prim::Circle { c, .. } => *c = p(c),
                Prim::Poly { pts, .. } => pts.iter_mut().for_each(|q| *q = p(q)),
                Prim::Text { p: at, .. } => *at = p(at),
            }
        }
    }
}

pub struct Opts {
    pub labels: bool,
    pub hover: Option<usize>,
    /// Overrides the bench colour, which the export uses to put the circuit on
    /// a light, site-like background instead of the dark desktop theme.
    pub bench: Option<Rgba>,
    /// Arched jumpers or straight lines.
    pub style: WireStyle,
    /// The part the user is working with; drawn with a visible frame.
    pub selected: Option<usize>,
    /// Every part in a marquee selection gets the same frame.
    pub selected_many: Vec<usize>,
    /// Freeze the frame while a part is being dragged, so the drawing does not
    /// re-centre under the pointer on every mouse move.
    pub frame: Option<Rect>,
    /// The cable the user is working with: drawn a little heavier, and its
    /// hand-made bends get grips.
    pub active_wire: Option<usize>,
    /// Grip positions for the bends of the active cable, in bench millimetres.
    pub handles: Vec<Pt>,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            labels: false,
            hover: None,
            style: WireStyle::default(),
            selected: None,
            selected_many: Vec::new(),
            frame: None,
            active_wire: None,
            handles: Vec::new(),
            bench: None,
        }
    }
}

const BOARD_TOP: Rgba = Rgba::rgb(0xE8, 0xE8, 0xE6);
const BOARD_EDGE: Rgba = Rgba::rgb(0xC2, 0xC2, 0xBE);
const BOARD_CHANNEL: Rgba = Rgba::rgb(0xD3, 0xD3, 0xD0);
const HOLE: Rgba = Rgba::rgb(0x54, 0x54, 0x57);
const HOLE_IN: Rgba = Rgba::rgb(0x33, 0x33, 0x36);
const SILK: Rgba = Rgba::rgb(0x9E, 0x9E, 0x9A);
const SILK_DARK: Rgba = Rgba::rgb(0x77, 0x77, 0x73);
const PCB: Rgba = Rgba::rgb(0x2A, 0x75, 0xB8);
const PCB_EDGE: Rgba = Rgba::rgb(0x1C, 0x5A, 0x93);
const ARDUINO_PCB: Rgba = Rgba::rgb(0x0F, 0x73, 0x91);
const ARDUINO_EDGE: Rgba = Rgba::rgb(0x0A, 0x59, 0x70);
const HEADER: Rgba = Rgba::rgb(0x3F, 0x3F, 0x41);
const HEADER_GOLD: Rgba = Rgba::rgb(0xB7, 0xA6, 0x78);
const CHIP: Rgba = Rgba::rgb(0x33, 0x33, 0x36);
const USB_METAL: Rgba = Rgba::rgb(0xB4, 0xB4, 0xB6);
const SILK_W: Rgba = Rgba::rgb(0xF2, 0xF2, 0xF2);
const PLASTIC: Rgba = Rgba::rgb(0x22, 0x22, 0x24);
const BEIGE: Rgba = Rgba::rgb(0xDA, 0xC8, 0x9E);
const SILVER: Rgba = Rgba::rgb(0xC6, 0xC8, 0xCB);
const BLUE: Rgba = Rgba::rgb(0x2B, 0x6C, 0xC4);
const GREY: Rgba = Rgba::rgb(0xB6, 0xB9, 0xBE);
const LABEL: Rgba = Rgba::rgb(0x4C, 0x47, 0x40);
const WIRE_VCC: Rgba = Rgba::rgb(0xD2, 0x3A, 0x35);
const WIRE_GND: Rgba = Rgba::rgb(0x30, 0x30, 0x33);
const WIRE_SIG: Rgba = Rgba::rgb(0x37, 0xA6, 0x4C);
const WIRE_AN: Rgba = Rgba::rgb(0x2C, 0x7B, 0xD1);

/// The colour box of a real jumper set, as Tinkercad offers it. Power stays
/// red and ground black, the way a schematic reads; signal cables take a
/// colour of their own so a bundle does not look like one wire.
pub const WIRE_PALETTE: [Rgba; 10] = [
    WIRE_SIG,
    Rgba::rgb(0xE8, 0x59, 0x0C), // orange
    WIRE_AN,
    Rgba::rgb(0x70, 0x48, 0xE8), // purple
    Rgba::rgb(0x0C, 0xA6, 0x78), // teal
    Rgba::rgb(0xD6, 0x33, 0x6C), // magenta
    Rgba::rgb(0x9C, 0x8A, 0x00), // olive
    Rgba::rgb(0x8B, 0x5A, 0x2B), // brown
    Rgba::rgb(0x10, 0x98, 0xAD), // cyan
    Rgba::rgb(0x5C, 0x94, 0x0D), // lime
];

fn run_color(w: &Wire) -> Rgba {
    if let Some(index) = w.colour {
        return WIRE_PALETTE[index as usize % WIRE_PALETTE.len()];
    }
    match w.class {
        NetClass::Vcc => WIRE_VCC,
        NetClass::Gnd => WIRE_GND,
        // The colour follows the two ends, so the same connection keeps its
        // colour between repaints and exports.
        NetClass::Signal | NetClass::Analog => {
            let h = crate::model::wire_seed(w.a, w.b)
                .wrapping_add(if w.class == NetClass::Analog { 5 } else { 0 });
            WIRE_PALETTE[(h % WIRE_PALETTE.len() as u32) as usize]
        }
    }
}

struct D {
    s: Scene,
    opt: Opts,
    accent: Rgba,
    /// Caption ink, and whether the bench is dark. Both follow the chosen
    /// background so the drawing stays readable on white and on black.
    ink: Rgba,
    on_dark: bool,
    /// The circuit's frame in bench millimetres. Labels are nudged inside it so
    /// a caption for a part on the left edge is never cropped.
    frame: Rect,
    pin_pts: Vec<Pt>,
    owner: Option<Owner>,
}

impl D {
    /// Paint a primitive and remember which object asked for it.
    fn push(&mut self, prim: Prim) {
        self.s.owners.push(self.owner);
        self.s.prims.push(prim);
    }

    fn rrect(&mut self, r: Rect, rad: f32, fill: Rgba) {
        self.push(Prim::Rect {
            r,
            radius: rad,
            fill,
            stroke: None,
        });
    }
    fn rrect_stroke(&mut self, r: Rect, rad: f32, fill: Rgba, stroke: Rgba, w: f32) {
        self.push(Prim::Rect {
            r,
            radius: rad,
            fill,
            stroke: Some((stroke, w)),
        });
    }
    fn circle(&mut self, c: Pt, r: f32, fill: Rgba) {
        self.push(Prim::Circle {
            c,
            r,
            fill,
            stroke: None,
        });
    }
    fn circle_stroke(&mut self, c: Pt, r: f32, fill: Rgba, stroke: Rgba, w: f32) {
        self.push(Prim::Circle {
            c,
            r,
            fill,
            stroke: Some((stroke, w)),
        });
    }
    fn line(&mut self, a: Pt, b: Pt, w: f32, color: Rgba) {
        self.push(Prim::Poly {
            pts: vec![a, b],
            w,
            color,
            closed: false,
        });
    }
    fn poly(&mut self, pts: Vec<Pt>, w: f32, color: Rgba, closed: bool) {
        self.push(Prim::Poly { pts, w, color, closed });
    }
    fn text(&mut self, p: Pt, size: f32, text: impl Into<String>, color: Rgba, anchor: Anchor) {
        self.push(Prim::Text {
            p,
            size,
            text: text.into(),
            color,
            anchor,
            bold: false,
            angle: 0.0,
        });
    }

    fn text_bold(&mut self, p: Pt, size: f32, text: impl Into<String>, color: Rgba, anchor: Anchor) {
        self.push(Prim::Text {
            p,
            size,
            text: text.into(),
            color,
            anchor,
            bold: true,
            angle: 0.0,
        });
    }

    fn text_rot(
        &mut self,
        p: Pt,
        size: f32,
        text: impl Into<String>,
        color: Rgba,
        anchor: Anchor,
        angle: f32,
    ) {
        self.push(Prim::Text {
            p,
            size,
            text: text.into(),
            color,
            anchor,
            bold: false,
            angle,
        });
    }

    /// A half-size breadboard drawn flat, the way a top-down bench photo
    /// reads: light grey body, dark holes, printed rail stripes, silk numbers.
    fn board(&mut self, b: &Board) {
        let r = b.rect();
        self.rrect(r.inflate(0.35), 1.6, BOARD_EDGE);
        self.rrect(r, 1.5, BOARD_TOP);

        // The centre channel is a real gap in the board.
        let ch = Rect::new(
            r.x + 0.6,
            b.row_y(Row::E) + PITCH / 2.0,
            r.w - 1.2,
            b.row_y(Row::F) - b.row_y(Row::E) - PITCH,
        );
        self.rrect(ch, 0.4, BOARD_CHANNEL);

        // Power rails: a red stripe by the plus holes, a blue one by the minus.
        for (rail, col) in [
            (Rail::TopPlus, Rgba::rgb(0xC4, 0x44, 0x40)),
            (Rail::TopMinus, Rgba::rgb(0x3E, 0x60, 0xB4)),
            (Rail::BotMinus, Rgba::rgb(0x3E, 0x60, 0xB4)),
            (Rail::BotPlus, Rgba::rgb(0xC4, 0x44, 0x40)),
        ] {
            let y = b.rail_y(rail) + (if rail == Rail::TopPlus || rail == Rail::BotPlus { -0.85 } else { 0.85 });
            let x0 = b.rail_x(RailHalf::Left, 0) - PITCH / 2.0;
            let x1 = b.rail_x(RailHalf::Right, Board::RAIL_HOLES - 1) + PITCH / 2.0;
            self.line(Pt::new(x0, y), Pt::new(x1, y), 0.48, col.alpha(0.75));
        }

        // Holes: a flat dark disc, no gradient, no shadow.
        for col in 1..=b.cols {
            for row in [
                Row::A, Row::B, Row::C, Row::D, Row::E, Row::F, Row::G, Row::H, Row::I, Row::J,
            ] {
                let p = b.hole(col, row);
                self.circle(p, 0.62, HOLE);
                self.circle(p, 0.38, HOLE_IN);
            }
        }
        for rail in [Rail::TopPlus, Rail::TopMinus, Rail::BotPlus, Rail::BotMinus] {
            for half in [RailHalf::Left, RailHalf::Right] {
                for i in 0..Board::RAIL_HOLES {
                    let p = b.rail_hole(rail, half, i);
                    self.circle(p, 0.62, HOLE);
                    self.circle(p, 0.38, HOLE_IN);
                }
            }
        }

        // Silk: letters down the left edge, column numbers top and bottom.
        for row in [Row::A, Row::B, Row::C, Row::D, Row::E, Row::F, Row::G, Row::H, Row::I, Row::J] {
            let y = b.row_y(row);
            self.text(Pt::new(b.at.x + 0.45, y), 1.05, row.letter(), SILK_DARK, Anchor::Center);
            // The far edge carries the same letters on a real board.
            self.text(
                Pt::new(b.at.x + b.width() - 0.45, y),
                1.05,
                row.letter(),
                SILK_DARK,
                Anchor::Center,
            );
        }
        for col in 1..=b.cols {
            let x = b.col_x(col);
            self.text(Pt::new(x, b.at.y + 0.55), 0.95, col.to_string(), SILK, Anchor::Center);
            self.text(
                Pt::new(x, b.at.y + Board::HEIGHT - 0.55),
                0.95,
                col.to_string(),
                SILK,
                Anchor::Center,
            );
        }
        for rail in [Rail::TopPlus, Rail::TopMinus, Rail::BotPlus, Rail::BotMinus] {
            let sign = if matches!(rail, Rail::TopPlus | Rail::BotPlus) { "+" } else { "–" };
            let y = b.rail_y(rail);
            self.text(Pt::new(b.at.x + 0.45, y), 0.9, sign, Rgba::rgb(0xC4, 0x44, 0x40), Anchor::Center);
            self.text(
                Pt::new(b.at.x + b.width() - 0.45, y),
                0.9,
                sign,
                Rgba::rgb(0xC4, 0x44, 0x40),
                Anchor::Center,
            );
        }
        // The gap under the middle of each rail, printed as a pair of lines.
        for rail in [Rail::TopPlus, Rail::TopMinus, Rail::BotPlus, Rail::BotMinus] {
            let x = b.at.x + b.width() / 2.0;
            let y = b.rail_y(rail);
            self.line(Pt::new(x, y - 1.0), Pt::new(x, y + 1.0), 0.3, SILK);
        }
    }

    /// A top-down Arduino Uno R3, laid out like the real board: teal PCB, USB
    /// and barrel jack hanging off the left edge, the digital headers on top,
    /// the power and analog headers on the bottom, and the silkscreen in
    /// between. Pin sockets are placed by `uno_pin_pos`, so the drawing and the
    /// wiring agree to the millimetre.
    fn arduino(&mut self, part: &Part) {
        let (w, h) = UNO_SIZE;
        let at = part.at;
        let p = |dx: f32, dy: f32| Pt::new(at.x + dx, at.y + dy);
        let r = Rect::new(at.x, at.y, w, h);
        self.rrect(r, 2.0, ARDUINO_PCB);
        self.rrect_stroke(r, 2.0, Rgba::rgba(0, 0, 0, 0), ARDUINO_EDGE, 0.3);

        // Through-holes at the four corners.
        for (dx, dy) in [(15.2, 1.6), (13.8, 53.0), (74.6, 1.6), (74.2, 52.6)] {
            self.circle(p(dx, dy), 1.55, Rgba::rgb(0xEF, 0xEF, 0xEF));
            self.circle(p(dx, dy), 1.15, ARDUINO_PCB.darken(0.18));
        }

        // USB-B socket, sticking out past the left edge.
        self.rrect_stroke(
            Rect::new(at.x - 8.5, at.y + 8.7, 20.0, 11.9),
            0.8,
            USB_METAL,
            USB_METAL.darken(0.28),
            0.3,
        );
        self.rrect(Rect::new(at.x - 1.0, at.y + 9.9, 11.5, 9.5), 0.4, USB_METAL.lighten(0.06));
        self.rrect(Rect::new(at.x - 8.0, at.y + 11.0, 4.2, 7.3), 0.3, USB_METAL.darken(0.22));

        // Barrel jack, also hanging off the left edge.
        self.rrect(Rect::new(at.x - 2.0, at.y + 41.3, 14.5, 9.2), 0.7, PLASTIC);
        self.rrect(Rect::new(at.x - 2.0, at.y + 43.0, 4.0, 5.8), 0.5, Rgba::rgb(0x0E, 0x0E, 0x10));
        self.circle(p(2.2, 45.9), 1.35, Rgba::rgb(0x07, 0x07, 0x09));
        self.rrect(Rect::new(at.x + 9.6, at.y + 42.8, 2.0, 6.2), 0.2, USB_METAL.darken(0.12));

        // Reset button: white cap, red plunger.
        self.rrect_stroke(
            Rect::new(at.x + 3.6, at.y + 0.6, 7.2, 5.8),
            0.55,
            Rgba::rgb(0xE9, 0xE9, 0xE8),
            Rgba::rgb(0xC9, 0xC9, 0xC7),
            0.25,
        );
        self.circle(p(7.2, 3.4), 1.85, Rgba::rgb(0xAE, 0x35, 0x30));
        self.circle(p(6.8, 3.0), 0.65, Rgba::rgb(0xC6, 0x53, 0x4A));
        self.text(p(7.2, 7.2), 0.78, "RESET", SILK_W, Anchor::Center);

        // The four header blocks.
        for (g, group) in UNO_GROUPS.iter().enumerate() {
            self.uno_header(at, group, UNO_GROUP_X[g], g < 2);
        }

        // ICSP2 sits under the first digital sockets, as on the real board.
        self.icsp(p(13.4, 5.0), false);
        self.text(p(16.0, 11.9), 0.72, "ICSP2", SILK_W, Anchor::Center);
        // The second ICSP block is vertical on the right edge.
        self.icsp(p(73.2, 22.2), true);
        self.text_rot(p(71.4, 26.0), 0.72, "ICSP", SILK_W, Anchor::Center, -90.0);

        // Regulator with its metal tab, then the big DIP.
        self.rrect(Rect::new(at.x + 7.8, at.y + 35.3, 6.0, 4.6), 0.25, PLASTIC);
        self.rrect(Rect::new(at.x + 8.6, at.y + 34.5, 4.4, 1.2), 0.15, USB_METAL);
        self.rrect_stroke(
            Rect::new(at.x + 32.2, at.y + 34.4, 42.0, 7.3),
            0.3,
            CHIP,
            Rgba::rgb(0x1A, 0x1A, 0x1D),
            0.2,
        );
        self.circle(p(33.7, 38.0), 0.55, Rgba::rgb(0x12, 0x12, 0x14));
        for i in 0..14 {
            let x = at.x + 33.4 + i as f32 * 2.85;
            self.line(Pt::new(x, at.y + 34.0), Pt::new(x, at.y + 33.3), 0.22, USB_METAL);
            self.line(Pt::new(x, at.y + 42.2), Pt::new(x, at.y + 42.9), 0.22, USB_METAL);
        }

        // Crystal and the two electrolytic capacitors.
        self.rrect_stroke(
            Rect::new(at.x + 24.4, at.y + 26.6, 5.6, 2.9),
            1.4,
            USB_METAL,
            USB_METAL.darken(0.3),
            0.2,
        );
        for cx in [22.1, 29.2] {
            self.circle(p(cx, 46.4), 4.1, Rgba::rgb(0xD8, 0xD8, 0xD8));
            self.circle(p(cx, 47.3), 3.85, Rgba::rgb(0x18, 0x18, 0x1A));
            self.circle(p(cx, 45.6), 3.6, Rgba::rgb(0xE6, 0xE6, 0xE6));
            self.circle(p(cx, 45.6), 2.2, Rgba::rgb(0xF2, 0xF2, 0xF2));
        }

        // LEDs and their labels: L, TX, RX by the USB, ON at the far right.
        for (dy, name, col) in [
            (11.6, "L", Rgba::rgb(0xE7, 0xC4, 0x35)),
            (15.0, "TX", Rgba::rgb(0xE7, 0xC4, 0x35)),
            (18.4, "RX", Rgba::rgb(0xE7, 0xC4, 0x35)),
        ] {
            self.rrect(Rect::new(at.x + 31.6, at.y + dy, 2.0, 1.25), 0.15, col);
            self.text(p(30.6, dy + 0.6), 0.72, name, SILK_W, Anchor::End);
        }
        self.rrect(Rect::new(at.x + 68.3, at.y + 17.7, 2.0, 1.25), 0.15, Rgba::rgb(0x57, 0xC8, 0x5B));
        self.text(p(67.2, 18.3), 0.72, "ON", SILK_W, Anchor::End);

        // Silk captions over the headers.
        self.text(p(44.0, 9.2), 1.15, "DIGITAL (PWM~)", SILK_W, Anchor::Center);
        self.text(p(45.0, 45.2), 0.95, "POWER", SILK_W, Anchor::Center);
        self.text(p(60.0, 45.2), 0.95, "ANALOG IN", SILK_W, Anchor::Center);

        // The Arduino infinity mark and the UNO badge.
        for cx in [38.6, 43.4] {
            self.circle_stroke(p(cx, 17.2), 2.2, Rgba::rgba(0, 0, 0, 0), SILK_W, 0.55);
        }
        self.line(p(37.6, 17.2), p(39.6, 17.2), 0.5, SILK_W);
        self.line(p(42.4, 17.2), p(44.4, 17.2), 0.5, SILK_W);
        self.line(p(43.4, 16.2), p(43.4, 18.2), 0.5, SILK_W);
        self.text_bold(p(41.0, 22.4), 2.35, "ARDUINO", SILK_W, Anchor::Center);
        self.text(p(41.0, 25.4), 1.25, "Arduino™", SILK_W.alpha(0.85), Anchor::Center);
        let badge = p(60.5, 14.6);
        for i in 0..36 {
            let a = i as f32 / 36.0 * std::f32::consts::TAU;
            self.circle(
                Pt::new(badge.x + a.cos() * 4.6, badge.y + a.sin() * 4.2),
                0.18,
                SILK_W,
            );
        }
        self.text_bold(p(60.5, 14.6), 3.1, "UNO", SILK_W, Anchor::Center);
    }

    /// One black header block with gold-ringed sockets and rotated names.
    /// Rotation is a text primitive, so this has to reach `self.s` through the
    /// normal helpers.
    fn uno_header(&mut self, at: Pt, group: &[usize], x0: f32, top: bool) {
        let (_, h) = UNO_SIZE;
        let y = if top { at.y + UNO_EDGE_INSET } else { at.y + h - UNO_EDGE_INSET };
        let x_start = at.x + x0;
        let span = (group.len() - 1) as f32 * PITCH;
        let strip_y = if top { at.y } else { at.y + h - 3.4 };
        self.rrect(Rect::new(x_start - 1.3, strip_y, span + 2.6, 3.4), 0.22, HEADER);
        for (slot, &idx) in group.iter().enumerate() {
            let x = x_start + slot as f32 * PITCH;
            self.rrect(Rect::new(x - 0.95, y - 0.95, 1.9, 1.9), 0.12, HEADER_GOLD);
            self.rrect(Rect::new(x - 0.62, y - 0.62, 1.24, 1.24), 0.1, Rgba::rgb(0x08, 0x08, 0x0A));
            let name = UNO_PINS[idx].0;
            if name == "NC" {
                continue;
            }
            if top {
                // First letter at the socket, reading down into the board.
                self.text_rot(Pt::new(x + 0.3, y + 1.55), 0.8, name, SILK_W, Anchor::Start, 90.0);
            } else {
                // First letter at the socket, reading up into the board.
                self.text_rot(Pt::new(x + 0.3, y - 1.55), 0.8, name, SILK_W, Anchor::Start, -90.0);
            }
        }
    }

    /// A 2×3 ICSP block, either lying flat or standing on the right edge.
    fn icsp(&mut self, at: Pt, vertical: bool) {
        let (dx, dy) = if vertical { (2.0, 0.0) } else { (1.65, 2.3) };
        let (cols, rows) = if vertical { (2, 3) } else { (3, 2) };
        let bg = Rect::new(
            at.x - 1.3,
            at.y - 1.3,
            (cols - 1) as f32 * dx + 2.6,
            (rows - 1) as f32 * dy + 2.6,
        );
        self.rrect(bg, 0.35, HEADER);
        for c in 0..cols {
            for r in 0..rows {
                let p = Pt::new(at.x + c as f32 * dx, at.y + r as f32 * dy);
                self.rrect(Rect::new(p.x - 0.55, p.y - 0.65, 1.1, 1.3), 0.1, HEADER_GOLD);
                self.rrect(Rect::new(p.x - 0.3, p.y - 0.38, 0.6, 0.76), 0.06, Rgba::rgb(0x0A,0x0A,0x0C));
            }
        }
    }

    fn resistor(&mut self, part: &Part) {
        let a = part.pins[0];
        let b = part.pins[1];
        let pa = self.pin_pos(a);
        let pb = self.pin_pos(b);
        // The body lies along the row, between the two legs, the way an axial
        // resistor sits in a breadboard.
        let body = Rect::new(pa.x + 0.7, pa.y - 1.35, (pb.x - pa.x) - 1.4, 2.7);
        self.rrect_stroke(body, 1.25, BEIGE, BEIGE.darken(0.20), 0.14);
        let bands = band_colours(&part.value);
        for (i, col) in bands.iter().enumerate() {
            let x = body.x + 1.05 + i as f32 * (body.w - 2.1) / 3.4;
            self.rrect(Rect::new(x, body.y + 0.12, 0.58, body.h - 0.24), 0.06, *col);
        }
    }

    fn led(&mut self, part: &Part) {
        let n = part.pins.len();
        let pins: Vec<Pt> = part.pins.iter().map(|&p| self.pin_pos(p)).collect();
        let cx = pins.iter().map(|p| p.x).sum::<f32>() / n as f32;
        let base_y = pins[0].y;
        let lens = led_colour(&part.value);
        for p in &pins {
            self.line(Pt::new(p.x, base_y), Pt::new(cx, base_y - 1.5), 0.5, SILVER.darken(0.25));
        }
        // The body sits above the board on its legs.
        let body = Rect::new(cx - 2.3, base_y - 7.4, 4.6, 6.0);
        self.rrect(body, 0.9, lens.darken(0.35));
        self.circle(Pt::new(cx, base_y - 5.4), 2.15, lens);
        self.circle(Pt::new(cx - 0.7, base_y - 6.1), 0.6, lens.lighten(0.55).alpha(0.85));
        let flat = Rect::new(cx - 2.3, base_y - 1.9, 4.6, 0.9);
        self.rrect(flat, 0.2, lens.darken(0.5));
        if n >= 4 {
            self.circle(Pt::new(cx + 1.2, base_y - 3.4), 0.4, lens.lighten(0.3).alpha(0.6));
        }
    }

    fn diode(&mut self, part: &Part) {
        let pa = self.pin_pos(part.pins[0]);
        let pk = self.pin_pos(part.pins[1]);
        let mid = pa.mid(pk);
        let body = Rect::new(mid.x - 3.1, mid.y - 1.35, 6.2, 2.7);
        self.line(pa, Pt::new(body.x, mid.y), 0.55, SILVER.darken(0.2));
        self.line(pk, Pt::new(body.right(), mid.y), 0.55, SILVER.darken(0.2));
        self.rrect_stroke(body, 1.2, PLASTIC, PLASTIC.lighten(0.25), 0.16);
        self.rrect(Rect::new(body.right() - 1.1, body.y, 1.0, body.h), 0.1, SILVER);
    }

    /// A ceramic disc capacitor: a small glazed blob on two short legs.
    fn capacitor(&mut self, part: &Part) {
        let a = self.pin_pos(part.pins[0]);
        let b = self.pin_pos(part.pins[1]);
        let cx = (a.x + b.x) / 2.0;
        let y = a.y;
        for p in [a, b] {
            self.line(p, Pt::new(p.x, y - 2.0), 0.42, SILVER.darken(0.25));
        }
        let body = Rect::new(cx - 2.5, y - 5.6, 5.0, 3.7);
        self.rrect_stroke(
            body,
            1.75,
            Rgba::rgb(0xD2, 0x86, 0x33),
            Rgba::rgb(0x9E, 0x5C, 0x1B),
            0.16,
        );
        // A glazed highlight along the top, then the printed value.
        self.rrect(
            Rect::new(body.x + 0.9, body.y + 0.4, body.w - 1.8, 0.9),
            0.45,
            Rgba::rgb(0xEE, 0xB4, 0x6A).alpha(0.75),
        );
        self.text(
            Pt::new(cx, body.bottom() - 0.65),
            0.52,
            part.value.split_whitespace().next().unwrap_or(""),
            Rgba::rgb(0x46, 0x27, 0x0B),
            Anchor::Center,
        );
    }

    /// A radial electrolytic: a blue can, a silver top and the pale stripe
    /// that marks the negative side.
    fn electrolytic(&mut self, part: &Part) {
        let a = self.pin_pos(part.pins[0]);
        let b = self.pin_pos(part.pins[1]);
        let cx = (a.x + b.x) / 2.0;
        let y = a.y;
        let body = Rect::new(cx - 3.1, y - 8.6, 6.2, 6.4);
        for p in [a, b] {
            self.line(p, Pt::new(p.x, body.bottom()), 0.45, SILVER.darken(0.2));
        }
        self.rrect(body, 1.0, Rgba::rgb(0x25, 0x39, 0x66));
        self.rrect(
            Rect::new(body.x + 0.3, body.y + 0.25, body.w - 0.6, 1.2),
            0.55,
            SILVER.lighten(0.10),
        );
        // The negative stripe runs down one side, with the can's vent on top.
        self.rrect(
            Rect::new(body.right() - 2.2, body.y + 1.4, 1.6, body.h - 1.9),
            0.25,
            Rgba::rgb(0xE6, 0xE6, 0xE0).alpha(0.9),
        );
        self.line(
            Pt::new(body.right() - 2.0, body.y + 3.0),
            Pt::new(body.right() - 0.9, body.y + 3.0),
            0.35,
            Rgba::rgb(0x25, 0x39, 0x66),
        );
        self.text(
            Pt::new(body.x + 1.1, body.y + 2.7),
            0.6,
            "+",
            SILVER.lighten(0.5),
            Anchor::Center,
        );
        self.text(
            Pt::new(cx - 0.5, body.bottom() - 1.0),
            0.55,
            part.value.split_whitespace().next().unwrap_or(""),
            SILVER.lighten(0.35),
            Anchor::Center,
        );
    }

    /// A glass reed switch: a clear tube with two overlapping leaves.
    fn reed(&mut self, part: &Part) {
        let pa = self.pin_pos(part.pins[0]);
        let pb = self.pin_pos(part.pins[1]);
        let mid = pa.mid(pb);
        let body = Rect::new(mid.x - 7.2, mid.y - 1.5, 14.4, 3.0);
        self.line(pa, Pt::new(body.x, mid.y), 0.5, SILVER.darken(0.2));
        self.line(pb, Pt::new(body.right(), mid.y), 0.5, SILVER.darken(0.2));
        self.rrect_stroke(
            body,
            1.5,
            Rgba::rgb(0xD8, 0xE2, 0xE8).alpha(0.55),
            Rgba::rgb(0x8E, 0x9A, 0xA2),
            0.18,
        );
        self.line(Pt::new(body.x + 2.2, mid.y), Pt::new(mid.x - 0.5, mid.y), 0.28, SILVER);
        self.line(Pt::new(body.right() - 2.2, mid.y), Pt::new(mid.x + 0.5, mid.y), 0.28, SILVER);
        self.circle(Pt::new(mid.x, mid.y), 0.4, Rgba::rgb(0xE8, 0xD8, 0x9A));
    }

    /// A thermistor bead: a small epoxy drop on two legs.
    fn thermistor(&mut self, part: &Part) {
        let pa = self.pin_pos(part.pins[0]);
        let pb = self.pin_pos(part.pins[1]);
        let cx = (pa.x + pb.x) / 2.0;
        let y = pa.y;
        for p in [pa, pb] {
            self.line(p, Pt::new(p.x, y - 2.2), 0.42, SILVER.darken(0.25));
        }
        let body = Rect::new(cx - 2.2, y - 7.2, 4.4, 5.4);
        self.rrect_stroke(
            body,
            2.2,
            Rgba::rgb(0x1E, 0x22, 0x28),
            Rgba::rgb(0x0C, 0x0E, 0x12),
            0.18,
        );
        self.circle(Pt::new(cx - 0.7, y - 5.6), 0.7, Rgba::rgb(0x5A, 0x62, 0x6E).alpha(0.7));
        self.text(
            Pt::new(cx, y - 1.4),
            0.52,
            part.value.split_whitespace().next().unwrap_or(""),
            SILVER.lighten(0.4),
            Anchor::Center,
        );
    }

    /// A Peltier element: two white ceramic plates with a row of pellets
    /// showing at the edge, and the cold side marked.
    fn peltier(&mut self, part: &Part) {
        let at = part.at;
        let body = Rect::new(at.x, at.y, 20.0, 20.0);
        self.rrect_stroke(body, 0.6, Rgba::rgb(0xE6, 0xE6, 0xDE), Rgba::rgb(0xA8, 0xA8, 0xA0), 0.22);
        let inner = Rect::new(at.x + 1.6, at.y + 1.6, 16.8, 16.8);
        self.rrect_stroke(inner, 0.4, Rgba::rgb(0xD2, 0xD2, 0xC8), Rgba::rgb(0xB0, 0xB0, 0xA6), 0.16);
        for row in 0..5 {
            for col in 0..5 {
                let p = Pt::new(
                    at.x + 3.2 + col as f32 * 3.4,
                    at.y + 3.2 + row as f32 * 3.4,
                );
                self.circle(p, 1.0, Rgba::rgb(0xB8, 0x8A, 0x4A));
                self.circle(p, 0.45, Rgba::rgb(0x8E, 0x66, 0x32));
            }
        }
        self.text(
            Pt::new(at.x + 10.0, at.y + 18.6),
            0.7,
            "TEC",
            Rgba::rgb(0x40, 0x48, 0x54),
            Anchor::Center,
        );
    }

    fn to92(&mut self, part: &Part) {
        let pins: Vec<Pt> = part.pins.iter().map(|&p| self.pin_pos(p)).collect();
        let cx = pins.iter().map(|p| p.x).sum::<f32>() / pins.len() as f32;
        let y = pins[0].y;
        for p in &pins {
            self.line(*p, Pt::new(p.x, y - 2.0), 0.45, SILVER.darken(0.25));
        }
        let body = Rect::new(cx - 2.9, y - 7.3, 5.8, 5.4);
        self.rrect_stroke(body, 1.6, PLASTIC, PLASTIC.lighten(0.3), 0.14);
        self.rrect(Rect::new(body.x + 4.6, body.y + 0.4, 1.1, body.h - 0.8), 0.1, PLASTIC.lighten(0.12));
        self.text(
            Pt::new(cx, body.center().y),
            0.72,
            part.value.split_whitespace().next().unwrap_or(""),
            SILVER.lighten(0.3),
            Anchor::Center,
        );
    }

    fn button(&mut self, part: &Part) {
        let pins: Vec<Pt> = part.pins.iter().map(|&p| self.pin_pos(p)).collect();
        let minx = pins.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        let maxx = pins.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        let miny = pins.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let maxy = pins.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let cx = (minx + maxx) / 2.0;
        let cy = (miny + maxy) / 2.0;
        for p in &pins {
            let far = if p.x < cx { minx } else { maxx };
            self.line(*p, Pt::new(far, cy), 0.42, SILVER.darken(0.3));
        }
        let body = Rect::new(cx - 3.4, cy - 3.4, 6.8, 6.8);
        self.rrect_stroke(body, 0.7, BLUE, BLUE.darken(0.35), 0.16);
        self.circle(Pt::new(cx, cy), 2.5, Rgba::rgb(0xDE, 0xE2, 0xE8));
        self.circle(Pt::new(cx, cy), 1.25, Rgba::rgb(0xC3, 0xC8, 0xD0));
    }

    fn ldr(&mut self, part: &Part) {
        let pa = self.pin_pos(part.pins[0]);
        let pb = self.pin_pos(part.pins[1]);
        let cx = (pa.x + pb.x) / 2.0;
        let y = pa.y;
        self.line(pa, Pt::new(pa.x, y - 2.2), 0.45, SILVER.darken(0.25));
        self.line(pb, Pt::new(pb.x, y - 2.2), 0.45, SILVER.darken(0.25));
        let body = Rect::new(cx - 2.4, y - 7.6, 4.8, 5.6);
        self.rrect_stroke(body, 2.2, Rgba::rgb(0xF0, 0xE6, 0xC9), Rgba::rgb(0xC9, 0xB9, 0x93), 0.18);
        let mut zig = Vec::new();
        for i in 0..9 {
            let t = i as f32 / 8.0;
            zig.push(Pt::new(
                body.x + 0.8 + t * (body.w - 1.6),
                body.y + body.h / 2.0 + if i % 2 == 0 { -1.1 } else { 1.1 },
            ));
        }
        self.poly(zig, 0.42, Rgba::rgb(0xC8, 0x5A, 0x28), false);
    }

    /// A hobby gearmotor seen from above: grey can, silver end cap, and the
    /// yellow gear on the output shaft.
    fn motor(&mut self, part: &Part) {
        let at = part.at;
        let body = Rect::new(at.x, at.y, 24.0, 14.0);
        self.rrect_stroke(body, 6.5, GREY, GREY.darken(0.30), 0.22);
        self.rrect(Rect::new(at.x + 1.2, at.y + 1.0, 21.0, 5.0), 3.6, GREY.lighten(0.20).alpha(0.8));
        let cap = Rect::new(at.x + 19.4, at.y + 1.0, 4.0, 12.0);
        self.rrect(cap, 1.8, GREY.darken(0.20));
        // Output shaft and gear.
        let gear = Pt::new(at.x + 3.4, at.y + 7.0);
        for i in 0..12 {
            let a = i as f32 * std::f32::consts::TAU / 12.0;
            let p = Pt::new(gear.x + a.cos() * 3.9, gear.y + a.sin() * 3.9);
            self.circle(p, 1.15, Rgba::rgb(0xE3, 0xB4, 0x2A));
        }
        self.circle(gear, 3.7, Rgba::rgb(0xF0, 0xC4, 0x38));
        self.circle(gear, 1.5, Rgba::rgb(0x3A, 0x3A, 0x3C));
        self.circle(gear, 0.7, Rgba::rgb(0xDD, 0xDD, 0xD8));
        // Terminals hang out of the gear end, two short legs.
        for (i, (_, dx, dy)) in bench_pins(Kind::Motor, "").iter().enumerate() {
            let p = Pt::new(at.x + dx, at.y + dy);
            self.line(Pt::new(p.x, body.bottom() - 0.4), p, 0.42, SILVER.darken(0.15));
            self.rrect(
                Rect::new(p.x - 1.0, p.y - 1.4, 2.0, 2.8),
                0.3,
                if i == 0 { WIRE_VCC } else { PLASTIC },
            );
        }
    }

    fn servo(&mut self, part: &Part) {
        let at = part.at;
        let body = Rect::new(at.x, at.y, 18.0, 24.0);
        self.rrect_stroke(body, 1.2, BLUE, BLUE.darken(0.35), 0.25);
        self.rrect(Rect::new(at.x + 1.2, at.y + 1.2, 15.6, 6.0), 0.8, BLUE.lighten(0.12));
        // Horn.
        self.rrect(Rect::new(at.x + 2.0, at.y + 9.0, 14.0, 3.0), 1.4, Rgba::rgb(0xE8, 0xEA, 0xEE));
        self.circle(Pt::new(at.x + 9.0, at.y + 10.5), 1.7, Rgba::rgb(0xCF, 0xD3, 0xD8));
        for (i, (name, dx, dy)) in bench_pins(Kind::Servo, "").iter().enumerate() {
            let p = Pt::new(at.x + dx, at.y + dy);
            let col = match *name {
                "+" => WIRE_VCC,
                "-" => PLASTIC,
                _ => Rgba::rgb(0xE0, 0x7A, 0x1F),
            };
            self.rrect(Rect::new(p.x - 0.9, p.y - 1.6, 1.8, 3.4), 0.3, col);
            self.text(Pt::new(p.x, p.y + 3.0), 0.8, i.to_string(), SILVER, Anchor::Center);
        }
    }

    fn buzzer(&mut self, part: &Part) {
        let at = part.at;
        let c = Pt::new(at.x + 8.0, at.y + 5.0);
        self.circle_stroke(c, 7.0, PLASTIC, PLASTIC.lighten(0.3), 0.3);
        self.circle(c, 2.0, Rgba::rgb(0x0c, 0x0c, 0x0d));
        self.circle(Pt::new(c.x - 2.2, c.y - 2.2), 1.2, PLASTIC.lighten(0.2).alpha(0.6));
        for (i, (_, dx, dy)) in bench_pins(part.kind, &part.value).iter().enumerate() {
            let p = Pt::new(at.x + dx, at.y + dy);
            self.rrect(
                Rect::new(p.x - 1.0, p.y - 1.5, 2.0, 3.2),
                0.35,
                if i == 0 { PLASTIC } else { WIRE_VCC },
            );
        }
    }

    fn lamp(&mut self, part: &Part) {
        let at = part.at;
        let c = Pt::new(at.x + 8.0, at.y + 9.0);
        // The glass bulb, then the filament drawn as the usual cross.
        self.circle_stroke(
            c,
            7.0,
            Rgba::rgb(0xF6, 0xE7, 0xB8).alpha(0.65),
            Rgba::rgb(0xB8, 0x8A, 0x2E),
            0.35,
        );
        for r in [4.6, 2.2] {
            let k = r * std::f32::consts::FRAC_1_SQRT_2;
            self.line(
                Pt::new(c.x - k, c.y - k),
                Pt::new(c.x + k, c.y + k),
                0.35,
                Rgba::rgb(0x8A, 0x66, 0x1E),
            );
            self.line(
                Pt::new(c.x - k, c.y + k),
                Pt::new(c.x + k, c.y - k),
                0.35,
                Rgba::rgb(0x8A, 0x66, 0x1E),
            );
        }
        // A screw base above the bulb: two ridges and stems to the terminals.
        for i in 0..2 {
            let y = at.y + 0.2 + i as f32 * 1.6;
            self.line(Pt::new(at.x + 4.0, y), Pt::new(at.x + 12.0, y), 0.3, SILVER);
        }
        for dx in [5.0, 11.0] {
            self.line(
                Pt::new(at.x + dx, at.y - 3.0),
                Pt::new(at.x + dx, at.y + 1.0),
                0.35,
                SILVER.darken(0.15),
            );
        }
        for (i, (_, dx, dy)) in bench_pins(part.kind, &part.value).iter().enumerate() {
            let p = Pt::new(at.x + dx, at.y + dy);
            self.rrect(
                Rect::new(p.x - 1.0, p.y - 1.5, 2.0, 3.2),
                0.35,
                if i == 0 { PLASTIC } else { WIRE_VCC },
            );
        }
    }

    fn speaker(&mut self, part: &Part) {
        let at = part.at;
        let c = Pt::new(at.x + 8.0, at.y + 6.0);
        self.circle_stroke(c, 8.0, Rgba::rgb(0x3A, 0x3C, 0x40), Rgba::rgb(0x22, 0x23, 0x26), 0.4);
        for r in [6.0, 4.2, 2.4] {
            self.circle_stroke(c, r, Rgba::rgba(0, 0, 0, 0), Rgba::rgb(0x55, 0x58, 0x5d), 0.35);
        }
        self.circle(c, 1.2, Rgba::rgb(0x1a, 0x1b, 0x1d));
        for (i, (_, dx, dy)) in bench_pins(part.kind, &part.value).iter().enumerate() {
            let p = Pt::new(at.x + dx, at.y + dy);
            self.rrect(
                Rect::new(p.x - 1.0, p.y - 1.5, 2.0, 3.2),
                0.35,
                if i == 0 { PLASTIC } else { WIRE_VCC },
            );
        }
    }

    fn relay(&mut self, part: &Part) {
        let at = part.at;
        let pcb = Rect::new(at.x, at.y, 34.0, 22.0);
        self.rrect_stroke(pcb, 0.7, Rgba::rgb(0x1D, 0x4F, 0xA8), Rgba::rgb(0x14, 0x3A, 0x7C), 0.25);
        self.rrect_stroke(
            Rect::new(at.x + 2.0, at.y + 2.0, 21.0, 15.0),
            0.5,
            Rgba::rgb(0x2A, 0x2B, 0x30),
            Rgba::rgb(0x15, 0x16, 0x19),
            0.3,
        );
        self.rrect(Rect::new(at.x + 3.4, at.y + 3.4, 18.0, 12.0), 0.4, Rgba::rgb(0x35, 0x37, 0x3d));
        self.rrect(Rect::new(at.x + 26.0, at.y + 3.0, 6.0, 16.0), 0.5, Rgba::rgb(0x27, 0x8A, 0x3E));
        for i in 0..3 {
            self.circle(Pt::new(at.x + 29.0, at.y + 6.0 + i as f32 * 5.0), 0.9, SILVER.darken(0.1));
        }
        for (i, (_, dx, dy)) in bench_pins(Kind::Relay, "").iter().enumerate() {
            let p = Pt::new(at.x + dx, at.y + dy);
            self.rrect(Rect::new(p.x - 0.9, p.y - 1.6, 1.8, 3.4), 0.3, PLASTIC);
            self.text(Pt::new(p.x + 1.6, p.y + 1.2), 0.7, bench_pins(Kind::Relay, "")[i].0, SILVER.lighten(0.4), Anchor::Start);
        }
    }

    fn pot(&mut self, part: &Part) {
        let at = part.at;
        let body = Rect::new(at.x, at.y, 12.0, 12.0);
        self.rrect_stroke(body, 0.9, BLUE, BLUE.darken(0.35), 0.22);
        self.circle(Pt::new(at.x + 6.0, at.y + 5.4), 4.0, Rgba::rgb(0xDC, 0xDF, 0xE3));
        self.line(Pt::new(at.x + 6.0, at.y + 5.4), Pt::new(at.x + 6.0, at.y + 2.4), 0.8, Rgba::rgb(0x44, 0x46, 0x4A));
        for (i, (_, dx, dy)) in bench_pins(Kind::Pot, "").iter().enumerate() {
            let p = Pt::new(at.x + dx, at.y + dy);
            self.line(Pt::new(p.x, at.y + 12.0), p, 0.45, SILVER.darken(0.2));
            self.text(Pt::new(p.x, p.y + 2.4), 0.9, (i + 1).to_string(), self.ink.lighten(0.2), Anchor::Center);
        }
    }

    /// A ready-made board from the course book. Every module shares a PCB,
    /// four mounting holes and a header; the look adds the details that make
    /// an L298N recognisable from an RFID reader.
    fn module(&mut self, part: &Part) {
        let Some(m) = crate::modules::find(&part.value) else {
            let r = Rect::new(part.at.x, part.at.y, 30.0, 20.0);
            self.rrect_stroke(r, 0.8, GREY, GREY.darken(0.3), 0.25);
            return;
        };
        let at = part.at;
        let (w, h) = (m.w, m.h);
        let base = Rgba::rgb(m.colour.0, m.colour.1, m.colour.2);
        self.rrect_stroke(
            Rect::new(at.x, at.y, w, h),
            0.9,
            base,
            base.darken(0.35),
            0.25,
        );
        for (x, y) in [(2.4, 2.4), (w - 2.4, 2.4), (2.4, h - 2.4), (w - 2.4, h - 2.4)] {
            let c = Pt::new(at.x + x, at.y + y);
            self.circle(c, 1.0, SILVER.darken(0.35));
            self.circle(c, 0.5, base.darken(0.55));
        }
        match m.look {
            Look::Driver if w > 40.0 => {
                // L298N: finned heatsink and three screw terminals.
                let hs = Rect::new(at.x + 13.2, at.y + 3.6, 16.4, 11.6);
                self.rrect_stroke(hs, 0.4, SILVER, SILVER.darken(0.35), 0.2);
                for i in 1..6 {
                    let x = hs.x + hs.w * i as f32 / 6.0;
                    self.line(
                        Pt::new(x, hs.y + 0.8),
                        Pt::new(x, hs.bottom() - 0.8),
                        0.22,
                        SILVER.darken(0.28),
                    );
                }
                for (i, y) in [9.0, 20.0, 31.0].iter().enumerate() {
                    let term = Rect::new(at.x + 2.2, at.y + y - 3.2, 9.0, 6.4);
                    self.rrect_stroke(term, 0.5, BLUE, BLUE.darken(0.4), 0.2);
                    self.circle(Pt::new(term.x + 4.5, term.y + term.h / 2.0), 1.8, SILVER.darken(0.15));
                    self.line(
                        Pt::new(term.x + 3.2, term.y + term.h / 2.0),
                        Pt::new(term.x + 5.8, term.y + term.h / 2.0),
                        0.5,
                        SILVER.darken(0.45),
                    );
                    let _ = i;
                }
                self.text(
                    Pt::new(at.x + w * 0.72, at.y + h * 0.74),
                    1.15,
                    "L298N",
                    Rgba::rgb(0xF0, 0xE6, 0xD6),
                    Anchor::Center,
                );
            }
            Look::Driver => {
                // L293D: a DIP-16 in the middle of a small board.
                let chip = Rect::new(at.x + w * 0.26, at.y + 4.2, w * 0.48, h * 0.42);
                self.rrect_stroke(chip, 0.3, PLASTIC, PLASTIC.lighten(0.15), 0.18);
                for i in 0..8 {
                    let y = chip.y + 1.4 + i as f32 * (chip.h - 2.8) / 7.0;
                    for x in [chip.x - 1.1, chip.right()] {
                        self.line(Pt::new(x, y), Pt::new(x + 1.1, y), 0.5, SILVER);
                    }
                }
                self.circle(Pt::new(chip.x + 1.8, chip.y + chip.h / 2.0), 0.7, base.darken(0.4));
            }
            Look::Stepper => {
                let c = Pt::new(at.x + w / 2.0, at.y + h / 2.0 - 1.0);
                let r = w.min(h) / 2.0 - 3.4;
                self.circle_stroke(c, r, SILVER, SILVER.darken(0.4), 0.35);
                self.circle_stroke(c, r * 0.72, Rgba::rgba(0, 0, 0, 0), SILVER.darken(0.25), 0.22);
                self.circle(c, 2.0, SILVER.darken(0.45));
                self.circle(c, 0.9, PLASTIC);
                for i in 0..3 {
                    let a = i as f32 * std::f32::consts::TAU / 3.0;
                    let p = Pt::new(c.x + a.cos() * r * 0.86, c.y + a.sin() * r * 0.86);
                    self.circle(p, 0.75, SILVER.darken(0.5));
                }
                self.text(
                    Pt::new(c.x, at.y + h - 2.2),
                    1.0,
                    "28BYJ-48",
                    SILVER.darken(0.55),
                    Anchor::Center,
                );
            }
            Look::Rfid => {
                // The printed coil is what makes an RC522 look like itself.
                for i in 0..5 {
                    let inset = 2.2 + i as f32 * 1.5;
                    let r = Rect::new(
                        at.x + inset,
                        at.y + inset,
                        w - inset * 2.0,
                        h - inset * 2.0 - 5.0,
                    );
                    self.rrect_stroke(r, 1.0, Rgba::rgba(0, 0, 0, 0), Rgba::rgb(0xC8, 0xA8, 0x5A), 0.28);
                }
                let chip = Rect::new(at.x + w / 2.0 - 3.6, at.y + h - 17.0, 7.2, 5.0);
                self.rrect_stroke(chip, 0.2, PLASTIC, PLASTIC.lighten(0.2), 0.15);
                self.circle(Pt::new(at.x + w / 2.0, at.y + h - 14.5), 0.8, SILVER.darken(0.2));
            }
            Look::Radio => {
                // A whip antenna and a small transceiver chip.
                let ax = at.x + w * 0.72;
                self.line(Pt::new(ax, at.y + 1.0), Pt::new(ax, at.y - 7.0), 0.35, SILVER);
                self.circle(Pt::new(ax, at.y - 7.0), 0.45, SILVER.lighten(0.3));
                let chip = Rect::new(at.x + w * 0.18, at.y + h * 0.22, w * 0.30, h * 0.38);
                self.rrect_stroke(chip, 0.2, PLASTIC, PLASTIC.lighten(0.18), 0.15);
                let xtal = Rect::new(at.x + w * 0.58, at.y + h * 0.28, 4.4, 2.6);
                self.rrect_stroke(xtal, 1.2, SILVER, SILVER.darken(0.3), 0.15);
            }
            Look::Lcd => {
                let screen = Rect::new(at.x + 3.0, at.y + 2.6, w - 6.0, h - 6.0);
                self.rrect_stroke(screen, 0.6, Rgba::rgb(0x9A, 0xC8, 0x5A), Rgba::rgb(0x3C, 0x5A, 0x28), 0.25);
                for row in 0..2 {
                    for col in 0..16 {
                        let cell = Rect::new(
                            screen.x + 1.8 + col as f32 * (screen.w - 3.6) / 16.0,
                            screen.y + 1.1 + row as f32 * (screen.h - 2.2) / 2.0,
                            (screen.w - 3.6) / 16.0 - 0.5,
                            (screen.h - 2.2) / 2.0 - 0.6,
                        );
                        self.rrect(cell, 0.2, Rgba::rgb(0x6E, 0x9C, 0x3E).alpha(0.55));
                    }
                }
            }
            Look::Board => {
                // A blue breakout: DIP chip, crystal and a pin-1 dot.
                let chip = Rect::new(at.x + w * 0.26, at.y + h * 0.30, w * 0.32, h * 0.36);
                self.rrect_stroke(chip, 0.25, PLASTIC, PLASTIC.lighten(0.2), 0.18);
                for i in 0..7 {
                    let y = chip.y + 1.2 + i as f32 * (chip.h - 2.4) / 6.0;
                    for x in [chip.x - 1.0, chip.right()] {
                        self.line(Pt::new(x, y), Pt::new(x + 1.0, y), 0.45, SILVER);
                    }
                }
                let xtal = Rect::new(at.x + w * 0.66, at.y + h * 0.34, 5.0, 3.0);
                self.rrect_stroke(xtal, 1.4, SILVER, SILVER.darken(0.3), 0.15);
                self.circle(Pt::new(at.x + w * 0.18, at.y + 4.0), 0.8, Rgba::rgb(0xE8, 0xDC, 0xB0));
            }
            Look::Sensor => {
                // A small breakout: a metal can or a plastic dome, a trimmer
                // and the usual pair of mounting holes. The exact shape
                // follows the size, so a DHT and an MQ still read apart.
                if h >= 28.0 {
                    // Gas sensor: a round mesh cap over the heater.
                    let c = Pt::new(at.x + w / 2.0, at.y + h * 0.42);
                    let r = w.min(h) * 0.30;
                    self.circle_stroke(c, r, SILVER.darken(0.15), SILVER.darken(0.45), 0.3);
                    for i in 1..4 {
                        let rr = r * i as f32 / 4.0;
                        self.circle_stroke(c, rr, Rgba::rgba(0, 0, 0, 0), SILVER.darken(0.35), 0.16);
                    }
                    self.circle(c, r * 0.18, PLASTIC);
                    let trim = Rect::new(at.x + w - 9.0, at.y + h - 12.0, 6.0, 4.0);
                    self.rrect_stroke(trim, 0.5, BLUE, BLUE.darken(0.4), 0.15);
                    self.line(
                        Pt::new(trim.x + 1.6, trim.y + 2.0),
                        Pt::new(trim.x + 4.4, trim.y + 2.0),
                        0.3,
                        SILVER,
                    );
                } else {
                    // A sensor dome or grille above a tiny chip.
                    let c = Pt::new(at.x + w * 0.36, at.y + h * 0.42);
                    let r = w.min(h) * 0.24;
                    self.circle_stroke(c, r, PLASTIC.lighten(0.18), PLASTIC.darken(0.3), 0.25);
                    self.circle(Pt::new(c.x - r * 0.25, c.y - r * 0.3), r * 0.28, Rgba::rgb(0x8A, 0x8E, 0x96));
                    let chip = Rect::new(at.x + w * 0.60, at.y + h * 0.28, w * 0.24, h * 0.30);
                    self.rrect_stroke(chip, 0.2, PLASTIC, PLASTIC.lighten(0.2), 0.12);
                    let trim = Rect::new(at.x + w * 0.62, at.y + h * 0.68, 4.6, 2.8);
                    self.rrect_stroke(trim, 0.4, BLUE, BLUE.darken(0.4), 0.12);
                }
                self.text(
                    Pt::new(at.x + w * 0.30, at.y + h - 2.0),
                    0.62,
                    part.value.as_str(),
                    SILVER.lighten(0.35),
                    Anchor::Center,
                );
            }
            Look::Display => {
                // A glass panel with a dark bezel and a ribbon or header.
                let screen = Rect::new(at.x + 4.0, at.y + 3.5, w - 8.0, h - 10.0);
                self.rrect_stroke(
                    screen,
                    0.5,
                    Rgba::rgb(0x0B, 0x12, 0x1E),
                    Rgba::rgb(0x30, 0x38, 0x48),
                    0.2,
                );
                self.rrect(
                    Rect::new(screen.x + 1.2, screen.y + 1.0, screen.w - 2.4, screen.h * 0.32),
                    0.2,
                    Rgba::rgb(0x2A, 0x48, 0x78).alpha(0.55),
                );
                // A couple of lit pixels, so the panel is not a black hole.
                for (x, y, col) in [
                    (0.22, 0.30, Rgba::rgb(0x63, 0xD2, 0xE8)),
                    (0.52, 0.42, Rgba::rgb(0xEB, 0xC2, 0x54)),
                    (0.74, 0.66, Rgba::rgb(0x66, 0xD0, 0x7A)),
                ] {
                    self.circle(
                        Pt::new(screen.x + screen.w * x, screen.y + screen.h * y),
                        0.55,
                        col.alpha(0.8),
                    );
                }
                self.text(
                    Pt::new(at.x + w / 2.0, at.y + h - 2.6),
                    0.75,
                    part.value.as_str(),
                    SILVER.lighten(0.4),
                    Anchor::Center,
                );
            }
            Look::Segments => {
                let digits = if w > 36.0 { 4 } else { 1 };
                let gap = 1.2;
                let dw = (w - 3.0 - gap * (digits as f32 - 1.0)) / digits as f32;
                for d in 0..digits {
                    let x = at.x + 1.5 + d as f32 * (dw + gap);
                    let panel = Rect::new(x, at.y + 2.0, dw, h - 4.0);
                    self.rrect(panel, 0.35, Rgba::rgb(0x14, 0x15, 0x19));
                    let lit = if d == 0 { 0b0111_1110 } else { 0b0101_1011 };
                    for (i, (sx, sy, sw, sh)) in [
                        (0.18, 0.04, 0.64, 0.10),
                        (0.74, 0.12, 0.10, 0.36),
                        (0.74, 0.52, 0.10, 0.36),
                        (0.18, 0.86, 0.64, 0.10),
                        (0.08, 0.52, 0.10, 0.36),
                        (0.08, 0.12, 0.10, 0.36),
                        (0.18, 0.45, 0.64, 0.10),
                    ]
                    .iter()
                    .enumerate()
                    {
                        let on = lit & (1 << i) != 0;
                        let col = if on {
                            Rgba::rgb(0xF0, 0x53, 0x3A)
                        } else {
                            Rgba::rgb(0x40, 0x18, 0x14)
                        };
                        self.rrect(
                            Rect::new(
                                panel.x + panel.w * sx,
                                panel.y + panel.h * sy,
                                panel.w * sw,
                                panel.h * sh,
                            ),
                            0.12,
                            col,
                        );
                    }
                }
            }
            Look::Matrix => {
                let panel = Rect::new(at.x + 2.0, at.y + 2.0, w - 4.0, h - 4.0);
                self.rrect(panel, 0.4, Rgba::rgb(0x14, 0x15, 0x19));
                for row in 0..8 {
                    for col in 0..8 {
                        let on = (row + col) % 5 == 0;
                        self.circle(
                            Pt::new(
                                panel.x + panel.w * (col as f32 + 0.5) / 8.0,
                                panel.y + panel.h * (row as f32 + 0.5) / 8.0,
                            ),
                            panel.w.min(panel.h) * 0.040,
                            if on { Rgba::rgb(0xE8, 0x4A, 0x38) } else { Rgba::rgb(0x38, 0x1A, 0x18) },
                        );
                    }
                }
            }
            Look::Panel => {
                let panel = Rect::new(at.x + 2.0, at.y + 2.0, w - 4.0, h - 4.0);
                self.rrect(panel, 0.4, Rgba::rgb(0x0E, 0x0F, 0x12));
                let rgb = part.value.to_lowercase().contains("rgb");
                for row in 0..8 {
                    for col in 0..16 {
                        let on = (row * 3 + col) % 7 < 2;
                        let colour = if !rgb {
                            Rgba::rgb(0xE8, 0x4A, 0x38)
                        } else {
                            match col % 3 {
                                0 => Rgba::rgb(0xE8, 0x4A, 0x38),
                                1 => Rgba::rgb(0x4F, 0xD0, 0x5C),
                                _ => Rgba::rgb(0x46, 0x86, 0xEE),
                            }
                        };
                        self.circle(
                            Pt::new(
                                panel.x + panel.w * (col as f32 + 0.5) / 16.0,
                                panel.y + panel.h * (row as f32 + 0.5) / 8.0,
                            ),
                            panel.w.min(panel.h) * 0.030,
                            if on { colour } else { colour.alpha(0.16) },
                        );
                    }
                }
            }
            Look::Ring => {
                let c = Pt::new(at.x + w / 2.0, at.y + h / 2.0);
                let r = w.min(h) / 2.0 - 2.2;
                self.circle_stroke(c, r, Rgba::rgb(0x14, 0x15, 0x19), Rgba::rgb(0x33, 0x36, 0x3C), 0.4);
                for i in 0..12 {
                    let a = i as f32 * std::f32::consts::TAU / 12.0;
                    let p = Pt::new(c.x + a.cos() * (r - 1.6), c.y + a.sin() * (r - 1.6));
                    let col = match i % 3 {
                        0 => Rgba::rgb(0xE8, 0x4A, 0x38),
                        1 => Rgba::rgb(0x4F, 0xD0, 0x5C),
                        _ => Rgba::rgb(0x46, 0x86, 0xEE),
                    };
                    self.circle(p, 1.15, col);
                }
                self.circle(c, 2.4, PLASTIC);
                self.circle(c, 1.0, SILVER.darken(0.3));
            }
            Look::Keypad => {
                // A PS/2 socket: a round DIN connector on a small PCB.
                let c = Pt::new(at.x + w * 0.42, at.y + h * 0.46);
                self.circle_stroke(c, w.min(h) * 0.30, Rgba::rgb(0x28, 0x2A, 0x30), SILVER, 0.5);
                self.circle(c, w.min(h) * 0.20, Rgba::rgb(0x18, 0x19, 0x1D));
                for (dx, dy) in [(-1.3, -0.7), (1.3, -0.7), (0.0, 1.4), (-1.3, 0.9), (1.3, 0.9)] {
                    self.circle(Pt::new(c.x + dx, c.y + dy), 0.45, HEADER_GOLD);
                }
                self.text(
                    Pt::new(at.x + w * 0.80, at.y + h * 0.80),
                    0.7,
                    "PS/2",
                    SILVER.lighten(0.4),
                    Anchor::Center,
                );
            }
        }
        // The header, wherever the module table put each pin.
        for (name, dx, dy) in bench_pins(part.kind, &part.value).iter() {
            let p = Pt::new(at.x + dx, at.y + dy);
            let horizontal = *dx < 0.0 || *dx > w;
            // A pin inside the outline is a screw terminal: the block is the
            // contact, so only a small wire point is added, not a header pad.
            let inside = *dx > 1.0 && *dx < w - 1.0 && *dy > 1.0 && *dy < h - 1.0;
            if inside {
                self.circle(p, 0.55, Rgba::rgb(0xD8, 0xB4, 0x3C));
            } else {
                let pad = if horizontal {
                    Rect::new(p.x - 1.7, p.y - 0.95, 3.4, 1.9)
                } else {
                    Rect::new(p.x - 0.95, p.y - 1.7, 1.9, 3.4)
                };
                self.rrect_stroke(pad, 0.3, SILVER.lighten(0.2), SILVER.darken(0.3), 0.18);
                self.circle(p, 0.42, Rgba::rgb(0xD8, 0xB4, 0x3C));
            }
            // Names sit on the silkscreen, inside the board outline: below
            // the PCB the caption would be white-on-white.
            // A wire leaves a pad along the edge normal, straight through the
            // middle of the pad, and the next pad is only a few millimetres
            // away, so a name lying flat under the pad would be hidden by a
            // cable. Names on the top and bottom edges therefore run along the
            // cable, like the silkscreen on the real boards.
            let ink = if base.0 as u16 + base.1 as u16 + base.2 as u16 > 480 {
                Rgba::rgb(0x24, 0x24, 0x24)
            } else {
                Rgba::rgb(0xF0, 0xEC, 0xE0)
            };
            if inside {
                // On a screw terminal the name goes on the block, left of the
                // screw: there it never fights the heatsink or the cable.
                self.text(
                    Pt::new(p.x - 4.0, p.y + 0.5),
                    0.78,
                    *name,
                    ink.alpha(0.9),
                    Anchor::Start,
                );
            } else if *dx < 0.0 {
                self.text(Pt::new(at.x + 1.5, p.y + 0.4), 0.78, *name, ink.alpha(0.9), Anchor::Start);
            } else if *dx > w {
                self.text(Pt::new(at.x + w - 1.5, p.y + 0.4), 0.78, *name, ink.alpha(0.9), Anchor::End);
            } else if *dy < 0.0 {
                // Wire leaves upward: read down into the board.
                self.text_rot(
                    Pt::new(p.x + 0.3, at.y + 1.6),
                    0.72,
                    *name,
                    ink.alpha(0.9),
                    Anchor::Start,
                    90.0,
                );
            } else {
                // Wire leaves downward: read up into the board.
                self.text_rot(
                    Pt::new(p.x + 0.3, at.y + h - 1.6),
                    0.72,
                    *name,
                    ink.alpha(0.9),
                    Anchor::Start,
                    -90.0,
                );
            }
        }
    }

    fn pin_pos(&self, pin: usize) -> Pt {
        self.pin_pts[pin]
    }

    fn label(&mut self, part: &Part, at: Pt) {
        if !self.opt.labels || part.kind == Kind::Arduino {
            return;
        }
        let text = if part.value.is_empty() {
            part.label.clone()
        } else {
            format!("{}  {}", part.label, part.value)
        };
        let p = match part.kind {
            Kind::Motor | Kind::Servo | Kind::Buzzer | Kind::Speaker | Kind::Relay | Kind::Pot
            | Kind::Module => Pt::new(at.x + 1.0, at.y + body_height(part) + 3.4),
            _ => Pt::new(at.x, at.y - 8.6),
        };
        // A long caption under a part at the edge of the bench would stick out
        // of the exported frame, so keep its centre inside the frame. The
        // painting is still centred; only the anchor moves.
        let size = 1.9;
        let w = text_width(size, &text);
        let h = size * 1.15;
        let margin = 0.8;
        let (mut x, mut y) = (p.x, p.y);
        if self.frame.w > w + margin * 2.0 {
            x = x.clamp(
                self.frame.x + margin + w / 2.0,
                self.frame.right() - margin - w / 2.0,
            );
        }
        if self.frame.h > h + margin * 2.0 {
            y = y.clamp(
                self.frame.y + margin + h / 2.0,
                self.frame.bottom() - margin - h / 2.0,
            );
        }
        self.text(Pt::new(x, y), size, text, self.ink, Anchor::Center);
    }

    /// A visible frame and corner grips, so the user can tell which part is
    /// selected and that it can be dragged. A wash plus a two-tone outline
    /// stays readable over board holes and jumpers.
    fn selection_frame(&mut self, r: &Rect) {
        let ghost = Rgba::rgba(0, 0, 0, 0);
        self.rrect(*r, 2.2, self.accent.alpha(0.12));
        self.rrect_stroke(*r, 2.2, ghost, Rgba::rgba(0x10, 0x12, 0x16, 170), 1.1);
        self.rrect_stroke(*r, 2.2, ghost, self.accent, 0.55);
        for (x, y) in [
            (r.x, r.y),
            (r.right(), r.y),
            (r.right(), r.bottom()),
            (r.x, r.bottom()),
        ] {
            let c = Pt::new(x, y);
            self.circle(c, 1.0, Rgba::rgba(0x10, 0x12, 0x16, 190));
            self.circle(c, 0.76, self.accent);
            self.circle(c, 0.3, self.accent.darken(0.45));
        }
    }

    fn wires(&mut self, c: &Circuit) {
        let straight = self.opt.style == WireStyle::Straight;
        for (i, w) in c.wires_id() {
            self.owner = Some(Owner::Wire(i));
            let a = c.end_pos(w.a);
            let b = c.end_pos(w.b);
            let pts = c.wire_points(w, self.opt.style);
            // The ground run is nearly black on a light bench; on a black
            // one it has to step up or the cable disappears.
            let core = if self.on_dark && w.class == NetClass::Gnd {
                Rgba::rgb(0xD6, 0xD6, 0xDA)
            } else {
                run_color(w)
            };
            // The active cable sits a touch higher, so it is easy to see which
            // one the grips belong to.
            let active = self.opt.active_wire == Some(i);
            let (edge, body) = if active { (2.1, 1.55) } else { (1.75, 1.25) };
            // A flat cable: dark edge, saturated body, a thin lighter top.
            self.poly(pts.clone(), edge, core.darken(0.38), false);
            self.poly(pts.clone(), body, core, false);
            let hi: Vec<Pt> = if straight {
                // Shift the shine sideways so it stays visible on a taut run.
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let len = (dx * dx + dy * dy).sqrt().max(1e-3);
                pts.iter()
                    .map(|p| Pt::new(p.x - dy / len * 0.28, p.y + dx / len * 0.28))
                    .collect()
            } else {
                pts.iter().map(|p| p.off(0.0, -0.28)).collect()
            };
            self.poly(hi, 0.36, core.lighten(0.45).alpha(0.65), false);
            // The plug sits in the hole at either end.
            for p in [a, b] {
                self.circle(p, 0.9, core.darken(0.42));
                self.circle(p, 0.58, core);
            }
        }
        self.owner = None;
        // Bend grips come last, on top of every cable, so a crowded board
        // never hides the one the user is about to pull.
        let handles = self.opt.handles.clone();
        for p in handles {
            self.circle(p, 1.2, Rgba::rgb(0xF7, 0xF7, 0xF4));
            self.circle_stroke(p, 1.2, Rgba::rgba(0, 0, 0, 0), self.accent, 0.45);
            self.circle(p, 0.42, self.accent.darken(0.15));
        }
    }

    fn hover_rings(&mut self, c: &Circuit, nets: &Nets) {
        let Some(hover) = self.opt.hover else { return };
        let root = nets.root(hover);
        for &pin in nets.pins_in(root) {
            let p = c.pins[pin].pos;
            self.circle_stroke(p, 1.5, Rgba::rgba(0, 0, 0, 0), self.accent, 0.45);
        }
        let p = c.pins[hover].pos;
        self.circle_stroke(p, 2.1, Rgba::rgba(0, 0, 0, 0), self.accent.lighten(0.2), 0.55);
    }
}

fn body_height(part: &Part) -> f32 {
    match part.kind {
        Kind::Module => crate::modules::find(&part.value).map(|m| m.h).unwrap_or(20.0),
        Kind::Motor => 14.0,
        Kind::Servo => 24.0,
        Kind::Lamp => 20.0,
        Kind::Buzzer => 12.0,
        Kind::Speaker => 16.0,
        Kind::Relay => 22.0,
        Kind::Pot => 12.0,
        Kind::Electrolytic => 9.0,
        Kind::Reed => 4.0,
        Kind::Thermistor => 7.0,
        Kind::Peltier => 20.0,
        Kind::Arduino => UNO_SIZE.1,
        _ => 6.0,
    }
}


fn led_colour(value: &str) -> Rgba {
    let v = value.to_lowercase();
    if v.contains("зел") || v.contains("green") {
        Rgba::rgb(0x46, 0xC8, 0x5A)
    } else if v.contains("син") || v.contains("blue") {
        Rgba::rgb(0x3D, 0x7B, 0xE8)
    } else if v.contains("rgb") {
        Rgba::rgb(0xD8, 0x6A, 0xC8)
    } else if v.contains("ик") || v.contains("infra") {
        Rgba::rgb(0x6E, 0x3A, 0x9A)
    } else if v.contains("бел") || v.contains("white") {
        Rgba::rgb(0xEE, 0xEE, 0xEA)
    } else if v.contains("жёлт") || v.contains("yellow") {
        Rgba::rgb(0xE8, 0xC8, 0x33)
    } else {
        Rgba::rgb(0xD8, 0x3A, 0x32)
    }
}

/// Resistor colour code, so a 220 Ω part is drawn with real bands.
fn band_colours(value: &str) -> Vec<Rgba> {
    const DIGITS: [Rgba; 10] = [
        Rgba::rgb(0x1A, 0x1A, 0x1A),
        Rgba::rgb(0x7A, 0x3B, 0x1A),
        Rgba::rgb(0xD1, 0x2A, 0x22),
        Rgba::rgb(0xE0, 0x8A, 0x1E),
        Rgba::rgb(0xE8, 0xC8, 0x2E),
        Rgba::rgb(0x2E, 0x92, 0x3E),
        Rgba::rgb(0x25, 0x5C, 0xC8),
        Rgba::rgb(0x7B, 0x35, 0xA8),
        Rgba::rgb(0x9A, 0x9A, 0x9A),
        Rgba::rgb(0xF0, 0xF0, 0xF0),
    ];
    let mut digits = String::new();
    let mut scale = 0i32;
    for ch in value.chars() {
        match ch {
            '0'..='9' | '.' => digits.push(ch),
            // "1 кОм" has a space before the multiplier, so spaces are skipped
            // rather than ending the number.
            ' ' | '\u{00A0}' => {}
            'к' | 'k' | 'K' => {
                scale = 3;
                break;
            }
            'М' | 'M' => {
                scale = 6;
                break;
            }
            _ => {
                if !digits.is_empty() {
                    break;
                }
            }
        }
    }
    let Ok(num) = digits.parse::<f32>() else {
        return vec![DIGITS[2], DIGITS[2], DIGITS[1]];
    };
    let mut ohm = num * 10f32.powi(scale);
    if ohm < 10.0 {
        ohm *= 10.0;
    }
    let mut exp = 0;
    while ohm >= 100.0 && exp < 9 {
        ohm /= 10.0;
        exp += 1;
    }
    let d0 = (ohm as u32 / 10) as usize % 10;
    let d1 = (ohm as u32 % 10) as usize;
    vec![DIGITS[d0], DIGITS[d1], DIGITS[(exp as usize).min(9)]]
}

pub fn draw(c: &Circuit, theme: &Theme, opt: &Opts) -> Scene {
    // A constant pad around the circuit. It has to be the same whether a part
    // is selected or not: otherwise picking a part re-frames the drawing and
    // everything slides out from under the cursor.
    let bb = match opt.frame {
        Some(f) => f,
        None => c.bbox_with(opt.style).inflate(PAD),
    };
    let bench = opt.bench.unwrap_or(theme.bg);
    let on_dark = (0.299 * bench.0 as f32 + 0.587 * bench.1 as f32 + 0.114 * bench.2 as f32) < 120.0;
    let mut d = D {
        s: Scene {
            prims: Vec::new(),
            owners: Vec::new(),
            bg: bench,
            w: bb.w,
            h: bb.h,
            origin: Pt::new(bb.x, bb.y),
        },
        ink: if on_dark {
            Rgba::rgb(0xE6, 0xE2, 0xDA)
        } else {
            LABEL
        },
        on_dark,
        opt: Opts {
            labels: opt.labels,
            hover: opt.hover,
            bench: opt.bench,
            style: opt.style,
            selected: opt.selected,
            selected_many: opt.selected_many.clone(),
            frame: opt.frame,
            active_wire: opt.active_wire,
            handles: opt.handles.clone(),
        },
        accent: theme.accent,
        frame: bb,
        // Painters draw a part upright, so they see pins turned back into the
        // part's own frame; wires and hit-testing use the real positions.
        owner: None,
        pin_pts: c
            .pins
            .iter()
            .map(|p| c.part_xform(&c.parts[p.part]).inverse(p.pos))
            .collect(),
    };
    if !c.bare {
        // The board painter is written for a flat board, so paint a flat copy
        // and then turn the finished primitives around the real centre.
        let mark = d.s.prims.len();
        let flat = Board { rot: 0, ..c.board };
        d.board(&flat);
        d.s.rotate_prims(mark, c.board.center(), c.board.rot as i32);
    }
    for (id, part) in c.parts_id().filter(|(_, p)| p.kind == Kind::Arduino) {
        let x = c.part_xform(part);
        let mark = d.s.prims.len();
        d.owner = Some(Owner::Part(id));
        d.arduino(part);
        d.s.rotate_prims(mark, x.pivot, x.quarter);
        d.owner = None;
    }
    let nets = c.nets();
    for (id, part) in c.parts_id() {
        let x = c.part_xform(part);
        let mark = d.s.prims.len();
        d.owner = Some(Owner::Part(id));
        match part.kind {
            Kind::Arduino => {}
            Kind::Resistor => d.resistor(part),
            Kind::Led => d.led(part),
            Kind::Diode => d.diode(part),
            Kind::Nmos | Kind::Npn => d.to92(part),
            Kind::Button => d.button(part),
            Kind::Ldr => d.ldr(part),
            Kind::Tmp36 => d.to92(part),
            Kind::Motor => d.motor(part),
            Kind::Servo => d.servo(part),
            Kind::Lamp => d.lamp(part),
            Kind::Buzzer => d.buzzer(part),
            Kind::Speaker => d.speaker(part),
            Kind::Relay => d.relay(part),
            Kind::Pot => d.pot(part),
            Kind::Capacitor => d.capacitor(part),
            Kind::Electrolytic => d.electrolytic(part),
            Kind::Reed => d.reed(part),
            Kind::Thermistor => d.thermistor(part),
            Kind::Peltier => d.peltier(part),
            Kind::Module => d.module(part),
        }
        d.s.rotate_prims(mark, x.pivot, x.quarter);
        d.owner = None;
    }
    d.wires(c);
    for part in c.parts() {
        let at = c.part_xform(part).apply(part.at);
        d.label(part, at);
    }
    let mut marked: Vec<usize> = opt.selected_many.clone();
    if let Some(i) = opt.selected {
        if !marked.contains(&i) {
            marked.push(i);
        }
    }
    for i in marked {
        if let Some(part) = c.parts.get(i).filter(|p| p.active) {
            d.selection_frame(&c.part_rect(part).inflate(1.4));
        }
    }
    d.hover_rings(c, &nets);
    let mut s = d.s;
    s.translate(-bb.x, -bb.y);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks;
    use crate::model::{End, NetClass, Wire};
    use crate::parse::Intent;
    use crate::templates;
    use crate::text::Lang;
    use crate::theme::Theme;

    /// Signal jumpers come out of a whole colour box, while the two power
    /// rails keep the colours a schematic expects.
    #[test]
    fn signal_wires_take_colours_from_the_box() {
        let wire = |pin: usize, class| Wire {
            a: End::Pin(pin),
            b: End::Pin(23),
            class,
            lift: 0.0,
            via: vec![],
            colour: None,
            active: true,
        };
        let mut colours = std::collections::BTreeSet::new();
        for pin in 1..20 {
            colours.insert(run_color(&wire(pin, NetClass::Signal)).0);
        }
        assert!(colours.len() >= 6, "only {} cable colours", colours.len());
        assert!(!colours.contains(&WIRE_VCC.0), "signal took the power red");
        assert!(!colours.contains(&WIRE_GND.0), "signal took the ground black");
        assert_eq!(run_color(&wire(3, NetClass::Vcc)).0, WIRE_VCC.0);
        assert_eq!(run_color(&wire(3, NetClass::Gnd)).0, WIRE_GND.0);
    }

    fn all() -> Vec<(String, crate::model::Circuit)> {
        let cases = [
            (crate::parse::Load::Motor, Some(crate::parse::Input::Button)),
            (crate::parse::Load::Led, Some(crate::parse::Input::Button)),
            (crate::parse::Load::Led, None),
            (crate::parse::Load::Led, Some(crate::parse::Input::Ldr)),
            (crate::parse::Load::Servo, None),
            (crate::parse::Load::Buzzer, None),
        ];
        let mut out = Vec::new();
        for (load, input) in cases {
            for choice in 0..3 {
                let intent = Intent {
                    load,
                    input,
                    task: Default::default(),
                    named: Default::default(),
                    transistor: false,
                    presses: false,
                    lesson: None,
                    action: None,
                    raw: String::new(),
                    condition: None,
                    recipe: None,
                    text_screens: None,
                };
                let built = templates::build(&intent, choice, Lang::Ru).unwrap();
                out.push((format!("{} #{choice}", built.title), built.circuit));
            }
        }
        out
    }

    /// Nothing may be cropped: every painted primitive has to sit inside the
    /// frame the export writes, which is what the user sees.
    fn fits_frame(name: &str, circuit: &crate::model::Circuit, labels: bool) {
        let theme = Theme::default();
        let sc = draw(
            circuit,
            &theme,
            &Opts { labels, bench: Some(Rgba::rgb(0xEF, 0xEE, 0xEB)), ..Opts::default() },
        );
        let b = sc.bounds();
        let slack = 1.5;
        assert!(
            b.x >= -slack && b.y >= -slack,
            "{name}: paints at ({:.1}, {:.1}) outside the frame",
            b.x,
            b.y
        );
        assert!(
            b.right() <= sc.w + slack && b.bottom() <= sc.h + slack,
            "{name}: paints to ({:.1}, {:.1}) past {:.1}×{:.1}",
            b.right(),
            b.bottom(),
            sc.w,
            sc.h
        );
    }

    #[test]
    fn every_scene_fits_its_frame() {
        for (name, circuit) in all() {
            for labels in [false, true] {
                fits_frame(&name, &circuit, labels);
            }
        }
    }

    /// Every book lesson is a scene too: its bench and its modules must fit
    /// the frame that the export writes, labels included.
    #[test]
    fn every_lesson_scene_fits_its_frame() {
        for (i, l) in crate::lessons::ALL.iter().enumerate() {
            let intent = Intent {
                task: Default::default(),
                load: crate::parse::Load::Led,
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
            let built = templates::build(&intent, 0, Lang::Ru).unwrap();
            fits_frame(l.title.ru, &built.circuit, true);
        }
    }

    /// Text scenes put wide boards side by side, so they are a separate
    /// layout and need their own frame check.
    #[test]
    fn text_scenes_fit_their_frame() {
        for text in [
            "текст на экране",
            "текст на двух экранах",
            "текст на трёх экранах",
            "текст на четырёх экранах",
            "текст на двух LCD2004",
            "две надписи на двух OLED",
        ] {
            let parsed = crate::parse::parse(text);
            let plan = templates::plan(&parsed.intent, Lang::Ru)
                .unwrap_or_else(|| panic!("{text}: does not plan"));
            for choice in 0..plan.choices.len() {
                let built = templates::build(&parsed.intent, choice, Lang::Ru)
                    .unwrap_or_else(|| panic!("{text} #{choice}: does not build"));
                for labels in [false, true] {
                    fits_frame(&format!("{text} #{choice}"), &built.circuit, labels);
                }
            }
        }
    }

    /// Third and fourth turns are still exports, so they must not crop either.
    /// Turning the board, and turning every loose part, both go through the
    /// same painter.
    #[test]
    fn turned_scenes_still_fit_their_frame() {
        for (name, circuit) in all() {
            let mut turned = circuit.clone();
            assert!(turned.rotate_board(1));
            fits_frame(&format!("{name} board90"), &turned, true);
            let mut turned = circuit.clone();
            assert!(turned.rotate_board(2));
            fits_frame(&format!("{name} board180"), &turned, true);

            let mut loose = circuit.clone();
            for i in 0..loose.parts.len() {
                if !loose.part_is_seated(&loose.parts[i]) {
                    assert!(loose.rotate_part(i, 1), "{name}: part {i} did not turn");
                }
            }
            fits_frame(&format!("{name} parts90"), &loose, true);
        }
    }

    /// The selected part must be visibly different from every other part.
    #[test]
    fn a_selected_part_gets_a_frame() {
        let theme = Theme::default();
        for (_, circuit) in all() {
            let plain = draw(&circuit, &theme, &Opts::default());
            let picked = draw(
                &circuit,
                &theme,
                &Opts { selected: Some(0), ..Opts::default() },
            );
            assert!(
                picked.prims.len() >= plain.prims.len() + 5,
                "the selection frame did not paint"
            );
            let b = picked.bounds();
            assert!(
                b.x >= -1.5 && b.right() <= picked.w + 1.5 && b.bottom() <= picked.h + 1.5,
                "the selection frame is cropped: {b:?} in {:.1}×{:.1}",
                picked.w,
                picked.h
            );
        }
    }

    /// Hiding the board must remove its footprint, not just its paint.
    #[test]
    fn a_bare_scene_is_only_the_parts() {
        use crate::model::Circuit;
        let mut c = Circuit::new();
        c.add_arduino(crate::model::uno_spot());
        let with_board = draw(&c, &Theme::default(), &Opts::default());
        assert!(c.set_bare(true));
        let bare = draw(&c, &Theme::default(), &Opts::default());
        assert!(
            bare.w < with_board.w - 20.0,
            "the hidden board still reserves width: {:.1} vs {:.1}",
            bare.w,
            with_board.w
        );
        fits_frame("bare", &c, true);
    }

    /// The scene is translated to the origin, so the drawing and its frame
    /// start at zero together.
    #[test]
    fn the_frame_starts_at_the_origin() {
        let theme = Theme::default();
        for (name, circuit) in all() {
            let sc = draw(&circuit, &theme, &Opts::default());
            let b = sc.bounds();
            assert!(b.x > -2.0 && b.y > -2.0, "{name}: {b:?}");
        }
    }

    /// Clicking is mapped through the scene's own frame, so the mapping has to
    /// land back on the circuit's coordinates exactly.
    #[test]
    fn scene_coordinates_map_back_to_the_circuit() {
        let theme = Theme::default();
        for (name, circuit) in all() {
            let sc = draw(&circuit, &theme, &Opts::default());
            let bb = circuit.bbox_with(WireStyle::default());
            assert_eq!(
                sc.origin,
                Pt::new(bb.x - PAD, bb.y - PAD),
                "{name}: frame origin"
            );
            for pin in &circuit.pins {
                let local = sc.to_scene(pin.pos);
                assert!(local.x >= -0.5 && local.y >= -0.5, "{name}: pin outside the scene");
                let back = sc.to_bench(local);
                assert!(
                    (back.x - pin.pos.x).abs() < 0.01 && (back.y - pin.pos.y).abs() < 0.01,
                    "{name}: pin drifted to ({:.2}, {:.2})",
                    back.x,
                    back.y
                );
            }
        }
    }

    /// Straight routing must not reserve the arc's headroom, or the drawing
    /// would float in empty space above the board.
    #[test]
    fn straight_wires_stay_between_their_ends() {
        use crate::model::{Circuit, End, Hole, NetClass, Row};
        let mut c = Circuit::new();
        let a = End::Hole(Hole::Col(5, Row::A));
        let b = End::Hole(Hole::Col(25, Row::A));
        c.wire(a, b, NetClass::Signal);
        let arc = c.bbox_with(WireStyle::Arc);
        let straight = c.bbox_with(WireStyle::Straight);
        assert!(
            straight.h < arc.h - 1.0,
            "straight frame still reserves the arc: {:.1} vs {:.1}",
            straight.h,
            arc.h
        );
        // The board itself is the topmost thing once the cable is straight;
        // the arched cable is what pokes out above it.
        assert!(straight.y >= -0.01, "the straight frame starts above the board");
        assert!(
            arc.y < straight.y - 1.0,
            "the arc does not rise above the board: {:.1} vs {:.1}",
            arc.y,
            straight.y
        );
    }

    #[test]
    fn both_routings_paint_wires() {
        let theme = Theme::default();
        for (name, circuit) in all() {
            let arc = draw(&circuit, &theme, &Opts::default());
            let straight = draw(
                &circuit,
                &theme,
                &Opts { style: WireStyle::Straight, ..Opts::default() },
            );
            // Same cables, same number of primitives: only the paths differ.
            assert_eq!(
                arc.prims.iter().filter(|p| matches!(p, Prim::Poly { closed: false, .. })).count(),
                straight.prims.iter().filter(|p| matches!(p, Prim::Poly { closed: false, .. })).count(),
                "{name}: the two routings disagree on the cable count"
            );
        }
    }

    #[test]
    fn hover_marks_the_whole_net() {
        let theme = Theme::default();
        let (_, circuit) = all().into_iter().next().unwrap();
        let plain = draw(&circuit, &theme, &Opts::default());
        let lit = draw(&circuit, &theme, &Opts { hover: Some(0), ..Opts::default() });
        assert!(lit.prims.len() > plain.prims.len(), "hover painted no rings");
        // The rings are stroked circles; nothing should break the frame.
        let b = lit.bounds();
        assert!(b.right() <= lit.w + 2.0 && b.bottom() <= lit.h + 2.0);
    }

    #[test]
    fn the_generated_circuits_have_pins() {
        for (name, c) in all() {
            assert!(!c.pins.is_empty(), "{name} has no pins");
            assert!(c.arduino.is_some(), "{name} has no Arduino");
            let report = checks::run_circuit(&c, Lang::Ru, None);
            assert!(!report.checks.is_empty(), "{name} has no checks");
        }
    }

    /// The resistor colour code follows the value, so a 1 kΩ and a 10 kΩ part
    /// are told apart at a glance.
    #[test]
    fn resistor_values_get_their_own_colour_code() {
        let black = Rgba::rgb(0x1A, 0x1A, 0x1A);
        let brown = Rgba::rgb(0x7A, 0x3B, 0x1A);
        let red = Rgba::rgb(0xD1, 0x2A, 0x22);
        assert_eq!(band_colours("220 Ω"), vec![red, red, brown]);
        assert_eq!(band_colours("1 кОм"), vec![brown, black, red]);
        assert_eq!(band_colours("10 кОм"), vec![brown, black, Rgba::rgb(0xE0, 0x8A, 0x1E)]);
        assert_eq!(band_colours("100 кОм"), vec![brown, black, Rgba::rgb(0xE8, 0xC8, 0x2E)]);
    }

    /// The capacitor variants paint inside their own footprint, so nothing is
    /// cropped and a click lands on the body.
    #[test]
    fn capacitor_variants_fit_their_frame() {
        use crate::model::*;
        let mut c = Circuit::new();
        c.add_arduino(uno_spot());
        for (kind, value, span, col) in [
            (Kind::Capacitor, "100 нФ", 1u8, 6u8),
            (Kind::Electrolytic, "10 мкФ", 2, 10),
        ] {
            let a = c.board.hole(col, Row::A);
            let b = c.board.hole(col + span, Row::A);
            c.add_part(
                kind,
                "C1",
                value,
                a.mid(b),
                vec![
                    PinSpec { name: "1", pos: a, hole: Hole::Col(col, Row::A), kind: PinKind::Passive },
                    PinSpec { name: "2", pos: b, hole: Hole::Col(col + span, Row::A), kind: PinKind::Passive },
                ],
                vec![],
            );
        }
        let sc = draw(&c, &Theme::default(), &Opts::default());
        assert!(sc.prims.len() > 20, "the capacitors painted nothing");
        fits_frame("capacitors", &c, true);
        for part in c.parts().filter(|p| p.kind != Kind::Arduino) {
            let r = c.part_rect(part);
            assert!(r.h > 4.5, "{:?} has no body above the board", part.kind);
        }
    }
}
