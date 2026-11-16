//! The circuit model.
//!
//! Geometry is kept in millimetres on a virtual bench, so the same numbers
//! drive the on-screen painter and the exported SVG. The breadboard is not a
//! decorative image: its strips and rails are part of the netlist, which is
//! what makes the electrical checks mean something.

use std::cell::RefCell;
use std::collections::BTreeMap;

thread_local! {
    /// Routes are expensive A* searches. A circuit's `uid` and `rev` keep a
    /// cached polyline tied to exactly the geometry it was computed from.
    static WIRE_ROUTES: RefCell<std::collections::HashMap<u64, Vec<Pt>>> =
        RefCell::new(std::collections::HashMap::new());
}

/// Standard breadboard hole pitch.
pub const PITCH: f32 = 2.54;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn off(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }
    pub fn lerp(self, o: Pt, t: f32) -> Pt {
        Pt::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    pub fn mid(self, o: Pt) -> Pt {
        self.lerp(o, 0.5)
    }
    pub fn dist(self, o: Pt) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
    /// Rotate clockwise by whole quarter turns around a pivot. Screen space
    /// has y pointing down, so the 90° case maps right to down.
    pub fn rotate(self, pivot: Pt, quarter: i32) -> Pt {
        let (dx, dy) = (self.x - pivot.x, self.y - pivot.y);
        match quarter.rem_euclid(4) {
            1 => Pt::new(pivot.x - dy, pivot.y + dx),
            2 => Pt::new(pivot.x - dx, pivot.y - dy),
            3 => Pt::new(pivot.x + dy, pivot.y - dx),
            _ => self,
        }
    }
}

/// A rigid quarter-turn about a pivot. Parts are painted upright and then
/// moved by this, so a rotated body and its rotated pins stay in step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xform {
    pub pivot: Pt,
    pub quarter: i32,
}

impl Xform {
    pub fn new(pivot: Pt, quarter: i32) -> Self {
        Self { pivot, quarter: quarter.rem_euclid(4) }
    }
    pub fn is_identity(&self) -> bool {
        self.quarter.rem_euclid(4) == 0
    }
    pub fn apply(&self, p: Pt) -> Pt {
        p.rotate(self.pivot, self.quarter)
    }
    pub fn inverse(&self, p: Pt) -> Pt {
        p.rotate(self.pivot, -self.quarter)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Row {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
}

impl Row {
    pub const fn idx(self) -> u8 {
        match self {
            Row::A => 0,
            Row::B => 1,
            Row::C => 2,
            Row::D => 3,
            Row::E => 4,
            Row::F => 5,
            Row::G => 6,
            Row::H => 7,
            Row::I => 8,
            Row::J => 9,
        }
    }
    pub const fn half(self) -> Half {
        if self.idx() < 5 { Half::Top } else { Half::Bottom }
    }
    pub const fn from_idx(idx: u8) -> Row {
        match idx {
            0 => Row::A,
            1 => Row::B,
            2 => Row::C,
            3 => Row::D,
            4 => Row::E,
            5 => Row::F,
            6 => Row::G,
            7 => Row::H,
            8 => Row::I,
            _ => Row::J,
        }
    }
    pub const fn letter(self) -> &'static str {
        match self {
            Row::A => "a",
            Row::B => "b",
            Row::C => "c",
            Row::D => "d",
            Row::E => "e",
            Row::F => "f",
            Row::G => "g",
            Row::H => "h",
            Row::I => "i",
            Row::J => "j",
        }
    }
    /// Mirror a row across the centre channel, which is how a part that
    /// straddles the gap is placed.
    pub const fn mirror(self) -> Row {
        match self {
            Row::A => Row::J,
            Row::B => Row::I,
            Row::C => Row::H,
            Row::D => Row::G,
            Row::E => Row::F,
            Row::F => Row::E,
            Row::G => Row::D,
            Row::H => Row::C,
            Row::I => Row::B,
            Row::J => Row::A,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Half {
    Top,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Rail {
    TopPlus,
    TopMinus,
    BotPlus,
    BotMinus,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum RailHalf {
    Left,
    Right,
}

/// Where a pin sits. Only pins that land in a hole take part in the board's
/// implicit connectivity.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Hole {
    Col(u8, Row),
    Rail(Rail, RailHalf, u8),
    OffBoard,
}

#[derive(Clone, Copy, Debug)]
pub struct Board {
    /// The top-left corner of the board's unrotated footprint. Rotation keeps
    /// the centre, so `at` stays a stable logical anchor for layouts.
    pub at: Pt,
    pub cols: u8,
    /// Quarter turns clockwise around the board's centre.
    pub rot: u8,
}

impl Board {
    pub const MARGIN_X: f32 = 2.0;
    pub const TOP_PLUS: f32 = 2.5;
    pub const TOP_MINUS: f32 = 2.5 + PITCH;
    pub const ROW_A: f32 = 2.5 + 3.0 * PITCH;
    pub const ROW_F: f32 = Self::ROW_A + 6.0 * PITCH;
    pub const BOT_MINUS: f32 = Self::ROW_F + 6.0 * PITCH;
    pub const BOT_PLUS: f32 = Self::BOT_MINUS + PITCH;
    pub const HEIGHT: f32 = Self::BOT_PLUS + 2.5;
    /// Holes in each half of a power rail. A half-size board has one hole
    /// above and below every column, split at the middle.
    pub const RAIL_HOLES: u8 = 15;

    pub const fn new(at: Pt) -> Self {
        Self { at, cols: 30, rot: 0 }
    }

    /// The unrotated footprint's centre: rotation turns the board around this
    /// point, leaving the middle of the board where it was.
    pub fn center(&self) -> Pt {
        Pt::new(self.at.x + self.width() / 2.0, self.at.y + Self::HEIGHT / 2.0)
    }

    /// Logical board coordinates to where the board actually sits.
    fn turn(&self, p: Pt) -> Pt {
        p.rotate(self.center(), self.rot as i32)
    }

    pub fn width(&self) -> f32 {
        Self::MARGIN_X * 2.0 + (self.cols as f32 - 1.0) * PITCH
    }

    pub fn rect(&self) -> Rect {
        let (w, h) = if self.rot % 2 == 0 {
            (self.width(), Self::HEIGHT)
        } else {
            (Self::HEIGHT, self.width())
        };
        let c = self.center();
        Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h)
    }

    /// Logical column coordinate, before the board's rotation. The painter
    /// uses a flat copy of the board and turns the finished primitives.
    pub fn col_x(&self, col: u8) -> f32 {
        self.at.x + Self::MARGIN_X + (col as f32 - 1.0) * PITCH
    }

    pub fn row_y(&self, row: Row) -> f32 {
        let local = if row.idx() < 5 {
            Self::ROW_A + row.idx() as f32 * PITCH
        } else {
            Self::ROW_F + (row.idx() - 5) as f32 * PITCH
        };
        self.at.y + local
    }

    pub fn rail_y(&self, rail: Rail) -> f32 {
        let local = match rail {
            Rail::TopPlus => Self::TOP_PLUS,
            Rail::TopMinus => Self::TOP_MINUS,
            Rail::BotMinus => Self::BOT_MINUS,
            Rail::BotPlus => Self::BOT_PLUS,
        };
        self.at.y + local
    }

    /// Rail holes line up with the terminal columns, with a visible break
    /// under the middle of the board.
    pub fn rail_x(&self, half: RailHalf, idx: u8) -> f32 {
        let col = match half {
            RailHalf::Left => idx + 1,
            RailHalf::Right => idx + 1 + Self::RAIL_HOLES,
        };
        self.col_x(col)
    }

    /// Column under a given x, clamped to the board.
    pub fn col_at(&self, x: f32) -> u8 {
        let raw = ((x - self.at.x - Self::MARGIN_X) / PITCH).round() + 1.0;
        (raw.clamp(1.0, self.cols as f32)) as u8
    }

    pub fn hole(&self, col: u8, row: Row) -> Pt {
        self.turn(Pt::new(self.col_x(col), self.row_y(row)))
    }

    pub fn rail_hole(&self, rail: Rail, half: RailHalf, idx: u8) -> Pt {
        self.turn(Pt::new(self.rail_x(half, idx), self.rail_y(rail)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn center(&self) -> Pt {
        Pt::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    pub fn shift(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
    pub fn inflate(&self, d: f32) -> Rect {
        Rect::new(self.x - d, self.y - d, self.w + 2.0 * d, self.h + 2.0 * d)
    }
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }
    /// Whether two boxes share any bench space. Touching edges count, so a
    /// part that merely leans on the search window still widens it.
    pub fn intersects(&self, other: Rect) -> bool {
        self.x <= other.right()
            && other.x <= self.right()
            && self.y <= other.bottom()
            && other.y <= self.bottom()
    }
    /// The smallest box holding both.
    pub fn union(&self, other: Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(x, y, self.right().max(other.right()) - x, self.bottom().max(other.bottom()) - y)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Arduino,
    Resistor,
    Led,
    Diode,
    Nmos,
    Npn,
    Button,
    Motor,
    Servo,
    Buzzer,
    Speaker,
    Pot,
    Ldr,
    Tmp36,
    Relay,
    /// Ceramic disc capacitor, non-polarised.
    Capacitor,
    /// Radial electrolytic capacitor, the tall can with a polarity stripe.
    Electrolytic,
    /// A glass reed switch: two contacts that close in a magnetic field.
    Reed,
    /// A thermistor bead on two legs.
    Thermistor,
    /// A Peltier module: two ceramic plates with a current through them.
    Peltier,
    /// A ready-made board: a motor driver, a radio, a GSM modem. `Part.value`
    /// names which one, and `modules::find` gives its shape and header.
    Module,
}

impl Kind {
    /// Whether the part needs a path for its coil's current when it is
    /// switched off. A speaker is driven with audio, not switched, and a diode
    /// across it would clip the signal, so it is not listed here.
    pub fn inductive(self) -> bool {
        matches!(self, Kind::Motor | Kind::Relay)
    }
    pub fn is_led(self) -> bool {
        matches!(self, Kind::Led)
    }
    pub fn is_transistor(self) -> bool {
        matches!(self, Kind::Nmos | Kind::Npn)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PinKind {
    Power,
    Ground,
    Signal,
    Analog,
    Passive,
}

/// Stable identifiers of the objects on the bench. They are indices into
/// append-only slot lists: deleting an object marks its slot dead, so an id
/// never changes or points at another object afterwards.
pub type PartId = usize;
pub type PinId = usize;
pub type WireId = usize;

#[derive(Clone, Debug)]
pub struct Pin {
    pub part: PartId,
    pub name: &'static str,
    pub pos: Pt,
    pub hole: Hole,
    pub kind: PinKind,
    /// False for a removed slot, kept so that the ids of all other objects
    /// stay valid.
    pub active: bool,
}

#[derive(Clone, Debug)]
pub struct Part {
    pub kind: Kind,
    pub label: String,
    pub value: String,
    pub at: Pt,
    pub pins: Vec<PinId>,
    /// Pins that are joined inside the part (switch contacts, coil terminals).
    pub internal: Vec<(PinId, PinId)>,
    /// Quarter turns clockwise around the part's own pivot. Pins are stored in
    /// their turned positions; the painter draws the body upright and turns the
    /// primitives, which keeps every part on the same simple geometry.
    pub rot: u8,
    /// False for a removed slot, kept so that ids never get reused.
    pub active: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetClass {
    Vcc,
    Gnd,
    Signal,
    Analog,
}

/// A wire end. Cables routinely land in an empty breadboard hole, so a hole is
/// a first-class endpoint and takes part in the netlist just like a pin.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum End {
    Pin(PinId),
    Hole(Hole),
}

/// A stable number for a wire end. Two cables that meet the same pin but come
/// from different places get different numbers, which gives each its own step
/// at the header and its own colour.
pub fn end_hash(end: End) -> u32 {
    match end {
        End::Pin(p) => (p as u32).wrapping_mul(2).wrapping_add(1),
        End::Hole(Hole::Col(c, r)) => 0x1000_0000u32
            .wrapping_add((c as u32) << 9)
            .wrapping_add(r.idx() as u32 * 2),
        End::Hole(Hole::Rail(rail, half, i)) => 0x2000_0000u32
            .wrapping_add((rail as u32) << 12)
            .wrapping_add((half as u32) << 8)
            .wrapping_add(i as u32 * 2),
        End::Hole(Hole::OffBoard) => 0x3FFF_FFFF,
    }
}

/// A well-mixed number for both ends together. Pin indices are small and
/// mostly odd, so a plain sum would only reach half the colours in the box.
pub fn wire_seed(a: End, b: End) -> u32 {
    let mut h = end_hash(a).wrapping_mul(0x9E37_79B9)
        ^ end_hash(b).wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^ h >> 15
}

/// How cables are routed across the bench.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WireStyle {
    /// A taut jumper that bows above the board.
    Arc,
    /// A straight line from pin to pin, the way a schematic reads.
    Straight,
    /// Right-angle bends around the parts, the way a tidy build is wired.
    #[default]
    Ortho,
}

#[derive(Clone, Debug)]
pub struct Wire {
    pub a: End,
    pub b: End,
    pub class: NetClass,
    /// How high the cable arcs above the bench, in millimetres. Zero means the
    /// painter picks a height from the distance.
    pub lift: f32,
    /// Corners the user pulled out by hand, in bench millimetres, in the order
    /// the cable passes through them. Empty means the style decides the route.
    pub via: Vec<Pt>,
    /// False for a removed slot, kept so that wire ids stay valid.
    pub active: bool,
}

pub struct PinSpec {
    pub name: &'static str,
    pub pos: Pt,
    pub hole: Hole,
    pub kind: PinKind,
}

#[derive(Clone)]
pub struct Circuit {
    /// Identity and revision of this bench, used to key the wire-route cache.
    /// They are not saved: a loaded project gets a fresh identity.
    pub uid: u64,
    pub rev: u64,
    pub board: Board,
    pub parts: Vec<Part>,
    pub pins: Vec<Pin>,
    pub wires: Vec<Wire>,
    pub arduino: Option<PartId>,
    pub notes: Vec<&'static str>,
    /// No breadboard: parts sit on the bench and are wired pin to pin.
    pub bare: bool,
    /// User-given names of nets, keyed by the sorted ids of their pins.
    /// A key changes only when the net itself changes, so a name survives
    /// adding another wire to the same net.
    pub net_names: std::collections::BTreeMap<String, String>,
}

impl Default for Circuit {
    fn default() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_UID: AtomicU64 = AtomicU64::new(1);
        Self {
            uid: NEXT_UID.fetch_add(1, Ordering::Relaxed),
            rev: 0,
            board: Board::default(),
            parts: Vec::new(),
            pins: Vec::new(),
            wires: Vec::new(),
            arduino: None,
            notes: Vec::new(),
            bare: false,
            net_names: std::collections::BTreeMap::new(),
        }
    }
}

impl Default for Board {
    fn default() -> Self {
        // The board sits above the Uno, the way a bench build reads.
        Board::new(Pt::new(6.0, 6.0))
    }
}

impl Circuit {
    pub fn new() -> Self {
        Self {
            board: Board::default(),
            ..Default::default()
        }
    }

    /// Mark the geometry as changed. Every operation that moves a pin or a
    /// bend calls this so the route cache is invalidated.
    pub fn touch(&mut self) {
        self.rev = self.rev.wrapping_add(1);
    }

    /// Live objects only. The vectors also hold the slots of deleted objects
    /// so that ids never change; every drawing, check and netlist walks these
    /// iterators instead of the raw vectors.
    pub fn parts(&self) -> impl Iterator<Item = &Part> {
        self.parts.iter().filter(|p| p.active)
    }
    pub fn parts_mut(&mut self) -> impl Iterator<Item = &mut Part> {
        self.parts.iter_mut().filter(|p| p.active)
    }
    pub fn pins(&self) -> impl Iterator<Item = &Pin> {
        self.pins.iter().filter(|p| p.active)
    }
    pub fn pins_mut(&mut self) -> impl Iterator<Item = &mut Pin> {
        self.pins.iter_mut().filter(|p| p.active)
    }
    pub fn wires(&self) -> impl Iterator<Item = &Wire> {
        self.wires.iter().filter(|w| w.active)
    }
    pub fn wires_mut(&mut self) -> impl Iterator<Item = &mut Wire> {
        self.wires.iter_mut().filter(|w| w.active)
    }
    /// Live objects with their stable ids, for hit-testing and selection.
    pub fn parts_id(&self) -> impl Iterator<Item = (PartId, &Part)> {
        self.parts.iter().enumerate().filter(|(_, p)| p.active)
    }
    pub fn pins_id(&self) -> impl Iterator<Item = (PinId, &Pin)> {
        self.pins.iter().enumerate().filter(|(_, p)| p.active)
    }
    pub fn wires_id(&self) -> impl Iterator<Item = (WireId, &Wire)> {
        self.wires.iter().enumerate().filter(|(_, w)| w.active)
    }
    /// The first free slot id, used when a new object is added.
    pub fn next_part_id(&self) -> PartId {
        self.parts.len()
    }
    pub fn next_pin_id(&self) -> PinId {
        self.pins.len()
    }
    pub fn next_wire_id(&self) -> WireId {
        self.wires.len()
    }

    pub fn add_part(
        &mut self,
        kind: Kind,
        label: &str,
        value: &str,
        at: Pt,
        pins: Vec<PinSpec>,
        internal: Vec<(&'static str, &'static str)>,
    ) -> usize {
        let idx = self.parts.len();
        let mut ids = Vec::with_capacity(pins.len());
        for p in pins {
            ids.push(self.pins.len());
            self.pins.push(Pin {
                part: idx,
                name: p.name,
                pos: p.pos,
                hole: p.hole,
                kind: p.kind,
                active: true,
            });
        }
        // Push the part first: the internal links are resolved by name, and
        // that lookup needs the part to exist already.
        self.parts.push(Part {
            kind,
            label: label.to_string(),
            value: value.to_string(),
            at,
            pins: ids,
            internal: Vec::new(),
            rot: 0,
            active: true,
        });
        let mut links = Vec::new();
        for (a, b) in internal {
            let (ia, ib) = (self.pin(idx, a), self.pin(idx, b));
            links.push((ia, ib));
        }
        self.parts[idx].internal = links;
        self.touch();
        idx
    }

    pub fn pin(&self, part: usize, name: &str) -> usize {
        self.parts[part]
            .pins
            .iter()
            .copied()
            .find(|&p| self.pins[p].name == name)
            .unwrap_or_else(|| panic!("part {} has no pin {name}", self.parts[part].label))
    }

    pub fn wire(&mut self, a: End, b: End, class: NetClass) {
        self.wire_lift(a, b, class, 0.0);
    }

    pub fn wire_lift(&mut self, a: End, b: End, class: NetClass, lift: f32) {
        if a == b {
            return;
        }
        self.wires.push(Wire { a, b, class, lift, via: Vec::new(), active: true });
        self.touch();
    }

    /// Remove a part without renumbering anything. Its slot and the slots of
    /// its pins stay in the vectors, marked dead, so every id held by the UI,
    /// the checks or a saved project still names the same thing.
    pub fn remove_part(&mut self, victim: PartId) {
        if victim >= self.parts.len() || !self.parts[victim].active {
            return;
        }
        self.parts[victim].active = false;
        let victim_pins = self.parts[victim].pins.clone();
        for &pin in &victim_pins {
            self.pins[pin].active = false;
        }
        for w in &mut self.wires {
            let touches = |e: End| matches!(e, End::Pin(p) if victim_pins.contains(&p));
            if touches(w.a) || touches(w.b) {
                w.active = false;
            }
        }
        if self.arduino == Some(victim) {
            self.arduino = None;
        }
        self.touch();
    }

    /// Put a previously removed part back into its original slot, keeping
    /// the ids that other cables and the UI still point at.
    pub fn restore_part(&mut self, id: PartId, part: Part, pins: Vec<Pin>) {
        for (k, pin) in pins.into_iter().enumerate() {
            let slot = part.pins.get(k).copied().unwrap_or(0);
            if slot < self.pins.len() {
                self.pins[slot] = pin;
            } else {
                self.pins.push(pin);
            }
        }
        if id < self.parts.len() {
            self.parts[id] = part;
        } else {
            self.parts.push(part);
        }
        if self.parts[id].kind == Kind::Arduino {
            self.arduino = Some(id);
        }
        self.touch();
    }

    pub fn restore_wire(&mut self, id: WireId, wire: Wire) {
        if id < self.wires.len() {
            self.wires[id] = wire;
        } else {
            self.wires.push(wire);
        }
        self.touch();
    }

    pub fn remove_wire(&mut self, id: WireId) {
        if id < self.wires.len() {
            self.wires[id].active = false;
            self.touch();
        }
    }

    pub fn hole_pos(&self, hole: Hole) -> Option<Pt> {
        match hole {
            Hole::Col(col, row) => Some(self.board.hole(col, row)),
            Hole::Rail(r, h, i) => Some(self.board.rail_hole(r, h, i)),
            Hole::OffBoard => None,
        }
    }

    /// A part may only be seated in holes that no live pin already uses.
    /// The electrical model joins every pin in a hole, so sharing one by
    /// accident would silently short two components together.
    pub fn holes_free(&self, holes: &[Hole]) -> bool {
        holes.iter().all(|h| {
            *h == Hole::OffBoard || !self.pins().any(|p| p.hole == *h)
        })
    }

    pub fn end_pos(&self, end: End) -> Pt {
        match end {
            End::Pin(p) => self.pins[p].pos,
            End::Hole(h) => self.hole_pos(h).unwrap_or_default(),
        }
    }

    /// The two control points of a jumper's cubic. The painter and the bounds
    /// share them, so an arc that bows above the parts is never cropped.
    pub fn wire_controls(&self, w: &Wire) -> (Pt, Pt) {
        let a = self.end_pos(w.a);
        let b = self.end_pos(w.b);
        let d = a.dist(b);
        let lift = if w.lift > 0.0 {
            w.lift
        } else {
            (d * 0.26).clamp(4.0, 18.0)
        };
        let lean = ((b.x - a.x) * 0.12).clamp(-5.0, 5.0);
        (
            Pt::new(a.x + lean, a.y - lift),
            Pt::new(b.x - lean, b.y - lift),
        )
    }

    /// The points a cable has to pass through, in order: its two terminals
    /// with any bends the user pulled out in between.
    pub fn wire_stops(&self, w: &Wire) -> Vec<Pt> {
        let mut stops = Vec::with_capacity(w.via.len() + 2);
        stops.push(self.end_pos(w.a));
        stops.extend(w.via.iter().copied());
        stops.push(self.end_pos(w.b));
        stops
    }

    /// The legs of a cable: one run between every two consecutive stops. The
    /// angle style routes each leg around the parts, so a hand-made bend still
    /// comes out as right angles on both sides.
    pub fn wire_legs(&self, w: &Wire, style: WireStyle) -> Vec<Vec<Pt>> {
        let skip: Vec<usize> = [w.a, w.b]
            .iter()
            .filter_map(|e| self.end_part(*e))
            .collect();
        self.wire_stops(w)
            .windows(2)
            .map(|s| match style {
                WireStyle::Ortho => self.route(s[0], s[1], &skip),
                _ => vec![s[0], s[1]],
            })
            .collect()
    }

    /// The key includes the circuit identity, its revision, both terminals,
    /// every hand bend and the style. A moved pin or a new bend gets a new
    /// key, so a stale route can never be returned.
    fn route_key(&self, w: &Wire, style: WireStyle) -> u64 {
        let mut h = self.uid
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ self.rev.wrapping_mul(0xC2B2_AE3D_282F_713B)
            ^ (wire_seed(w.a, w.b) as u64)
            ^ ((w.via.len() as u64) << 24)
            ^ (style as u64);
        for v in &w.via {
            h = h
                .wrapping_mul(0x100_0000_01B3)
                .wrapping_add((v.x.to_bits() as u64).rotate_left(17) ^ v.y.to_bits() as u64);
        }
        h
    }

    /// The polyline a wire is painted and hit-tested along. Every style ends
    /// exactly on the two terminals; only the way between them changes.
    pub fn wire_points(&self, w: &Wire, style: WireStyle) -> Vec<Pt> {
        let key = self.route_key(w, style);
        if let Some(pts) = WIRE_ROUTES.with(|cache| cache.borrow().get(&key).cloned()) {
            return pts;
        }
        let pts = self.wire_points_uncached(w, style);
        WIRE_ROUTES.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.len() > 4096 {
                cache.clear();
            }
            cache.insert(key, pts.clone());
        });
        pts
    }

    fn wire_points_uncached(&self, w: &Wire, style: WireStyle) -> Vec<Pt> {
        if !w.via.is_empty() {
            // The bends belong to the user, so every style honours them.
            let mut pts: Vec<Pt> = Vec::new();
            for leg in self.wire_legs(w, style) {
                for p in leg {
                    if pts.last().map(|q: &Pt| q.dist(p) > 1e-3).unwrap_or(true) {
                        pts.push(p);
                    }
                }
            }
            return pts;
        }
        let a = self.end_pos(w.a);
        let b = self.end_pos(w.b);
        let mut pts = match style {
            WireStyle::Straight => vec![a, b],
            WireStyle::Ortho => self
                .wire_legs(w, style)
                .into_iter()
                .next()
                .unwrap_or_else(|| vec![a, b]),
            WireStyle::Arc => {
                let (p1, p2) = self.wire_controls(w);
                bezier(a, p1, p2, b, 18)
            }
        };
        // A tidy build meets its pins square, which makes cables that share a
        // run merge into one line right at the header. A short step beside the
        // pin keeps every cable visible up to its own terminal.
        if style == WireStyle::Ortho {
            self.elbow_at(&mut pts, w, true);
            self.elbow_at(&mut pts, w, false);
        }
        pts
    }

    /// A small square step beside a pin: out, along, back, then into the
    /// terminal. Two cables on the same pin, or on neighbours in one header,
    /// leave in their own lanes instead of one thick line.
    fn elbow_at(&self, pts: &mut Vec<Pt>, w: &Wire, start: bool) {
        let end = if start { w.a } else { w.b };
        if !matches!(end, End::Pin(_)) || pts.len() < 2 {
            return;
        }
        let (pin, next) = if start {
            (pts[0], pts[1])
        } else {
            (pts[pts.len() - 1], pts[pts.len() - 2])
        };
        let (dx, dy) = (next.x - pin.x, next.y - pin.y);
        // The step is built from the terminal run and its perpendicular, so
        // it stays square like the rest of an angled cable.
        if (dx.abs() > 1e-3) == (dy.abs() > 1e-3) {
            return;
        }
        /// Length of the step, and how far from the pin it starts.
        const LEAD: f32 = 1.6;
        if dx.abs() + dy.abs() < LEAD * 2.2 {
            return;
        }
        let d = Pt::new(if dx.abs() > 1e-3 { dx.signum() } else { 0.0 },
                        if dy.abs() > 1e-3 { dy.signum() } else { 0.0 });
        // The two ends of one cable pick the same side of their runs, while
        // cables that share a pin differ: the hash covers both terminals.
        let h = wire_seed(w.a, w.b);
        let side = if h % 2 == 0 { 1.0 } else { -1.0 };
        let s = 0.7 + 0.2 * ((h / 2) % 3) as f32;
        let n = if dx.abs() > 1e-3 {
            Pt::new(0.0, side)
        } else {
            Pt::new(side, 0.0)
        };
        let at = |k: f32| Pt::new(pin.x + d.x * LEAD * k, pin.y + d.y * LEAD * k);
        let mut step = Vec::with_capacity(4);
        if start {
            step.push(Pt::new(pin.x + n.x * s, pin.y + n.y * s));
            step.push(Pt::new(step[0].x + d.x * LEAD, step[0].y + d.y * LEAD));
            step.push(at(1.0));
            pts.splice(1..1, step);
        } else {
            let last = pts.len() - 1;
            step.push(at(2.0));
            step.push(Pt::new(step[0].x + n.x * s, step[0].y + n.y * s));
            step.push(Pt::new(step[1].x - d.x * LEAD, step[1].y - d.y * LEAD));
            step.push(at(1.0));
            pts.splice(last..last, step);
        }
    }

    /// The part a wire end belongs to. A cable has to be able to reach its own
    /// terminal, so that part's case is not treated as a wall.
    fn end_part(&self, end: End) -> Option<usize> {
        match end {
            End::Pin(p) => Some(self.pins[p].part),
            End::Hole(h) if h != Hole::OffBoard => {
                self.pins().find(|p| p.hole == h).map(|p| p.part)
            }
            End::Hole(_) => None,
        }
    }

    fn route(&self, a: Pt, b: Pt, skip: &[usize]) -> Vec<Pt> {
        // A tight squeeze between two cases is still worth a try, so the
        // search gets a second pass on a finer grid before giving up.
        for step in [PITCH, PITCH / 2.0] {
            if let Some(path) = self.route_grid(a, b, skip, step) {
                return simplify(path);
            }
        }
        // No clean way round: a straight run still joins the two points.
        vec![a, b]
    }

    fn route_grid(&self, a: Pt, b: Pt, skip: &[usize], step: f32) -> Option<Vec<Pt>> {
        use std::cmp::Reverse;
        use std::collections::{BinaryHeap, HashMap};

        /// Room to walk around a part before giving up.
        const MARGIN: f32 = 12.7;
        /// A wire may leave the case it starts on, so endpoints get a pass.
        const CLEAR: f32 = 3.4;
        const STRAIGHT: i32 = 10;
        const TURN: i32 = 7;

        let mut obstacles: Vec<Rect> = self
            .parts
            .iter()
            .enumerate()
            // A case is only let through when a terminal actually sits under
            // it, as on a transistor: otherwise a cable would be free to cut
            // straight across the board it is plugged into. The Uno's and a
            // module's header is at the outline, so their bodies stay walls.
            .filter(|&(i, p)| {
                let r = self.part_rect(p);
                match p.kind {
                    // The Uno's header sits on its own PCB, and its inner
                    // board is already walled off; the part is never crossed.
                    Kind::Arduino => true,
                    // A module is a wall even for the cable that ends on it,
                    // unless its terminal is on the board itself, as a screw
                    // terminal is: there the cable has to start inside.
                    Kind::Module => {
                        !(skip.contains(&i)
                            && (r.inflate(-0.5).contains(a) || r.inflate(-0.5).contains(b)))
                    }
                    // A transistor or a button has its legs under the case,
                    // so its own case must not shut the cable out.
                    _ => {
                        !skip.contains(&i)
                            || !(r.inflate(1.5).contains(a) || r.inflate(1.5).contains(b))
                    }
                }
            })
            .map(|(_, p)| match p.kind {
                // The Uno's headers sit on its own board, so cables have to
                // reach its edges. Only the middle of the PCB is off limits.
                Kind::Arduino => self.part_rect(p).inflate(-4.0),
                _ => self.part_rect(p).inflate(0.9),
            })
            .filter(|r| r.w > 0.5 && r.h > 0.5)
            .collect();
        // A cable that belongs to the bench has no business crossing the
        // breadboard: it walks round it instead. Cables that start or end on
        // the board still need to reach their hole.
        let board = self.board.rect().inflate(2.5);
        if !board.contains(a) && !board.contains(b) {
            obstacles.push(board.inflate(0.5));
        }
        let blocked = |p: Pt| {
            if p.dist(a) < CLEAR || p.dist(b) < CLEAR {
                return false;
            }
            obstacles.iter().any(|r| r.contains(p))
        };

        // The window starts as the box around both ends, but a cable often has
        // to walk around a board that is wider than that box. Every obstacle
        // touching the window widens it, so the search can use the free bench
        // beside a module instead of giving up and cutting straight through it.
        let mut window = Rect::new(
            a.x.min(b.x) - MARGIN,
            a.y.min(b.y) - MARGIN,
            (a.x - b.x).abs() + 2.0 * MARGIN,
            (a.y - b.y).abs() + 2.0 * MARGIN,
        );
        // Growing without end would turn the search into a crawl over the
        // whole bench, so a busy cluster stops at a sane number of cells. A
        // window that stays small is still correct: the path cannot leave it.
        const MAX_CELLS: f32 = 250_000.0;
        loop {
            let grown = obstacles.iter().fold(window, |r, o| {
                if r.intersects(*o) { r.union(*o) } else { r }
            });
            let same = (grown.x - window.x).abs() < 0.01
                && (grown.y - window.y).abs() < 0.01
                && (grown.right() - window.right()).abs() < 0.01
                && (grown.bottom() - window.bottom()).abs() < 0.01;
            if same || (grown.w / step) * (grown.h / step) > MAX_CELLS {
                break;
            }
            window = grown;
        }
        let x0 = window.x - step;
        let y0 = window.y - step;
        let nx = ((window.right() - x0) / step).ceil() as i32 + 1;
        let ny = ((window.bottom() - y0) / step).ceil() as i32 + 1;
        let at = |ix: i32, iy: i32| Pt::new(x0 + ix as f32 * step, y0 + iy as f32 * step);
        let ix = |x: f32| ((x - x0) / step).round() as i32;
        let iy = |y: f32| ((y - y0) / step).round() as i32;

        let start = (ix(a.x), iy(a.y));
        let goal = (ix(b.x), iy(b.y));
        let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        // State is a cell plus the direction it was entered from, so the
        // search can charge for a bend.
        type Key = (i32, i32, u8);
        let mut cost: HashMap<Key, i32> = HashMap::new();
        let mut from: HashMap<Key, Key> = HashMap::new();
        let mut heap: BinaryHeap<Reverse<(i32, i32, Key)>> = BinaryHeap::new();
        cost.insert((start.0, start.1, 4), 0);
        heap.push(Reverse((0, 0, (start.0, start.1, 4))));
        let mut end: Option<Key> = None;

        while let Some(Reverse((_, g, key))) = heap.pop() {
            let (cx, cy, dir) = key;
            if (cx, cy) == goal {
                end = Some(key);
                break;
            }
            if g > *cost.get(&key).unwrap_or(&i32::MAX) {
                continue;
            }
            for (d, (dx, dy)) in dirs.iter().enumerate() {
                let (mx, my) = (cx + dx, cy + dy);
                if mx < 0 || my < 0 || mx > nx || my > ny {
                    continue;
                }
                if !((mx, my) == start || (mx, my) == goal) && blocked(at(mx, my)) {
                    continue;
                }
                let step_cost = STRAIGHT + if dir != 4 && dir != d as u8 { TURN } else { 0 };
                let ng = g + step_cost;
                let nkey = (mx, my, d as u8);
                if ng < *cost.get(&nkey).unwrap_or(&i32::MAX) {
                    cost.insert(nkey, ng);
                    from.insert(nkey, key);
                    let h = (mx - goal.0).abs() + (my - goal.1).abs();
                    heap.push(Reverse((ng + h * STRAIGHT, ng, nkey)));
                }
            }
        }

        let mut key = end?;
        let mut cells = vec![at(key.0, key.1)];
        while let Some(parent) = from.get(&key) {
            key = *parent;
            cells.push(at(key.0, key.1));
        }
        cells.reverse();

        // Pin the ends to the real terminals. The grid does not have to pass
        // through them, so either end may need one right-angle lead.
        let mut out = vec![a];
        let first = cells[0];
        if (first.x - a.x).abs() > 1e-3 && (first.y - a.y).abs() > 1e-3 {
            let corner = Pt::new(a.x, first.y);
            out.push(if blocked(corner) { Pt::new(first.x, a.y) } else { corner });
        }
        out.extend(cells);
        let last = *out.last().unwrap_or(&a);
        if (last.x - b.x).abs() > 1e-3 && (last.y - b.y).abs() > 1e-3 {
            let corner = Pt::new(last.x, b.y);
            out.push(if blocked(corner) { Pt::new(b.x, last.y) } else { corner });
        }
        out.push(b);
        Some(out)
    }

    /// Pins reachable from each other through wires, part internals, and the
    /// breadboard's own strips and rails.
    pub fn nets(&self) -> Nets {
        let n = self.pins.len();
        // Every hole that is used, either by a pin sitting in it or by a cable
        // ending in it, gets a node of its own.
        let mut holes: BTreeMap<Hole, usize> = BTreeMap::new();
        let mut node_of_hole = |h: Hole, next: &mut usize| -> usize {
            if let Some(&id) = holes.get(&h) {
                id
            } else {
                let id = *next;
                *next += 1;
                holes.insert(h, id);
                id
            }
        };
        let mut next = n;
        let mut hole_edges: Vec<(usize, Hole)> = Vec::new();
        for pin in self.pins() {
            if pin.hole != Hole::OffBoard {
                let id = node_of_hole(pin.hole, &mut next);
                hole_edges.push((id, pin.hole));
            }
        }
        let mut wire_ends: Vec<(usize, Hole)> = Vec::new();
        for w in self.wires() {
            for end in [w.a, w.b] {
                if let End::Hole(h) = end {
                    let id = node_of_hole(h, &mut next);
                    wire_ends.push((id, h));
                }
            }
        }
        let mut uf = UnionFind::new(next);
        for w in self.wires() {
            uf.union(end_node(w.a, &holes, &self.pins), end_node(w.b, &holes, &self.pins));
        }
        for p in self.parts() {
            for &(a, b) in &p.internal {
                uf.union(a, b);
            }
        }
        // Two parts may share a hole (a resistor's leg and a LED's anode do
        // exactly that in most builds), so every pin in the hole joins it,
        // not just the first one found.
        for &(id, hole) in &hole_edges {
            for (pid, p) in self.pins_id() {
                if p.hole == hole {
                    uf.union(id, pid);
                }
            }
        }
        // Board strips: a column half is one net, a rail segment is one net.
        // Every used hole takes part, whether a component leg or a cable end
        // sits in it — otherwise a jumper into an empty rail hole would float.
        let mut strips: BTreeMap<(u8, Half), usize> = BTreeMap::new();
        let mut rails: BTreeMap<(Rail, RailHalf), usize> = BTreeMap::new();
        for (&hole, &id) in &holes {
            match hole {
                Hole::Col(col, row) => {
                    if let Some(&j) = strips.get(&(col, row.half())) {
                        uf.union(id, j);
                    } else {
                        strips.insert((col, row.half()), id);
                    }
                }
                Hole::Rail(r, h, _) => {
                    if let Some(&j) = rails.get(&(r, h)) {
                        uf.union(id, j);
                    } else {
                        rails.insert((r, h), id);
                    }
                }
                Hole::OffBoard => {}
            }
        }
        // Arduino header grounds are one net inside the board.
        if let Some(a) = self.arduino {
            let gnds: Vec<usize> = self.parts[a]
                .pins
                .iter()
                .copied()
                .filter(|&p| self.pins[p].kind == PinKind::Ground)
                .collect();
            for w in gnds.windows(2) {
                uf.union(w[0], w[1]);
            }
        }
        let mut roots = Vec::with_capacity(n);
        for i in 0..n {
            roots.push(uf.find(i));
        }
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, &r) in roots.iter().enumerate() {
            groups.entry(r).or_default().push(i);
        }
        Nets { roots, groups }
    }

    /// A net is named by its members, so adding one more wire to the same net
    /// does not lose the name the user typed.
    pub fn net_key(&self, nets: &Nets, root: usize) -> String {
        let mut pins: Vec<usize> = nets
            .pins_in(root)
            .iter()
            .copied()
            .filter(|&p| self.pins.get(p).is_some_and(|pin| pin.active))
            .collect();
        pins.sort_unstable();
        pins.dedup();
        pins.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",")
    }

    /// The name shown in the inspector and written to a netlist.
    pub fn net_name(&self, nets: &Nets, root: usize) -> String {
        let key = self.net_key(nets, root);
        if let Some(name) = self.net_names.get(&key) {
            return name.clone();
        }
        let pins = nets.pins_in(root);
        if pins.iter().any(|&p| self.pins[p].kind == PinKind::Ground) {
            "GND".to_string()
        } else if pins.iter().any(|&p| self.pins[p].kind == PinKind::Power) {
            "+5V".to_string()
        } else {
            let code = nets.roots_sorted().iter().position(|&r| r == root).unwrap_or(0) + 1;
            format!("N${code}")
        }
    }

    pub fn set_net_name(&mut self, nets: &Nets, root: usize, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 32 || name.contains(')') {
            return false;
        }
        let key = self.net_key(nets, root);
        if self.net_names.get(&key).map(String::as_str) == Some(name) {
            return false;
        }
        self.net_names.insert(key, name.to_string());
        true
    }

    /// The pivot a part turns around. The Uno turns around the middle of its
    /// PCB, not its top-left corner, so the board stays where the user put it.
    pub fn part_pivot(&self, part: &Part) -> Pt {
        match part.kind {
            Kind::Arduino => Pt::new(
                part.at.x + UNO_SIZE.0 / 2.0,
                part.at.y + UNO_SIZE.1 / 2.0,
            ),
            _ => part.at,
        }
    }

    /// Whether any of the part's legs are plugged into the breadboard.
    pub fn part_is_seated(&self, part: &Part) -> bool {
        part.pins
            .iter()
            .any(|&p| self.pins[p].hole != Hole::OffBoard)
    }

    /// How a part's upright drawing has to move to reach the bench: its own
    /// quarter turns, or the turn of the board it is plugged into.
    pub fn part_xform(&self, part: &Part) -> Xform {
        if self.part_is_seated(part) {
            Xform::new(self.board.center(), self.board.rot as i32)
        } else {
            Xform::new(self.part_pivot(part), part.rot as i32)
        }
    }

    /// Rotate a part that sits on the bench. Seated parts move with the board
    /// instead, because their legs are in fixed holes.
    pub fn rotate_part(&mut self, idx: usize, quarter: i32) -> bool {
        if idx >= self.parts.len() || quarter == 0 {
            return false;
        }
        if self.part_is_seated(&self.parts[idx]) {
            return false;
        }
        let pivot = self.part_pivot(&self.parts[idx]);
        let pins = self.parts[idx].pins.clone();
        for p in pins {
            self.pins[p].pos = self.pins[p].pos.rotate(pivot, quarter);
        }
        let part = &mut self.parts[idx];
        part.rot = ((part.rot as i32 + quarter).rem_euclid(4)) as u8;
        self.touch();
        true
    }

    /// Turn the whole breadboard, carrying everything plugged into it. Bench
    /// parts stay where they are; cables follow their endpoints.
    pub fn rotate_board(&mut self, quarter: i32) -> bool {
        if quarter == 0 || self.bare {
            return false;
        }
        self.board.rot = ((self.board.rot as i32 + quarter).rem_euclid(4)) as u8;
        for i in 0..self.pins.len() {
            if let Some(p) = self.hole_pos(self.pins[i].hole) {
                self.pins[i].pos = p;
            }
        }
        // A bend belongs to the board it was laid over, so it turns with the
        // holes instead of being left behind.
        let pivot = self.board.center();
        for w in &mut self.wires {
            for v in &mut w.via {
                *v = v.rotate(pivot, quarter);
            }
        }
        self.touch();
        true
    }

    /// Move a loose part by a delta, carrying its legs and therefore its
    /// cables. Parts plugged into the board are re-seated instead.
    /// Bend points are stored in bench coordinates. When a part moves, the
    /// bends near its pins travel with it instead of being left behind.
    fn carry_vias(&mut self, moved: &[PinId], old: &[(PinId, Pt)], dx: f32, dy: f32) {
        if moved.is_empty() || (dx.abs() < 1e-4 && dy.abs() < 1e-4) {
            return;
        }
        let plans: Vec<(Vec<Pt>, Vec<Pt>)> = self.wires.iter().map(|w| {
            if !w.active || w.via.is_empty() {
                return (Vec::new(), Vec::new());
            }
            let anchors: Vec<Pt> = [w.a, w.b]
                .into_iter()
                .filter_map(|e| match e {
                    End::Pin(p) if moved.contains(&p) => {
                        old.iter().find(|(id, _)| *id == p).map(|(_, pos)| *pos)
                    }
                    _ => None,
                })
                .collect();
            let far: Vec<Pt> = [w.a, w.b]
                .into_iter()
                .filter_map(|e| match e {
                    End::Pin(p) if moved.contains(&p) => None,
                    other => Some(self.end_pos(other)),
                })
                .collect();
            (anchors, far)
        }).collect();
        for (w, (anchors, far)) in self.wires.iter_mut().zip(plans) {
            if anchors.is_empty() {
                continue;
            }
            for v in &mut w.via {
                let near = anchors.iter().map(|a| v.dist(*a)).fold(f32::MAX, f32::min);
                let away = far.iter().map(|a| v.dist(*a)).fold(f32::MAX, f32::min);
                if near <= away {
                    *v = v.off(dx, dy);
                }
            }
        }
    }

    /// Move without dragging bends along: used when undo restores a saved
    /// position while the saved bends are restored separately.
    pub fn move_part_silent(&mut self, idx: usize, to: Pt) -> bool {
        if idx >= self.parts.len() || self.part_is_seated(&self.parts[idx]) {
            return false;
        }
        let from = self.parts[idx].at;
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        if dx.abs() < 1e-4 && dy.abs() < 1e-4 {
            return false;
        }
        for p in self.parts[idx].pins.clone() {
            self.pins[p].pos = self.pins[p].pos.off(dx, dy);
        }
        self.parts[idx].at = to;
        self.touch();
        true
    }

    pub fn move_part(&mut self, idx: usize, to: Pt) -> bool {
        if idx >= self.parts.len() || self.part_is_seated(&self.parts[idx]) {
            return false;
        }
        let from = self.parts[idx].at;
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        if dx.abs() < 1e-4 && dy.abs() < 1e-4 {
            return false;
        }
        self.parts[idx].at = to;
        let pins = self.parts[idx].pins.clone();
        let old: Vec<(PinId, Pt)> = pins.iter().map(|&p| (p, self.pins[p].pos)).collect();
        self.carry_vias(&pins, &old, dx, dy);
        for p in pins {
            self.pins[p].pos = self.pins[p].pos.off(dx, dy);
        }
        self.touch();
        true
    }

    /// Shift a plugged-in part by whole holes, keeping every leg in the board.
    /// Refused when a leg would leave the board or land on another part's leg.
    pub fn reseat_part(&mut self, idx: usize, dcol: i32, drow: i32) -> bool {
        if idx >= self.parts.len() || (dcol == 0 && drow == 0) {
            return false;
        }
        let part = &self.parts[idx];
        if !self.part_is_seated(part) {
            return false;
        }
        let me = part.pins.clone();
        let mut plan = Vec::new();
        for &pin in &me {
            match self.pins[pin].hole {
                Hole::Col(col, row) => {
                    let nc = col as i32 + dcol;
                    let nr = row.idx() as i32 + drow;
                    if nc < 1 || nc > self.board.cols as i32 || nr < 0 || nr > 9 {
                        return false;
                    }
                    plan.push((pin, nc as u8, nr as u8));
                }
                _ => return false,
            }
        }
        // The holes must be free: a leg may not share a socket with another
        // part, or the netlist would join things that are only touching.
        for p in self.pins() {
            if p.part == idx {
                continue;
            }
            if let Hole::Col(c, r) = p.hole {
                if plan.iter().any(|&(_, nc, nr)| nc == c && nr == r.idx()) {
                    return false;
                }
            }
        }
        let at = self.parts[idx].at;
        let moved = self.parts[idx].pins.clone();
        let old: Vec<(PinId, Pt)> = moved.iter().map(|&p| (p, self.pins[p].pos)).collect();
        let (dx, dy) = (dcol as f32 * PITCH, drow as f32 * PITCH);
        self.carry_vias(&moved, &old, dx, dy);
        self.parts[idx].at = Pt::new(at.x + dx, at.y + dy);
        for (pin, c, r) in plan {
            let row = Row::from_idx(r);
            self.pins[pin].hole = Hole::Col(c, row);
            self.pins[pin].pos = self.board.hole(c, row);
        }
        self.touch();
        true
    }

    /// Show or hide the breadboard. Hiding is only allowed once nothing is
    /// plugged into it, so the netlist never keeps strips the drawing hides.
    pub fn set_bare(&mut self, bare: bool) -> bool {
        if bare == self.bare {
            return true;
        }
        if bare && self.parts().any(|p| self.part_is_seated(p)) {
            return false;
        }
        self.bare = bare;
        self.touch();
        true
    }

    /// A part's pins in its own upright frame, before the part or the board
    /// turns. Body footprints are measured from these, not from `at`, so the
    /// hit box matches what the painter actually draws.
    fn pin_local(&self, part: &Part) -> Vec<Pt> {
        let x = self.part_xform(part);
        part.pins
            .iter()
            .map(|&p| x.inverse(self.pins[p].pos))
            .collect()
    }

    /// The upright footprint, before the part's rotation.
    fn base_part_rect(&self, p: &Part) -> Rect {
        let pts = self.pin_local(p);
        let (minx, maxx, y) = if pts.is_empty() {
            (p.at.x, p.at.x, p.at.y)
        } else {
            (
                pts.iter().map(|q| q.x).fold(f32::MAX, f32::min),
                pts.iter().map(|q| q.x).fold(f32::MIN, f32::max),
                pts[0].y,
            )
        };
        let cx = (minx + maxx) / 2.0;
        let span = maxx - minx;
        match p.kind {
            Kind::Arduino => Rect::new(p.at.x - 9.0, p.at.y - 0.5, UNO_SIZE.0 + 9.0, UNO_SIZE.1 + 1.0),
            // The terminals hang below the case, so the box stops at the
            // drawn body: a cable to them must walk round the motor instead
            // of cutting through the gear.
            Kind::Motor => Rect::new(p.at.x - 5.0, p.at.y - 4.0, 30.0, 18.0),
            Kind::Servo => Rect::new(p.at.x, p.at.y - 2.0, 18.0, 26.0),
            Kind::Buzzer => Rect::new(p.at.x - 1.0, p.at.y - 3.0, 19.0, 17.0),
            Kind::Speaker => Rect::new(p.at.x - 1.0, p.at.y - 4.0, 19.0, 20.0),
            Kind::Relay => Rect::new(p.at.x, p.at.y, 34.0, 22.0),
            Kind::Pot => Rect::new(p.at.x, p.at.y, 12.0, 17.0),
            // Capacitors stand up on their legs like the LEDs do.
            Kind::Capacitor => Rect::new(cx - 2.7, y - 6.0, 5.4, 6.4),
            Kind::Electrolytic => Rect::new(cx - 3.2, y - 9.0, 6.4, 9.4),
            Kind::Module => match crate::modules::find(&p.value) {
                Some(m) => Rect::new(p.at.x, p.at.y, m.w, m.h),
                None => Rect::new(p.at.x, p.at.y, 30.0, 20.0),
            },
            // Axial parts lie along their pins, thin and low.
            Kind::Resistor => Rect::new(minx - 0.9, y - 2.3, span + 1.8, 4.6),
            Kind::Reed => Rect::new(minx - 1.2, y - 2.2, span + 2.4, 4.4),
            Kind::Diode => Rect::new(cx - 3.6, y - 2.2, 7.2, 4.4),
            // The case stands on its legs, above the holes.
            Kind::Led | Kind::Ldr => Rect::new(cx - 2.7, y - 8.2, 5.4, 8.6),
            Kind::Thermistor => Rect::new(cx - 2.4, y - 7.4, 4.8, 8.0),
            Kind::Peltier => Rect::new(p.at.x, p.at.y, 20.0, 20.0),
            Kind::Nmos | Kind::Npn | Kind::Tmp36 => Rect::new(cx - 3.2, y - 8.0, 6.4, 8.5),
            Kind::Button => Rect::new(minx - 0.8, y - 0.8, span + 1.6, 1.6 + (pts.iter().map(|q| q.y).fold(f32::MIN, f32::max) - y)),
        }
    }

    /// The bench footprint of a part, used for framing and hit-testing.
    pub fn part_rect(&self, p: &Part) -> Rect {
        let r = self.base_part_rect(p);
        let x = self.part_xform(p);
        if x.is_identity() {
            return r;
        }
        let corners = [
            Pt::new(r.x, r.y),
            Pt::new(r.right(), r.y),
            Pt::new(r.right(), r.bottom()),
            Pt::new(r.x, r.bottom()),
        ];
        let mut min = Pt::new(f32::MAX, f32::MAX);
        let mut max = Pt::new(f32::MIN, f32::MIN);
        for c in corners {
            let q = x.apply(c);
            min = Pt::new(min.x.min(q.x), min.y.min(q.y));
            max = Pt::new(max.x.max(q.x), max.y.max(q.y));
        }
        Rect::new(min.x, min.y, max.x - min.x, max.y - min.y)
    }

    pub fn bbox(&self) -> Rect {
        self.bbox_with(WireStyle::Arc)
    }

    /// The frame a particular routing needs: straight cables do not reach
    /// above their ends, so they should not reserve the arc's headroom.
    pub fn bbox_with(&self, style: WireStyle) -> Rect {
        let mut min = Pt::new(f32::MAX, f32::MAX);
        let mut max = Pt::new(f32::MIN, f32::MIN);
        let mut eat = |p: Pt| {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        };
        if !self.bare {
            let b = self.board.rect();
            eat(Pt::new(b.x, b.y));
            eat(Pt::new(b.right(), b.bottom()));
        }
        for p in self.pins() {
            eat(p.pos);
        }
        for p in self.parts() {
            let r = self.part_rect(p);
            eat(Pt::new(r.x, r.y));
            eat(Pt::new(r.right(), r.bottom()));
        }
        for w in self.wires() {
            eat(self.end_pos(w.a));
            eat(self.end_pos(w.b));
            match style {
                WireStyle::Arc => {
                    // The arc rises above both ends; its control points bound it.
                    let (p1, p2) = self.wire_controls(w);
                    eat(p1);
                    eat(p2);
                }
                // A detour can wander away from the straight run, so every
                // bend of the path belongs to the frame.
                WireStyle::Ortho => {
                    for p in self.wire_points(w, style) {
                        eat(p);
                    }
                }
                WireStyle::Straight => {}
            }
        }
        Rect::new(min.x, min.y, max.x - min.x, max.y - min.y).inflate(6.0)
    }

    pub fn parts_of(&self, kind: Kind) -> impl Iterator<Item = &Part> {
        self.parts().filter(move |p| p.kind == kind)
    }

    /// Names of the parts sharing a net with `pin`, for the hover readout.
    pub fn net_label(&self, pin: usize, nets: &Nets) -> String {
        let root = nets.root(pin);
        let mut names: Vec<String> = nets
            .pins_in(root)
            .iter()
            .map(|&p| {
                let info = &self.pins[p];
                format!("{}.{}", self.parts[info.part].label, info.name)
            })
            .collect();
        names.sort();
        names.dedup();
        names.join(" · ")
    }
}

pub struct Nets {
    roots: Vec<usize>,
    groups: BTreeMap<usize, Vec<usize>>,
}

impl Nets {
    pub fn root(&self, pin: usize) -> usize {
        self.roots[pin]
    }
    pub fn same(&self, a: usize, b: usize) -> bool {
        self.roots[a] == self.roots[b]
    }
    pub fn pins_in(&self, root: usize) -> &[usize] {
        self.groups.get(&root).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
    /// Roots ordered by their first pin, so export and numbering are stable.
    pub fn roots_sorted(&self) -> Vec<usize> {
        let mut roots: Vec<usize> = self.groups.keys().copied().collect();
        roots.sort_by_key(|r| self.groups.get(r).and_then(|p| p.first()).copied().unwrap_or(usize::MAX));
        roots
    }
    /// True when both pins sit on some third pin's net, i.e. they are wired
    /// together, directly or through the board.
    pub fn path_exists(&self, a: usize, b: usize) -> bool {
        self.same(a, b)
    }
}

fn end_node(end: End, holes: &BTreeMap<Hole, usize>, _pins: &[Pin]) -> usize {
    match end {
        End::Pin(p) => p,
        End::Hole(h) => *holes.get(&h).expect("hole node was allocated"),
    }
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }
    fn find(&mut self, mut i: usize) -> usize {
        while self.parent[i] != i {
            self.parent[i] = self.parent[self.parent[i]];
            i = self.parent[i];
        }
        i
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return;
        }
        if self.rank[ra] < self.rank[rb] {
            self.parent[ra] = rb;
        } else if self.rank[ra] > self.rank[rb] {
            self.parent[rb] = ra;
        } else {
            self.parent[rb] = ra;
            self.rank[ra] += 1;
        }
    }
}

/// The Arduino Uno header, in the order it is silkscreened: power, analog,
/// then the two digital blocks.
pub const UNO_PINS: [(&str, PinKind); 32] = [
    ("NC", PinKind::Power),
    ("IOREF", PinKind::Power),
    ("RESET", PinKind::Signal),
    ("3V3", PinKind::Power),
    ("5V", PinKind::Power),
    ("GND1", PinKind::Ground),
    ("GND2", PinKind::Ground),
    ("VIN", PinKind::Power),
    ("A0", PinKind::Analog),
    ("A1", PinKind::Analog),
    ("A2", PinKind::Analog),
    ("A3", PinKind::Analog),
    ("A4", PinKind::Analog),
    ("A5", PinKind::Analog),
    ("D0", PinKind::Signal),
    ("D1", PinKind::Signal),
    ("D2", PinKind::Signal),
    ("D3", PinKind::Signal),
    ("D4", PinKind::Signal),
    ("D5", PinKind::Signal),
    ("D6", PinKind::Signal),
    ("D7", PinKind::Signal),
    ("GND3", PinKind::Ground),
    ("AREF", PinKind::Power),
    ("D8", PinKind::Signal),
    ("D9", PinKind::Signal),
    ("D10", PinKind::Signal),
    ("D11", PinKind::Signal),
    ("D12", PinKind::Signal),
    ("D13", PinKind::Signal),
    ("SDA", PinKind::Signal),
    ("SCL", PinKind::Signal),
];

/// The Uno footprint, in millimetres: 78 × 55 keeps its real 1.4 ratio and
/// the 2.54 mm header pitch.
pub const UNO_SIZE: (f32, f32) = (78.0, 55.0);

/// Where the Uno sits: to the right of the breadboard, with its header rows
/// level with the board's own edges. Wires then stay short and never have to
/// cross the board.
pub const UNO_X: f32 = 96.0;
pub const UNO_Y: f32 = 4.0;

pub fn uno_spot() -> Pt {
    Pt::new(UNO_X, UNO_Y)
}

/// The four header blocks, as indices into [`UNO_PINS`], read left to right
/// along the edge. Two blocks sit on the top edge, two on the bottom, the way
/// the real board is silkscreened.
pub const UNO_GROUPS: [&[usize]; 4] = [
    // Top left: SCL, SDA, AREF, GND, then 13 down to 8.
    &[31, 30, 23, 22, 29, 28, 27, 26, 25, 24],
    // Top right: 7 down to 0.
    &[21, 20, 19, 18, 17, 16, 15, 14],
    // Bottom left: NC, IOREF, RESET, 3V3, 5V, GND, GND, VIN.
    &[0, 1, 2, 3, 4, 5, 6, 7],
    // Bottom right: A0..A5.
    &[8, 9, 10, 11, 12, 13],
];

/// Leftmost socket of each block, in millimetres from the board's left edge.
pub const UNO_GROUP_X: [f32; 4] = [20.5, 52.0, 31.9, 56.2];

/// The socket row of each block: inset from the top or bottom edge.
pub const UNO_EDGE_INSET: f32 = 2.0;

/// Where a header socket sits on the board. `idx` is an index into
/// [`UNO_PINS`].
pub fn uno_pin_pos(at: Pt, idx: usize) -> Pt {
    for (g, group) in UNO_GROUPS.iter().enumerate() {
        if let Some(slot) = group.iter().position(|&p| p == idx) {
            let x = at.x + UNO_GROUP_X[g] + slot as f32 * PITCH;
            let y = if g < 2 {
                at.y + UNO_EDGE_INSET
            } else {
                at.y + UNO_SIZE.1 - UNO_EDGE_INSET
            };
            return Pt::new(x, y);
        }
    }
    at
}

impl Circuit {
    pub fn add_arduino(&mut self, at: Pt) -> usize {
        let mut pins = Vec::new();
        for (i, (name, kind)) in UNO_PINS.iter().enumerate() {
            pins.push(PinSpec {
                name,
                pos: uno_pin_pos(at, i),
                hole: Hole::OffBoard,
                kind: *kind,
            });
        }
        let idx = self.add_part(Kind::Arduino, "U1", "Arduino Uno R3", at, pins, vec![]);
        self.arduino = Some(idx);
        idx
    }
}

/// A cubic Bézier sampled into a polyline.
pub fn bezier(p0: Pt, p1: Pt, p2: Pt, p3: Pt, n: usize) -> Vec<Pt> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            Pt::new(
                u * u * u * p0.x + 3.0 * u * u * t * p1.x + 3.0 * u * t * t * p2.x + t * t * t * p3.x,
                u * u * u * p0.y + 3.0 * u * u * t * p1.y + 3.0 * u * t * t * p2.y + t * t * t * p3.y,
            )
        })
        .collect()
}

/// Drop repeated and collinear points, so a routed wire is a list of corners.
pub fn simplify(pts: Vec<Pt>) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::new();
    for p in pts {
        if out.last().map(|q| q.dist(p) < 1e-3).unwrap_or(false) {
            continue;
        }
        out.push(p);
    }
    let mut i = 1;
    while i + 1 < out.len() {
        let (a, b, c) = (out[i - 1], out[i], out[i + 1]);
        let straight = ((a.x - b.x).abs() < 1e-3 && (b.x - c.x).abs() < 1e-3)
            || ((a.y - b.y).abs() < 1e-3 && (b.y - c.y).abs() < 1e-3);
        if straight {
            out.remove(i);
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bench() -> Circuit {
        let mut c = Circuit::new();
        c.add_arduino(uno_spot());
        c
    }

    /// A leg in a column strip reaches every other hole on that half of the
    /// board, while the two halves stay apart.
    #[test]
    fn column_strips_join_one_half() {
        let mut c = bench();
        c.add_part(
            Kind::Resistor,
            "R1",
            "220 Ω",
            Pt::new(20.0, 10.0),
            vec![
                PinSpec { name: "1", pos: c.board.hole(5, Row::A), hole: Hole::Col(5, Row::A), kind: PinKind::Passive },
                PinSpec { name: "2", pos: c.board.hole(5, Row::C), hole: Hole::Col(5, Row::C), kind: PinKind::Passive },
            ],
            vec![],
        );
        let p = c.pin(1, "1");
        let q = c.pin(1, "2");
        let nets = c.nets();
        assert!(nets.same(p, q), "A and C of one column must be joined");

        c.add_part(
            Kind::Resistor,
            "R2",
            "220 Ω",
            Pt::new(20.0, 30.0),
            vec![
                PinSpec { name: "1", pos: c.board.hole(5, Row::F), hole: Hole::Col(5, Row::F), kind: PinKind::Passive },
                PinSpec { name: "2", pos: c.board.hole(5, Row::J), hole: Hole::Col(5, Row::J), kind: PinKind::Passive },
            ],
            vec![],
        );
        let below = c.pin(2, "1");
        let nets = c.nets();
        assert!(!nets.same(p, below), "the channel must separate the halves");
        assert!(nets.same(below, c.pin(2, "2")));
    }

    /// A cable may end in an empty rail hole: the rail itself is the net.
    #[test]
    fn rail_holes_are_one_net() {
        let mut c = bench();
        let far = Hole::Rail(Rail::BotPlus, RailHalf::Right, 9);
        let near = Hole::Rail(Rail::BotPlus, RailHalf::Right, 1);
        let five = c.pin(0, "5V");
        let gnd = c.pin(0, "GND1");
        c.wire(End::Pin(five), End::Hole(near), NetClass::Vcc);
        c.wire(End::Pin(gnd), End::Hole(far), NetClass::Gnd);
        // Nothing sits at `far`, but the rail still joins the two cables.
        let nets = c.nets();
        assert!(nets.same(five, gnd), "both sides of a rail must be one net");
    }

    /// Wires bow above their ends; the frame has to account for that.
    #[test]
    fn the_frame_covers_the_arcs() {
        let mut c = bench();
        let a = End::Pin(c.pin(0, "D9"));
        let b = End::Hole(Hole::Col(20, Row::J));
        c.wire(a, b, NetClass::Signal);
        let bb = c.bbox();
        let pa = c.end_pos(a);
        let pb = c.end_pos(b);
        let peak = bb.y;
        assert!(
            peak < pa.y.min(pb.y) - 2.0,
            "the frame does not reach above the cable: {peak:.1}"
        );
    }

    #[test]
    fn removing_a_part_keeps_the_rest_addressed() {
        let mut c = bench();
        for col in 3..6u8 {
            c.add_part(
                Kind::Resistor,
                "R",
                "220 Ω",
                c.board.hole(col, Row::A).mid(c.board.hole(col + 2, Row::A)),
                vec![
                    PinSpec { name: "1", pos: c.board.hole(col, Row::A), hole: Hole::Col(col, Row::A), kind: PinKind::Passive },
                    PinSpec { name: "2", pos: c.board.hole(col + 2, Row::A), hole: Hole::Col(col + 2, Row::A), kind: PinKind::Passive },
                ],
                vec![],
            );
        }
        let tail = c.pin(3, "2");
        c.wire(End::Pin(tail), End::Hole(Hole::Col(10, Row::B)), NetClass::Signal);
        c.remove_part(2);
        assert_eq!(c.parts().count(), 3, "one part went away");
        assert!(!c.parts[2].active, "the removed slot stays in place");
        assert_eq!(c.arduino, Some(0));
        assert!(c.pins[tail].active, "a surviving id still names its pin");
        assert_eq!(c.pins[tail].name, "2");
        for (i, pin) in c.pins_id() {
            assert!(pin.part < c.parts.len());
            assert!(c.parts[pin.part].pins.contains(&i));
        }
        let kept = c.wires().find(|w| matches!(w.a, End::Pin(_))).expect("wire survived");
        if let End::Pin(p) = kept.a {
            assert_eq!(c.parts[c.pins[p].part].label, "R");
        }
    }

    #[test]
    fn dropping_the_uno_clears_the_pointer() {
        let mut c = bench();
        c.remove_part(0);
        assert!(c.arduino.is_none());
        assert_eq!(c.parts().count(), 0);
        assert_eq!(c.pins().count(), 0);
    }

    /// Every logical pin has exactly one physical socket, all of them sit on
    /// the board, and the header blocks do not collide.
    #[test]
    fn every_uno_pin_has_one_socket() {
        let at = uno_spot();
        let mut seen = vec![false; UNO_PINS.len()];
        let mut all = Vec::new();
        for group in UNO_GROUPS {
            for &idx in group {
                assert!(!seen[idx], "{} is placed twice", UNO_PINS[idx].0);
                seen[idx] = true;
                let p = uno_pin_pos(at, idx);
                assert!(p.x >= at.x && p.x <= at.x + UNO_SIZE.0, "{:?} off the board", UNO_PINS[idx].0);
                assert!(p.y >= at.y && p.y <= at.y + UNO_SIZE.1);
                all.push(p);
            }
        }
        assert!(seen.iter().all(|v| *v), "some pins never got a socket");
        assert_eq!(all.len(), UNO_PINS.len());
        for (i, a) in all.iter().enumerate() {
            for b in all.iter().skip(i + 1) {
                let d = a.dist(*b);
                assert!(d > PITCH * 0.95, "two sockets share one spot");
            }
        }
        // The power pins face the breadboard side differently from the logic:
        // 5 V is on the bottom edge, D9 on the top.
        assert!(uno_pin_pos(at, 4).y > at.y + UNO_SIZE.1 / 2.0);
        assert!(uno_pin_pos(at, 25).y < at.y + UNO_SIZE.1 / 2.0);
        // The groups keep their left-to-right order.
        assert!(uno_pin_pos(at, 31).x < uno_pin_pos(at, 24).x);
        assert!(uno_pin_pos(at, 21).x < uno_pin_pos(at, 14).x);
    }

    #[test]
    fn the_board_geometry_is_a_real_breadboard() {
        let b = Board::default();
        assert_eq!(b.cols, 30);
        assert!(b.rect().w > 70.0 && b.rect().w < 90.0);
        assert_eq!(b.col_at(b.hole(1, Row::A).x), 1);
        assert_eq!(b.col_at(b.hole(30, Row::A).x), 30);
        for row in [Row::A, Row::B, Row::C, Row::D, Row::E] {
            assert!(b.row_y(row) < b.row_y(Row::F));
        }
    }

    /// A quarter turn four times is the identity, and the reverse turn undoes
    /// the forward one.
    #[test]
    fn quarter_turns_are_reversible() {
        let p = Pt::new(13.0, 7.5);
        let pivot = Pt::new(4.0, 4.0);
        let mut q = p;
        for _ in 0..4 {
            q = q.rotate(pivot, 1);
        }
        assert!(q.dist(p) < 1e-4, "four turns moved the point");
        let there = p.rotate(pivot, 1);
        assert!(there.rotate(pivot, -1).dist(p) < 1e-4);
        assert!(p.rotate(pivot, 2).rotate(pivot, 2).dist(p) < 1e-4);
    }

    /// Turning a bench part moves its legs around the anchor and leaves the
    /// anchor alone, so it can be turned repeatedly without drifting.
    #[test]
    fn a_bench_part_turns_around_its_anchor() {
        let mut c = bench();
        let at = Pt::new(8.0, 70.0);
        let idx = c.add_part(
            Kind::Buzzer,
            "BZ1",
            "пьезо",
            at,
            vec![
                PinSpec { name: "-", pos: Pt::new(13.0, 67.0), hole: Hole::OffBoard, kind: PinKind::Ground },
                PinSpec { name: "+", pos: Pt::new(19.0, 67.0), hole: Hole::OffBoard, kind: PinKind::Signal },
            ],
            vec![],
        );
        let before: Vec<Pt> = c.parts[idx].pins.iter().map(|&p| c.pins[p].pos).collect();
        assert!(c.rotate_part(idx, 1));
        assert_eq!(c.parts[idx].at, at, "the anchor moved");
        assert_eq!(c.parts[idx].rot, 1);
        let turned: Vec<Pt> = c.parts[idx].pins.iter().map(|&p| c.pins[p].pos).collect();
        for (a, b) in before.iter().zip(&turned) {
            assert!(a.dist(*b) > 0.5, "a leg did not move");
            assert!((a.dist(at) - b.dist(at)).abs() < 1e-3, "the leg changed radius");
        }
        // The painted footprint follows the legs.
        let r = c.part_rect(&c.parts[idx]);
        assert!(r.w > 0.0 && r.h > 0.0);
        assert!(c.rotate_part(idx, 3));
        assert_eq!(c.parts[idx].rot, 0);
        for (a, b) in before.iter().zip(c.parts[idx].pins.iter().map(|&p| c.pins[p].pos)) {
            assert!(a.dist(b) < 1e-3, "four turns did not restore the part");
        }
    }

    /// The Uno spins around the middle of its PCB, so the board stays put.
    #[test]
    fn the_uno_turns_around_its_centre() {
        let mut c = bench();
        let idx = c.arduino.unwrap();
        let pivot = c.part_pivot(&c.parts[idx]);
        let pins: Vec<Pt> = c.parts[idx].pins.iter().map(|&p| c.pins[p].pos).collect();
        assert!(c.rotate_part(idx, 1));
        assert!(c.part_pivot(&c.parts[idx]).dist(pivot) < 1e-4);
        for (i, &p) in pins.iter().enumerate() {
            let pin = c.parts[idx].pins[i];
            assert!((c.pins[pin].pos.dist(pivot) - p.dist(pivot)).abs() < 1e-3);
        }
        assert!(c.rotate_part(idx, 3));
        for (p, pin) in pins.iter().zip(&c.parts[idx].pins) {
            assert!(c.pins[*pin].pos.dist(*p) < 1e-3);
        }
    }

    /// Turning the breadboard carries its parts and keeps every leg in a hole;
    /// loose bench parts stay where they are.
    #[test]
    fn turning_the_board_carries_the_parts_plugged_into_it() {
        let mut c = bench();
        let seat = c.board.hole(5, Row::A);
        let idx = c.add_part(
            Kind::Resistor,
            "R1",
            "220 Ω",
            Pt::new(20.0, 10.0),
            vec![
                PinSpec { name: "1", pos: seat, hole: Hole::Col(5, Row::A), kind: PinKind::Passive },
                PinSpec { name: "2", pos: c.board.hole(8, Row::A), hole: Hole::Col(8, Row::A), kind: PinKind::Passive },
            ],
            vec![],
        );
        let bench_part = c.add_part(
            Kind::Buzzer,
            "BZ1",
            "пьезо",
            Pt::new(8.0, 70.0),
            vec![
                PinSpec { name: "-", pos: Pt::new(13.0, 67.0), hole: Hole::OffBoard, kind: PinKind::Ground },
                PinSpec { name: "+", pos: Pt::new(19.0, 67.0), hole: Hole::OffBoard, kind: PinKind::Signal },
            ],
            vec![],
        );
        let loose = c.pins[c.parts[bench_part].pins[0]].pos;
        assert!(c.rotate_board(1));
        for &pin in &c.parts[idx].pins {
            let hole = c.pins[pin].hole;
            assert_eq!(Some(c.pins[pin].pos), c.hole_pos(hole), "a leg left its hole");
            assert_ne!(c.pins[pin].pos, seat, "the leg did not move");
        }
        assert_eq!(c.pins[c.parts[bench_part].pins[0]].pos, loose);
        assert_eq!(c.board.rot, 1);
        // Picture the group: the seated part is drawn in the board frame.
        assert_eq!(c.part_xform(&c.parts[idx]).quarter, 1);
        assert!(c.part_xform(&c.parts[bench_part]).is_identity());
        assert!(c.rotate_board(3));
        for &pin in &c.parts[idx].pins {
            assert_eq!(Some(c.pins[pin].pos), c.hole_pos(c.pins[pin].hole));
        }
    }

    /// Hiding the board is only allowed once it is empty; the frame then drops
    /// the board's footprint.
    #[test]
    fn a_bare_bench_has_no_breadboard() {
        let mut c = bench();
        c.add_part(
            Kind::Resistor,
            "R1",
            "220 Ω",
            Pt::new(20.0, 10.0),
            vec![
                PinSpec { name: "1", pos: c.board.hole(5, Row::A), hole: Hole::Col(5, Row::A), kind: PinKind::Passive },
                PinSpec { name: "2", pos: c.board.hole(8, Row::A), hole: Hole::Col(8, Row::A), kind: PinKind::Passive },
            ],
            vec![],
        );
        assert!(!c.set_bare(true), "the board still has a part in a hole");
        let empty = Circuit::new();
        let with_board = empty.bbox();
        let mut empty = empty;
        assert!(empty.set_bare(true));
        let bare = empty.bbox();
        assert!(bare.w < with_board.w && bare.h < with_board.h);
        assert!(empty.board.rect().w > 0.0);
    }

    /// A seated part cannot be turned on its own: its legs are in fixed holes.
    #[test]
    fn a_seated_part_is_not_turned_directly() {
        let mut c = bench();
        let idx = c.add_part(
            Kind::Led,
            "D1",
            "красный",
            Pt::new(20.0, 12.0),
            vec![
                PinSpec { name: "1", pos: c.board.hole(5, Row::A), hole: Hole::Col(5, Row::A), kind: PinKind::Passive },
                PinSpec { name: "2", pos: c.board.hole(7, Row::A), hole: Hole::Col(7, Row::A), kind: PinKind::Passive },
            ],
            vec![],
        );
        assert!(!c.rotate_part(idx, 1));
        assert_eq!(c.parts[idx].rot, 0);
    }

    /// Dragging a loose part carries its terminals with it, so the cables stay
    /// attached and the hit box follows.
    #[test]
    fn a_loose_part_can_be_dragged() {
        let mut c = bench();
        let idx = c.add_part(
            Kind::Buzzer,
            "BZ1",
            "пьезо",
            Pt::new(8.0, 70.0),
            vec![
                PinSpec { name: "-", pos: Pt::new(13.0, 67.0), hole: Hole::OffBoard, kind: PinKind::Ground },
                PinSpec { name: "+", pos: Pt::new(19.0, 67.0), hole: Hole::OffBoard, kind: PinKind::Signal },
            ],
            vec![],
        );
        let pin = c.parts[idx].pins[0];
        let old = c.pins[pin].pos;
        assert!(c.move_part(idx, Pt::new(30.0, 80.0)));
        assert_eq!(c.parts[idx].at, Pt::new(30.0, 80.0));
        assert_eq!(c.pins[pin].pos, old.off(22.0, 10.0));
        // A dragged part is still a loose part.
        assert!(!c.part_is_seated(&c.parts[idx]));
    }

    /// A seated part moves by whole holes, keeping every leg in the board and
    /// staying clear of other legs.
    #[test]
    fn a_seated_part_hops_between_holes() {
        let mut c = bench();
        let idx = c.add_part(
            Kind::Resistor,
            "R1",
            "220 Ω",
            Pt::new(20.0, 10.0),
            vec![
                PinSpec { name: "1", pos: c.board.hole(5, Row::A), hole: Hole::Col(5, Row::A), kind: PinKind::Passive },
                PinSpec { name: "2", pos: c.board.hole(8, Row::A), hole: Hole::Col(8, Row::A), kind: PinKind::Passive },
            ],
            vec![],
        );
        assert!(c.reseat_part(idx, 3, 2));
        assert_eq!(c.pins[c.parts[idx].pins[0]].hole, Hole::Col(8, Row::C));
        assert_eq!(c.pins[c.parts[idx].pins[1]].hole, Hole::Col(11, Row::C));
        assert_eq!(Some(c.pins[c.parts[idx].pins[0]].pos), c.hole_pos(Hole::Col(8, Row::C)));
        // Off the edge is refused and nothing moves.
        assert!(!c.reseat_part(idx, 40, 0));
        assert_eq!(c.pins[c.parts[idx].pins[0]].hole, Hole::Col(8, Row::C));
        // Landing on another part's leg is refused too.
        let other = c.add_part(
            Kind::Led,
            "D1",
            "красный",
            Pt::new(20.0, 20.0),
            vec![
                PinSpec { name: "1", pos: c.board.hole(11, Row::C), hole: Hole::Col(11, Row::C), kind: PinKind::Passive },
                PinSpec { name: "2", pos: c.board.hole(13, Row::C), hole: Hole::Col(13, Row::C), kind: PinKind::Passive },
            ],
            vec![],
        );
        assert!(!c.reseat_part(idx, 3, 0), "dropped a leg onto another leg");
        assert!(other < c.parts.len());
    }

    /// A board with a couple of real jumpers on it.
    fn wired() -> Circuit {
        let mut c = bench();
        let r = c.add_part(
            Kind::Resistor,
            "R1",
            "220 Ω",
            Pt::new(20.0, 16.0),
            vec![
                PinSpec { name: "1", pos: c.board.hole(5, Row::A), hole: Hole::Col(5, Row::A), kind: PinKind::Passive },
                PinSpec { name: "2", pos: c.board.hole(8, Row::A), hole: Hole::Col(8, Row::A), kind: PinKind::Passive },
            ],
            vec![],
        );
        let uno = c.arduino.unwrap();
        let gnd = c.pin(uno, "GND1");
        let vcc = c.pin(uno, "5V");
        let r1 = c.parts[r].pins[0];
        let r2 = c.parts[r].pins[1];
        c.wire_lift(End::Pin(r1), End::Pin(gnd), NetClass::Signal, 0.0);
        c.wire_lift(End::Pin(r2), End::Pin(vcc), NetClass::Vcc, 0.0);
        c
    }

    /// Angled routing: every wire is a chain of axis-aligned runs that starts
    /// and ends on its terminals.
    #[test]
    fn angled_wires_bend_at_right_angles() {
        let circuit = wired();
        assert_eq!(circuit.wires.len(), 2, "the sample circuit has no wires");
        for w in circuit.wires() {
            let pts = circuit.wire_points(w, WireStyle::Ortho);
            assert!(pts.len() >= 2, "a wire needs at least two points");
            assert_eq!(pts[0], circuit.end_pos(w.a), "the wire left its start");
            assert_eq!(*pts.last().unwrap(), circuit.end_pos(w.b), "the wire missed its end");
            for s in pts.windows(2) {
                let flat = (s[0].x - s[1].x).abs() < 1e-3;
                let tall = (s[0].y - s[1].y).abs() < 1e-3;
                assert!(flat || tall, "diagonal run {:?}..{:?}", s[0], s[1]);
            }
        }
    }

    /// A wire must not be drawn across a part's case: it steps around it.
    #[test]
    fn angled_wires_go_around_the_parts() {
        let mut c = bench();
        // A transistor body right in the way of a run from the board to the Uno.
        let q = c.add_part(
            Kind::Nmos,
            "Q1",
            "IRLZ44N",
            c.board.hole(24, Row::E).mid(c.board.hole(26, Row::E)),
            vec![
                PinSpec { name: "G", pos: c.board.hole(24, Row::E), hole: Hole::Col(24, Row::E), kind: PinKind::Signal },
                PinSpec { name: "S", pos: c.board.hole(25, Row::E), hole: Hole::Col(25, Row::E), kind: PinKind::Passive },
                PinSpec { name: "D", pos: c.board.hole(26, Row::E), hole: Hole::Col(26, Row::E), kind: PinKind::Signal },
            ],
            vec![],
        );
        let a = c.pins[c.parts[q].pins[1]].pos; // source pin, under the case
        let b = Pt::new(100.0, 40.0);
        let path = c.route(a, b, &[]);
        let case = c.part_rect(&c.parts[q]);
        for s in path.windows(2) {
            for i in 1..20 {
                let t = i as f32 / 20.0;
                let p = s[0].lerp(s[1], t);
                let near_end = p.dist(a) < 3.5 || p.dist(b) < 3.5;
                assert!(
                    near_end || !case.inflate(0.6).contains(p),
                    "the wire crosses the transistor case at ({:.1}, {:.1})",
                    p.x,
                    p.y
                );
            }
        }
    }

    /// A module is a wall even for the cable that ends on its header: the
    /// search has to walk round the PCB instead of cutting across it.
    #[test]
    fn angled_wires_go_around_a_module() {
        let mut c = bench();
        // A wide board with its header on the lower edge, as the ULN2003 has.
        let m = c.add_part(
            Kind::Module,
            "U2",
            "ULN2003",
            Pt::new(110.0, 20.0),
            vec![PinSpec {
                name: "IN1",
                pos: Pt::new(114.0, 53.0),
                hole: Hole::OffBoard,
                kind: PinKind::Signal,
            }],
            vec![],
        );
        let b = c.pins[c.parts[m].pins[0]].pos;
        let a = Pt::new(120.0, 10.0);
        let path = c.route(a, b, &[]);
        assert!(path.len() > 2, "the wire fell back to a straight run: {path:?}");
        let body = c.part_rect(&c.parts[m]);
        for s in path.windows(2) {
            for i in 1..20 {
                let t = i as f32 / 20.0;
                let p = s[0].lerp(s[1], t);
                let near_end = p.dist(a) < 3.5 || p.dist(b) < 3.5;
                assert!(
                    near_end || !body.inflate(0.6).contains(p),
                    "the wire crosses the module at ({:.1}, {:.1})",
                    p.x,
                    p.y
                );
            }
        }
    }

    /// The L298N's supply pads sit on its screw terminals, inside the board
    /// outline. A cable must still be able to leave such a pad, otherwise the
    /// router gives up and draws a diagonal across the bench.
    #[test]
    fn a_cable_leaves_a_screw_terminal() {
        // Away from the Uno, so only the module itself can get in the way.
        let mut c = Circuit::new();
        let m = c.add_part(
            Kind::Module,
            "U3",
            "L298N",
            Pt::new(110.0, 20.0),
            vec![PinSpec {
                name: "+12V",
                pos: Pt::new(116.6, 29.0),
                hole: Hole::OffBoard,
                kind: PinKind::Power,
            }],
            vec![],
        );
        let b = c.pins[c.parts[m].pins[0]].pos;
        let a = Pt::new(100.0, 45.0);
        let path = c.route(a, b, &[m]);
        assert_eq!(path[0], a);
        assert_eq!(*path.last().unwrap(), b);
        for s in path.windows(2) {
            let flat = (s[0].x - s[1].x).abs() < 1e-3;
            let tall = (s[0].y - s[1].y).abs() < 1e-3;
            assert!(flat || tall, "diagonal run {:?}..{:?}", s[0], s[1]);
        }
    }

    /// The routing is deterministic: the same circuit always produces the same
    /// picture, which matters for a screenshot and for hitting the wire.
    #[test]
    fn angled_routing_is_stable() {
        let circuit = wired();
        for w in circuit.wires() {
            let a = circuit.wire_points(w, WireStyle::Ortho);
            let b = circuit.wire_points(w, WireStyle::Ortho);
            assert_eq!(a, b);
        }
    }

    /// Every wire in every generated circuit routes at right angles, which
    /// also catches the case where the search gives up and falls back to a
    /// straight diagonal run.
    #[test]
    fn every_generated_wire_is_angled() {
        let tasks = [
            ("мотор и кнопка", "Увеличить скорость двигателя кнопкой на MOSFET"),
            ("светодиод и кнопка", "Светодиод включается и выключается кнопкой"),
            ("ночник", "Ночник: светодиод зажигается, когда темно"),
            ("сервопривод", "Сервопривод поворачивается туда и обратно"),
            ("пьезо", "Пьезоизлучатель играет мелодию"),
        ];
        for (name, task) in tasks {
            let parsed = crate::parse::parse(task);
            let Some(built) = crate::templates::build(&parsed.intent, 0, crate::text::Lang::Ru) else {
                continue;
            };
            for (i, w) in built.circuit.wires_id() {
                let pts = built.circuit.wire_points(w, WireStyle::Ortho);
                for seg in pts.windows(2) {
                    let flat = (seg[0].x - seg[1].x).abs() < 1e-3;
                    let tall = (seg[0].y - seg[1].y).abs() < 1e-3;
                    assert!(
                        flat || tall,
                        "{name}, wire {i}: no clean route, diagonal {:?}..{:?}",
                        seg[0],
                        seg[1]
                    );
                }
            }
        }
    }

    /// At a pin an angled cable makes a short step aside and back, so a
    /// bundle of cables still parts into its own lines at the header.
    #[test]
    fn a_pin_lead_has_a_small_step() {
        let c = wired();
        let has_step = |pts: &[Pt]| {
            pts.windows(2).any(|s| {
                let d = s[0].dist(s[1]);
                (0.55..=1.15).contains(&d)
                    && ((s[0].x - s[1].x).abs() < 1e-3 || (s[0].y - s[1].y).abs() < 1e-3)
            })
        };
        let mut stepped = 0;
        for w in c.wires() {
            let pts = c.wire_points(w, WireStyle::Ortho);
            if matches!(w.a, End::Pin(_)) && has_step(&pts) {
                stepped += 1;
            }
            if matches!(w.b, End::Pin(_)) && has_step(&pts) {
                stepped += 1;
            }
        }
        assert!(stepped > 0, "no pin-ended cable has a step");
    }

    /// A bend pulled by hand is honoured: the cable goes through it, and it
    /// still leaves and reaches its terminals square to the board.
    #[test]
    fn a_hand_made_bend_is_followed() {
        let mut c = wired();
        let bend = Pt::new(58.0, 40.0);
        c.wires[0].via.push(bend);
        let w = &c.wires[0];
        for style in [WireStyle::Ortho, WireStyle::Straight, WireStyle::Arc] {
            let pts = c.wire_points(w, style);
            assert!(
                pts.iter().any(|p| p.dist(bend) < 1e-3),
                "{style:?} skipped the bend: {pts:?}"
            );
            assert_eq!(pts[0], c.end_pos(w.a), "{style:?} left the start");
            assert_eq!(*pts.last().unwrap(), c.end_pos(w.b), "{style:?} missed the end");
        }
        // The angle style still bends square on both sides of the bend.
        let pts = c.wire_points(&c.wires[0], WireStyle::Ortho);
        for seg in pts.windows(2) {
            let flat = (seg[0].x - seg[1].x).abs() < 1e-3;
            let tall = (seg[0].y - seg[1].y).abs() < 1e-3;
            assert!(flat || tall, "diagonal run {:?}..{:?}", seg[0], seg[1]);
        }
    }

    /// Hand-made bends belong to the board: turning it takes them along.
    #[test]
    fn a_bend_turns_with_the_board() {
        let mut c = wired();
        let bend = Pt::new(58.0, 40.0);
        c.wires[0].via.push(bend);
        let pivot = c.board.center();
        assert!(c.rotate_board(1));
        assert_eq!(c.wires[0].via[0], bend.rotate(pivot, 1));
        assert!(c.rotate_board(-1));
        assert_eq!(c.wires[0].via[0], bend);
    }
}
