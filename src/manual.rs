//! Drawing a circuit by hand.
//!
//! The AI mode hands over a finished build; this module is the other half:
//! place parts on the same breadboard, pull wires between pins and holes, undo
//! what went wrong. Everything works on the same `Circuit`, so the checks and
//! the screenshot keep working without a sketch.

use crate::model::*;
use crate::templates::{bench_pin_kind, bench_pins};
use crate::text::{tr, Lang};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Resistor,
    Capacitor,
    Led,
    RgbLed,
    IrLed,
    Diode,
    Nmos,
    Npn,
    Reed,
    Thermistor,
    Button,
    Ldr,
    Motor,
    Servo,
    Buzzer,
    Speaker,
    Pot,
    Relay,
    Peltier,
    /// Any board from [`crate::modules`]; the palette picks which one.
    Module,
    Uno,
}

impl Place {
    pub fn name(self, lang: Lang) -> &'static str {
        match self {
            Place::Resistor => tr(lang, "резистор", "resistor"),
            Place::Capacitor => tr(lang, "конденсатор", "capacitor"),
            Place::Led => tr(lang, "светодиод", "LED"),
            Place::RgbLed => tr(lang, "светодиод RGB", "RGB LED"),
            Place::IrLed => tr(lang, "ИК-светодиод", "IR LED"),
            Place::Diode => tr(lang, "диод 1N4007", "diode 1N4007"),
            Place::Nmos => tr(lang, "MOSFET IRLZ44N", "MOSFET IRLZ44N"),
            Place::Npn => tr(lang, "транзистор 2N2222", "transistor 2N2222"),
            Place::Reed => tr(lang, "геркон", "reed switch"),
            Place::Thermistor => tr(lang, "термистор 10 кОм", "thermistor 10 kΩ"),
            Place::Button => tr(lang, "кнопка", "button"),
            Place::Ldr => tr(lang, "фоторезистор", "photoresistor"),
            Place::Motor => tr(lang, "мотор", "motor"),
            Place::Servo => tr(lang, "сервопривод", "servo"),
            Place::Buzzer => tr(lang, "пьезо", "piezo"),
            Place::Speaker => tr(lang, "динамик", "speaker"),
            Place::Pot => tr(lang, "потенциометр", "potentiometer"),
            Place::Relay => tr(lang, "реле", "relay"),
            Place::Peltier => tr(lang, "элемент Пельтье", "Peltier element"),
            Place::Module => tr(lang, "модуль", "module"),
            Place::Uno => tr(lang, "Arduino Uno", "Arduino Uno"),
        }
    }

    /// A caption that fits a narrow palette cell. Model numbers stay in the
    /// tooltip and on the bench label, where there is room for them.
    pub fn short_name(self, lang: Lang) -> &'static str {
        match self {
            Place::Diode => "1N4007",
            Place::Nmos => "MOSFET",
            Place::Npn => "2N2222",
            Place::RgbLed => "RGB",
            Place::IrLed => "ИК",
            Place::Thermistor => "10 кОм",
            Place::Peltier => "Пельтье",
            Place::Servo => tr(lang, "серво", "servo"),
            other => other.name(lang),
        }
    }

    /// Which palette section the part belongs to, so the panel can group a
    /// dozen buttons under small headings.
    pub fn group(self) -> usize {
        match self {
            Place::Uno => 0,
            Place::Resistor | Place::Capacitor => 1,
            Place::Led
            | Place::RgbLed
            | Place::IrLed
            | Place::Diode
            | Place::Nmos
            | Place::Npn
            | Place::Ldr
            | Place::Thermistor => 2,
            Place::Button | Place::Pot | Place::Reed => 3,
            Place::Motor
            | Place::Servo
            | Place::Buzzer
            | Place::Speaker
            | Place::Relay
            | Place::Peltier => 4,
            // Ready-made boards are their own section; the palette lists each
            // board by name instead of one shared "module" slot.
            Place::Module => 5,
        }
    }

    /// Axial parts take a row of holes; the rest sit where they are dropped.
    fn on_board(self) -> bool {
        !matches!(
            self,
            Place::Motor
                | Place::Servo
                | Place::Buzzer
                | Place::Speaker
                | Place::Pot
                | Place::Relay
                | Place::Peltier
                | Place::Module
        )
    }
}

/// Palette headings, in the order `Place::group` numbers them.
pub const GROUPS: [&str; 6] = [
    "group.board",
    "group.passive",
    "group.semi",
    "group.input",
    "group.output",
    "group.modules",
];

/// One value a part can take. Most parts have no choices; a resistor, a
/// capacitor and an LED do, and the small window beside the drawing lets the
/// user pick one instead of crowding the palette with a button per value.
pub struct Variant {
    pub value: &'static str,
    pub kind: Kind,
}

pub const RESISTORS: [Variant; 5] = [
    Variant { value: "220 Ω", kind: Kind::Resistor },
    Variant { value: "1 кОм", kind: Kind::Resistor },
    Variant { value: "10 кОм", kind: Kind::Resistor },
    Variant { value: "100 кОм", kind: Kind::Resistor },
    Variant { value: "1 МОм", kind: Kind::Resistor },
];

pub const CAPACITORS: [Variant; 3] = [
    Variant { value: "100 нФ", kind: Kind::Capacitor },
    Variant { value: "10 мкФ", kind: Kind::Electrolytic },
    Variant { value: "100 мкФ", kind: Kind::Electrolytic },
];

pub const LEDS: [Variant; 3] = [
    Variant { value: "красный 5 мм", kind: Kind::Led },
    Variant { value: "зелёный 5 мм", kind: Kind::Led },
    Variant { value: "синий 5 мм", kind: Kind::Led },
];

/// The choices for a placed part, looked up by what it is.
pub fn variants_for(kind: Kind) -> &'static [Variant] {
    match kind {
        Kind::Resistor => &RESISTORS,
        Kind::Capacitor | Kind::Electrolytic => &CAPACITORS,
        Kind::Led => &LEDS,
        _ => &[],
    }
}

pub const PALETTE: [Place; 21] = [
    Place::Uno,
    Place::Resistor,
    Place::Capacitor,
    Place::Led,
    Place::RgbLed,
    Place::IrLed,
    Place::Diode,
    Place::Nmos,
    Place::Npn,
    Place::Thermistor,
    Place::Button,
    Place::Reed,
    Place::Ldr,
    Place::Motor,
    Place::Servo,
    Place::Buzzer,
    Place::Speaker,
    Place::Pot,
    Place::Relay,
    Place::Peltier,
    Place::Module,
];

/// How many undo steps to keep, and roughly how much heap they may hold.
/// Sixty copies of a small circuit cost a few hundred kilobytes; the byte
/// cap is what stops a dense one from eating the window's memory budget.
const HISTORY_STEPS: usize = 60;
const HISTORY_BYTES: usize = 4 << 20;

/// A rough size of a saved circuit, including the heap its strings and
/// vectors own. It does not have to be exact, only proportional, so the cap
/// bites at the right time.
fn history_bytes(c: &Circuit) -> usize {
    let text: usize = c.parts.iter().map(|p| p.label.len() + p.value.len()).sum();
    let heap = std::mem::size_of::<Circuit>()
        + c.parts.len() * (std::mem::size_of::<Part>() + 8)
        + c.pins.len() * (std::mem::size_of::<Pin>() + 4)
        + c.wires.len() * (std::mem::size_of::<Wire>() + 4)
        + c.wires.iter().map(|w| w.via.len() * std::mem::size_of::<Pt>()).sum::<usize>()
        + text;
    // Vector slack and the allocator's own headers.
    heap + heap / 4
}

pub struct Draft {
    pub circuit: Circuit,
    /// Which ready-made board a `Place::Module` click drops on the bench.
    pub module: &'static str,
    /// The first end of a wire that is being pulled.
    pub pending: Option<End>,
    pub selected: Option<usize>,
    undo: Vec<Circuit>,
    redo: Vec<Circuit>,
}

impl Default for Draft {
    fn default() -> Self {
        Self::new()
    }
}

impl Draft {
    pub fn new() -> Self {
        let mut circuit = Circuit::new();
        circuit.add_arduino(uno_spot());
        Self {
            circuit,
            module: crate::modules::DEFAULT,
            pending: None,
            selected: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Save a snapshot before a change; the stacks are what Ctrl+Z walks.
    ///
    /// Two limits keep the history honest: a count, so a long session cannot
    /// grow an endless menu of steps, and a byte estimate, because a circuit
    /// with a hundred wires is not the same size as an empty one.
    pub fn snapshot(&mut self) {
        self.undo.push(self.circuit.clone());
        let mut total: usize = self.undo.iter().map(history_bytes).sum();
        while self.undo.len() > HISTORY_STEPS
            || (total > HISTORY_BYTES && self.undo.len() > 1)
        {
            total -= history_bytes(&self.undo[0]);
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Drop the snapshot a drag pushed when it never moved the part.
    pub fn cancel_snapshot(&mut self) {
        self.undo.pop();
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(&mut self.circuit, prev));
        self.pending = None;
        self.selected = None;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(&mut self.circuit, next));
        self.pending = None;
        self.selected = None;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Take over a generated circuit and forget the AI history.
    pub fn adopt(&mut self, circuit: Circuit) {
        self.circuit = circuit;
        self.undo.clear();
        self.redo.clear();
        self.pending = None;
        self.selected = None;
    }

    pub fn clear(&mut self) {
        self.snapshot();
        let mut fresh = Circuit::new();
        fresh.add_arduino(uno_spot());
        self.circuit = fresh;
        self.pending = None;
        self.selected = None;
    }

    pub fn place(&mut self, what: Place, at: Pt) -> bool {
        if what == Place::Uno {
            if let Some(uno) = self.circuit.arduino {
                self.selected = Some(uno);
                return false;
            }
            self.snapshot();
            let idx = self.circuit.add_arduino(uno_spot());
            self.selected = Some(idx);
            return true;
        }
        // On a bare bench every part is a loose component, placed exactly
        // where the user clicked. On a breadboard only the axial and three-pin
        // parts plug into holes; the rest are free as well.
        if self.circuit.bare {
            let (kind, label, value, links) = self.spec(what);
            self.snapshot();
            let idx = self.add_bench(kind, &label, &value, at, links);
            self.selected = Some(idx);
            return true;
        }
        if what.on_board() {
            let Some((col, row)) = self.snap_hole(at) else { return false };
            self.snapshot();
            let ok = match what {
                Place::Resistor => self.axial(Kind::Resistor, "220 Ω", col, row, 3),
                Place::Capacitor => self.axial(Kind::Capacitor, "100 нФ", col, row, 1),
                Place::Led => self.axial(Kind::Led, "красный 5 мм", col, row, 2),
                Place::RgbLed => self.rgb(col, row),
                Place::IrLed => self.axial(Kind::Led, "ИК 940 нм", col, row, 1),
                Place::Diode => self.axial(Kind::Diode, "1N4007", col, row, 3),
                Place::Reed => self.axial(Kind::Reed, "геркон", col, row, 3),
                Place::Thermistor => self.axial(Kind::Thermistor, "10 кОм", col, row, 2),
                Place::Ldr => self.axial(Kind::Ldr, "GL5528", col, row, 2),
                Place::Nmos => self.three_pin(Kind::Nmos, "IRLZ44N", col, row, ["G", "S", "D"]),
                Place::Npn => self.three_pin(Kind::Npn, "2N2222", col, row, ["B", "C", "E"]),
                Place::Button => self.button(col, row),
                _ => false,
            };
            if !ok {
                self.undo.pop();
                self.redo.clear();
            }
            return ok;
        }
        // Bench parts go where the click landed. A click over the board would
        // bury the part in holes, so it slides below the board instead.
        let on_board = self.circuit.board.rect().contains(at);
        let spot = if on_board {
            match self.free_bench_spot(what) {
                Some(s) => s,
                None => return false,
            }
        } else {
            Pt::new(at.x.round(), at.y.round())
        };
        let (kind, label, value, links) = self.spec(what);
        self.snapshot();
        let idx = self.add_bench(kind, &label, &value, spot, links);
        self.selected = Some(idx);
        true
    }

    /// Kind, label, value and internal links for a palette entry.
    fn spec(&self, what: Place) -> (Kind, String, String, Vec<(&'static str, &'static str)>) {
        let mut links = Vec::new();
        let (kind, label, value) = match what {
            Place::Resistor => (Kind::Resistor, self.next_label("R"), "220 Ω".to_string()),
            Place::Capacitor => (Kind::Capacitor, self.next_label("C"), "100 нФ".to_string()),
            Place::Led => (Kind::Led, self.next_label("D"), "красный 5 мм".to_string()),
            Place::RgbLed => (Kind::Led, self.next_label("D"), "RGB 5 мм".to_string()),
            Place::IrLed => (Kind::Led, self.next_label("D"), "ИК 940 нм".to_string()),
            Place::Diode => (Kind::Diode, self.next_label("D"), "1N4007".to_string()),
            Place::Nmos => (Kind::Nmos, self.next_label("Q"), "IRLZ44N".to_string()),
            Place::Npn => (Kind::Npn, self.next_label("Q"), "2N2222".to_string()),
            Place::Reed => {
                links = vec![("1", "2")];
                (Kind::Reed, self.next_label("SW"), "геркон".to_string())
            }
            Place::Thermistor => (Kind::Thermistor, self.next_label("TH"), "10 кОм".to_string()),
            Place::Button => {
                links = vec![("1", "2"), ("3", "4")];
                (Kind::Button, self.next_label("B"), "такт 6×6".to_string())
            }
            Place::Ldr => (Kind::Ldr, self.next_label("LDR"), "GL5528".to_string()),
            Place::Motor => (Kind::Motor, self.next_label("M"), "5 В DC".to_string()),
            Place::Servo => (Kind::Servo, self.next_label("S"), "SG90".to_string()),
            Place::Buzzer => (Kind::Buzzer, self.next_label("BZ"), "пьезо".to_string()),
            Place::Speaker => (Kind::Speaker, self.next_label("LS"), "8 Ω".to_string()),
            Place::Pot => (Kind::Pot, self.next_label("RV"), "10 кОм".to_string()),
            Place::Relay => (Kind::Relay, self.next_label("K"), "SRD-05VDC".to_string()),
            Place::Peltier => (Kind::Peltier, self.next_label("TE"), "TEC1-12706".to_string()),
            Place::Module => (
                Kind::Module,
                self.next_label("U"),
                self.module.to_string(),
            ),
            Place::Uno => (Kind::Arduino, "U1".to_string(), "Arduino Uno R3".to_string()),
        };
        (kind, label, value, links)
    }

    /// Lay a part on the bench with its terminals at `at`.
    fn add_bench(
        &mut self,
        kind: Kind,
        label: &str,
        value: &str,
        at: Pt,
        links: Vec<(&'static str, &'static str)>,
    ) -> usize {
        let mut pins = Vec::new();
        for (name, dx, dy) in bench_pins(kind, value) {
            pins.push(PinSpec {
                name,
                pos: Pt::new(at.x + dx, at.y + dy),
                hole: Hole::OffBoard,
                kind: bench_pin_kind(kind, value, name),
            });
        }
        self.circuit.add_part(kind, label, value, at, pins, links)
    }

    /// Give a part another value, and for a capacitor another type. The pins
    /// stay where they are, so a part already plugged into the board keeps its
    /// holes; only the body and the colour code change.
    pub fn set_variant(&mut self, part: usize, value: &str, kind: Kind) -> bool {
        let Some(p) = self.circuit.parts.get(part) else { return false };
        if p.value == value && p.kind == kind {
            return false;
        }
        if !variants_for(p.kind).iter().any(|v| v.value == value && v.kind == kind) {
            return false;
        }
        self.snapshot();
        let p = &mut self.circuit.parts[part];
        p.kind = kind;
        p.value = value.to_string();
        true
    }

    /// Turn the selected part by quarter turns. Seated parts belong to the
    /// board and are moved with it instead.
    pub fn rotate_selected(&mut self, quarter: i32) -> bool {
        let Some(i) = self.selected_part() else { return false };
        if self.circuit.part_is_seated(&self.circuit.parts[i]) {
            return false;
        }
        self.snapshot();
        let ok = self.circuit.rotate_part(i, quarter);
        if !ok {
            self.undo.pop();
            self.redo.clear();
        }
        ok
    }

    /// Turn the breadboard together with everything plugged into it.
    pub fn rotate_board(&mut self, quarter: i32) -> bool {
        self.snapshot();
        let ok = self.circuit.rotate_board(quarter);
        if !ok {
            self.undo.pop();
            self.redo.clear();
        }
        ok
    }

    /// Hide the breadboard for a bench-only build. Refused while a part is
    /// still plugged in, so the drawing and the netlist never disagree.
    pub fn set_bare(&mut self, bare: bool) -> bool {
        if self.circuit.bare == bare {
            return true;
        }
        self.snapshot();
        let ok = self.circuit.set_bare(bare);
        if !ok {
            self.undo.pop();
            self.redo.clear();
        }
        ok
    }

    /// The nearest board hole to a click, so placement snaps like a real board.
    pub fn snap_hole(&self, at: Pt) -> Option<(u8, Row)> {
        if self.circuit.bare {
            return None;
        }
        let board = self.circuit.board;
        let mut best: Option<(f32, u8, Row)> = None;
        for col in 1..=board.cols {
            for row in [Row::A, Row::B, Row::C, Row::D, Row::E, Row::F, Row::G, Row::H, Row::I, Row::J] {
                let p = board.hole(col, row);
                let d = p.dist(at);
                if d < 1.9 && best.map(|(b, _, _)| d < b).unwrap_or(true) {
                    best = Some((d, col, row));
                }
            }
        }
        best.map(|(_, c, r)| (c, r))
    }

    fn axial(&mut self, kind: Kind, value: &str, col: u8, row: Row, span: u8) -> bool {
        let board = self.circuit.board;
        if col + span > board.cols {
            return false;
        }
        let a = board.hole(col, row);
        let z = board.hole(col + span, row);
        let label = self.next_label(match kind {
            Kind::Led => "D",
            Kind::Ldr => "LDR",
            Kind::Capacitor | Kind::Electrolytic => "C",
            _ => "R",
        });
        let p = self.circuit.add_part(
            kind,
            &label,
            value,
            a.mid(z),
            vec![
                PinSpec { name: "1", pos: a, hole: Hole::Col(col, row), kind: PinKind::Passive },
                PinSpec { name: "2", pos: z, hole: Hole::Col(col + span, row), kind: PinKind::Passive },
            ],
            vec![],
        );
        self.selected = Some(p);
        true
    }

    /// A four-leg RGB LED lying along one row: R, G, B and the common cathode.
    fn rgb(&mut self, col: u8, row: Row) -> bool {
        let board = self.circuit.board;
        if col + 3 > board.cols {
            return false;
        }
        let names = ["R", "G", "B", "-"];
        let mut pins = Vec::new();
        for (i, name) in names.iter().enumerate() {
            pins.push(PinSpec {
                name,
                pos: board.hole(col + i as u8, row),
                hole: Hole::Col(col + i as u8, row),
                kind: PinKind::Passive,
            });
        }
        let at = board.hole(col, row).mid(board.hole(col + 3, row));
        let label = self.next_label("D");
        let p = self.circuit.add_part(Kind::Led, &label, "RGB 5 мм", at, pins, vec![]);
        self.selected = Some(p);
        true
    }

    fn three_pin(&mut self, kind: Kind, value: &str, col: u8, row: Row, names: [&'static str; 3]) -> bool {
        let board = self.circuit.board;
        if col + 2 > board.cols {
            return false;
        }
        let mut pins = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let c = col + i as u8;
            pins.push(PinSpec {
                name,
                pos: board.hole(c, row),
                hole: Hole::Col(c, row),
                kind: match *name {
                    "S" | "E" => PinKind::Passive,
                    _ => PinKind::Signal,
                },
            });
        }
        let at = board.hole(col, row).mid(board.hole(col + 2, row));
        let label = self.next_label("Q");
        let p = self.circuit.add_part(kind, &label, value, at, pins, vec![]);
        self.selected = Some(p);
        true
    }

    fn button(&mut self, col: u8, row: Row) -> bool {
        let board = self.circuit.board;
        if col + 3 > board.cols {
            return false;
        }
        let row = if row.idx() < 5 { row } else { row.mirror() };
        let tops = [board.hole(col, row), board.hole(col + 3, row)];
        let bots = [board.hole(col, row.mirror()), board.hole(col + 3, row.mirror())];
        let pins = vec![
            PinSpec { name: "1", pos: tops[0], hole: Hole::Col(col, row), kind: PinKind::Passive },
            PinSpec { name: "2", pos: tops[1], hole: Hole::Col(col + 3, row), kind: PinKind::Passive },
            PinSpec { name: "3", pos: bots[0], hole: Hole::Col(col, row.mirror()), kind: PinKind::Passive },
            PinSpec { name: "4", pos: bots[1], hole: Hole::Col(col + 3, row.mirror()), kind: PinKind::Passive },
        ];
        let label = self.next_label("B");
        let p = self.circuit.add_part(
            Kind::Button,
            &label,
            "такт 6×6",
            tops[0].mid(bots[1]),
            pins,
            vec![("1", "2"), ("3", "4")],
        );
        self.selected = Some(p);
        true
    }

    /// A free spot below the board, where the bench parts live now that the
    /// Uno occupies the right-hand side.
    fn free_bench_spot(&self, what: Place) -> Option<Pt> {
        let board = self.circuit.board;
        let x = board.rect().x + 2.0;
        let (w, h) = match what {
            Place::Relay => (36.0, 24.0),
            Place::Servo => (20.0, 28.0),
            Place::Pot => (14.0, 19.0),
            _ => (32.0, 22.0),
        };
        let step = h + 5.0;
        let existing: Vec<Rect> = self
            .circuit
            .parts
            .iter()
            .filter(|p| p.kind != Kind::Arduino)
            .map(|p| self.circuit.part_rect(p))
            .collect();
        for i in 0..10 {
            let at = Pt::new(x, board.rect().bottom() + 6.0 + i as f32 * step);
            let rect = Rect::new(at.x - 5.0, at.y - 4.0, w, h);
            if !existing.iter().any(|r| overlap(r, &rect)) {
                return Some(at);
            }
        }
        None
    }

    fn next_label(&self, prefix: &str) -> String {
        let n = self
            .circuit
            .parts
            .iter()
            .filter(|p| p.label.starts_with(prefix))
            .count();
        format!("{prefix}{}", n + 1)
    }

    pub fn connect(&mut self, a: End, b: End) -> bool {
        if a == b || self.circuit.wires.iter().any(|w| (w.a == a && w.b == b) || (w.a == b && w.b == a)) {
            return false;
        }
        self.snapshot();
        let class = self.class_between(a, b);
        self.circuit.wire(a, b, class);
        true
    }

    pub fn class_between(&self, a: End, b: End) -> NetClass {
        let kind = |e: End| match e {
            End::Pin(p) => self.circuit.pins[p].kind,
            End::Hole(h) => match h {
                Hole::Rail(r, _, _) => match r {
                    Rail::TopPlus | Rail::BotPlus => PinKind::Power,
                    _ => PinKind::Ground,
                },
                _ => PinKind::Passive,
            },
        };
        let (ka, kb) = (kind(a), kind(b));
        if ka == PinKind::Ground || kb == PinKind::Ground {
            NetClass::Gnd
        } else if ka == PinKind::Power || kb == PinKind::Power {
            NetClass::Vcc
        } else if ka == PinKind::Analog || kb == PinKind::Analog {
            NetClass::Analog
        } else {
            NetClass::Signal
        }
    }

    /// Nearest pin to a click, in millimetres.
    pub fn pin_at(&self, at: Pt, radius: f32) -> Option<usize> {
        self.circuit
            .pins
            .iter()
            .enumerate()
            .filter(|(_, p)| p.pos.dist(at) <= radius)
            .min_by(|a, b| a.1.pos.dist(at).total_cmp(&b.1.pos.dist(at)))
            .map(|(i, _)| i)
    }

    /// A click anywhere: pin, hole, wire or part.
    pub fn end_at(&self, at: Pt, radius: f32) -> End {
        if let Some(p) = self.pin_at(at, radius) {
            return End::Pin(p);
        }
        if let Some((col, row)) = self.snap_hole(at) {
            return End::Hole(Hole::Col(col, row));
        }
        End::Hole(Hole::OffBoard)
    }

    pub fn part_at(&self, at: Pt) -> Option<usize> {
        // Footprints can touch on a crowded board. The smallest one under the
        // cursor wins, so the part whose case is drawn there is picked rather
        // than a neighbour that merely shares a bounding box.
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in self.circuit.parts.iter().enumerate() {
            let r = self.circuit.part_rect(p);
            if r.inflate(0.8).contains(at) {
                let area = r.w * r.h;
                if best.map(|(_, a)| area < a).unwrap_or(true) {
                    best = Some((i, area));
                }
            }
        }
        best.map(|(i, _)| i)
    }

    pub fn wire_at(&self, at: Pt, style: WireStyle) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for (i, w) in self.circuit.wires.iter().enumerate() {
            let pts = self.circuit.wire_points(w, style);
            let d = pts
                .windows(2)
                .map(|s| dist_to_segment(at, s[0], s[1]))
                .fold(f32::MAX, f32::min);
            if d < 1.5 && best.map(|(b, _)| d < b).unwrap_or(true) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// A hand-made bend under the pointer, as (wire, bend).
    pub fn via_at(&self, at: Pt) -> Option<(usize, usize)> {
        let mut best: Option<(f32, (usize, usize))> = None;
        for (i, w) in self.circuit.wires.iter().enumerate() {
            for (j, v) in w.via.iter().enumerate() {
                let d = v.dist(at);
                if d < 1.7 && best.map(|(b, _)| d < b).unwrap_or(true) {
                    best = Some((d, (i, j)));
                }
            }
        }
        best.map(|(_, h)| h)
    }

    /// Pull a new bend into a wire at `at`, on the leg closest to the click.
    /// The bend lands on the click's projection, so the cable does not jump
    /// away from the pointer. Returns the new bend's index.
    pub fn insert_via(&mut self, wire: usize, at: Pt, style: WireStyle) -> Option<usize> {
        let legs = self.circuit.wire_legs(self.circuit.wires.get(wire)?, style);
        let mut best: Option<(f32, usize, Pt)> = None;
        for (i, leg) in legs.iter().enumerate() {
            for seg in leg.windows(2) {
                let d = dist_to_segment(at, seg[0], seg[1]);
                if best.map(|(b, _, _)| d < b).unwrap_or(true) {
                    best = Some((d, i, project(at, seg[0], seg[1])));
                }
            }
        }
        let (_, leg, point) = best?;
        let w = &mut self.circuit.wires[wire];
        w.via.insert(leg, snap_via(point));
        Some(leg)
    }

    /// Move a bend. Returns false when it would not actually move, so the
    /// caller can keep the drag in the "nothing happened yet" state.
    pub fn move_via(&mut self, wire: usize, via: usize, to: Pt) -> bool {
        let Some(w) = self.circuit.wires.get_mut(wire) else { return false };
        let Some(v) = w.via.get_mut(via) else { return false };
        let to = snap_via(to);
        if v.dist(to) < 1e-3 {
            return false;
        }
        *v = to;
        true
    }

    /// Drop one bend, leaving the cable free to route itself again.
    pub fn remove_via(&mut self, wire: usize, via: usize) -> bool {
        let Some(w) = self.circuit.wires.get_mut(wire) else { return false };
        if via >= w.via.len() {
            return false;
        }
        w.via.remove(via);
        true
    }

    pub fn delete_at(&mut self, at: Pt, style: WireStyle) -> bool {
        if let Some((wire, via)) = self.via_at(at) {
            self.snapshot();
            self.circuit.wires[wire].via.remove(via);
            return true;
        }
        if let Some(part) = self.part_at(at) {
            if self.circuit.arduino == Some(part) {
                return false;
            }
            self.snapshot();
            self.circuit.remove_part(part);
            self.pending = None;
            self.selected = None;
            return true;
        }
        if let Some(w) = self.wire_at(at, style) {
            self.snapshot();
            self.circuit.remove_wire(w);
            return true;
        }
        false
    }

    pub fn selected_part(&self) -> Option<usize> {
        self.selected.filter(|&i| i < self.circuit.parts.len())
    }
}

fn overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// Bends snap to half a hole, which keeps hand-made corners on the same
/// rhythm as the board without forcing them onto a single row.
pub fn snap_via(p: Pt) -> Pt {
    let step = PITCH / 2.0;
    Pt::new((p.x / step).round() * step, (p.y / step).round() * step)
}

/// The point of the segment closest to `p`.
fn project(p: Pt, a: Pt, b: Pt) -> Pt {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = dx * dx + dy * dy;
    if len <= f32::EPSILON {
        return a;
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len).clamp(0.0, 1.0);
    Pt::new(a.x + dx * t, a.y + dy * t)
}

fn dist_to_segment(p: Pt, a: Pt, b: Pt) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = dx * dx + dy * dy;
    if len <= f32::EPSILON {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len).clamp(0.0, 1.0);
    p.dist(Pt::new(a.x + dx * t, a.y + dy * t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{self, Sev};
    use crate::parse::Intent;

    fn at(d: &Draft, col: u8, row: Row) -> Pt {
        d.circuit.board.hole(col, row)
    }

    /// Every pin must point at its own part, and every part at its own pins.
    fn consistent(c: &Circuit) -> bool {
        for (i, pin) in c.pins.iter().enumerate() {
            if pin.part >= c.parts.len() {
                return false;
            }
            if !c.parts[pin.part].pins.contains(&i) {
                return false;
            }
        }
        for (i, part) in c.parts.iter().enumerate() {
            for &p in &part.pins {
                if p >= c.pins.len() || c.pins[p].part != i {
                    return false;
                }
            }
        }
        for w in &c.wires {
            for e in [w.a, w.b] {
                if let End::Pin(p) = e {
                    if p >= c.pins.len() {
                        return false;
                    }
                }
            }
        }
        true
    }

    #[test]
    fn every_palette_part_lands_on_the_bench() {
        for place in PALETTE {
            let mut d = Draft::new();
            // The Uno is already on the bench; its palette button puts the
            // cursor on that board instead of adding a second one.
            if place == Place::Uno {
                assert!(!d.place(Place::Uno, Pt::default()));
                assert_eq!(d.circuit.parts.len(), 1);
                assert_eq!(d.selected, d.circuit.arduino);
                continue;
            }
            let spot = if place.on_board() {
                at(&d, 8, Row::C)
            } else {
                Pt::new(100.0, 100.0)
            };
            assert!(d.place(place, spot), "{place:?} could not be placed");
            assert_eq!(d.circuit.parts.len(), 2, "{place:?}");
            assert!(consistent(&d.circuit), "{place:?} broke the pin indices");
            let part = &d.circuit.parts[1];
            assert!(!part.pins.is_empty(), "{place:?} has no pins");
            if place.on_board() && place != Place::Uno {
                for &p in &part.pins {
                    assert_ne!(
                        d.circuit.pins[p].hole,
                        Hole::OffBoard,
                        "{place:?} hangs off the board"
                    );
                }
            }
            d.undo();
            assert!(consistent(&d.circuit));
        }
    }

    #[test]
    fn two_bench_parts_do_not_overlap() {
        let mut d = Draft::new();
        assert!(d.place(Place::Motor, Pt::new(2.0, 80.0)));
        assert!(d.place(Place::Motor, Pt::new(40.0, 80.0)));
        let a = d.circuit.part_rect(&d.circuit.parts[1]);
        let b = d.circuit.part_rect(&d.circuit.parts[2]);
        assert!(!overlap(&a, &b), "motors stacked on top of each other");
    }

    /// The user asked to arrange parts by hand, so a click out on the bench is
    /// the position, not a hint to pick the next free slot.
    #[test]
    fn a_bench_part_lands_where_it_was_clicked() {
        let mut d = Draft::new();
        let at = Pt::new(12.0, 74.0);
        assert!(d.place(Place::Buzzer, at));
        assert_eq!(d.circuit.parts[1].at, at);
        let pin = d.circuit.parts[1].pins[0];
        assert_eq!(d.circuit.pins[pin].pos, Pt::new(17.0, 71.0));
    }

    #[test]
    fn wiring_and_undo_keep_the_netlist() {
        let mut d = Draft::new();
        d.place(Place::Resistor, at(&d, 5, Row::A));
        let r = d.circuit.parts[1].pins[0];
        let gnd = d.circuit.pin(0, "GND1");
        assert!(d.connect(End::Pin(r), End::Pin(gnd)));
        assert_eq!(d.circuit.wires.len(), 1);
        let roots = d.circuit.nets();
        assert_eq!(roots.root(r), roots.root(gnd));
        assert!(d.undo());
        assert!(d.circuit.wires.is_empty());
        assert!(d.redo());
        assert_eq!(d.circuit.wires.len(), 1);
        assert!(consistent(&d.circuit));
    }

    #[test]
    fn a_wire_too_is_not_tied_twice() {
        let mut d = Draft::new();
        let a = End::Pin(d.circuit.pin(0, "D9"));
        let b = End::Pin(d.circuit.pin(0, "GND1"));
        assert!(d.connect(a, b));
        assert!(!d.connect(a, b), "duplicate wire accepted");
        assert!(!d.connect(b, a), "reversed duplicate accepted");
        assert_eq!(d.circuit.wires.len(), 1);
    }

    #[test]
    fn removing_a_part_takes_its_wires() {
        let mut d = Draft::new();
        d.place(Place::Led, at(&d, 5, Row::A));
        let led = d.circuit.parts[1].pins[0];
        d.connect(End::Pin(led), End::Hole(Hole::Col(5, Row::A)));
        assert_eq!(d.circuit.wires.len(), 1);
        d.circuit.remove_part(1);
        assert_eq!(d.circuit.parts.len(), 1);
        assert!(d.circuit.wires.is_empty());
        assert_eq!(d.circuit.pins.len(), 32);
        assert!(consistent(&d.circuit));
    }

    #[test]
    fn wire_colour_follows_the_pins() {
        let d = Draft::new();
        let gnd = d.circuit.pin(0, "GND1");
        let sig = d.circuit.pin(0, "D9");
        let pwr = d.circuit.pin(0, "5V");
        let ana = d.circuit.pin(0, "A0");
        assert_eq!(d.class_between(End::Pin(gnd), End::Pin(sig)), NetClass::Gnd);
        assert_eq!(d.class_between(End::Pin(pwr), End::Pin(sig)), NetClass::Vcc);
        assert_eq!(d.class_between(End::Pin(ana), End::Pin(sig)), NetClass::Analog);
        assert_eq!(d.class_between(End::Pin(sig), End::Pin(sig)), NetClass::Signal);
        assert_eq!(
            d.class_between(End::Hole(Hole::Rail(Rail::BotPlus, RailHalf::Right, 0)), End::Pin(sig)),
            NetClass::Vcc
        );
        assert_eq!(
            d.class_between(End::Hole(Hole::Rail(Rail::BotMinus, RailHalf::Left, 0)), End::Pin(sig)),
            NetClass::Gnd
        );
    }

    #[test]
    fn clicks_find_pins_before_holes() {
        let d = Draft::new();
        let pin = d.circuit.pin(0, "D9");
        let pos = d.circuit.pins[pin].pos;
        assert_eq!(d.end_at(pos, 1.6), End::Pin(pin));
        // A hole away from every pin is a hole.
        let far = d.circuit.board.hole(30, Row::J);
        assert_eq!(d.end_at(far, 0.4), End::Hole(Hole::Col(30, Row::J)));
        // And empty bench is not an end at all.
        assert_eq!(d.end_at(Pt::new(200.0, 200.0), 1.0), End::Hole(Hole::OffBoard));
    }

    #[test]
    fn delete_hits_parts_and_wires() {
        let mut d = Draft::new();
        d.place(Place::Resistor, at(&d, 5, Row::A));
        let mid = d.circuit.parts[1].at;
        assert_eq!(d.part_at(mid), Some(1));
        assert!(d.delete_at(mid, WireStyle::Arc));
        assert_eq!(d.circuit.parts.len(), 1);
        assert!(!d.delete_at(mid, WireStyle::Arc), "nothing left to delete");

        let a = End::Pin(d.circuit.pin(0, "D9"));
        let b = End::Hole(Hole::Col(20, Row::J));
        d.connect(a, b);
        let pa = d.circuit.end_pos(a);
        let pb = d.circuit.end_pos(b);
        let (p1, p2) = d.circuit.wire_controls(&d.circuit.wires[0]);
        let somewhere = Pt::new(
            0.125 * pa.x + 0.375 * p1.x + 0.375 * p2.x + 0.125 * pb.x,
            0.125 * pa.y + 0.375 * p1.y + 0.375 * p2.y + 0.125 * pb.y,
        );
        assert!(
            d.wire_at(somewhere, WireStyle::Arc).is_some(),
            "arched wire not found under the cursor"
        );
        let mid = Pt::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0);
        assert!(
            d.wire_at(mid, WireStyle::Straight).is_some(),
            "straight wire not found at its midpoint"
        );
    }

    /// The routing changes where the cable is, so the hit test has to follow.
    #[test]
    fn the_hit_test_follows_the_routing() {
        let mut d = Draft::new();
        let a = End::Pin(d.circuit.pin(0, "D9"));
        let b = End::Hole(Hole::Col(20, Row::J));
        assert!(d.connect(a, b));
        let pa = d.circuit.end_pos(a);
        let pb = d.circuit.end_pos(b);
        let mid = Pt::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0);
        let (p1, p2) = d.circuit.wire_controls(&d.circuit.wires[0]);
        let arc_mid = Pt::new(
            0.125 * pa.x + 0.375 * p1.x + 0.375 * p2.x + 0.125 * pb.x,
            0.125 * pa.y + 0.375 * p1.y + 0.375 * p2.y + 0.125 * pb.y,
        );
        assert!(d.wire_at(mid, WireStyle::Straight).is_some());
        assert!(
            d.wire_at(arc_mid, WireStyle::Arc).is_some(),
            "the arc is not where its own curve says it is"
        );
        // High above the straight run there is nothing to grab.
        let above = Pt::new(mid.x, arc_mid.y);
        assert!(
            d.wire_at(above, WireStyle::Straight).is_none(),
            "a straight wire should not answer for the empty space above it"
        );
        assert!(d.wire_at(above, WireStyle::Arc).is_some(), "the arc is missing");
    }

    #[test]
    fn an_ai_circuit_survives_the_handover() {
        let intent = Intent {
            load: crate::parse::Load::Motor,
            input: Some(crate::parse::Input::Button),
            transistor: true,
            presses: true,
            lesson: None,
            action: None,
            raw: String::new(),
            condition: None,
            recipe: None,
        };
        let built = crate::templates::build(&intent, 0, crate::text::Lang::Ru).unwrap();
        let mut d = Draft::new();
        d.adopt(built.circuit.clone());
        assert!(consistent(&d.circuit));
        assert_eq!(d.circuit.wires.len(), built.circuit.wires.len());
        assert!(!d.can_undo(), "handover must start with a clean history");

        let report = checks::run_circuit(&d.circuit, crate::text::Lang::Ru, None);
        let failed: Vec<&str> = report
            .checks
            .iter()
            .filter(|c| c.sev == Sev::Fail)
            .map(|c| c.name.ru)
            .collect();
        assert!(failed.is_empty(), "handover fails checks: {failed:?}");
    }

    #[test]
    fn an_empty_draft_has_only_the_board_and_the_uno() {
        let d = Draft::new();
        assert_eq!(d.circuit.parts.len(), 1);
        assert_eq!(d.circuit.parts[0].kind, Kind::Arduino);
        assert!(d.circuit.wires.is_empty());
        assert_eq!(d.circuit.pins.len(), 32);
        let report = checks::run_circuit(&d.circuit, crate::text::Lang::Ru, None);
        assert_ne!(
            report.worst(),
            Sev::Pass,
            "an empty board must not read as finished"
        );
    }

    /// Turning a loose part is an edit and must be undoable; a part plugged
    /// into the board is turned with the board instead.
    #[test]
    fn the_selected_part_turns_and_undo_puts_it_back() {
        let mut d = Draft::new();
        assert!(d.place(Place::Servo, Pt::new(10.0, 74.0)));
        let idx = d.selected.unwrap();
        let before = d.circuit.pins[d.circuit.parts[idx].pins[0]].pos;
        assert!(d.rotate_selected(1));
        assert_eq!(d.circuit.parts[idx].rot, 1);
        assert!(d.circuit.pins[d.circuit.parts[idx].pins[0]].pos.dist(before) > 0.5);
        assert!(d.undo());
        assert_eq!(d.circuit.parts[idx].rot, 0);
        assert!(d.circuit.pins[d.circuit.parts[idx].pins[0]].pos.dist(before) < 1e-3);
    }

    /// A part in a hole belongs to the board: turning it alone is refused, and
    /// turning the board carries it.
    #[test]
    fn the_board_turns_its_plugged_parts() {
        let mut d = Draft::new();
        assert!(d.place(Place::Resistor, at(&d, 5, Row::A)));
        let idx = d.selected.unwrap();
        assert!(!d.rotate_selected(1), "a seated part must not turn on its own");
        let before = d.circuit.pins[d.circuit.parts[idx].pins[0]].pos;
        assert!(d.rotate_board(1));
        assert_ne!(d.circuit.pins[d.circuit.parts[idx].pins[0]].pos, before);
        for &pin in d.circuit.parts[idx].pins.clone().iter() {
            assert_eq!(Some(d.circuit.pins[pin].pos), d.circuit.hole_pos(d.circuit.pins[pin].hole));
        }
        assert!(d.undo());
        assert_eq!(d.circuit.board.rot, 0);
    }

    /// Bare mode lays every part out on the bench: no holes, no board.
    #[test]
    fn a_bare_draft_places_parts_on_the_bench() {
        let mut d = Draft::new();
        assert!(d.set_bare(true));
        assert!(d.circuit.bare);
        assert!(d.snap_hole(Pt::new(20.0, 20.0)).is_none());
        let at = Pt::new(20.0, 74.0);
        assert!(d.place(Place::Resistor, at));
        let idx = d.selected.unwrap();
        assert_eq!(d.circuit.parts[idx].at, at);
        for &pin in &d.circuit.parts[idx].pins {
            assert_eq!(d.circuit.pins[pin].hole, Hole::OffBoard);
        }
        // The board is not painted, so its holes cannot pull the frame around.
        let scene = crate::scene::draw(&d.circuit, &crate::theme::Theme::default(), &crate::scene::Opts::default());
        assert!(scene.h > 0.0);
        assert!(scene.bounds().h > 0.0);
        // Going back is always allowed and keeps the bench parts.
        assert!(d.set_bare(false));
        assert_eq!(d.circuit.parts.len(), 2);
    }

    /// Turning the board off is refused while a part is plugged in, so the
    /// hidden board never keeps a part alive in the netlist.
    #[test]
    fn bare_mode_waits_for_the_board_to_be_empty() {
        let mut d = Draft::new();
        assert!(d.place(Place::Led, at(&d, 5, Row::A)));
        assert!(!d.set_bare(true));
        assert!(!d.circuit.bare);
    }

    /// A resistor sits above the transistor that shares its columns. Clicking
    /// the transistor's case must pick the transistor, not the resistor whose
    /// bounding box reaches down to it.
    #[test]
    fn the_part_under_the_cursor_is_picked() {
        let mut d = Draft::new();
        let b = d.circuit.board;
        let q = d.circuit.add_part(
            Kind::Nmos,
            "Q1",
            "IRLZ44N",
            b.hole(25, Row::E).mid(b.hole(27, Row::E)),
            vec![
                PinSpec { name: "G", pos: b.hole(25, Row::E), hole: Hole::Col(25, Row::E), kind: PinKind::Signal },
                PinSpec { name: "S", pos: b.hole(26, Row::E), hole: Hole::Col(26, Row::E), kind: PinKind::Passive },
                PinSpec { name: "D", pos: b.hole(27, Row::E), hole: Hole::Col(27, Row::E), kind: PinKind::Signal },
            ],
            vec![],
        );
        let r = d.circuit.add_part(
            Kind::Resistor,
            "R2",
            "10 кОм",
            b.hole(25, Row::A).mid(b.hole(28, Row::A)),
            vec![
                PinSpec { name: "1", pos: b.hole(25, Row::A), hole: Hole::Col(25, Row::A), kind: PinKind::Passive },
                PinSpec { name: "2", pos: b.hole(28, Row::A), hole: Hole::Col(28, Row::A), kind: PinKind::Passive },
            ],
            vec![],
        );
        let case = Pt::new(b.hole(26, Row::E).x, b.hole(26, Row::E).y - 4.2);
        assert_eq!(d.part_at(case), Some(q), "clicking the case picked the resistor");
        assert_eq!(d.part_at(b.hole(26, Row::A)), Some(r));
        // Well clear of both is empty bench.
        assert_eq!(d.part_at(Pt::new(10.0, 90.0)), None);
    }

    /// A cable on the bench between a resistor and the Uno's ground.
    fn bendable() -> Draft {
        let mut d = Draft::new();
        d.place(Place::Resistor, at(&d, 5, Row::A));
        let r = d.circuit.parts[1].pins[0];
        let gnd = d.circuit.pin(0, "GND1");
        assert!(d.connect(End::Pin(r), End::Pin(gnd)));
        d
    }

    /// Pull a bend out of a cable, move it, grab it again and drop it.
    #[test]
    fn a_cable_bend_can_be_pulled_moved_and_dropped() {
        let mut d = bendable();
        let style = WireStyle::Ortho;
        let pts = d.circuit.wire_points(&d.circuit.wires[0], style);
        let mid = pts[0].lerp(pts[1], 0.5);
        let idx = d.insert_via(0, mid, style).expect("no bend inserted");
        assert_eq!(d.circuit.wires[0].via.len(), 1);
        assert_eq!(idx, 0);

        let pulled = Pt::new(mid.x, mid.y + 12.0);
        assert!(d.move_via(0, idx, pulled));
        let target = snap_via(pulled);
        assert_eq!(d.circuit.wires[0].via[0], target);
        let pts = d.circuit.wire_points(&d.circuit.wires[0], style);
        assert!(
            pts.iter().any(|p| p.dist(target) < 1e-3),
            "the cable ignored the moved bend: {pts:?}"
        );
        for seg in pts.windows(2) {
            let flat = (seg[0].x - seg[1].x).abs() < 1e-3;
            let tall = (seg[0].y - seg[1].y).abs() < 1e-3;
            assert!(flat || tall, "a pulled bend went diagonal");
        }

        assert_eq!(d.via_at(target), Some((0, 0)), "the bend cannot be grabbed");
        assert!(d.delete_at(target, style));
        assert!(d.circuit.wires[0].via.is_empty());
        assert!(d.circuit.wires.len() == 1, "deleting a bend took the cable");
    }

    /// A second bend lands on the leg that was grabbed, not at the ends.
    #[test]
    fn a_second_bend_keeps_the_order_of_the_stops() {
        let mut d = bendable();
        let style = WireStyle::Ortho;
        let pts = d.circuit.wire_points(&d.circuit.wires[0], style);
        let first = d.insert_via(0, pts[0].lerp(pts[1], 0.4), style).unwrap();
        assert_eq!(first, 0);
        let legs = d.circuit.wire_legs(&d.circuit.wires[0], style);
        assert_eq!(legs.len(), 2);
        // A point on the second leg, so the leg is unambiguous.
        let grabbed = legs[1][0].lerp(legs[1][1], 0.5);
        let second = d.insert_via(0, grabbed, style).unwrap();
        assert_eq!(second, 1, "the new bend jumped to the wrong leg");
        let stops = d.circuit.wire_stops(&d.circuit.wires[0]);
        assert_eq!(stops.len(), 4);
        assert!(d.move_via(0, 0, stops[1].off(-4.0, 6.0)));
        assert!(d.move_via(0, 1, stops[2].off(4.0, 6.0)));
    }

    /// One palette button per part, and the value comes from the little
    /// window. Changing it must not pull a seated part out of its holes.
    #[test]
    fn a_placed_part_can_take_another_value_in_place() {
        let mut d = Draft::new();
        assert!(d.place(Place::Resistor, at(&d, 4, Row::A)));
        assert_eq!(d.circuit.parts[1].value, "220 Ω");
        let holes: Vec<Hole> = d.circuit.parts[1]
            .pins
            .iter()
            .map(|&p| d.circuit.pins[p].hole)
            .collect();
        for value in ["1 кОм", "10 кОм", "100 кОм", "1 МОм"] {
            assert!(d.set_variant(1, value, Kind::Resistor), "{value}");
            assert_eq!(d.circuit.parts[1].value, value);
            let now: Vec<Hole> = d.circuit.parts[1]
                .pins
                .iter()
                .map(|&p| d.circuit.pins[p].hole)
                .collect();
            assert_eq!(now, holes, "{value} moved the part");
        }

        assert!(d.place(Place::Capacitor, at(&d, 12, Row::A)));
        let cap = d.circuit.parts.len() - 1;
        let holes: Vec<Hole> = d.circuit.parts[cap]
            .pins
            .iter()
            .map(|&p| d.circuit.pins[p].hole)
            .collect();
        assert!(d.set_variant(cap, "10 мкФ", Kind::Electrolytic));
        assert_eq!(d.circuit.parts[cap].kind, Kind::Electrolytic);
        let now: Vec<Hole> = d.circuit.parts[cap]
            .pins
            .iter()
            .map(|&p| d.circuit.pins[p].hole)
            .collect();
        assert_eq!(now, holes, "swapping the capacitor type moved it");
        assert!(!d.set_variant(cap, "220 Ω", Kind::Resistor), "wrong family accepted");
        assert!(consistent(&d.circuit));
    }

    /// The palette offers one entry per part, with its choices behind it.
    #[test]
    fn variant_parts_offer_their_values() {
        assert_eq!(variants_for(Kind::Resistor).len(), 5);
        assert_eq!(variants_for(Kind::Capacitor).len(), 3);
        assert_eq!(variants_for(Kind::Electrolytic).len(), 3);
        assert_eq!(variants_for(Kind::Led).len(), 3);
        assert!(variants_for(Kind::Diode).is_empty());
        // One button per part: the values live behind the button, not beside
        // it in a wall of near-identical entries. Group 0 is the board, group
        // 1 the two passive kinds.
        assert_eq!(PALETTE.iter().filter(|p| p.group() == 0).count(), 1);
        assert_eq!(PALETTE.iter().filter(|p| p.group() == 1).count(), 2);
    }

    /// In a bare build a part becomes a loose component, and picking another
    /// value still works on it.
    #[test]
    fn a_bare_part_keeps_its_value_choices() {
        let mut d = Draft::new();
        assert!(d.set_bare(true));
        assert!(d.place(Place::Capacitor, Pt::new(20.0, 90.0)));
        assert_eq!(d.circuit.parts[1].value, "100 нФ");
        assert!(d.set_variant(1, "100 мкФ", Kind::Electrolytic));
        assert_eq!(d.circuit.parts[1].value, "100 мкФ");
        assert!(d.place(Place::Resistor, Pt::new(40.0, 90.0)));
        assert!(d.set_variant(2, "10 кОм", Kind::Resistor));
        assert!(consistent(&d.circuit));
    }

    /// Each ready-made board has its own palette button now, and one click
    /// has to drop it on the bench with its pins.
    #[test]
    fn every_module_button_places_its_board() {
        for m in crate::modules::ALL {
            let mut d = Draft::new();
            d.module = m.name;
            assert!(
                d.place(Place::Module, Pt::new(90.0, 100.0)),
                "{} could not be placed",
                m.name
            );
            assert_eq!(d.circuit.parts.len(), 2, "{}", m.name);
            assert_eq!(d.circuit.parts[1].value, m.name);
            assert_eq!(d.circuit.parts[1].kind, Kind::Module);
            assert!(!d.circuit.parts[1].pins.is_empty(), "{} has no pins", m.name);
            assert!(consistent(&d.circuit), "{}", m.name);
        }
    }

    /// The two captions exist for a reason: the palette cell is narrow, the
    /// bench label is not.
    #[test]
    fn short_names_fit_the_palette_and_keep_the_long_one() {
        for lang in [Lang::Ru, Lang::En] {
            for place in PALETTE {
                let short = place.short_name(lang);
                assert!(!short.is_empty(), "{place:?}");
                assert!(
                    short.chars().count() <= 13,
                    "{place:?} caption {short:?} is too wide"
                );
                assert!(!place.name(lang).is_empty());
            }
        }
        assert_eq!(Place::Nmos.short_name(Lang::Ru), "MOSFET");
        assert!(Place::Nmos.name(Lang::Ru).contains("IRLZ44N"));
        assert_eq!(Place::Npn.short_name(Lang::En), "2N2222");
    }

    /// The history honours a step count and a byte budget, so a busy bench
    /// cannot grow the window's memory without bound.
    #[test]
    fn history_respects_its_size_budget() {
        use crate::model::{End, Hole, NetClass};

        // A plain circuit hits the step cap first.
        let mut d = Draft::new();
        for _ in 0..200 {
            d.snapshot();
        }
        assert_eq!(d.undo.len(), HISTORY_STEPS);

        // A dense bench hits the byte cap: older steps are dropped, the
        // newest one always survives.
        let mut d = Draft::new();
        d.circuit.wire(
            End::Hole(Hole::Col(1, Row::A)),
            End::Hole(Hole::Col(2, Row::A)),
            NetClass::Signal,
        );
        let seed = d.circuit.wires[0].clone();
        for _ in 0..1200 {
            d.circuit.wires.push(seed.clone());
        }
        let one = history_bytes(&d.circuit);
        assert!(one * 4 < HISTORY_BYTES, "fixture is too small to test the cap");
        for _ in 0..HISTORY_STEPS {
            d.snapshot();
        }
        let total: usize = d.undo.iter().map(history_bytes).sum();
        assert!(
            total <= HISTORY_BYTES || d.undo.len() == 1,
            "{total} bytes in {} steps",
            d.undo.len()
        );
        assert!(d.undo.len() < HISTORY_STEPS, "the cap never bit");
        assert!(d.can_undo());
    }
}
