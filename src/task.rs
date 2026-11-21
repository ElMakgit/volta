//! Layer 4: a task as typed slots, not as a bag of guessed flags.
//!
//! The parser still reads words with tables, but it no longer votes a `Load`
//! into office: every recognised part becomes a slot with its evidence, and a
//! plan is judged by the slots it actually closes. A tie is shown as two
//! readings, never settled by the order of a keyword array.

use std::collections::BTreeMap;

use crate::parse::{Action, Input, Load, NamedDevices, ScreenKind, Screens};
use crate::spec::{Composition, Condition, SinkId, SourceId};
use crate::text::{Lang, L};

/// What a sentence asks to drive. It is wider than [`Load`] because a relay
/// is a real output that the template router does not treat as a motor.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum OutputKind {
    Led,
    Lamp,
    Motor,
    Motors,
    Stepper,
    Servo,
    Buzzer,
    Relay,
    Rfid,
    Radio433,
    RadioNrf,
    Bluetooth,
    Gsm,
    Ethernet,
}

impl OutputKind {
    pub fn load(self) -> Option<Load> {
        Some(match self {
            OutputKind::Led => Load::Led,
            OutputKind::Lamp => Load::Lamp,
            OutputKind::Motor => Load::Motor,
            OutputKind::Motors => Load::Motors,
            OutputKind::Stepper => Load::Stepper,
            OutputKind::Servo => Load::Servo,
            OutputKind::Buzzer => Load::Buzzer,
            OutputKind::Rfid => Load::Rfid,
            OutputKind::Radio433 => Load::Radio433,
            OutputKind::RadioNrf => Load::RadioNrf,
            OutputKind::Bluetooth => Load::Bluetooth,
            OutputKind::Gsm => Load::Gsm,
            OutputKind::Ethernet => Load::Ethernet,
            OutputKind::Relay => return None,
        })
    }

    pub fn label(self) -> L {
        match self {
            OutputKind::Led => L::new("светодиод", "LED"),
            OutputKind::Lamp => L::new("лампа накаливания", "incandescent lamp"),
            OutputKind::Motor => L::new("двигатель", "motor"),
            OutputKind::Motors => L::new("драйвер двигателей", "motor driver"),
            OutputKind::Stepper => L::new("шаговый двигатель", "stepper"),
            OutputKind::Servo => L::new("сервопривод", "servo"),
            OutputKind::Buzzer => L::new("звук", "sound"),
            OutputKind::Relay => L::new("реле", "relay"),
            OutputKind::Rfid => L::new("RFID-считыватель", "RFID reader"),
            OutputKind::Radio433 => L::new("радио 433 МГц", "433 MHz radio"),
            OutputKind::RadioNrf => L::new("радио NRF24L01", "NRF24L01 radio"),
            OutputKind::Bluetooth => L::new("Bluetooth-модуль", "Bluetooth module"),
            OutputKind::Gsm => L::new("GSM-модем", "GSM modem"),
            OutputKind::Ethernet => L::new("Ethernet", "Ethernet"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum LinkKind {
    Rfid,
    Radio433,
    RadioNrf,
    Bluetooth,
    Gsm,
    Ethernet,
    Ir,
}

impl LinkKind {
    pub fn label(self) -> L {
        match self {
            LinkKind::Rfid => L::new("RFID", "RFID"),
            LinkKind::Radio433 => L::new("радио 433 МГц", "433 MHz radio"),
            LinkKind::RadioNrf => L::new("радио NRF24L01", "NRF24L01 radio"),
            LinkKind::Bluetooth => L::new("Bluetooth", "Bluetooth"),
            LinkKind::Gsm => L::new("GSM", "GSM"),
            LinkKind::Ethernet => L::new("Ethernet", "Ethernet"),
            LinkKind::Ir => L::new("ИК-приёмник", "IR receiver"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum LinkRole {
    Reader,
    Transmitter,
    Receiver,
    Peer,
    Gateway,
}

/// One named output and the words that named it. `specificity` is 3 for a
/// board or a model, 2 for a technical noun and 1 for an everyday synonym;
/// it is evidence, not a vote, and it is shown in the trace.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputSlot {
    pub kind: OutputKind,
    pub count: usize,
    pub words: Vec<String>,
    pub specificity: u8,
    /// First token position, so two equal readings can be ordered by what the
    /// sentence said first instead of by a table index.
    pub at: usize,
}

impl OutputSlot {
    /// Evidence weight. A count is a build instruction, not extra proof that
    /// the sentence is about this part: "two motors" is not a stronger claim
    /// than a named driver board.
    pub fn weight(&self) -> u32 {
        u32::from(self.specificity)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlSlot {
    pub kind: Input,
    pub count: usize,
    pub words: Vec<String>,
    pub specificity: u8,
    pub at: usize,
}

impl ControlSlot {
    pub fn weight(&self) -> u32 {
        u32::from(self.specificity)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SensorSlot {
    pub source: SourceId,
    pub count: usize,
    pub words: Vec<String>,
    pub condition: Option<Condition>,
    pub at: usize,
}

impl SensorSlot {
    pub fn weight(&self) -> u32 {
        3 * (self.count.clamp(1, 3) as u32)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenSlot {
    pub kind: ScreenKind,
    pub count: usize,
    pub matrix: bool,
    pub segments: bool,
    pub words: Vec<String>,
    pub at: usize,
}

impl ScreenSlot {
    pub fn weight(&self) -> u32 {
        3 * (self.count.clamp(1, 3) as u32)
    }

    pub fn from_screens(screens: Screens, words: Vec<String>, at: usize) -> Self {
        Self {
            kind: screens.kind,
            count: screens.count,
            matrix: screens.matrix,
            segments: screens.segments,
            words,
            at,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum CountKind {
    Led,
    Rgb,
    Button,
    Motor,
    Servo,
    Buzzer,
    Relay,
    Screen,
    Sensor(SourceId),
}

/// Typed counts: "three buttons and two LEDs" is two entries, not a pair of
/// optional integers that the builder has to remember to read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts(pub BTreeMap<CountKind, usize>);

impl Counts {
    pub fn set(&mut self, kind: CountKind, count: usize) {
        if count > 0 {
            self.0.insert(kind, count);
        }
    }

    pub fn get(&self, kind: CountKind) -> usize {
        self.0.get(&kind).copied().unwrap_or(0)
    }
}

/// A typed reading of one sentence.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Task {
    pub sensors: Vec<SensorSlot>,
    pub outputs: Vec<OutputSlot>,
    pub links: Vec<(LinkKind, LinkRole)>,
    pub counts: Counts,
    pub conditions: Vec<Condition>,
    pub screens: Vec<ScreenSlot>,
    pub controls: Vec<ControlSlot>,
    pub behaviors: Vec<Action>,
    pub named: NamedDevices,
    /// A book lesson, when the printed table is the reading.
    pub lesson: Option<usize>,
    /// The composed reading: a sensor, a sink and a condition.
    pub recipe: Option<Composition>,
    /// A text message on screens, which is not a sensor readout.
    pub text_screens: Option<Screens>,
    pub transistor: bool,
    pub presses: bool,
    /// The output the parser recommends, and the controls it reads with it.
    pub primary: Option<OutputKind>,
    pub primary_control: Option<Input>,
    pub raw: String,
    /// How much of the typed task the recommended reading closes, 0..1.
    pub confidence: f32,
    /// Slots the recommended reading leaves open, for the explanation.
    pub missing: Vec<SlotName>,
}

impl Task {
    pub fn new(raw: impl Into<String>) -> Self {
        Self {
            raw: raw.into(),
            ..Self::default()
        }
    }

    pub fn output(&self, kind: OutputKind) -> Option<&OutputSlot> {
        self.outputs.iter().find(|s| s.kind == kind)
    }

    /// A screen plus controls and no named output: the phrase reads as
    /// "show what the buttons do", not as an LED switch with an ignored
    /// display.
    pub fn wants_buttons_screen(&self) -> bool {
        self.screens
            .iter()
            .any(|s| !s.matrix && !s.segments && s.count == 1)
            && !self.controls.is_empty()
            && !self.controls.iter().any(|c| c.kind != Input::Button)
            && self.outputs.is_empty()
            && self.recipe.is_none()
            && self.lesson.is_none()
            && self.text_screens.is_none()
    }

    pub fn control(&self, kind: Input) -> Option<&ControlSlot> {
        self.controls.iter().find(|s| s.kind == kind)
    }

    /// The primary output as a template load. A relay has no load of its own:
    /// it is a sink of a composition or an added part, never a blink route.
    pub fn load(&self) -> Load {
        self.primary
            .and_then(OutputKind::load)
            .unwrap_or(Load::Led)
    }

    pub fn total_weight(&self) -> u32 {
        let outputs: u32 = self.outputs.iter().map(OutputSlot::weight).sum();
        let controls: u32 = self.controls.iter().map(ControlSlot::weight).sum();
        let sensors: u32 = self.sensors.iter().map(SensorSlot::weight).sum();
        let screens: u32 = self.screens.iter().map(ScreenSlot::weight).sum();
        let links = self.links.len() as u32 * 2;
        let conditions = self.conditions.len().min(1) as u32 * 2;
        let behaviors = self.behaviors.len().min(1) as u32;
        outputs + controls + sensors + screens + links + conditions + behaviors
    }

    /// How much of the task a plan closes. `closed` and `total` are weights,
    /// so a named board counts more than an everyday synonym and a missing
    /// screen cannot be hidden by three incidental resistors.
    pub fn coverage(&self, cover: &SlotCover) -> Coverage {
        let mut closed = 0;
        let mut missing = Vec::new();
        for slot in &self.outputs {
            if cover.outputs.contains(&slot.kind) {
                closed += slot.weight();
            } else {
                missing.push(SlotName::Output(slot.kind));
            }
        }
        for slot in &self.controls {
            if cover.controls.contains(&slot.kind) {
                closed += slot.weight();
            } else {
                missing.push(SlotName::Control(slot.kind));
            }
        }
        for slot in &self.sensors {
            if cover.sensors.contains(&slot.source) {
                closed += slot.weight();
            } else {
                missing.push(SlotName::Sensor(slot.source));
            }
        }
        for slot in &self.screens {
            if cover.screens.iter().any(|s| s.satisfies(slot)) {
                closed += slot.weight();
            } else {
                missing.push(SlotName::Screen);
            }
        }
        for (kind, _) in &self.links {
            if cover.links.contains(kind) {
                closed += 2;
            } else {
                missing.push(SlotName::Link(*kind));
            }
        }
        if !self.conditions.is_empty() {
            if cover.condition {
                closed += 2;
            } else {
                missing.push(SlotName::Condition);
            }
        }
        if !self.behaviors.is_empty() {
            if cover.behaviors.iter().any(|b| self.behaviors.contains(b)) {
                closed += 1;
            } else {
                missing.push(SlotName::Behavior);
            }
        }
        let total = self.total_weight();
        let extra_out: Vec<OutputKind> = cover
            .outputs
            .iter()
            .copied()
            .filter(|k| !self.outputs.iter().any(|s| s.kind == *k))
            .collect();
        let mut extra: Vec<SlotName> =
            extra_out.iter().map(|k| SlotName::Output(*k)).collect();
        // A plan that draws a button, a sensor or a screen the sentence never
        // named is not closing this task; it is answering a different one.
        for kind in &cover.controls {
            if !self.controls.iter().any(|s| s.kind == *kind) {
                extra.push(SlotName::Control(*kind));
            }
        }
        for source in &cover.sensors {
            if !self.sensors.iter().any(|s| s.source == *source) {
                extra.push(SlotName::Sensor(*source));
            }
        }
        for drawn in &cover.screens {
            if !self.screens.iter().any(|slot| drawn.satisfies(slot)) {
                extra.push(SlotName::Screen);
            }
        }
        Coverage {
            closed,
            total,
            missing,
            extra,
        }
    }

    /// The template that closes the most of this task, judged by slots rather
    /// than by a keyword vote. Equal readings stay equal and are offered as
    /// alternatives by the caller; the order of a keyword table never decides.
    pub fn best_route(&self) -> Option<(Load, Option<Input>)> {
        use crate::templates::{route, Tid};
        let candidates: [(Tid, Load, Option<Input>); 18] = [
            (Tid::MotorSteps, Load::Motor, Some(Input::Button)),
            (Tid::MotorPot, Load::Motor, Some(Input::Pot)),
            (Tid::MotorTest, Load::Motor, None),
            (Tid::MotorDriver, Load::Motors, None),
            (Tid::Stepper, Load::Stepper, None),
            (Tid::ServoPot, Load::Servo, Some(Input::Pot)),
            (Tid::ServoSweep, Load::Servo, None),
            (Tid::LedToggle, Load::Led, Some(Input::Button)),
            (Tid::NightLight, Load::Led, Some(Input::Ldr)),
            (Tid::LedBlink, Load::Led, None),
            (Tid::Lamp, Load::Lamp, None),
            (Tid::BuzzerTone, Load::Buzzer, None),
            (Tid::Rfid, Load::Rfid, None),
            (Tid::Radio433, Load::Radio433, None),
            (Tid::RadioNrf, Load::RadioNrf, None),
            (Tid::Bluetooth, Load::Bluetooth, None),
            (Tid::Gsm, Load::Gsm, None),
            (Tid::Ethernet, Load::Ethernet, None),
        ];
        let primary = self
            .outputs
            .iter()
            .find(|s| s.kind.load().is_some())
            .map(|s| s.kind);
        let mut best: Option<((i32, usize), (Load, Option<Input>))> = None;
        for (tid, load, input) in candidates {
            // The table above must agree with `route`; a mismatch is a bug in
            // this model, and skipping it keeps the reading honest.
            if route(load, input) != Some(tid) {
                continue;
            }
            let cover = template_cover(tid, 0);
            let mut score = 0i32;
            // The subject is the most specific named output. A plan that
            // draws it is the reading; a plan that only draws a minor extra
            // is not.
            if primary.is_some_and(|kind| cover.outputs.contains(&kind)) {
                score += 6;
            }
            for slot in &self.outputs {
                if cover.outputs.contains(&slot.kind) {
                    score += 1;
                }
            }
            for slot in &self.controls {
                if cover.controls.contains(&slot.kind) {
                    score += 2;
                }
            }
            for slot in &self.screens {
                if cover.screens.iter().any(|s| s.satisfies(slot)) {
                    score += 3;
                }
            }
            // A part the sentence never named is not a closed slot: it is a
            // different circuit that happens to share a template.
            for kind in &cover.outputs {
                if !self.outputs.iter().any(|s| s.kind == *kind) {
                    score -= 4;
                }
            }
            for kind in &cover.controls {
                if !self.controls.iter().any(|s| s.kind == *kind) {
                    score -= 2;
                }
            }
            if score <= 0 {
                continue;
            }
            let position = self
                .outputs
                .iter()
                .find(|s| cover.outputs.contains(&s.kind))
                .map(|s| s.at)
                .or_else(|| {
                    input.and_then(|want| {
                        self.controls.iter().find(|s| s.kind == want).map(|s| s.at)
                    })
                })
                .unwrap_or(usize::MAX);
            let key = (score, usize::MAX - position.min(usize::MAX - 1));
            if best.as_ref().map_or(true, |(seen, _)| key > *seen) {
                best = Some((key, (load, input)));
            }
        }
        best.map(|(_, route)| route)
    }

    /// Coverage as a percentage. A task with no slots is not "100% sure";
    /// it has nothing to be sure about, so the caller shows no number.
    pub fn confidence(&self, cover: &SlotCover) -> f32 {
        let total = self.total_weight();
        if total == 0 {
            return 0.0;
        }
        self.coverage(cover).closed as f32 / total as f32
    }

    /// The labels of what the reading leaves open, for `volta explain` and
    /// the plan note.
    pub fn missing_labels(&self, cover: &SlotCover, lang: Lang) -> Vec<String> {
        self.coverage(cover)
            .missing
            .iter()
            .map(|s| s.label().get(lang).to_string())
            .collect()
    }
}

/// What one plan closes. A plan does not get credit for a slot it merely
/// mentions in its title: this list is the circuit's own shape.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotCover {
    pub outputs: Vec<OutputKind>,
    pub controls: Vec<Input>,
    pub sensors: Vec<SourceId>,
    pub screens: Vec<ScreenCover>,
    pub links: Vec<LinkKind>,
    pub condition: bool,
    pub behaviors: Vec<Action>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenCover {
    pub kind: ScreenKind,
    pub matrix: bool,
    pub segments: bool,
}

impl ScreenCover {
    pub fn satisfies(&self, slot: &ScreenSlot) -> bool {
        if slot.matrix && !self.matrix {
            return false;
        }
        if slot.segments && !self.segments {
            return false;
        }
        if self.matrix && !slot.matrix {
            return false;
        }
        if self.segments && !slot.segments {
            return false;
        }
        slot.kind == ScreenKind::Any || self.kind == ScreenKind::Any || slot.kind == self.kind
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotName {
    Output(OutputKind),
    Control(Input),
    Sensor(SourceId),
    Screen,
    Link(LinkKind),
    Condition,
    Behavior,
}

impl SlotName {
    pub fn label(&self) -> L {
        match self {
            SlotName::Output(kind) => kind.label(),
            SlotName::Control(Input::Button) => L::new("кнопка", "button"),
            SlotName::Control(Input::Pot) => L::new("потенциометр", "potentiometer"),
            SlotName::Control(Input::Ldr) => L::new("освещённость", "light level"),
            SlotName::Sensor(id) => match id {
                SourceId::Temperature => L::new("датчик температуры", "temperature sensor"),
                SourceId::Thermistor => L::new("термистор", "thermistor"),
                SourceId::Humidity => L::new("датчик влажности", "humidity sensor"),
                SourceId::Pressure => L::new("датчик давления", "pressure sensor"),
                SourceId::Light => L::new("датчик света", "light sensor"),
                SourceId::Range => L::new("дальномер", "distance sensor"),
                SourceId::Gas => L::new("датчик газа", "gas sensor"),
                SourceId::Soil => L::new("датчик влажности почвы", "soil sensor"),
                _ => L::new("датчик", "sensor"),
            },
            SlotName::Screen => L::new("экран", "display"),
            SlotName::Link(kind) => kind.label(),
            SlotName::Condition => L::new("условие", "condition"),
            SlotName::Behavior => L::new("действие", "action"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Coverage {
    pub closed: u32,
    pub total: u32,
    pub missing: Vec<SlotName>,
    pub extra: Vec<SlotName>,
}

impl Coverage {
    pub fn percent(&self) -> u8 {
        if self.total == 0 {
            return 0;
        }
        ((self.closed as f32 / self.total as f32) * 100.0).round() as u8
    }

    pub fn full(&self) -> bool {
        self.total > 0 && self.missing.is_empty()
    }

    /// The same sentence in the plan header and in a lesson's `why`: how much
    /// of the typed task the plan closes and what it leaves open.
    pub fn describe(&self, lang: Lang) -> String {
        let mut out = String::new();
        if self.total > 0 {
            out.push_str(&format!(
                "{} {}% ({} {}/{})",
                crate::text::tr(lang, "покрытие слотов", "slot coverage"),
                self.percent(),
                crate::text::tr(lang, "закрыто", "closed"),
                self.closed,
                self.total
            ));
        }
        if !self.missing.is_empty() {
            if !out.is_empty() {
                out.push_str("; ");
            }
            let missing: Vec<String> = self
                .missing
                .iter()
                .map(|s| s.label().get(lang).to_string())
                .collect();
            out.push_str(&format!(
                "{}: {}",
                crate::text::tr(lang, "не закрыто", "left open"),
                missing.join(", ")
            ));
        }
        out
    }
}

/// The slots a built-in template closes. Kept next to the task model so a
/// template cannot quietly claim a screen or a sensor it does not draw.
pub fn template_cover(tid: crate::templates::Tid, choice: usize) -> SlotCover {
    use crate::templates::Tid;
    match tid {
        Tid::MotorSteps => SlotCover {
            outputs: vec![OutputKind::Motor],
            controls: vec![Input::Button],
            behaviors: vec![Action::Measure],
            ..SlotCover::default()
        },
        Tid::MotorPot => SlotCover {
            outputs: vec![OutputKind::Motor],
            controls: vec![Input::Pot],
            ..SlotCover::default()
        },
        Tid::MotorTest => SlotCover {
            outputs: vec![OutputKind::Motor],
            ..SlotCover::default()
        },
        Tid::MotorDriver => SlotCover {
            outputs: vec![OutputKind::Motors],
            ..SlotCover::default()
        },
        Tid::Stepper => SlotCover {
            outputs: vec![OutputKind::Stepper],
            ..SlotCover::default()
        },
        Tid::LedToggle => SlotCover {
            outputs: vec![OutputKind::Led],
            controls: vec![Input::Button],
            ..SlotCover::default()
        },
        Tid::LedBlink => SlotCover {
            outputs: vec![OutputKind::Led],
            ..SlotCover::default()
        },
        Tid::NightLight => SlotCover {
            outputs: vec![OutputKind::Led],
            controls: vec![Input::Ldr],
            ..SlotCover::default()
        },
        Tid::Lamp => SlotCover {
            outputs: vec![OutputKind::Lamp],
            ..SlotCover::default()
        },
        Tid::ServoSweep => SlotCover {
            outputs: vec![OutputKind::Servo],
            ..SlotCover::default()
        },
        Tid::ServoPot => SlotCover {
            outputs: vec![OutputKind::Servo],
            controls: vec![Input::Pot],
            ..SlotCover::default()
        },
        Tid::BuzzerTone => SlotCover {
            outputs: vec![OutputKind::Buzzer],
            ..SlotCover::default()
        },
        Tid::Rfid => {
            let mut cover = SlotCover {
                outputs: vec![OutputKind::Rfid],
                links: vec![LinkKind::Rfid],
                ..SlotCover::default()
            };
            match choice {
                1 => cover.screens.push(ScreenCover {
                    kind: ScreenKind::Any,
                    matrix: false,
                    segments: false,
                }),
                2 => cover.outputs.push(OutputKind::Relay),
                _ => {}
            }
            cover
        }
        Tid::Radio433 => SlotCover {
            outputs: vec![OutputKind::Radio433],
            links: vec![LinkKind::Radio433],
            ..SlotCover::default()
        },
        Tid::RadioNrf => SlotCover {
            outputs: vec![OutputKind::RadioNrf],
            links: vec![LinkKind::RadioNrf],
            ..SlotCover::default()
        },
        Tid::Bluetooth => SlotCover {
            outputs: vec![OutputKind::Bluetooth],
            links: vec![LinkKind::Bluetooth],
            ..SlotCover::default()
        },
        Tid::Gsm => SlotCover {
            outputs: vec![OutputKind::Gsm],
            links: vec![LinkKind::Gsm],
            ..SlotCover::default()
        },
        Tid::Ethernet => SlotCover {
            outputs: vec![OutputKind::Ethernet],
            links: vec![LinkKind::Ethernet],
            ..SlotCover::default()
        },
        Tid::ButtonsScreen => SlotCover {
            controls: vec![Input::Button],
            screens: vec![ScreenCover {
                kind: ScreenKind::Any,
                matrix: false,
                segments: false,
            }],
            behaviors: vec![Action::Display],
            ..SlotCover::default()
        },
        Tid::TextScreens => SlotCover {
            screens: vec![ScreenCover {
                kind: ScreenKind::Any,
                matrix: false,
                segments: false,
            }],
            ..SlotCover::default()
        },
        Tid::Composed | Tid::Lesson(_) => SlotCover::default(),
    }
}

/// The slots a composition closes. The source is always the task's sensor,
/// the sink is the requested output and the condition is the spoken one.
pub fn recipe_cover(recipe: Composition) -> SlotCover {
    let output = match recipe.sink {
        SinkId::Led | SinkId::Rgb => Some(OutputKind::Led),
        SinkId::Buzzer => Some(OutputKind::Buzzer),
        SinkId::Relay => Some(OutputKind::Relay),
        SinkId::Servo => Some(OutputKind::Servo),
        SinkId::Motor => Some(OutputKind::Motor),
        SinkId::Lcd | SinkId::Lcd2004 | SinkId::Oled | SinkId::Serial => None,
    };
    let mut cover = SlotCover {
        sensors: vec![recipe.source],
        outputs: output.into_iter().collect(),
        condition: recipe.condition.is_some(),
        ..SlotCover::default()
    };
    if matches!(recipe.sink, SinkId::Lcd | SinkId::Lcd2004 | SinkId::Oled) {
        cover.screens.push(ScreenCover {
            kind: match recipe.sink {
                SinkId::Lcd2004 => ScreenKind::Lcd2004,
                SinkId::Oled => ScreenKind::Oled,
                _ => ScreenKind::Lcd,
            },
            matrix: false,
            segments: false,
        });
    }
    cover
}

/// The slots a printed lesson closes. Boards are the strongest evidence; the
/// loose extras and the title fill in the rest.
pub fn lesson_cover(i: usize) -> SlotCover {
    use crate::lessons::Extra;
    let Some(lesson) = crate::lessons::ALL.get(i) else {
        return SlotCover::default();
    };
    let mut cover = SlotCover::default();
    for board in lesson.boards {
        if let Some(module) = crate::modules::find(board.name) {
            use crate::modules::Look;
            match module.look {
                Look::Lcd => cover.screens.push(ScreenCover {
                    kind: ScreenKind::Lcd,
                    matrix: false,
                    segments: false,
                }),
                Look::Display => cover.screens.push(ScreenCover {
                    kind: ScreenKind::Any,
                    matrix: false,
                    segments: false,
                }),
                Look::Segments => cover.screens.push(ScreenCover {
                    kind: ScreenKind::Any,
                    matrix: false,
                    segments: true,
                }),
                Look::Matrix => cover.screens.push(ScreenCover {
                    kind: ScreenKind::Any,
                    matrix: true,
                    segments: false,
                }),
                Look::Panel => cover.screens.push(ScreenCover {
                    kind: ScreenKind::Any,
                    matrix: true,
                    segments: false,
                }),
                Look::Rfid => {
                    cover.outputs.push(OutputKind::Rfid);
                    cover.links.push(LinkKind::Rfid);
                }
                Look::Radio => {
                    cover.links.push(LinkKind::RadioNrf);
                }
                _ => {}
            }
            if let Some(source) = source_for_module(module.name) {
                if !cover.sensors.contains(&source) {
                    cover.sensors.push(source);
                }
            }
        }
    }
    for extra in lesson.extras {
        match *extra {
            Extra::Led { .. } | Extra::Rgb { .. } => cover.outputs.push(OutputKind::Led),
            Extra::Button { .. } => cover.controls.push(Input::Button),
            Extra::Ldr { .. } => cover.controls.push(Input::Ldr),
            Extra::Pot { .. } => cover.controls.push(Input::Pot),
            Extra::Buzzer { .. } => cover.outputs.push(OutputKind::Buzzer),
            Extra::IrLed { .. } => cover.links.push(LinkKind::Ir),
            Extra::Reed { .. } => cover.controls.push(Input::Button),
            Extra::Mosfet { .. } | Extra::Relay { .. } => cover.outputs.push(OutputKind::Motor),
        }
    }
    // The lesson's title names the part it teaches. This matters for a sensor
    // lesson whose board is just a breakout: "Термометр" must count as a
    // temperature reading, but a DS18B20 board must not satisfy "термистор".
    for source in crate::spec::SOURCES {
        let named = lesson.keys.iter().any(|key| {
            source.keys.iter().any(|source_key| key.contains(source_key))
        });
        if named && !cover.sensors.contains(&source.id) {
            cover.sensors.push(source.id);
        }
    }
    // A board can teach a different quantity than its title: a DHT11 board
    // reads both temperature and humidity, and the lesson title often names
    // only one of them.
    for board in lesson.boards {
        if let Some(source) = source_for_module(board.name) {
            if !cover.sensors.contains(&source) {
                cover.sensors.push(source);
            }
        }
    }
    // A display lesson that merely shows a clock does not have an RTC board;
    // the clock is the picture, not a part. That is a recorded loss, not a
    // closed slot, so the cover stays honest.
    cover
}

/// The sensor a board actually measures. A display, a driver or a radio has
/// no source; a DHT11 is both a thermometer and a hygrometer, which is why a
/// single title word cannot stand for the board's whole contribution.
fn source_for_module(name: &str) -> Option<SourceId> {
    let name = name.to_ascii_lowercase();
    if name.starts_with("ds18b20") || name.starts_with("lm35") || name.starts_with("tmp36") {
        return Some(SourceId::Temperature);
    }
    if name.starts_with("dht") || name.starts_with("bme") {
        return Some(if name.starts_with("dht") {
            SourceId::Humidity
        } else {
            SourceId::Pressure
        });
    }
    if name.starts_with("bmp") {
        return Some(SourceId::Pressure);
    }
    if name.starts_with("mq-2") || name.starts_with("mq2") {
        return Some(SourceId::Gas);
    }
    if name.starts_with("mq-3") || name.starts_with("mq3") {
        return Some(SourceId::Alcohol);
    }
    if name.starts_with("hc-sr04") || name.starts_with("us-") {
        return Some(SourceId::Range);
    }
    if name.starts_with("hc-sr501") || name.starts_with("pir") {
        return Some(SourceId::Motion);
    }
    if name.starts_with("gl5528") || name.starts_with("ldr") {
        return Some(SourceId::Light);
    }
    if name.starts_with("ntc") || name.starts_with("thermistor") {
        return Some(SourceId::Thermistor);
    }
    if name.starts_with("soil") || name.starts_with("fc-28") || name.starts_with("yl-69") {
        return Some(SourceId::Soil);
    }
    if name.starts_with("acs712") {
        return Some(SourceId::Current);
    }
    if name.starts_with("hx711") {
        return Some(SourceId::Weight);
    }
    if name.starts_with("tcs3200") {
        return Some(SourceId::Color);
    }
    if name.starts_with("mpu6050") {
        return Some(SourceId::Tilt);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_closes_only_the_slots_it_draws() {
        let mut task = Task::new("две кнопки и дисплей");
        task.controls.push(ControlSlot {
            kind: Input::Button,
            count: 2,
            words: vec!["кнопк".into()],
            specificity: 1,
            at: 0,
        });
        task.screens.push(ScreenSlot {
            kind: ScreenKind::Any,
            count: 1,
            matrix: false,
            segments: false,
            words: vec!["дисплей".into()],
            at: 2,
        });
        let led = template_cover(crate::templates::Tid::LedToggle, 0);
        let screen = template_cover(crate::templates::Tid::ButtonsScreen, 0);
        let led_cov = task.coverage(&led);
        let screen_cov = task.coverage(&screen);
        assert!(led_cov.missing.iter().any(|s| *s == SlotName::Screen));
        assert!(screen_cov.full(), "{screen_cov:?}");
        assert!(screen_cov.percent() > led_cov.percent());
    }

    #[test]
    fn a_named_board_outweighs_a_generic_word() {
        let mut task = Task::new("мотор и RFID");
        task.outputs.push(OutputSlot {
            kind: OutputKind::Motor,
            count: 1,
            words: vec!["мотор".into()],
            specificity: 2,
            at: 0,
        });
        task.outputs.push(OutputSlot {
            kind: OutputKind::Rfid,
            count: 1,
            words: vec!["rfid".into()],
            specificity: 3,
            at: 2,
        });
        let rfid = template_cover(crate::templates::Tid::Rfid, 0);
        let led = template_cover(crate::templates::Tid::LedBlink, 0);
        assert!(task.confidence(&rfid) > task.confidence(&led));
    }
}
