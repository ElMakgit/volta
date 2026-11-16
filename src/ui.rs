//! The window.
//!
//! Two ways in, one bench. AI mode takes a sentence, offers three part sets and
//! builds the circuit; manual mode puts the same breadboard in your hands, with
//! a palette and a wire tool. Both feed the same `Circuit`, so the checks and
//! the PNG export are identical.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{
    self, pos2, vec2, Align, Align2, Button, Color32, CornerRadius, FontId, Frame, Layout, Margin, Pos2,
    RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Vec2,
};

use crate::checks::{self, Report, Sev};
use crate::icons::{self, Icon};
use crate::manual::{self, Draft, Place};
use crate::model::{Circuit, Kind, NetClass, Pt, WireStyle};
use crate::parse::{self, Parse};
use crate::project;
use crate::scene::{self, Anchor, Prim, Rgba, Scene};
use crate::spec::{self, Behavior, Composition, Condition, SINKS, SOURCES};
use crate::svg;
use crate::templates::{self, Built, Plan};
use crate::text::{Lang, L, Tr};
use crate::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Ai,
    Manual,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Circuit,
    Parts,
    Code,
    Checks,
}

/// What the save window is about to write. Each kind picks its own default
/// name and its own writer, but the folder part is shared.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveKind {
    Png,
    Svg,
    Sketch,
    Csv,
    /// A KiCad-readable connectivity file.
    Netlist,
    /// A printable one-page PDF.
    Pdf,
    /// SVG plus the sketch in one folder.
    Bundle,
}

impl SaveKind {
    fn key(self) -> &'static str {
        match self {
            SaveKind::Png => "png",
            SaveKind::Svg => "svg",
            SaveKind::Sketch => "sketch",
            SaveKind::Csv => "csv",
            SaveKind::Netlist => "netlist",
            SaveKind::Pdf => "pdf",
            SaveKind::Bundle => "bundle",
        }
    }
    fn from_key(key: &str) -> Option<SaveKind> {
        match key {
            "png" => Some(SaveKind::Png),
            "svg" => Some(SaveKind::Svg),
            "sketch" => Some(SaveKind::Sketch),
            "csv" => Some(SaveKind::Csv),
            "netlist" => Some(SaveKind::Netlist),
            "pdf" => Some(SaveKind::Pdf),
            "bundle" => Some(SaveKind::Bundle),
            _ => None,
        }
    }
    fn default_name(self) -> &'static str {
        match self {
            SaveKind::Png => "circuit.png",
            SaveKind::Svg => "circuit.svg",
            SaveKind::Sketch => "sketch.ino",
            SaveKind::Csv => "components.csv",
            SaveKind::Netlist => "circuit.net",
            SaveKind::Pdf => "circuit.pdf",
            SaveKind::Bundle => "circuit",
        }
    }
    /// The format tag on the one Export button.
    fn short(self) -> &'static str {
        match self {
            SaveKind::Png => "PNG",
            SaveKind::Svg => "SVG",
            SaveKind::Sketch => ".ino",
            SaveKind::Csv => "CSV",
            SaveKind::Netlist => ".net",
            SaveKind::Pdf => "PDF",
            SaveKind::Bundle => "SVG+.ino",
        }
    }
}

/// Which field of "I read it as" a chip is offering to change.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Chip {
    Source,
    Condition,
    Sink,
}

/// The mini-picker behind a chip. It holds the whole recipe, so changing one
/// field keeps the other two exactly as the user last saw them.
struct RecipeAsk {
    chip: Chip,
    recipe: Composition,
    /// The number the threshold buttons apply.
    number: f32,
}

/// A failure of the checks tab, pinned to the bench while the user looks at
/// it. The name is kept for the chip over the drawing.
struct CheckFocus {
    focus: checks::Focus,
    name: String,
    sev: Sev,
}

/// One ready-made circuit on the start screen.
struct Sample {
    icon: Icon,
    task: L,
}

/// Six circuits that the composer already understands, one click each. The
/// phrases are the same ones `volta plan` accepts, so a card cannot promise a
/// circuit the planner would refuse.
const GALLERY: [Sample; 6] = [
    Sample {
        icon: Icon::Module("DS18B20"),
        task: L::new(
            "температура выше 30 включи вентилятор",
            "temperature above 30 turn on the motor",
        ),
    },
    Sample {
        icon: Icon::Module("MQ-2"),
        task: L::new("газ выше 400 включи сирену", "gas above 400 turn on the buzzer"),
    },
    Sample {
        icon: Icon::Module("HC-SR04"),
        task: L::new(
            "расстояние меньше 20 пищалка",
            "distance below 20 turn on the buzzer",
        ),
    },
    Sample {
        icon: Icon::Module("DHT22"),
        task: L::new(
            "влажность ниже 40 включи насос",
            "humidity below 40 turn on the motor",
        ),
    },
    Sample {
        icon: Icon::Module("HC-SR501"),
        task: L::new("движение включи светодиод", "motion turn on the led"),
    },
    Sample {
        icon: Icon::Part(Place::Relay),
        task: L::new(
            "температура ниже 18 включи реле",
            "temperature below 18 turn on the relay",
        ),
    },
];

/// The save window: a folder to browse, a name to type. It replaces the
/// silent write into `~/Pictures/volta` that used to happen on export.
struct SaveAsk {
    kind: SaveKind,
    dir: PathBuf,
    folders: Vec<String>,
    name: String,
    error: Option<String>,
}

/// One cell of the palette: a plain part or a named ready-made board.
#[derive(Clone, Copy, PartialEq)]
enum Entry {
    Part(Place),
    Module(&'static str),
}

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Select,
    Wire,
    Place(Place),
}

/// How far the drawing may be pulled in and pushed out. The bench is drawn in
/// millimetres, so at the far end a hole is a few pixels wide; at the near end
/// a part fills the window.
const MIN_ZOOM: f32 = 0.35;
const MAX_ZOOM: f32 = 12.0;

/// How dark the bench goes under a window. A dialog needs to stand out, but
/// the circuit behind it stays readable: at 150 the screen turned into a black
/// sheet and the user lost the context the question is about.
const DIM_ALPHA: u8 = 84;

struct View {
    zoom: f32,
    pan: Vec2,
    fitted: bool,
}

impl Default for View {
    fn default() -> Self {
        Self { zoom: 1.0, pan: Vec2::ZERO, fitted: false }
    }
}

/// What the canvas saw this frame, in bench millimetres.
struct Hit {
    click: Option<Pt>,
    right: Option<Pt>,
    hover: Option<Pt>,
    pin: Option<usize>,
    origin: Pos2,
    scale: f32,
    rect: egui::Rect,
}

impl Default for Hit {
    fn default() -> Self {
        Self {
            click: None,
            right: None,
            hover: None,
            pin: None,
            origin: Pos2::ZERO,
            scale: 1.0,
            rect: egui::Rect::NOTHING,
        }
    }
}

#[derive(Default)]
struct Drag {
    part: Option<usize>,
    /// A cable bend being pulled: which cable, which bend.
    via: Option<(usize, usize)>,
    /// The bend was created by this drag; letting go without moving drops it.
    via_new: bool,
    /// Where the pointer grabbed the part, in bench millimetres.
    pointer: Pt,
    /// The part's anchor when the drag began.
    at: Pt,
    /// How far a seated part has already been re-seated, in holes.
    applied: (i32, i32),
    seated: bool,
    moved: bool,
    /// The scene frame, frozen for the duration of the drag.
    frame: Option<crate::model::Rect>,
}

enum InspectAction {
    Label(String),
    Value(String),
    Variant(String, Kind),
    Rotate,
    WireClass(NetClass),
    NetName(String),
    WireColour(Option<u8>),
    ClearVias,
}

impl Drag {
    fn active(&self) -> bool {
        self.part.is_some() || self.via.is_some()
    }
}

pub struct App {
    lang: Lang,
    tr: Tr,
    theme: Theme,
    mode: Mode,
    task: String,
    parsed: Option<Parse>,
    plan: Option<Plan>,
    chosen: usize,
    built: Option<Built>,
    scene: Option<Scene>,
    draft: Draft,
    draft_scene: Option<Scene>,
    manual_report: Option<Report>,
    tool: Tool,
    palette_filter: String,
    /// Draft text of the property inspector, committed on Enter or blur.
    inspect_for: String,
    inspect_label: String,
    inspect_value: String,
    inspect_net: String,
    wire_style: WireStyle,
    /// The cable whose grips are shown: the one under the pointer or being
    /// bent by hand.
    active_wire: Option<usize>,
    /// Where the pointer last hovered, in bench millimetres. A drag starts a
    /// frame or two after the press, and this is the finger on the cable.
    hover_world: Option<Pt>,
    /// The bench point under the last press, kept until a drag consumes it.
    press_world: Option<Pt>,
    /// First corner of a rubber-band selection, in bench coordinates.
    marquee: Option<Pt>,
    tab: Tab,
    labels: bool,
    /// A faint breadboard-pitch grid, toggled with G.
    show_grid: bool,
    hover: Option<usize>,
    drag: Drag,
    view: View,
    help: bool,
    /// The "about the author" easter egg: the window and its photo, decoded
    /// once on first open and kept for the session.
    about: bool,
    about_tex: Option<egui::TextureHandle>,
    quit_ask: bool,
    status: String,
    status_bad: bool,
    error: Option<String>,
    saved_png: bool,
    /// The save window, open while the user picks a folder and a name.
    save_ask: Option<SaveAsk>,
    /// White sheet or black: the circuit background the user picked.
    bench_dark: bool,
    /// The format the one Export button repeats.
    export_kind: SaveKind,
    /// A task left unfinished last time, offered at start.
    resume: Option<(String, bool)>,
    /// Side panels folded away to icons, with the canvas taking the window.
    narrow: bool,
    /// Checks show every line, not just the failures.
    checks_all: bool,
    /// The chip of "I read it as" whose picker is open.
    recipe_ask: Option<RecipeAsk>,
    /// The failure of the checks tab shown on the bench.
    check_focus: Option<CheckFocus>,
    /// The projects window and the file it is working with.
    projects_open: bool,
    project_name: String,
    project_path: Option<PathBuf>,
    project_list: Vec<project::Entry>,
    project_error: Option<String>,
    /// A project the user asked to delete, waiting for a second click.
    project_delete: Option<PathBuf>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        let theme = Theme::load();
        let lang = Lang::from_tag(&crate::theme::language_tag());
        let draft = Draft::new();
        Self {
            lang,
            tr: Tr::new(lang),
            theme,
            mode: Mode::Ai,
            task: String::new(),
            parsed: None,
            plan: None,
            chosen: 0,
            built: None,
            scene: None,
            draft,
            draft_scene: None,
            manual_report: None,
            tool: Tool::Select,
            palette_filter: String::new(),
            inspect_for: String::new(),
            inspect_label: String::new(),
            inspect_value: String::new(),
            inspect_net: String::new(),
            wire_style: crate::config::load_wire_style(),
            active_wire: None,
            hover_world: None,
            press_world: None,
            marquee: None,
            tab: Tab::Circuit,
            labels: false,
            show_grid: false,
            hover: None,
            drag: Drag::default(),
            view: View::default(),
            help: false,
            about: false,
            about_tex: None,
            quit_ask: false,
            status: String::new(),
            status_bad: false,
            error: None,
            saved_png: false,
            save_ask: None,
            bench_dark: crate::config::load_bench_dark(),
            export_kind: crate::config::load_export_kind()
                .as_deref()
                .and_then(SaveKind::from_key)
                .unwrap_or(SaveKind::Png),
            resume: crate::config::load_draft(),
            narrow: false,
            checks_all: false,
            recipe_ask: None,
            check_focus: None,
            projects_open: false,
            project_name: String::new(),
            project_path: None,
            project_list: Vec::new(),
            project_error: None,
            project_delete: None,
        }
    }

    /// `VOLTA_TASK` pre-fills the sentence and builds it, `VOLTA_MODE=manual`
    /// opens the bench. Handy for screenshots and for checking a build.
    fn demo(mut self) -> Self {
        // The routing has to be set before the first build, or the scene is
        // drawn in the old style.
        if let Ok(style) = std::env::var("VOLTA_WIRES") {
            self.wire_style = match style.as_str() {
                "arc" => WireStyle::Arc,
                "straight" => WireStyle::Straight,
                _ => WireStyle::Ortho,
            };
        }
        if let Ok(task) = std::env::var("VOLTA_TASK") {
            self.task = task;
            // A screenshot hook starts a fresh task: the continuation offer
            // belongs to a normal start, not to a scripted one.
            self.resume = None;
            self.plan_now();
        }
        match std::env::var("VOLTA_RESUME").as_deref() {
            Ok("1") => self.resume = crate::config::load_draft(),
            Ok(_) => self.resume = None,
            Err(_) => {}
        }
        if std::env::var("VOLTA_NARROW").is_ok() {
            self.narrow = true;
        }
        if std::env::var("VOLTA_MODE").map(|m| m == "manual").unwrap_or(false) {
            if self.built.is_some() {
                self.edit_by_hand();
            } else {
                self.switch_mode(Mode::Manual);
            }
        }
        // Screenshot hooks: turn the board or drop it, so the rotated layouts
        // can be checked without a mouse.
        if let Ok(n) = std::env::var("VOLTA_BOARD_ROT") {
            let n: i32 = n.parse().unwrap_or(1);
            match self.mode {
                Mode::Ai => {
                    if let Some(b) = &mut self.built {
                        b.circuit.rotate_board(n);
                    }
                    self.refresh_ai();
                }
                Mode::Manual => {
                    self.draft.rotate_board(n);
                    self.refresh_manual();
                }
            }
        }
        if std::env::var("VOLTA_BARE").is_ok() && self.mode == Mode::Manual {
            if self.draft.set_bare(true) {
                self.refresh_manual();
            }
        }
        // The about window is a screenshot hook too: it needs a click otherwise.
        if std::env::var("VOLTA_ABOUT").is_ok() {
            self.about = true;
        }
        if std::env::var("VOLTA_LABELS").is_ok() {
            self.labels = true;
            match self.mode {
                Mode::Ai => self.refresh_ai(),
                Mode::Manual => self.refresh_manual(),
            }
        }
        if let Ok(tab) = std::env::var("VOLTA_TAB") {
            self.tab = match tab.as_str() {
                "parts" => Tab::Parts,
                "code" => Tab::Code,
                "checks" => Tab::Checks,
                _ => Tab::Circuit,
            };
        }
        if std::env::var("VOLTA_BENCH").map(|b| b == "dark").unwrap_or(false) {
            self.bench_dark = true;
            match self.mode {
                Mode::Ai => self.refresh_ai(),
                Mode::Manual => self.refresh_manual(),
            }
        }
        if std::env::var("VOLTA_SAVE").is_ok() {
            self.ask_save(SaveKind::Png);
        }
        self
    }

    fn t(&self, key: &str) -> String {
        self.tr.t(key)
    }

    /// Switching the routing is a preference, not a circuit edit, so the
    /// undo stack stays untouched and the choice survives a restart.
    fn set_wire_style(&mut self, style: WireStyle) {
        self.wire_style = style;
        crate::config::save_wire_style(style);
    }

    // ---------------------------------------------------------------- AI side

    fn plan_now(&mut self) {
        self.error = None;
        if self.task.trim().is_empty() {
            self.error = Some(self.t("err.no_task"));
            return;
        }
        self.plan_parsed(parse::parse(&self.task));
        self.stash_draft();
    }

    /// Build the bench from an already parsed sentence. A sentence the parser
    /// did not understand must not turn into an LED blink just because that is
    /// the default load: say so and keep the bench as it is. Ambiguity keeps
    /// the parsed sentence so its one-click readings stay on screen.
    fn plan_parsed(&mut self, parsed: parse::Parse) {
        // A named lesson carries its own circuit and checks.
        if let Some(msg) = parsed.unsupported {
            self.error = Some(msg.get(self.lang).to_string());
            self.plan = None;
            self.built = None;
            self.scene = None;
            self.parsed = (!parsed.alternatives.is_empty()).then_some(parsed);
            return;
        }
        let Some(plan) = templates::plan(&parsed.intent, self.lang) else {
            let msg = parsed
                .unsupported
                .map(|l| l.get(self.lang).to_string())
                .unwrap_or_else(|| self.t("err.unsupported"));
            self.error = Some(msg);
            self.plan = None;
            self.built = None;
            self.scene = None;
            self.parsed = (!parsed.alternatives.is_empty()).then_some(parsed);
            return;
        };
        self.chosen = plan.recommended.min(plan.choices.len().saturating_sub(1));
        self.plan = Some(plan);
        self.parsed = Some(parsed);
        self.build_now();
    }

    /// Take one of the offered readings of an ambiguous sentence.
    fn choose_alternative(&mut self, i: usize) {
        let Some(parsed) = &self.parsed else { return };
        let Some(alt) = parsed.alternatives.get(i) else { return };
        self.task = alt.intent.raw.clone();
        let next = parse::Parse {
            intent: alt.intent.clone(),
            matched: Vec::new(),
            understood: vec![alt.label],
            unsupported: None,
            alternatives: Vec::new(),
        };
        self.plan_parsed(next);
    }

    /// Change one field of the read recipe and rebuild the bench, without
    /// touching the sentence the user typed.
    fn apply_recipe(&mut self, recipe: Composition) {
        let Some(parsed) = self.parsed.clone() else { return };
        let mut intent = parsed.intent;
        intent.recipe = Some(recipe);
        intent.condition = recipe.condition;
        let next = parse::Parse {
            intent,
            matched: Vec::new(),
            understood: crate::compose::recipe_words(recipe),
            unsupported: None,
            alternatives: Vec::new(),
        };
        self.plan_parsed(next);
        crate::config::save_draft(&self.task, self.mode == Mode::Manual);
    }

    /// The draft follows the sentence as it is typed, so a closed window does
    /// not throw away the work.
    fn stash_draft(&self) {
        // Only the sentence is a project that can be rebuilt exactly; a
        // hand-made bench is not serialised, so it is not promised back.
        crate::config::save_draft(&self.task, false);
    }

    fn resume_task(&mut self) {
        let Some((task, manual)) = self.resume.take() else { return };
        self.task = task;
        if manual {
            self.switch_mode(Mode::Manual);
        } else {
            self.switch_mode(Mode::Ai);
            self.plan_now();
        }
        self.status = self.t("status.ready");
        self.status_bad = false;
    }

    fn start_fresh(&mut self) {
        self.resume = None;
        self.task.clear();
        self.parsed = None;
        self.plan = None;
        self.built = None;
        self.scene = None;
        self.error = None;
        self.check_focus = None;
        self.draft.clear();
        self.draft_scene = None;
        self.manual_report = None;
        self.mode = Mode::Ai;
        crate::config::clear_draft();
        self.status = self.t("status.ready");
        self.status_bad = false;
    }

    fn build_now(&mut self) {
        self.error = None;
        self.check_focus = None;
        let Some(parsed) = &self.parsed else { return };
        let Some(built) = templates::build(&parsed.intent, self.chosen, self.lang) else {
            self.error = Some(self.t("err.unsupported"));
            return;
        };
        self.built = Some(built);
        self.scene = None;
        // A new circuit means a new frame: fit it instead of keeping the zoom
        // and pan of whatever was on the bench before.
        self.view = View::default();
        self.refresh_ai();
    }

    fn refresh_ai(&mut self) {
        let Some(built) = &self.built else { return };
        self.scene = Some(scene::draw(
            &built.circuit,
            &self.theme,
            &scene::Opts {
                labels: self.labels,
                hover: self.hover,
                bench: self.bench(),
                style: self.wire_style,
                selected: None,
                selected_many: Vec::new(),
                frame: None,
                active_wire: None,
                handles: Vec::new(),
            },
        ));
    }

    // ------------------------------------------------------------ manual side

    fn refresh_manual(&mut self) {
        self.draft_scene = Some(scene::draw(
            &self.draft.circuit,
            &self.theme,
            &scene::Opts {
                labels: self.labels,
                hover: self.hover,
                bench: self.bench(),
                style: self.wire_style,
                selected: self.draft.selected_part(),
                selected_many: self.draft.selected_parts.iter().copied().collect(),
                frame: self.drag.frame,
                active_wire: self.active_wire,
                handles: self
                    .active_wire
                    .and_then(|i| self.draft.circuit.wires.get(i))
                    .map(|w| w.via.clone())
                    .unwrap_or_default(),
            },
        ));
        self.manual_report = Some(checks::run_circuit(
            &self.draft.circuit,
            self.lang,
            self.draft.code.as_deref(),
        ));
    }

    fn edit_by_hand(&mut self) {
        let Some(built) = &self.built else { return };
        self.draft.adopt(built.circuit.clone(), Some(built.code.clone()));
        // The generated picture is replaced by the editable one right away.
        self.scene = None;
        self.check_focus = None;
        self.mode = Mode::Manual;
        self.tool = Tool::Select;
        self.tab = Tab::Circuit;
        self.view = View::default();
        self.hover = None;
        self.refresh_manual();
        self.status = self.t("manual.from_ai");
        self.status_bad = false;
    }

    fn switch_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        // Both sides keep their circuits, but only one bench is on screen.
        // A scene is several megabytes of primitives, so the hidden one is
        // dropped and redrawn when the user comes back to it.
        match mode {
            Mode::Ai => {
                self.draft_scene = None;
                self.manual_report = None;
            }
            Mode::Manual => self.scene = None,
        }
        self.mode = mode;
        self.hover = None;
        self.check_focus = None;
        self.view = View::default();
        match mode {
            Mode::Ai => {
                self.refresh_ai();
                self.tab = Tab::Circuit;
            }
            Mode::Manual => {
                self.refresh_manual();
                self.tab = Tab::Circuit;
            }
        }
    }

    // ------------------------------------------------------------- data access

    fn active_scene(&self) -> Option<&Scene> {
        match self.mode {
            Mode::Ai => self.scene.as_ref(),
            Mode::Manual => self.draft_scene.as_ref(),
        }
    }

    fn active_circuit(&self) -> Option<&Circuit> {
        match self.mode {
            Mode::Ai => self.built.as_ref().map(|b| &b.circuit),
            Mode::Manual => Some(&self.draft.circuit),
        }
    }

    fn ai_report(&self) -> Option<Report> {
        self.built.as_ref().map(|b| checks::run(b, self.lang))
    }

    // -------------------------------------------------------------- projects

    /// The current bench as a project. AI projects keep the sentence and the
    /// chosen option and are rebuilt on open; the hand-drawn circuit is stored
    /// because there is no sentence that could reproduce it.
    fn project_snapshot(&self, name: &str) -> project::Project {
        project::Project {
            name: name.to_string(),
            task: self.task.clone(),
            manual: self.mode == Mode::Manual,
            chosen: self.chosen,
            recipe: self.parsed.as_ref().and_then(|p| p.intent.recipe),
            circuit: match self.mode {
                Mode::Manual => Some(self.draft.circuit.clone()),
                Mode::Ai => None,
            },
            view: project::ViewData {
                zoom: self.view.zoom,
                pan_x: self.view.pan.x,
                pan_y: self.view.pan.y,
            },
            saved: 0,
        }
    }

    fn refresh_projects(&mut self) {
        self.project_list = project::list();
    }

    fn open_projects(&mut self) {
        self.refresh_projects();
        if self.project_name.trim().is_empty() {
            self.project_name = if self.task.trim().is_empty() {
                self.t("projects.untitled")
            } else {
                self.task.trim().chars().take(40).collect()
            };
        }
        self.project_error = None;
        self.project_delete = None;
        self.projects_open = true;
    }

    fn save_project(&mut self) {
        let name = if self.project_name.trim().is_empty() {
            let from_task: String = self.task.trim().chars().take(40).collect();
            if from_task.is_empty() {
                self.t("projects.untitled")
            } else {
                from_task
            }
        } else {
            self.project_name.trim().to_string()
        };
        let p = self.project_snapshot(&name);
        let path = project::dir().join(project::file_name(&name));
        match project::save(&path, &p) {
            Ok(()) => {
                self.project_name = name;
                self.project_path = Some(path);
                self.project_error = None;
                self.status = self.t("projects.saved");
                self.status_bad = false;
                self.refresh_projects();
            }
            Err(e) => self.project_error = Some(e),
        }
    }

    fn open_project(&mut self, path: &Path) {
        // Opening another project must not throw away what is on the bench.
        // The autosave is the safety net, and opening the autosave itself is
        // the one case where writing it first would destroy what is being
        // read.
        if !project::is_autosave(path) {
            self.autosave();
        }
        let p = match project::load(path) {
            Ok(p) => p,
            Err(e) => {
                self.project_error = Some(e);
                return;
            }
        };
        self.projects_open = false;
        self.project_delete = None;
        self.project_error = None;
        self.resume = None;
        self.check_focus = None;
        self.error = None;
        self.status = self.t("projects.opened");
        self.status_bad = false;
        self.project_path = Some(path.to_path_buf());
        self.project_name = if p.name.trim().is_empty() {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .trim_end_matches(project::SUFFIX)
                .to_string()
        } else {
            p.name.clone()
        };
        self.task = p.task.clone();
        if p.manual {
            if let Some(circuit) = p.circuit {
                self.draft.adopt(circuit, None);
            }
            self.mode = Mode::Manual;
            self.scene = None;
            self.draft_scene = None;
            self.manual_report = None;
            self.parsed = None;
            self.plan = None;
            self.built = None;
            self.tab = Tab::Circuit;
            self.refresh_manual();
        } else {
            self.mode = Mode::Ai;
            self.scene = None;
            self.draft_scene = None;
            self.manual_report = None;
            self.plan_now();
            if self.parsed.is_some() {
                if let Some(recipe) = p.recipe {
                    self.apply_recipe(recipe);
                }
                if self.plan.is_some() {
                    let last = self
                        .plan
                        .as_ref()
                        .map(|pl| pl.choices.len().saturating_sub(1))
                        .unwrap_or(0);
                    self.chosen = p.chosen.min(last);
                    self.build_now();
                }
            }
        }
        self.view = View {
            zoom: p.view.zoom.clamp(MIN_ZOOM, MAX_ZOOM),
            pan: vec2(p.view.pan_x, p.view.pan_y),
            fitted: true,
        };
    }

    fn remove_project(&mut self, path: &Path) {
        let _ = std::fs::remove_file(path);
        if self.project_path.as_deref() == Some(path) {
            self.project_path = None;
        }
        self.project_delete = None;
        self.refresh_projects();
    }

    /// The bench written on exit, so a closed window never loses the work.
    /// It is a project like any other and shows up in the list as autosave.
    fn autosave(&self) {
        let has_work = !self.task.trim().is_empty()
            || self.built.is_some()
            || self.draft.circuit.parts.len() > 1
            || !self.draft.circuit.wires.is_empty();
        if !has_work {
            return;
        }
        let name = if self.project_name.trim().is_empty() {
            self.t("projects.autosave")
        } else {
            format!("{} · {}", self.project_name.trim(), self.t("projects.autosave"))
        };
        let p = self.project_snapshot(&name);
        let _ = project::save(&project::autosave_path(), &p);
    }

    // ---------------------------------------------------------------- export

    /// The chosen bench colour. White is the default, because a circuit
    /// printed for a notebook or a class looks like the Tinkercad canvas.
    fn bench(&self) -> Option<Rgba> {
        Some(if self.bench_dark {
            Rgba::rgb(0x14, 0x14, 0x16)
        } else {
            Rgba::rgb(0xFF, 0xFF, 0xFF)
        })
    }

    fn toggle_bench(&mut self) {
        self.bench_dark = !self.bench_dark;
        crate::config::save_bench_dark(self.bench_dark);
        match self.mode {
            Mode::Ai => self.refresh_ai(),
            Mode::Manual => self.refresh_manual(),
        }
        self.status = self.t(if self.bench_dark { "bg.black" } else { "bg.white" });
        self.status_bad = false;
    }

    fn export_files(&mut self) {
        self.ask_save(SaveKind::Bundle);
    }

    fn save_sketch(&mut self) {
        self.ask_save(SaveKind::Sketch);
    }

    /// The one Export button: repeat the format chosen last time.
    fn export_kind_now(&mut self) {
        let kind = match (self.export_kind, self.mode) {
            // A manual bench has no sketch, so the pair reads as an SVG.
            (SaveKind::Sketch, Mode::Manual) => SaveKind::Bundle,
            (kind, _) => kind,
        };
        self.ask_save(kind);
    }

    /// A format picked from the Export menu becomes the one the big button
    /// repeats next time.
    fn pick_export(&mut self, kind: SaveKind) {
        self.export_kind = kind;
        crate::config::save_export_kind(kind.key());
        self.ask_save(kind);
    }

    /// Open the save window. It starts in the folder of the last export, or in
    /// `~/Pictures/volta`, and pre-fills the name that suits the kind.
    fn ask_save(&mut self, kind: SaveKind) {
        if self.active_scene().is_none() {
            return;
        }
        if kind == SaveKind::Sketch && self.built.is_none() {
            return;
        }
        crate::config::save_export_kind(kind.key());
        let mut dir = crate::config::load_export_dir()
            .filter(|p| p.is_dir())
            .unwrap_or_else(crate::png::default_dir);
        if !dir.is_dir() {
            let _ = std::fs::create_dir_all(&dir);
        }
        if !dir.is_dir() {
            dir = crate::paths::home();
        }
        self.save_ask = Some(SaveAsk {
            kind,
            folders: folders_in(&dir),
            dir,
            name: kind.default_name().to_string(),
            error: None,
        });
    }

    /// Write what the window promises. A bad name or a failed write keeps the
    /// window open with the reason, instead of losing the user's choice.
    fn finish_save(&mut self) {
        let Some(ask) = self.save_ask.take() else { return };
        let name = ask.name.trim().to_string();
        if name.is_empty() || name.contains('/') {
            let mut ask = ask;
            ask.error = Some(self.t("save.badname"));
            self.save_ask = Some(ask);
            return;
        }
        let path = ask.dir.join(&name);
        let csv_lang = self.lang;
        let result: Result<PathBuf, String> = match ask.kind {
            SaveKind::Png => {
                let Some(sc) = self.active_scene().cloned() else {
                    return;
                };
                let out = with_ext(&path, "png");
                crate::png::png_from_scene(&sc, &out, 2400).map(|()| out)
            }
            SaveKind::Svg => {
                let Some(sc) = self.active_scene().cloned() else {
                    return;
                };
                let out = with_ext(&path, "svg");
                match std::fs::write(&out, svg::render(&sc, 1.0)) {
                    Ok(()) => Ok(out),
                    Err(e) => Err(format!("{}: {e}", out.display())),
                }
            }
            SaveKind::Sketch => {
                let Some(b) = &self.built else { return };
                let out = with_ext(&path, "ino");
                match std::fs::write(&out, &b.code) {
                    Ok(()) => Ok(out),
                    Err(e) => Err(format!("{}: {e}", out.display())),
                }
            }
            SaveKind::Csv => {
                let Some(circuit) = self.active_circuit().cloned() else {
                    return;
                };
                let rows = templates::component_rows(&circuit, csv_lang);
                let out = with_ext(&path, "csv");
                match std::fs::write(&out, templates::component_csv(&rows)) {
                    Ok(()) => Ok(out),
                    Err(e) => Err(format!("{}: {e}", out.display())),
                }
            }
            SaveKind::Netlist => {
                let Some(circuit) = self.active_circuit().cloned() else {
                    return;
                };
                let out = with_ext(&path, "net");
                match std::fs::write(&out, crate::netlist::netlist(&circuit)) {
                    Ok(()) => Ok(out),
                    Err(e) => Err(format!("{}: {e}", out.display())),
                }
            }
            SaveKind::Pdf => {
                let Some(sc) = self.active_scene().cloned() else {
                    return;
                };
                let out = with_ext(&path, "pdf");
                match crate::pdf::pdf_from_scene(&sc, &out, 1600) {
                    Ok(()) => Ok(out),
                    Err(e) => Err(e),
                }
            }
            SaveKind::Bundle => {
                let Some(sc) = self.active_scene().cloned() else {
                    return;
                };
                let svg_out = path.with_extension("svg");
                match std::fs::write(&svg_out, svg::render(&sc, 1.0)) {
                    Err(e) => Err(format!("{}: {e}", svg_out.display())),
                    Ok(()) => match (self.mode, &self.built) {
                        (Mode::Ai, Some(b)) => {
                            let ino_out = path.with_extension("ino");
                            match std::fs::write(&ino_out, &b.code) {
                                Ok(()) => Ok(ino_out),
                                Err(e) => Err(format!("{}: {e}", ino_out.display())),
                            }
                        }
                        _ => Ok(svg_out),
                    },
                }
            }
        };
        match result {
            Ok(out) => {
                crate::config::save_export_dir(&ask.dir);
                if matches!(ask.kind, SaveKind::Png | SaveKind::Bundle) {
                    self.saved_png = true;
                }
                self.status = format!(
                    "{}: {}",
                    self.t(match ask.kind {
                        SaveKind::Png => "shot.saved",
                        _ => "export.saved",
                    }),
                    out.display()
                );
                self.status_bad = false;
            }
            Err(e) => {
                let mut ask = ask;
                ask.error = Some(e);
                self.save_ask = Some(ask);
            }
        }
    }

    fn c32(&self, c: Rgba) -> Color32 {
        Color32::from_rgba_unmultiplied(c.0, c.1, c.2, c.3)
    }

    fn dirty(&self) -> bool {
        self.mode == Mode::Ai && self.built.is_some() && !self.saved_png
    }

    fn request_close(&mut self, ctx: &egui::Context) {
        if self.dirty() {
            self.quit_ask = true;
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn keys(&mut self, ctx: &egui::Context) {
        let (ctrl, shift, enter, s, e, z, q, f, r, esc, del, quest, b, a, c, v, x, dd, left, right, up, down, g, k1, k2, k3) =
            ctx.input(|i| {
            (
                i.modifiers.ctrl,
                i.modifiers.shift,
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::S),
                i.key_pressed(egui::Key::E),
                i.key_pressed(egui::Key::Z),
                i.key_pressed(egui::Key::Q),
                i.key_pressed(egui::Key::F),
                i.key_pressed(egui::Key::R),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                i.key_pressed(egui::Key::Questionmark),
                i.key_pressed(egui::Key::B),
                i.key_pressed(egui::Key::A),
                i.key_pressed(egui::Key::C),
                i.key_pressed(egui::Key::V),
                i.key_pressed(egui::Key::X),
                i.key_pressed(egui::Key::D),
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::G),
                i.key_pressed(egui::Key::Num1),
                i.key_pressed(egui::Key::Num2),
                i.key_pressed(egui::Key::Num3),
            )
            });
        let typing = ctx.memory(|m| m.focused().is_some());

        // While the save window is open it owns the keyboard: Escape closes
        // it, and the shortcuts below must not stack a second one.
        if self.save_ask.is_some() {
            if esc {
                self.save_ask = None;
            }
            return;
        }

        if ctrl && enter && self.mode == Mode::Ai {
            self.plan_now();
        }
        if ctrl && s {
            self.export_kind_now();
        }
        if ctrl && e {
            self.export_files();
        }
        if ctrl && b && !typing {
            self.narrow = !self.narrow;
        }
        if ctrl && z && self.mode == Mode::Manual {
            let changed = if shift { self.draft.redo() } else { self.draft.undo() };
            if changed {
                self.hover = None;
                self.refresh_manual();
            }
        }
        if del && !typing && self.mode == Mode::Manual {
            if !self.draft.selected_parts.is_empty() {
                if self.draft.delete_selected() {
                    self.hover = None;
                    self.active_wire = None;
                    self.refresh_manual();
                }
            } else if let Some(wire) = self.active_wire {
                self.draft.snapshot();
                self.draft.remove_wire_recorded(wire);
                self.active_wire = None;
                self.refresh_manual();
            }
        }
        if ctrl && !typing && self.mode == Mode::Manual {
            let mut changed = false;
            if a {
                self.draft.select_all();
                changed = true;
            }
            if c {
                changed |= self.draft.copy_selected();
            }
            if x {
                changed |= self.draft.cut_selected();
            }
            if v {
                let at = self.hover_world.unwrap_or_else(|| {
                    self.draft
                        .selected_part()
                        .and_then(|i| self.draft.circuit.parts.get(i))
                        .map(|p| Pt::new(p.at.x + 10.16, p.at.y))
                        .unwrap_or_default()
                });
                changed |= self.draft.paste(at);
            }
            if dd {
                changed |= self.draft.duplicate_selected();
            }
            if changed {
                self.active_wire = None;
                self.refresh_manual();
            }
        }
        if !ctrl && !typing && self.mode == Mode::Manual && !self.draft.selected_parts.is_empty() {
            let step = crate::model::PITCH * if shift { 0.5 } else { 1.0 };
            let dx = if left { -step } else if right { step } else { 0.0 };
            let dy = if up { -step } else if down { step } else { 0.0 };
            if (dx != 0.0 || dy != 0.0) && self.draft.nudge_selected(dx, dy) {
                self.refresh_manual();
            }
        }
        if g && !typing {
            self.show_grid = !self.show_grid;
        }
        if !typing && self.mode == Mode::Manual {
            if k1 {
                self.tool = Tool::Select;
            } else if k2 {
                self.tool = Tool::Wire;
                self.draft.pending = None;
            } else if k3 {
                self.tool = Tool::Place(Place::Resistor);
            }
        }
        if f && !typing {
            self.view = View::default();
        }
        if r && !typing && self.mode == Mode::Manual {
            if shift {
                if self.draft.rotate_board(1) {
                    self.refresh_manual();
                } else {
                    self.status = self.t("manual.bare_blocked");
                    self.status_bad = true;
                }
            } else if self.draft.rotate_selected(1) {
                self.refresh_manual();
            } else {
                self.status = self.t("manual.rotate_seated");
                self.status_bad = true;
            }
        }
        if esc {
            self.help = false;
            self.quit_ask = false;
            self.recipe_ask = None;
            self.draft.pending = None;
            self.tool = Tool::Select;
            self.check_focus = None;
            self.projects_open = false;
            self.project_delete = None;
        }
        if quest && !typing {
            self.help = !self.help;
        }
        if q && !typing && !ctrl {
            self.request_close(ctx);
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_ask {
            if self.dirty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.quit_ask = true;
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let ctx = &ctx;
        if self.theme.poll() {
            match self.mode {
                Mode::Ai => self.refresh_ai(),
                Mode::Manual => self.refresh_manual(),
            }
        }
        let lang = Lang::from_tag(&crate::theme::language_tag());
        if lang != self.lang {
            self.lang = lang;
            self.tr = Tr::new(lang);
            if self.parsed.is_some() {
                self.plan_now();
            }
            self.refresh_manual();
        }
        ctx.request_repaint_after(Duration::from_secs(2));
        self.keys(ctx);

        let accent = self.c32(self.theme.accent);
        let bg = self.c32(self.theme.bg);
        let fg = self.c32(self.theme.fg);
        let muted = self.c32(self.theme.muted);
        let panel = self.c32(self.theme.panel);
        crate::style::apply(root.style_mut(), accent, bg, fg, muted);
        // Windows (the value picker, help, the quit question) are drawn from
        // the context style, not from this ui, so they need the same flat
        // tokens or their frames and combo boxes come back rounded.
        ctx.all_styles_mut(|style| {
            crate::style::apply(style, accent, bg, fg, muted);
        });
        let radius = crate::style::RADIUS;

        egui::Panel::top("bar")
            .frame(Frame::NONE.fill(bg).inner_margin(Margin::symmetric(18, 12)))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.t("app.title"))
                            .size(crate::style::TEXT_BRAND)
                            .strong()
                            .color(accent),
                    );
                    ui.add_space(18.0);
                    // The two modes are the first decision on the screen.
                    for (mode, key) in [(Mode::Ai, "mode.ai"), (Mode::Manual, "mode.manual")] {
                        let on = self.mode == mode;
                        let label = self.t(key);
                        if tab_button(ui, &label, on, crate::style::TEXT_TITLE, accent, fg, muted).clicked() {
                            self.switch_mode(mode);
                        }
                    }
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new(self.t(if self.mode == Mode::Ai { "mode.ai.hint" } else { "mode.manual.hint" }))
                            .size(12.0)
                            .color(muted),
                    );
                    ui.add_space(14.0);
                    let projects = self.t("projects.title");
                    if tab_button(ui, &projects, self.projects_open, crate::style::TEXT_BODY, accent, fg, muted)
                        .on_hover_text(self.t("projects.hint"))
                        .clicked()
                    {
                        if self.projects_open {
                            self.projects_open = false;
                        } else {
                            self.open_projects();
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(Button::new(RichText::new("?").size(15.0)).min_size(vec2(30.0, 26.0)))
                            .on_hover_text(self.t("help.title"))
                            .clicked()
                        {
                            self.help = !self.help;
                        }
                        if ui
                            .add(
                                Button::new(RichText::new("i").size(14.0))
                                    .min_size(vec2(24.0, 26.0))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, muted.gamma_multiply(0.5)))
                                    .corner_radius(CornerRadius::ZERO),
                            )
                            .on_hover_text(self.t("about.hint"))
                            .clicked()
                        {
                            self.about = true;
                        }
                        if ui
                            .button(RichText::new(self.lang.code().to_uppercase()).size(12.0).color(accent))
                            .on_hover_text(self.t("help.language"))
                            .clicked()
                        {
                            self.lang = match self.lang {
                                crate::text::Lang::Ru => crate::text::Lang::En,
                                crate::text::Lang::En => crate::text::Lang::Ru,
                            };
                            self.tr = crate::text::Tr::new(self.lang);
                            self.refresh_ai();
                            self.refresh_manual();
                        }
                        if ui
                            .button(RichText::new("◐").size(12.0).color(muted))
                            .on_hover_text(self.t("help.bench"))
                            .clicked()
                        {
                            self.toggle_bench();
                            self.refresh_ai();
                            self.refresh_manual();
                        }
                        let summary = match self.mode {
                            Mode::Ai => self.ai_report(),
                            Mode::Manual => self.manual_report.clone(),
                        };
                        if let Some(r) = summary {
                            let warned = r.checks.iter().filter(|c| c.sev == Sev::Warn).count();
                            let (text, col, tip) = match r.worst() {
                                Sev::Pass => (
                                    format!("{} ✓", r.passed()),
                                    accent,
                                    self.t("checks.badge.ok").replace("{}", &r.passed().to_string()),
                                ),
                                Sev::Warn => (
                                    format!("{} ✓ {} !", r.passed(), warned),
                                    Color32::from_rgb(0xD8, 0xA3, 0x2E),
                                    self.t("checks.badge.warn")
                                        .replace("{}", &r.passed().to_string())
                                        .replace("{}", &warned.to_string()),
                                ),
                                Sev::Fail => (
                                    format!("{} ✓ {} ×", r.passed(), r.failed()),
                                    Color32::from_rgb(0xD2, 0x4B, 0x45),
                                    self.t("checks.badge.fail")
                                        .replace("{}", &r.passed().to_string())
                                        .replace("{}", &r.failed().to_string()),
                                ),
                            };
                            // The number is a door to the tab that explains it,
                            // not a dead label.
                            if ui
                                .add(
                                    Button::new(RichText::new(text).size(13.0).color(col))
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE)
                                        .corner_radius(CornerRadius::ZERO),
                                )
                                .on_hover_text(tip)
                                .clicked()
                            {
                                self.tab = Tab::Checks;
                            }
                        }
                    });
                });
            });

        egui::Panel::bottom("status")
            .frame(Frame::NONE.fill(bg).inner_margin(Margin::symmetric(18, 8)))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    if self.status.is_empty() {
                        ui.label(RichText::new(self.t("footer.hint")).size(12.0).color(muted));
                    } else {
                        let col = if self.status_bad { Color32::from_rgb(0xD2, 0x4B, 0x45) } else { fg };
                        ui.label(RichText::new(&self.status).size(12.0).color(col));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.mode == Mode::Manual {
                            if self.draft.selected_parts.len() > 1 {
                                for (key, edge) in [
                                    ("◧", manual::Align::Left),
                                    ("◨", manual::Align::Right),
                                    ("⌃", manual::Align::Top),
                                    ("⌄", manual::Align::Bottom),
                                ] {
                                    if ui.button(key).on_hover_text(self.t("manual.align")).clicked() {
                                        if self.draft.align_selected(edge) {
                                            self.refresh_manual();
                                        }
                                    }
                                }
                            }
                            if let Some(sel) = self.draft.selected_part() {
                                let p = &self.draft.circuit.parts[sel];
                                ui.label(
                                    RichText::new(format!("{}: {} {}", self.t("manual.selected"), p.label, p.value))
                                        .size(12.0)
                                        .color(muted),
                                );
                            } else if let Some(e) = self.draft.pending {
                                let pos = self.draft.circuit.end_pos(e);
                                ui.label(
                                    RichText::new(format!("{} ({:.1}, {:.1})", self.t("manual.pending"), pos.x, pos.y))
                                        .size(12.0)
                                        .color(accent),
                                );
                            }
                        }
                    });
                });
            });

        if self.narrow {
            // Folded to icons: the canvas gets the window, but the way back
            // and the two lists stay one click away.
            egui::Panel::left("task")
                .exact_size(38.0)
                .frame(Frame::NONE.fill(panel).inner_margin(Margin::symmetric(4, 8)))
                .show(root, |ui| {
                    ui.vertical_centered(|ui| {
                        let open = Button::new(RichText::new("›").size(15.0))
                            .min_size(vec2(26.0, 26.0));
                        if ui.add(open).on_hover_text(self.t("narrow.open")).clicked() {
                            self.narrow = false;
                        }
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(self.t(if self.mode == Mode::Ai {
                                "mode.ai"
                            } else {
                                "mode.manual"
                            }))
                            .size(10.0)
                            .color(muted),
                        );
                    });
                });
            egui::Panel::right("side")
                .exact_size(38.0)
                .frame(Frame::NONE.fill(panel).inner_margin(Margin::symmetric(4, 8)))
                .show(root, |ui| {
                    ui.vertical_centered(|ui| {
                        let open = Button::new(RichText::new("‹").size(15.0))
                            .min_size(vec2(26.0, 26.0));
                        if ui.add(open).on_hover_text(self.t("narrow.open")).clicked() {
                            self.narrow = false;
                        }
                        ui.add_space(12.0);
                        let parts = Button::new(RichText::new("▤").size(14.0))
                            .min_size(vec2(26.0, 26.0));
                        if ui.add(parts).on_hover_text(self.t("tab.parts")).clicked() {
                            self.tab = Tab::Parts;
                        }
                        ui.add_space(6.0);
                        let checks = Button::new(RichText::new("✓").size(14.0))
                            .min_size(vec2(26.0, 26.0));
                        if ui.add(checks).on_hover_text(self.t("tab.checks")).clicked() {
                            self.tab = Tab::Checks;
                        }
                    });
                });
        } else {
            egui::Panel::left("task")
                .resizable(true)
                .default_size(if self.mode == Mode::Ai { 340.0 } else { 290.0 })
                .size_range(260.0..=520.0)
                .frame(Frame::NONE.fill(panel).inner_margin(Margin::same(crate::style::PAD as i8)))
                .show(root, |ui| {
                    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.mode {
                        Mode::Ai => self.task_panel(ui, accent, fg, muted),
                        Mode::Manual => self.palette_panel(ui, accent, fg, muted),
                    });
                });

            egui::Panel::right("side")
                .resizable(true)
                .default_size(290.0)
                .size_range(240.0..=440.0)
                .frame(Frame::NONE.fill(panel).inner_margin(Margin::same(crate::style::PAD as i8)))
                .show(root, |ui| {
                    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        self.side_panel(ui, accent, fg, muted);
                    });
                });
        }

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(bg).inner_margin(Margin::symmetric(12, 10)))
            .show(root, |ui| {
                self.center_panel(ctx, ui, accent, fg, muted, panel, radius);
            });

        self.overlays(ctx, accent, fg, muted, panel, radius);
        self.author_window(ctx, accent, fg, muted, panel, radius);
        self.value_window(ctx, accent, fg, muted, panel, radius);
        self.save_window(ctx, accent, fg, muted, panel, radius);
        self.recipe_window(ctx, accent, fg, muted, panel, radius);
        self.projects_window(ctx, accent, fg, muted, panel, radius);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.autosave();
    }
}

impl App {
    fn task_panel(&mut self, ui: &mut egui::Ui, accent: Color32, fg: Color32, muted: Color32) {
        ui.label(RichText::new(self.t("task.title")).size(15.0).strong().color(fg));
        ui.add_space(2.0);
        ui.label(RichText::new(self.t("task.hint")).size(12.0).color(muted));
        ui.add_space(6.0);

        let hint = self.t("task.hint");
        let resp = ui.add(
            TextEdit::multiline(&mut self.task)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text(hint),
        );
        if resp.changed() {
            self.scene = None;
            self.stash_draft();
        }
        ui.add_space(4.0);
        // Six ready circuits instead of three text examples: one click from
        // an empty window to a finished bench.
        ui.label(RichText::new(self.t("task.gallery")).size(11.0).color(muted));
        ui.add_space(4.0);
        let width = ui.available_width();
        let col = ((width - crate::style::GAP) / 2.0).max(80.0);
        let mut sample_pick: Option<L> = None;
        for row in GALLERY.chunks(2) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = crate::style::GAP;
                for sample in row {
                    if sample_card(ui, sample, col, self.lang, accent, fg, muted).clicked() {
                        sample_pick = Some(sample.task);
                    }
                }
            });
        }
        if let Some(task) = sample_pick {
            self.task = task.get(self.lang).to_string();
            self.plan_now();
        }
        ui.add_space(8.0);
        let go = Button::new(RichText::new(self.t("task.plan")).size(14.0).strong())
            .fill(accent)
            .corner_radius(crate::style::RADIUS)
            .min_size(vec2(ui.available_width(), 34.0));
        if ui.add(go).clicked() {
            self.plan_now();
        }

        if let Some(e) = &self.error {
            ui.add_space(8.0);
            ui.label(RichText::new(e).size(12.0).color(Color32::from_rgb(0xD2, 0x4B, 0x45)));
        }

        // What the parser actually heard. A composition is a row of chips,
        // and each chip opens the picker for its own field: a wrong sensor
        // costs one click, not a re-typed sentence.
        if let Some(p) = &self.parsed {
            if let Some(recipe) = p.intent.recipe {
                let chips = recipe_chips(recipe, self.lang);
                let mut ask: Option<Chip> = None;
                ui.add_space(6.0);
                ui.label(RichText::new(self.t("task.understood")).size(11.0).color(muted));
                ui.add_space(3.0);
                ui.horizontal_wrapped(|ui| {
                    for (chip, text, hint) in chips {
                        let button = Button::new(RichText::new(format!("{text} ▾")).size(11.5))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(1.0, accent.gamma_multiply(0.6)))
                            .corner_radius(crate::style::RADIUS);
                        if ui.add(button).on_hover_text(hint).clicked() {
                            ask = Some(chip);
                        }
                    }
                });
                if let Some(chip) = ask {
                    let number = recipe_number(recipe);
                    self.recipe_ask = Some(RecipeAsk { chip, recipe, number });
                }
            } else if !p.understood.is_empty() {
                let heard: Vec<&str> = p.understood.iter().map(|l| l.get(self.lang)).collect();
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!("{} {}", self.t("task.understood"), heard.join(" · ")))
                        .size(11.0)
                        .color(muted),
                );
            }
        }

        // An ambiguous sentence offers its other readings as buttons instead
        // of picking one silently.
        let mut alt_pick = None;
        if let Some(p) = &self.parsed {
            if !p.alternatives.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(self.t("task.other")).size(11.0).color(muted));
                ui.horizontal_wrapped(|ui| {
                    for (i, alt) in p.alternatives.iter().enumerate() {
                        if ui
                            .add(
                                Button::new(RichText::new(alt.label.get(self.lang)).size(11.5))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, accent.gamma_multiply(0.7)))
                                    .corner_radius(crate::style::RADIUS),
                            )
                            .clicked()
                        {
                            alt_pick = Some(i);
                        }
                    }
                });
            }
        }
        if let Some(i) = alt_pick {
            self.choose_alternative(i);
        }

        if let Some(plan) = &self.plan {
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(8.0);
            let title_key = match plan.choices.len() {
                1 => "plan.title.one",
                2 => "plan.title.two",
                4 => "plan.title.four",
                _ => "plan.title",
            };
            let heading = match &plan.headline {
                Some(h) => h.clone(),
                None => self.t(title_key).to_string(),
            };
            ui.label(RichText::new(heading).size(15.0).strong().color(fg));
            ui.add_space(6.0);

            let mut pick = None;
            for (i, c) in plan.choices.iter().enumerate() {
                let selected = i == self.chosen;
                let stroke = if selected {
                    Stroke::new(1.6, accent)
                } else {
                    Stroke::new(1.0, muted.gamma_multiply(0.35))
                };
                Frame::NONE
                    .fill(if selected { accent.gamma_multiply(0.10) } else { Color32::TRANSPARENT })
                    .stroke(stroke)
                    .corner_radius(crate::style::RADIUS)
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{}. ", i + 1)).size(13.0).color(accent));
                            ui.label(RichText::new(c.name.get(self.lang)).size(13.5).strong().color(fg));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if i == plan.recommended && plan.choices.len() > 1 {
                                    ui.label(RichText::new(self.t("plan.recommended")).size(10.5).color(accent));
                                }
                            });
                        });
                        ui.label(RichText::new(c.blurb.get(self.lang)).size(11.5).color(muted));
                        for (k, v) in &c.specs {
                            ui.label(
                                RichText::new(format!("{}: {}", k.get(self.lang), v)).size(11.0).color(muted),
                            );
                        }
                        for p in &c.pros {
                            ui.label(
                                RichText::new(format!("+ {}", p.get(self.lang)))
                                    .size(11.0)
                                    .color(Color32::from_rgb(0x4F, 0xA0, 0x5C)),
                            );
                        }
                        for m in &c.cons {
                            ui.label(
                                RichText::new(format!("− {}", m.get(self.lang)))
                                    .size(11.0)
                                    .color(Color32::from_rgb(0xC0, 0x7A, 0x3A)),
                            );
                        }
                        if ui
                            .add(
                                Button::new(RichText::new(self.t("plan.pick")).size(12.0))
                                    .min_size(vec2(ui.available_width(), 0.0))
                                    .corner_radius(crate::style::RADIUS),
                            )
                            .clicked()
                        {
                            pick = Some(i);
                        }
                    });
                ui.add_space(6.0);
            }
            if let Some(i) = pick {
                self.chosen = i;
                self.build_now();
                self.tab = Tab::Circuit;
                self.status = self.t("plan.changed");
                self.status_bad = false;
            }
        }
    }

    fn palette_panel(&mut self, ui: &mut egui::Ui, accent: Color32, fg: Color32, muted: Color32) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.t("manual.palette")).size(15.0).strong().color(fg));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(Button::new(RichText::new(self.t("manual.clear")).size(11.0)))
                    .clicked()
                {
                    self.draft.clear();
                    self.hover = None;
                    self.refresh_manual();
                }
            });
        });
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            let select = self.tool == Tool::Select;
            if ui
                .add(
                    Button::new(RichText::new(self.t("manual.tool.select")).size(12.0))
                        .fill(if select { accent } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0, muted.gamma_multiply(0.5)))
                        .corner_radius(crate::style::RADIUS),
                )
                .clicked()
            {
                self.tool = Tool::Select;
                self.draft.pending = None;
            }
            let wire = self.tool == Tool::Wire;
            if ui
                .add(
                    Button::new(RichText::new(self.t("manual.tool.wire")).size(12.0))
                        .fill(if wire { accent } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0, muted.gamma_multiply(0.5)))
                        .corner_radius(crate::style::RADIUS),
                )
                .clicked()
            {
                self.tool = Tool::Wire;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let redo = Button::new(RichText::new("↷").size(13.0)).min_size(vec2(28.0, 24.0));
                if ui.add_enabled(self.draft.can_redo(), redo).clicked() && self.draft.redo() {
                    self.refresh_manual();
                }
                let undo = Button::new(RichText::new("↶").size(13.0)).min_size(vec2(28.0, 24.0));
                if ui.add_enabled(self.draft.can_undo(), undo).clicked() && self.draft.undo() {
                    self.refresh_manual();
                }
            });
        });

        if self.tool == Tool::Wire {
            ui.label(RichText::new(self.t("wire.colour")).size(10.5).color(muted));
            if let Some(colour) = self.colour_swatches(ui, self.draft.new_wire_colour) {
                self.draft.new_wire_colour = colour;
            }
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let has_part = self.draft.selected_part().is_some();
            let rotate = Button::new(RichText::new(self.t("manual.rotate_part")).size(11.5))
                .corner_radius(crate::style::RADIUS);
            if ui.add_enabled(has_part, rotate).on_hover_text(self.t("manual.rotate_hint")).clicked() {
                if self.draft.rotate_selected(1) {
                    self.refresh_manual();
                } else {
                    self.status = self.t("manual.rotate_seated");
                    self.status_bad = true;
                }
            }
            let board = Button::new(RichText::new(self.t("manual.rotate_board")).size(11.5))
                .corner_radius(crate::style::RADIUS);
            if ui.add(board).clicked() {
                if self.draft.rotate_board(1) {
                    self.refresh_manual();
                } else {
                    self.status = self.t("manual.bare_blocked");
                    self.status_bad = true;
                }
            }
        });
        ui.horizontal(|ui| {
            let bare = self.draft.circuit.bare;
            let button = Button::new(RichText::new(self.t("manual.bare")).size(11.5))
                .fill(if bare { accent.gamma_multiply(0.85) } else { Color32::TRANSPARENT })
                .stroke(Stroke::new(1.0, muted.gamma_multiply(0.5)))
                .corner_radius(crate::style::RADIUS);
            if ui.add(button).on_hover_text(self.t("manual.bare_hint")).clicked() {
                if self.draft.set_bare(!bare) {
                    self.status = self.t(if bare { "manual.bare_off" } else { "manual.bare_on" });
                    self.status_bad = false;
                    self.refresh_manual();
                } else {
                    self.status = self.t("manual.bare_blocked");
                    self.status_bad = true;
                }
            }
        });

        ui.add_space(10.0);
        // Thirty parts do not fit on one screen, and finding a board by eye
        // is slower than typing three letters.
        let filter_hint = self.t("manual.filter");
        ui.add(
            TextEdit::singleline(&mut self.palette_filter)
                .hint_text(filter_hint)
                .desired_width(f32::INFINITY),
        );
        ui.add_space(4.0);

        // The solid scroll bar sits inside the panel, so the two columns have
        // to be narrower than the pane or the right cell slides under it.
        let width = ui.available_width() - ui.spacing().scroll.allocated_width();
        let column = (width - crate::style::GAP) / 2.0;
        let needle = self.palette_filter.trim().to_lowercase();
        let lang = self.lang;
        let mut chosen: Option<Entry> = None;
        let mut shown = 0usize;
        for g in 0..manual::GROUPS.len() {
            let mut items: Vec<Entry> = Vec::new();
            if g == 5 {
                // Every ready-made board has its own button now.
                for m in crate::modules::ALL {
                    if needle.is_empty() || m.name.to_lowercase().contains(&needle) {
                        items.push(Entry::Module(m.name));
                    }
                }
            } else {
                for &p in manual::PALETTE.iter().filter(|p| p.group() == g) {
                    // The short caption is what the cell shows, but the full
                    // name is what people type, so both are searched.
                    let full = p.name(lang).to_lowercase();
                    let short = p.short_name(lang).to_lowercase();
                    if needle.is_empty() || full.contains(&needle) || short.contains(&needle) {
                        items.push(Entry::Part(p));
                    }
                }
            }
            if items.is_empty() {
                continue;
            }
            shown += items.len();
            ui.add_space(6.0);
            ui.label(
                RichText::new(self.t(manual::GROUPS[g]))
                    .size(crate::style::TEXT_MICRO)
                    .color(muted),
            );
            ui.add_space(2.0);
            for row in items.chunks(2) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = crate::style::GAP;
                    for &entry in row {
                        let (icon, label, on, hint) = match entry {
                            Entry::Part(Place::Uno) => (
                                Icon::Part(Place::Uno),
                                Place::Uno.name(lang),
                                self.draft.circuit.arduino == self.draft.selected_part(),
                                None::<String>,
                            ),
                            Entry::Part(p) => {
                                let on = self.tool == Tool::Place(p);
                                let full = p.name(lang);
                                let short = p.short_name(lang);
                                // Only pay for a tooltip when the cell has to
                                // drop part of the name.
                                let hint = (full != short).then(|| full.to_owned());
                                (Icon::Part(p), short, on, hint)
                            }
                            Entry::Module(name) => {
                                let on = self.tool == Tool::Place(Place::Module)
                                    && self.draft.module == name;
                                (
                                    Icon::Module(name),
                                    name,
                                    on,
                                    crate::modules::note(name).map(|l| l.get(lang).to_owned()),
                                )
                            }
                        };
                        let resp =
                            palette_button(ui, icon, label, on, column, accent, fg, muted);
                        let resp = match hint {
                            Some(h) => resp.on_hover_text(h),
                            None => resp,
                        };
                        if resp.clicked() {
                            chosen = Some(entry);
                        }
                    }
                });
            }
        }
        if shown == 0 {
            ui.add_space(6.0);
            ui.label(
                RichText::new(self.t("manual.no_match"))
                    .size(crate::style::TEXT_SMALL)
                    .color(muted),
            );
        }
        if let Some(entry) = chosen {
            match entry {
                Entry::Part(Place::Uno) => {
                    self.draft.place(Place::Uno, Pt::default());
                    self.tool = Tool::Select;
                    self.refresh_manual();
                }
                Entry::Part(p) => {
                    self.tool = Tool::Place(p);
                    self.draft.pending = None;
                }
                Entry::Module(name) => {
                    self.draft.module = name;
                    self.tool = Tool::Place(Place::Module);
                    self.draft.pending = None;
                }
            }
        }

        ui.add_space(10.0);
        ui.label(RichText::new(self.t("manual.tip")).size(11.0).color(muted));
        ui.label(RichText::new(self.t("manual.drag_hint")).size(11.0).color(muted));
        ui.label(RichText::new(self.t("manual.bend_hint")).size(11.0).color(muted));
        ui.label(RichText::new(self.t("manual.delete_hint")).size(11.0).color(muted));
        ui.label(RichText::new(self.t("manual.snap")).size(11.0).color(muted));

        if !self.draft.circuit.parts.is_empty() {
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(6.0);
            let count = self.draft.circuit.parts.len();
            ui.label(RichText::new(format!("{}: {count}", self.t("bom.title"))).size(12.0).color(muted));
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui, accent: Color32, fg: Color32, muted: Color32) {
        let Some(circuit) = self.active_circuit() else {
            ui.label(RichText::new(self.t("checks.none")).size(12.0).color(muted));
            return;
        };
        let bom = templates::bom(circuit, self.lang);
        let parts: usize = bom.iter().map(|(_, _, count)| *count).sum();
        let sim_summary = match self.mode {
            Mode::Ai => self.built.as_ref().map(|b| {
                (
                    b.title.clone(),
                    b.summary.get(self.lang).to_string(),
                    b.notes.iter().map(|l| l.get(self.lang).to_string()).collect::<Vec<_>>(),
                )
            }),
            Mode::Manual => None,
        };

        if let Some((title, summary, notes)) = &sim_summary {
            ui.label(RichText::new(title).size(15.0).strong().color(fg));
            ui.label(RichText::new(summary).size(12.0).color(muted));
            ui.add_space(10.0);
            let _ = notes;
            if !notes.is_empty() {
                let notes = notes.clone();
                ui.label(RichText::new(self.t("notes.title")).size(13.0).strong().color(fg));
                for n in &notes {
                    ui.label(RichText::new(format!("• {n}")).size(11.5).color(muted));
                }
                ui.add_space(10.0);
            }
        }
        // The full list lives in its own tab, the way Tinkercad keeps the
        // Component List out of the editing panel. Here it is one line and a
        // door, not a second copy that can drift.
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.t("bom.title")).size(13.0).strong().color(fg));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(parts.to_string()).size(12.0).color(accent));
            });
        });
        ui.add_space(3.0);
        let open_bom = Button::new(RichText::new(self.t("bom.open")).size(11.5))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, muted.gamma_multiply(0.5)))
            .corner_radius(crate::style::RADIUS)
            .min_size(vec2(ui.available_width(), 26.0));
        if ui.add(open_bom).clicked() {
            self.tab = Tab::Parts;
        }

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(8.0);
        // One Export button and a small menu of formats. The big button
        // repeats whatever was picked last time.
        let kind = match (self.export_kind, self.mode) {
            (SaveKind::Sketch, Mode::Manual) => SaveKind::Bundle,
            (kind, _) => kind,
        };
        let main_label = format!("{} {}", self.t("export.title"), kind.short());
        let hint = format!(
            "{} · {}",
            self.t(&format!("save.{}", kind.key())),
            self.t("export.more")
        );
        let save_png = self.t("save.png");
        let save_svg = self.t("save.svg");
        let save_bundle = self.t("save.bundle");
        let save_sketch = self.t("save.sketch");
        let save_csv = self.t("save.csv");
        let mut picked: Option<SaveKind> = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let arrow = 30.0;
            let main_w = (ui.available_width() - arrow - 4.0).max(60.0);
            let export = Button::new(RichText::new(main_label).size(12.5))
                .min_size(vec2(main_w, 30.0))
                .corner_radius(crate::style::RADIUS);
            if ui.add(export).on_hover_text(&hint).clicked() {
                self.export_kind_now();
            }
            let arrow_resp = ui.menu_button("▾", |ui| {
                if ui.button(save_png).clicked() {
                    picked = Some(SaveKind::Png);
                    ui.close();
                }
                if ui.button(save_svg).clicked() {
                    picked = Some(SaveKind::Svg);
                    ui.close();
                }
                if ui.button(save_bundle).clicked() {
                    picked = Some(SaveKind::Bundle);
                    ui.close();
                }
                if self.mode == Mode::Ai && ui.button(save_sketch).clicked() {
                    picked = Some(SaveKind::Sketch);
                    ui.close();
                }
                if ui.button(save_csv).clicked() {
                    picked = Some(SaveKind::Csv);
                    ui.close();
                }
                if ui.button(self.t("save.netlist")).clicked() {
                    picked = Some(SaveKind::Netlist);
                    ui.close();
                }
                if ui.button(self.t("save.pdf")).clicked() {
                    picked = Some(SaveKind::Pdf);
                    ui.close();
                }
            });
            arrow_resp.response.on_hover_text(self.t("export.more"));
        });
        if let Some(kind) = picked {
            self.pick_export(kind);
        }

        if self.mode == Mode::Ai && self.built.is_some() {
            ui.add_space(8.0);
            // The way out of the generated circuit: keep it, fix it by hand.
            // Undo and redo belong to that bench, not to this one.
            if ui
                .add(
                    Button::new(RichText::new(self.t("manual.edit")).size(12.0))
                        .fill(accent.gamma_multiply(0.18))
                        .stroke(Stroke::new(1.0, accent))
                        .corner_radius(crate::style::RADIUS)
                        .min_size(vec2(ui.available_width(), 30.0)),
                )
                .clicked()
            {
                self.edit_by_hand();
            }
        }
    }

    fn center_panel(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        accent: Color32,
        fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (tab, key) in [
                (Tab::Circuit, "tab.circuit"),
                (Tab::Parts, "tab.parts"),
                (Tab::Checks, "tab.checks"),
            ] {
                let on = self.tab == tab;
                let label = self.t(key);
                if tab_button(ui, &label, on, crate::style::TEXT_BODY, accent, fg, muted).clicked() {
                    self.tab = tab;
                }
            }
            if self.mode == Mode::Ai {
                let on = self.tab == Tab::Code;
                let label = self.t("tab.code");
                if tab_button(ui, &label, on, crate::style::TEXT_BODY, accent, fg, muted).clicked() {
                    self.tab = Tab::Code;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.tab == Tab::Circuit {
                    if let Some(f) = &self.check_focus {
                        let label = format!("{} · {} ×", self.t("checks.show"), f.name);
                        let resp = ui
                            .add(
                                Button::new(RichText::new(label).size(11.0))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, accent.gamma_multiply(0.7)))
                                    .corner_radius(radius),
                            )
                            .on_hover_text(self.t("checks.show.hint"));
                        if resp.clicked() {
                            self.check_focus = None;
                        }
                        ui.add_space(4.0);
                    }
                    ui.label(
                        RichText::new(format!("{}%", (self.view.zoom * 100.0).round() as i32))
                            .size(11.0)
                            .color(muted),
                    );
                    // One door to every drawing choice: background, labels,
                    // cable routing and "fit". Four buttons were four guesses.
                    let view_label = format!("{} ▾", self.t("view.title"));
                    let mut toggle_labels = false;
                    let mut toggle_bg = false;
                    let mut fit = false;
                    let mut style: Option<WireStyle> = None;
                    let labels_on = self.labels;
                    let dark = self.bench_dark;
                    let current = self.wire_style;
                    let (arc_lbl, straight_lbl, ortho_lbl) = (
                        self.t("wire.arc"),
                        self.t("wire.straight"),
                        self.t("wire.ortho"),
                    );
                    let (labels_lbl, fit_lbl) = (
                        self.t("canvas.labels"),
                        self.t("canvas.fit"),
                    );
                    let (bg_now, narrow_lbl) = (
                        self.t(if dark { "bg.black" } else { "bg.white" }),
                        self.t("view.narrow"),
                    );
                    ui.menu_button(view_label, |ui| {
                        let mark = |on: bool| if on { "✓ " } else { "  " };
                        if ui
                            .button(format!("{}{}", mark(labels_on), labels_lbl))
                            .clicked()
                        {
                            toggle_labels = true;
                            ui.close();
                        }
                        if ui.button(format!("{}{}", mark(dark), bg_now)).clicked() {
                            toggle_bg = true;
                            ui.close();
                        }
                        if ui.button(fit_lbl).clicked() {
                            fit = true;
                            ui.close();
                        }
                        if ui.button(narrow_lbl).clicked() {
                            self.narrow = true;
                            ui.close();
                        }
                        ui.separator();
                        for (choice, lbl, on) in [
                            (WireStyle::Arc, arc_lbl, current == WireStyle::Arc),
                            (WireStyle::Straight, straight_lbl, current == WireStyle::Straight),
                            (WireStyle::Ortho, ortho_lbl, current == WireStyle::Ortho),
                        ] {
                            if ui.button(format!("{}{}", mark(on), lbl)).clicked() {
                                style = Some(choice);
                                ui.close();
                            }
                        }
                    });
                    if fit {
                        self.view = View::default();
                    }
                    if toggle_bg {
                        self.toggle_bench();
                    }
                    if let Some(style) = style {
                        self.set_wire_style(style);
                        match self.mode {
                            Mode::Ai => self.refresh_ai(),
                            Mode::Manual => self.refresh_manual(),
                        }
                    }
                    if toggle_labels {
                        self.labels = !self.labels;
                        self.hover = None;
                        match self.mode {
                            Mode::Ai => self.refresh_ai(),
                            Mode::Manual => self.refresh_manual(),
                        }
                    }
                }
            });
        });
        // One rule under the whole strip ties the tabs to the sheet below
        // them and separates the toolbar from the drawing.
        let rule_y = ui.min_rect().bottom() + 4.0;
        ui.painter().hline(
            ui.max_rect().x_range(),
            rule_y,
            Stroke::new(1.0, muted.gamma_multiply(0.28)),
        );
        ui.add_space(10.0);

        match self.tab {
            Tab::Circuit => self.canvas(ui, accent, panel, radius),
            Tab::Parts => self.parts_tab(ui, radius),
            Tab::Code => self.code_tab(ui, accent, fg, muted, panel, radius),
            Tab::Checks => self.checks_tab(ui, accent, fg, muted, panel, radius, ctx),
        }
    }

    /// Move the grabbed part. A loose part follows the pointer; a seated one
    /// jumps by whole holes, carried by the board's own axes.
    fn drag_to(&mut self, part: usize, cursor: Pt) {
        if part >= self.draft.circuit.parts.len() {
            return;
        }
        let dx = cursor.x - self.drag.pointer.x;
        let dy = cursor.y - self.drag.pointer.y;
        if self.drag.seated {
            let turn = -(self.draft.circuit.board.rot as i32);
            let local = Pt::new(dx, dy).rotate(Pt::new(0.0, 0.0), turn);
            let dcol = (local.x / crate::model::PITCH).round() as i32;
            let drow = (local.y / crate::model::PITCH).round() as i32;
            let step = (dcol - self.drag.applied.0, drow - self.drag.applied.1);
            if step == (0, 0) {
                return;
            }
            if self.draft.reseat_recorded(part, step.0, step.1) {
                self.drag.applied = (dcol, drow);
                self.drag.moved = true;
                self.refresh_manual();
            }
        } else {
            let to = Pt::new(self.drag.at.x + dx, self.drag.at.y + dy);
            if self.draft.move_part_recorded(part, to) {
                self.drag.moved = true;
                self.refresh_manual();
            }
        }
    }

    /// Start a drag: a cable bend if the pointer is on one, otherwise a part.
    /// A cable is only a couple of millimetres wide, so a hit on it is meant
    /// for it even when it runs across a part's body.
    fn grab_at(&mut self, at: Pt, frame: crate::model::Rect, add: bool) -> bool {
        if std::env::var("VOLTA_DEBUG").is_ok() {
            eprintln!(
                "grab ({:.2},{:.2}) via {:?} wire {:?} part {:?}",
                at.x,
                at.y,
                self.draft.via_at(at),
                self.draft.wire_at(at, self.wire_style),
                self.draft.part_at(at)
            );
        }
        if self.tool == Tool::Select {
            if let Some((wire, via)) = self.draft.via_at(at) {
                self.draft.snapshot();
                self.drag = Drag {
                    via: Some((wire, via)),
                    pointer: at,
                    frame: Some(frame),
                    ..Drag::default()
                };
                self.active_wire = Some(wire);
                self.refresh_manual();
                return true;
            }
            if let Some(wire) = self.draft.wire_at(at, self.wire_style) {
                self.draft.snapshot();
                if let Some(via) = self.draft.insert_via(wire, at, self.wire_style) {
                    self.drag = Drag {
                        via: Some((wire, via)),
                        via_new: true,
                        pointer: at,
                        frame: Some(frame),
                        ..Drag::default()
                    };
                    self.active_wire = Some(wire);
                    self.refresh_manual();
                    return true;
                }
                self.draft.cancel_snapshot();
            }
        }
        let Some(part) = self.draft.part_at(at) else { return false };
        self.drag = Drag {
            part: Some(part),
            pointer: at,
            at: self.draft.circuit.parts[part].at,
            applied: (0, 0),
            seated: self.draft.circuit.part_is_seated(&self.draft.circuit.parts[part]),
            moved: false,
            frame: Some(frame),
            ..Drag::default()
        };
        self.draft.snapshot();
        self.active_wire = None;
        let changed = self.draft.selected != Some(part);
        self.draft.select_part(part, add);
        if changed {
            self.refresh_manual();
        }
        true
    }

    fn canvas(&mut self, ui: &mut egui::Ui, accent: Color32, panel: Color32, radius: CornerRadius) {
        let size = ui.available_size();
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let rect = resp.rect;
        painter.rect_filled(rect, radius, panel);

        let Some(scene) = self.active_scene().cloned() else {
            painter.rect_filled(rect, radius, panel);
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                self.t(match (self.mode, self.error.is_some()) {
                    (Mode::Ai, true) => "task.unknown",
                    (Mode::Ai, false) => "task.empty",
                    (Mode::Manual, _) => "manual.empty",
                }),
                FontId::proportional(14.0),
                self.c32(self.theme.muted),
            );
            return;
        };
        let circuit = match self.active_circuit() {
            Some(c) => c.clone(),
            None => return,
        };
        // The sheet covers the whole canvas, not just the drawing frame, so
        // the white or black background reads as a page rather than a patch.
        painter.rect_filled(rect, radius, self.c32(scene.bg));

        let base = (rect.width() / scene.w).min(rect.height() / scene.h) * 0.96;
        if !self.view.fitted {
            self.view.zoom = 1.0;
            self.view.pan = Vec2::ZERO;
            self.view.fitted = true;
        }
        let mut scale = base * self.view.zoom;
        let mut origin = rect.center() - vec2(scene.w * scale / 2.0, scene.h * scale / 2.0) + self.view.pan;

        if resp.hovered() {
            // The wheel zooms about the pointer; a pinch or ctrl+wheel arrives
            // as a separate multiplicative factor. Both are applied to the
            // same anchor, so whatever the hands do, the point under them
            // stays put.
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = (scroll * 0.0015).exp() * pinch;
            if (factor - 1.0).abs() > 1e-6 {
                if let Some(pos) = resp.hover_pos() {
                    let before = (pos - origin) / scale;
                    self.view.zoom = (self.view.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
                    scale = base * self.view.zoom;
                    let o2 = pos - vec2(before.x * scale, before.y * scale);
                    self.view.pan = o2 - rect.center() + vec2(scene.w * scale / 2.0, scene.h * scale / 2.0);
                    origin = o2;
                }
            }
            // egui spreads one wheel notch over several frames, and those
            // frames only happen if someone asks for them. Without this the
            // tail of a scroll waits for the next mouse move and lands in a
            // single lump, which is what makes the zoom jump now and then.
            if scroll.abs() > 0.01 {
                ui.ctx().request_repaint();
            }
        }
        if resp.dragged() && !self.drag.active() {
            self.view.pan += resp.drag_delta();
            origin += resp.drag_delta();
        }

        painter.rect_filled(
            egui::Rect::from_min_size(origin, vec2(scene.w * scale, scene.h * scale)),
            CornerRadius::ZERO,
            self.c32(scene.bg),
        );
        paint_scene(&painter, &scene, origin, scale, rect);
        if let Some(f) = &self.check_focus {
            paint_focus(&painter, &circuit, f, origin, scale, scene.origin, rect);
        }

        // The scene is drawn shifted to its own origin, so screen pixels have
        // to be mapped back through that shift before they mean anything to the
        // circuit.
        let to_world = |p: Pos2| screen_to_bench(p, origin, scale, scene.origin);
        if self.show_grid {
            let tl = to_world(rect.min);
            let br = to_world(rect.max);
            let step = crate::model::PITCH;
            let colour = Color32::from_rgba_unmultiplied(120, 140, 160, 28);
            let stroke = Stroke::new(0.6, colour);
            let mut col = (tl.x / step).floor() as i32;
            while col as f32 * step <= br.x {
                let x = col as f32 * step;
                let a = bench_to_screen(Pt::new(x, tl.y), origin, scale, scene.origin);
                let b = bench_to_screen(Pt::new(x, br.y), origin, scale, scene.origin);
                painter.line_segment([a, b], stroke);
                col += 1;
            }
            let mut row = (tl.y / step).floor() as i32;
            while row as f32 * step <= br.y {
                let y = row as f32 * step;
                let a = bench_to_screen(Pt::new(tl.x, y), origin, scale, scene.origin);
                let b = bench_to_screen(Pt::new(br.x, y), origin, scale, scene.origin);
                painter.line_segment([a, b], stroke);
                row += 1;
            }
        }
        if !resp.dragged() {
            if let Some(p) = resp.hover_pos() {
                self.hover_world = Some(to_world(p));
            }
        }
        // A press is followed by a drag several frames later, by which time
        // the pointer has already moved. The bench point under the press is
        // the one that decides whether the hand meant a cable or the part
        // behind it, so keep it while it is still fresh.
        if resp.contains_pointer() && ui.input(|i| i.pointer.primary_pressed()) {
            self.press_world = self.hover_world;
        }

        // Grabbing a part drags it: loose parts follow the pointer, plugged
        // parts hop from hole to hole. Empty canvas still pans.
        if self.mode == Mode::Manual {
            if resp.drag_started() {
                // Use where the pointer was when it pressed: by the time a
                // drag is reported the pointer may already have slid a few
                // millimetres, which is the whole width of a cable.
                let at = self.press_world.take().or(self.hover_world);
                if let Some(at) = at {
                    let frame = self
                        .draft
                        .circuit
                        .bbox_with(self.wire_style)
                        .inflate(crate::scene::PAD);
                    let add = ui.input(|i| i.modifiers.shift);
                    if !self.grab_at(at, frame, add) && self.mode == Mode::Manual && !add {
                        self.marquee = Some(at);
                    }
                }
            }
            if let (Some((wire, via)), Some(pos)) = (self.drag.via, resp.interact_pointer_pos()) {
                if resp.dragged() && self.draft.move_via(wire, via, to_world(pos)) {
                    self.drag.moved = true;
                    self.refresh_manual();
                }
            }
            if let (Some(part), Some(pos)) = (self.drag.part, resp.interact_pointer_pos()) {
                if resp.dragged() {
                    self.drag_to(part, to_world(pos));
                }
            }
            if let (Some(a), Some(pos)) = (self.marquee, resp.interact_pointer_pos()) {
                if resp.dragged() {
                    let b = to_world(pos);
                    let box_ = crate::model::Rect::new(
                        a.x.min(b.x),
                        a.y.min(b.y),
                        (a.x - b.x).abs(),
                        (a.y - b.y).abs(),
                    );
                    let ids: Vec<_> = circuit
                        .parts_id()
                        .filter(|(_, p)| box_.intersects(circuit.part_rect(p)))
                        .map(|(i, _)| i)
                        .collect();
                    self.draft.clear_selection();
                    self.draft.selected_parts.extend(ids);
                    self.draft.selected = self.draft.selected_parts.iter().next_back().copied();
                }
            }
            if resp.drag_stopped() && self.marquee.take().is_some() {
                self.refresh_manual();
            }
            if resp.drag_stopped() && self.drag.active() {
                if !self.drag.moved {
                    // A bend that was only clicked, not pulled, is dropped.
                    if let Some((wire, via)) = self.drag.via {
                        if self.drag.via_new && self.draft.remove_via(wire, via) {
                            self.refresh_manual();
                        }
                    }
                    self.draft.cancel_snapshot();
                } else {
                    self.hover = None;
                    self.refresh_manual();
                }
                self.drag = Drag::default();
            }
        }

        let mut hit = Hit { origin, scale, rect, ..Default::default() };
        if resp.clicked() {
            hit.click = resp.interact_pointer_pos().map(to_world);
            self.check_focus = None;
        }
        if resp.secondary_clicked() {
            hit.right = resp.interact_pointer_pos().map(to_world);
            self.check_focus = None;
        }
        if resp.hovered() {
            if let Some(p) = resp.hover_pos() {
                hit.hover = Some(to_world(p));
            }
        }
        if let Some(w) = hit.hover {
            let radius = 9.0 / scale;
            let mut best = (radius, None);
            for (i, pin) in circuit.pins_id() {
                let d = pin.pos.dist(Pt::new(w.x, w.y));
                if d < best.0 {
                    best = (d, Some(i));
                }
            }
            hit.pin = best.1;
            // A part under the pointer says what it is and why it is here.
            // The hint follows the pointer and disappears with it, so it
            // never competes with the bench for room.
            if let Some(i) = part_under(&circuit, w) {
                if hit.pin.is_none() {
                    let p = &circuit.parts[i];
                    let text = format!(
                        "{} {} · {}",
                        p.label,
                        p.value,
                        part_why(p.kind, self.lang)
                    );
                    let ink = self.c32(self.theme.fg);
                    resp.clone().on_hover_ui_at_pointer(|ui| {
                        ui.label(RichText::new(text).size(12.0).color(ink));
                    });
                }
            }
        }

        // A rubber cable while a wire is half-pulled.
        if self.mode == Mode::Manual {
            if let (Some(pending), Some(cursor)) = (self.draft.pending, hit.hover) {
                let pa = bench_to_screen(circuit.end_pos(pending), origin, scale, scene.origin);
                let pb = bench_to_screen(cursor, origin, scale, scene.origin);
                painter.add(egui::Shape::line_segment(
                    [pa, pb],
                    Stroke::new(2.2, self.c32(self.theme.accent)),
                ));
            }
        }

        // A cable under the pointer opens up its grips, so the bends are
        // discoverable without a special tool.
        let wire_hover = if self.mode == Mode::Manual && self.tool == Tool::Select {
            match (hit.hover, hit.pin, self.drag.via) {
                (_, _, Some((wire, _))) => Some(wire),
                (Some(w), None, None) if self.draft.part_at(w).is_none() => {
                    self.draft.wire_at(w, self.wire_style)
                }
                _ => None,
            }
        } else {
            None
        };
        let hover_changed = hit.pin != self.hover || wire_hover != self.active_wire;
        // Keep the pin coordinates before the borrows end.
        let pin = hit.pin;
        self.hover = pin;
        if self.drag.via.is_none() {
            self.active_wire = wire_hover;
        }
        if hover_changed {
            match self.mode {
                Mode::Ai => self.refresh_ai(),
                Mode::Manual => self.refresh_manual(),
            }
            ui.ctx().set_cursor_icon(if pin.is_some() || wire_hover.is_some() {
                egui::CursorIcon::PointingHand
            } else {
                egui::CursorIcon::Default
            });
            return;
        }
        if self.mode == Mode::Manual {
            let icon = if self.drag.active() {
                egui::CursorIcon::Grabbing
            } else if hit
                .hover
                .and_then(|w| self.draft.part_at(w))
                .is_some()
            {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::Default
            };
            ui.ctx().set_cursor_icon(icon);
        }

        if self.mode == Mode::Ai {
            if let Some(at) = hit.click {
                self.tab = Tab::Circuit;
                let _ = at;
            }
            return;
        }

        if let Some(at) = hit.right {
            if self.draft.delete_at(at, self.wire_style) {
                self.hover = None;
                self.active_wire = None;
                self.refresh_manual();
            }
            return;
        }
        let Some(at) = hit.click else { return };
        match self.tool {
            Tool::Wire => {
                let end = self.draft.end_at(at, 1.6);
                if end == crate::model::End::Hole(crate::model::Hole::OffBoard) {
                    return;
                }
                match self.draft.pending {
                    None => self.draft.pending = Some(end),
                    Some(first) => {
                        self.draft.pending = None;
                        if self.draft.connect(first, end) {
                            self.refresh_manual();
                        }
                    }
                }
            }
            Tool::Place(place) => {
                if self.draft.place(place, at) {
                    self.hover = None;
                    self.refresh_manual();
                } else {
                    self.status = self.t("manual.snap");
                    self.status_bad = true;
                }
            }
            Tool::Select => {
                let scene_at = Pt::new(at.x - scene.origin.x, at.y - scene.origin.y);
                let painted = scene.hit_owner(scene_at);
                let picked = match painted {
                    Some(crate::scene::Owner::Part(p)) => Some(p),
                    _ => self.draft.part_at(at),
                };
                let add = ui.input(|i| i.modifiers.shift);
                if let Some(wire) = match painted {
                    Some(crate::scene::Owner::Wire(w)) => Some(w),
                    _ => None,
                } {
                    let before = self.draft.selected_wires.clone();
                    self.draft.select_wire(wire, add);
                    self.active_wire = Some(wire);
                    if before != self.draft.selected_wires {
                        self.refresh_manual();
                    }
                } else if let Some(p) = picked {
                    let before = self.draft.selected_parts.clone();
                    self.draft.select_part(p, add);
                    if before != self.draft.selected_parts {
                        self.refresh_manual();
                    }
                } else if !add && !self.draft.selected_parts.is_empty() {
                    self.draft.clear_selection();
                    self.active_wire = None;
                    self.refresh_manual();
                }
            }
        }
        let _ = accent;
    }

    fn code_tab(
        &mut self,
        ui: &mut egui::Ui,
        accent: Color32,
        fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
    ) {
        let Some(b) = &self.built else {
            ui.label(RichText::new(self.t("checks.none")).size(12.0).color(muted));
            return;
        };
        let code = b.code.clone();
        ui.horizontal(|ui| {
            if ui
                .add(Button::new(RichText::new(self.t("code.copy")).size(11.5)))
                .clicked()
            {
                ui.ctx().copy_text(code.clone());
                self.status = self.t("code.copied");
            }
            if ui
                .add(Button::new(RichText::new(self.t("code.save")).size(11.5)))
                .clicked()
            {
                self.save_sketch();
            }
            ui.label(
                RichText::new(format!("{} · .ino", code.lines().count()))
                    .size(11.0)
                    .color(muted),
            );
        });
        ui.add_space(6.0);
        Frame::NONE
            .fill(panel)
            .corner_radius(radius)
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    let mut view = code.as_str();
                    ui.add(
                        TextEdit::multiline(&mut view)
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .interactive(false)
                            .text_color(fg),
                    );
                });
            });
        let _ = accent;
    }

    /// The component list, drawn the way Tinkercad draws it: a white sheet,
    /// a grey header of Name / Quantity / Component, one thin rule between
    /// rows, and the designators of identical parts stacked in the name cell.
    fn parts_tab(&mut self, ui: &mut egui::Ui, radius: CornerRadius) {
        let Some(circuit) = self.active_circuit().cloned() else {
            ui.label(
                RichText::new(self.t("checks.none"))
                    .size(12.0)
                    .color(self.c32(self.theme.muted)),
            );
            return;
        };
        let rows = templates::component_rows(&circuit, self.lang);
        let title = self.t("parts.title");
        let csv = self.t("parts.csv");
        let name_head = self.t("parts.name");
        let qty_head = self.t("parts.qty");
        let comp_head = self.t("parts.component");
        let ink = Color32::from_rgb(0x2F, 0x2F, 0x2F);
        let quiet = Color32::from_rgb(0x8A, 0x8A, 0x8A);
        let line = Color32::from_rgb(0xE2, 0xE0, 0xDC);
        let head = Color32::from_rgb(0xF1, 0xEF, 0xEB);
        let name_font = FontId::proportional(13.0);
        let body_font = FontId::proportional(13.5);
        let head_font = FontId::proportional(12.0);

        // The sheet takes the height of its rows, like the Tinkercad panel,
        // and the whole card scrolls when the list gets long.
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                Frame::NONE
                    .fill(Color32::WHITE)
                    .corner_radius(radius)
                    .inner_margin(Margin::same(18))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(title).size(16.0).strong().color(ink));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let button =
                                    Button::new(RichText::new(csv).size(12.0).color(ink))
                                        .fill(Color32::WHITE)
                                        .stroke(Stroke::new(
                                            1.0,
                                            Color32::from_rgb(0xCB, 0xC8, 0xC2),
                                        ))
                                        .corner_radius(crate::style::RADIUS);
                                if ui.add(button).clicked() {
                                    self.ask_save(SaveKind::Csv);
                                }
                            });
                        });
                        ui.add_space(12.0);

                        let width = ui.available_width();
                        let name_w = (width * 0.24).clamp(64.0, 190.0);
                        let qty_w = (width * 0.14).clamp(52.0, 110.0);
                        let comp_x = name_w + qty_w;
                        let wrap = (width - comp_x - 24.0).max(60.0);
                        let head_h = 30.0;
                        let galleys: Vec<Arc<egui::Galley>> = rows
                            .iter()
                            .map(|r| {
                                ui.painter().layout(
                                    r.component.clone(),
                                    body_font.clone(),
                                    ink,
                                    wrap,
                                )
                            })
                            .collect();
                        let heights: Vec<f32> = rows
                            .iter()
                            .zip(&galleys)
                            .map(|(r, g)| {
                                (r.names.len().max(1) as f32 * 17.0 + 12.0)
                                    .max(g.size().y + 12.0)
                                    .max(34.0)
                            })
                            .collect();
                        let total = head_h + heights.iter().sum::<f32>();
                        let (rect, _) =
                            ui.allocate_exact_size(vec2(width, total), Sense::hover());
                        let p = ui.painter_at(rect);
                        let left = rect.left();

                        p.rect_filled(
                            egui::Rect::from_min_size(rect.left_top(), vec2(width, head_h)),
                            CornerRadius::ZERO,
                            head,
                        );
                        p.text(
                            pos2(left + 12.0, rect.top() + head_h / 2.0),
                            Align2::LEFT_CENTER,
                            name_head,
                            head_font.clone(),
                            quiet,
                        );
                        p.text(
                            pos2(left + name_w + qty_w / 2.0, rect.top() + head_h / 2.0),
                            Align2::CENTER_CENTER,
                            qty_head,
                            head_font.clone(),
                            quiet,
                        );
                        p.text(
                            pos2(left + comp_x + 12.0, rect.top() + head_h / 2.0),
                            Align2::LEFT_CENTER,
                            comp_head,
                            head_font.clone(),
                            quiet,
                        );

                        let mut y = rect.top() + head_h;
                        for i in 0..rows.len() {
                            let row = &rows[i];
                            let height = heights[i];
                            let galley = &galleys[i];
                            // Names stack: M1 on the first line, M2 below it,
                            // while the quantity stays one number for both.
                            if row.names.is_empty() {
                                p.text(
                                    pos2(left + 12.0, y + height / 2.0),
                                    Align2::LEFT_CENTER,
                                    "—",
                                    name_font.clone(),
                                    quiet,
                                );
                            } else {
                                for (i, name) in row.names.iter().enumerate() {
                                    p.text(
                                        pos2(left + 12.0, y + 12.0 + i as f32 * 17.0),
                                        Align2::LEFT_CENTER,
                                        name,
                                        name_font.clone(),
                                        ink,
                                    );
                                }
                            }
                            p.text(
                                pos2(left + name_w + qty_w / 2.0, y + height / 2.0),
                                Align2::CENTER_CENTER,
                                row.quantity.to_string(),
                                body_font.clone(),
                                ink,
                            );
                            p.galley(
                                pos2(left + comp_x + 12.0, y + (height - galley.size().y) / 2.0),
                                galley.clone(),
                                ink,
                            );
                            y += height;
                            p.line_segment(
                                [pos2(left, y), pos2(left + width, y)],
                                Stroke::new(1.0, line),
                            );
                        }
                        p.rect_stroke(
                            egui::Rect::from_min_size(rect.left_top(), vec2(width, total)),
                            CornerRadius::ZERO,
                            Stroke::new(1.0, line),
                            StrokeKind::Inside,
                        );
                    });
            });
    }

    fn checks_tab(
        &mut self,
        ui: &mut egui::Ui,
        accent: Color32,
        fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
        ctx: &egui::Context,
    ) {
        let report = match self.mode {
            Mode::Ai => self.ai_report(),
            Mode::Manual => Some(checks::run_circuit(&self.draft.circuit, self.lang, None)),
        };
        let Some(r) = report else {
            ui.label(RichText::new(self.t("checks.none")).size(12.0).color(muted));
            return;
        };
        let warned = r.checks.iter().filter(|c| c.sev == Sev::Warn).count();
        let failed = r.failed();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} ✓", r.passed()))
                    .size(12.0)
                    .color(accent),
            );
            if warned > 0 {
                ui.label(
                    RichText::new(format!("{warned} !"))
                        .size(12.0)
                        .color(Color32::from_rgb(0xD8, 0xA3, 0x2E)),
                );
            }
            if failed > 0 {
                ui.label(
                    RichText::new(format!("{failed} ×"))
                        .size(12.0)
                        .color(Color32::from_rgb(0xD2, 0x4B, 0x45)),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (label, key) = if self.checks_all {
                    (self.t("checks.only_bad"), "checks.only_bad")
                } else {
                    (self.t("checks.all"), "checks.all")
                };
                let _ = key;
                if ui
                    .add(Button::new(RichText::new(label).size(11.0)).corner_radius(radius))
                    .clicked()
                {
                    self.checks_all = !self.checks_all;
                }
            });
        });
        ui.add_space(6.0);
        Frame::NONE
            .fill(panel)
            .corner_radius(radius)
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    // The failures are the reason this tab was opened; the
                    // green lines are evidence, shown on request.
                    let shown: Vec<&checks::Check> = r
                        .checks
                        .iter()
                        .filter(|c| self.checks_all || c.sev != Sev::Pass)
                        .collect();
                    if shown.is_empty() {
                        ui.label(
                            RichText::new(self.t("checks.clean")).size(12.0).color(accent),
                        );
                    }
                    for (row, c) in shown.iter().enumerate() {
                        let (mark, col) = match c.sev {
                            Sev::Pass => ("✓", accent),
                            Sev::Warn => ("!", Color32::from_rgb(0xD8, 0xA3, 0x2E)),
                            Sev::Fail => ("×", Color32::from_rgb(0xD2, 0x4B, 0x45)),
                        };
                        let can_show = c.focus.as_ref().map(|f| !f.is_empty()).unwrap_or(false);
                        let inner = ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(mark).size(13.0).color(col));
                            ui.label(
                                RichText::new(format!("{:<22}", c.name.get(self.lang)))
                                    .size(12.0)
                                    .color(fg),
                            );
                            ui.label(RichText::new(&c.detail).size(11.5).color(muted));
                            if can_show {
                                ui.label(RichText::new("→").size(11.0).color(accent));
                            }
                        });
                        if can_show {
                            // The row itself is the button, not the little
                            // arrow: the whole line is a comfortable target.
                            let resp = ui.interact(
                                inner.response.rect,
                                ui.make_persistent_id(("check-focus", row)),
                                Sense::click(),
                            );
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                if let Some(f) = c.focus.clone() {
                                    self.check_focus = Some(CheckFocus {
                                        focus: f,
                                        name: c.name.get(self.lang).to_string(),
                                        sev: c.sev,
                                    });
                                    self.tab = Tab::Circuit;
                                }
                            }
                        }
                        ui.add_space(3.0);
                    }
                });
            });
        if let Some(sim) = &r.sim {
            ui.add_space(10.0);
            ui.label(RichText::new(self.t("checks.sim")).size(12.0).strong().color(fg));
            let (resp, painter) = ui.allocate_painter(vec2(ui.available_width(), 90.0), Sense::hover());
            let area = resp.rect;
            painter.rect_filled(area, radius, panel);
            if sim.points.len() > 1 {
                let n = sim.points.len();
                let pts: Vec<Pos2> = sim
                    .points
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        pos2(
                            area.left() + area.width() * i as f32 / (n - 1) as f32,
                            area.bottom() - area.height() * (v / 255.0).clamp(0.0, 1.0) * 0.86 - 4.0,
                        )
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, Stroke::new(1.6, accent)));
            }
            let _ = ctx;
        }
    }

    /// A small window that follows the selection: pick the value of the part
    /// being worked on. It is shown only for parts that have a choice, and it
    /// does not crowd the palette with one button per value.
    /// The projects window: a name, Save, and the folder of what was saved
    /// before. It is a list, not a file manager: open, rename by saving under
    /// a new name, delete with one confirmation.
    fn projects_window(
        &mut self,
        ctx: &egui::Context,
        accent: Color32,
        fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
    ) {
        if !self.projects_open {
            return;
        }
        let title = self.t("projects.title");
        let name_lbl = self.t("projects.name");
        let save_lbl = self.t("projects.save");
        let list_lbl = self.t("projects.list");
        let empty_lbl = self.t("projects.empty");
        let open_lbl = self.t("projects.open");
        let del_lbl = self.t("projects.delete");
        let confirm_lbl = self.t("projects.confirm");
        let close_lbl = self.t("projects.close");
        let autosave_lbl = self.t("projects.autosave");
        let autosave_hint = self.t("projects.autosave.hint");
        let manual_lbl = self.t("projects.manual");
        let ai_lbl = self.t("projects.ai");
        let parts_lbl = self.t("projects.parts");
        let current_lbl = self.t("projects.current");
        let broken_lbl = self.t("projects.broken");

        let mut close = false;
        let mut save = false;
        let mut open: Option<PathBuf> = None;
        let mut remove: Option<PathBuf> = None;
        let mut name = self.project_name.clone();
        let list = self.project_list.clone();
        let current = self.project_path.clone();
        let mut pending = self.project_delete.clone();

        // Dim the bench under the window, like the export and help sheets.
        let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("projects-dim"));
        ctx.layer_painter(layer)
            .rect_filled(ctx.content_rect(), CornerRadius::ZERO, Color32::from_black_alpha(DIM_ALPHA));

        egui::Window::new(title.clone())
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .frame(
                Frame::NONE
                    .fill(panel)
                    .stroke(Stroke::new(1.0, accent.gamma_multiply(0.6)))
                    .corner_radius(radius)
                    .inner_margin(Margin::same(18)),
            )
            .show(ctx, |ui| {
                ui.set_width(520.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&title).size(16.0).strong().color(accent));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(&close_lbl).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(10.0);
                ui.label(RichText::new(&name_lbl).size(11.0).color(muted));
                let resp = ui.add(TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    save = true;
                }
                ui.add_space(6.0);
                let save_btn = Button::new(RichText::new(&save_lbl).strong().size(12.5))
                    .fill(accent)
                    .corner_radius(crate::style::RADIUS)
                    .min_size(vec2(150.0, 30.0));
                if ui.add(save_btn).clicked() {
                    save = true;
                }
                if let Some(e) = &self.project_error {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(e)
                            .size(11.5)
                            .color(Color32::from_rgb(0xD2, 0x4B, 0x45)),
                    );
                }
                ui.add_space(12.0);
                ui.label(RichText::new(&list_lbl).size(11.0).color(muted));
                ui.add_space(4.0);
                ScrollArea::vertical()
                    .max_height(300.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if list.is_empty() {
                            ui.label(RichText::new(&empty_lbl).size(11.5).color(muted));
                        }
                        for e in &list {
                            let auto = project::is_autosave(&e.path);
                            let label = e.name.clone();
                            Frame::NONE
                                .fill(if current.as_deref() == Some(e.path.as_path()) {
                                    accent.gamma_multiply(0.10)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .corner_radius(crate::style::RADIUS)
                                .inner_margin(Margin::symmetric(8, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            let mut head = label.clone();
                                            if auto {
                                                head = format!("{head} · {autosave_lbl}");
                                            }
                                            ui.label(RichText::new(head).size(12.5).strong().color(
                                                if e.broken {
                                                    Color32::from_rgb(0xD2, 0x4B, 0x45)
                                                } else {
                                                    fg
                                                },
                                            ));
                                            let mut meta = if e.manual {
                                                format!(
                                                    "{} · {} · {}",
                                                    manual_lbl,
                                                    parts_lbl.replace("{}", &e.parts.to_string()),
                                                    project::date_text(e.saved)
                                                )
                                            } else {
                                                format!("{} · {}", ai_lbl, project::date_text(e.saved))
                                            };
                                            if e.broken {
                                                meta = broken_lbl.clone();
                                            }
                                            if auto {
                                                meta = autosave_hint.clone();
                                            }
                                            if current.as_deref() == Some(e.path.as_path()) {
                                                meta = format!("{meta} · {current_lbl}");
                                            }
                                            ui.label(RichText::new(meta).size(10.5).color(muted));
                                        });
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            if pending.as_deref() == Some(e.path.as_path()) {
                                                let btn = Button::new(
                                                    RichText::new(&confirm_lbl)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(0xD2, 0x4B, 0x45)),
                                                )
                                                .corner_radius(radius)
                                                .min_size(vec2(74.0, 24.0));
                                                if ui.add(btn).clicked() {
                                                    remove = Some(e.path.clone());
                                                }
                                            } else {
                                                let btn = Button::new(RichText::new(&del_lbl).size(11.0))
                                                    .corner_radius(radius)
                                                    .min_size(vec2(74.0, 24.0));
                                                if ui.add(btn).clicked() {
                                                    pending = Some(e.path.clone());
                                                }
                                            }
                                            let op = Button::new(RichText::new(&open_lbl).size(11.0))
                                                .corner_radius(radius)
                                                .min_size(vec2(74.0, 24.0));
                                            if ui
                                                .add(op)
                                                .on_hover_text(if auto {
                                                    autosave_hint.as_str()
                                                } else {
                                                    ""
                                                })
                                                .clicked()
                                            {
                                                open = Some(e.path.clone());
                                            }
                                        });
                                    });
                                });
                            ui.add_space(2.0);
                        }
                    });
            });

        self.project_name = name;
        self.project_delete = pending;
        if close {
            self.projects_open = false;
            self.project_delete = None;
        } else if let Some(path) = remove {
            self.remove_project(&path);
        } else if let Some(path) = open {
            self.open_project(&path);
        } else if save {
            self.save_project();
        }
    }

    /// "Save as", in the app's own clothes: the folder to browse, the name
    /// to type, Save and Cancel. No portal and no extra dependency, so the
    /// build stays offline and the window matches the rest of the app.
    fn save_window(
        &mut self,
        ctx: &egui::Context,
        accent: Color32,
        fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
    ) {
        let Some(kind) = self.save_ask.as_ref().map(|a| a.kind) else {
            return;
        };
        let title = self.t(&format!("save.{}", kind.key()));
        let folder_lbl = self.t("save.folder");
        let name_lbl = self.t("save.name");
        let save_lbl = self.t("save.save");
        let cancel_lbl = self.t("save.cancel");
        let empty_lbl = self.t("save.empty");
        let up_hint = self.t("save.up");
        let Some(ask) = self.save_ask.as_mut() else {
            return;
        };
        let mut close = false;
        let mut submit = false;
        let mut enter: Option<PathBuf> = None;

        // Dim the bench under the window, the way the help sheet does: the
        // save question is the only thing that matters while it is open.
        let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("save-dim"));
        ctx.layer_painter(layer)
            .rect_filled(ctx.content_rect(), CornerRadius::ZERO, Color32::from_black_alpha(DIM_ALPHA));

        egui::Window::new(title.clone())
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .frame(
                Frame::NONE
                    .fill(panel)
                    .stroke(Stroke::new(1.0, accent.gamma_multiply(0.6)))
                    .corner_radius(radius)
                    .inner_margin(Margin::same(18)),
            )
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&title).size(16.0).strong().color(accent));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(&cancel_lbl).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(10.0);
                ui.label(RichText::new(&folder_lbl).size(11.0).color(muted));
                ui.label(
                    RichText::new(ask.dir.display().to_string())
                        .monospace()
                        .size(11.5)
                        .color(fg),
                );
                ui.add_space(6.0);
                if ui
                    .add(Button::new(RichText::new("↑").size(13.0)).min_size(vec2(28.0, 22.0)))
                    .on_hover_text(&up_hint)
                    .clicked()
                {
                    if let Some(parent) = ask.dir.parent() {
                        enter = Some(parent.to_path_buf());
                    }
                }
                ScrollArea::vertical()
                    .max_height(190.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if ask.folders.is_empty() {
                            ui.label(RichText::new(&empty_lbl).size(11.5).color(muted));
                        }
                        for name in &ask.folders {
                            let row = Button::new(RichText::new(format!("▸ {name}")).size(12.5))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .min_size(vec2(ui.available_width(), 24.0));
                            if ui.add(row).clicked() {
                                enter = Some(ask.dir.join(name));
                            }
                        }
                    });
                ui.add_space(10.0);
                ui.label(RichText::new(&name_lbl).size(11.0).color(muted));
                let resp = ui.add(
                    TextEdit::singleline(&mut ask.name).desired_width(f32::INFINITY),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if let Some(e) = &ask.error {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(e)
                            .size(11.5)
                            .color(Color32::from_rgb(0xD2, 0x4B, 0x45)),
                    );
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let save = Button::new(
                        RichText::new(&save_lbl).strong().size(12.5),
                    )
                    .fill(accent)
                    .corner_radius(crate::style::RADIUS)
                    .min_size(vec2(110.0, 30.0));
                    if ui.add(save).clicked() {
                        submit = true;
                    }
                    if ui
                        .add(
                            Button::new(RichText::new(&cancel_lbl).size(12.0))
                                .corner_radius(crate::style::RADIUS)
                                .min_size(vec2(90.0, 30.0)),
                        )
                        .clicked()
                    {
                        close = true;
                    }
                });
            });

        if let Some(dir) = enter {
            if dir.is_dir() {
                ask.dir = dir;
                ask.folders = folders_in(&ask.dir);
                ask.error = None;
            }
        }
        if close {
            self.save_ask = None;
        } else if submit {
            self.finish_save();
        }
    }

    /// The picker behind a chip of "I read it as". One field changes; the
    /// sentence, the other two fields and the reader's intent stay as they
    /// were.
    fn recipe_window(
        &mut self,
        ctx: &egui::Context,
        accent: Color32,
        fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
    ) {
        let Some(chip) = self.recipe_ask.as_ref().map(|a| a.chip) else {
            return;
        };
        let title = self.t("recipe.title");
        let threshold_lbl = self.t("recipe.threshold");
        let above_lbl = self.t("recipe.above");
        let below_lbl = self.t("recipe.below");
        let active_lbl = self.t("recipe.active");
        let close_lbl = self.t("recipe.close");
        let source_head = self.t("recipe.source");
        let condition_head = self.t("recipe.condition");
        let sink_head = self.t("recipe.sink");
        let action = self.parsed.as_ref().and_then(|p| p.intent.action);
        let Some(ask) = self.recipe_ask.as_mut() else { return };
        let mut picked = false;
        let mut close = false;
        egui::Window::new(title.clone())
            .id(egui::Id::unique("recipe-window"))
            .anchor(Align2::LEFT_TOP, [360.0, 120.0])
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .frame(
                Frame::NONE
                    .fill(panel)
                    .stroke(Stroke::new(1.0, accent.gamma_multiply(0.6)))
                    .corner_radius(radius)
                    .inner_margin(Margin::same(12)),
            )
            .show(ctx, |ui| {
                ui.set_width(300.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&title).size(13.0).strong().color(accent));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(&close_lbl).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(6.0);
                match chip {
                    Chip::Source => {
                        ui.label(RichText::new(&source_head).size(11.0).color(muted));
                        ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for s in SOURCES {
                                let on = ask.recipe.source == s.id;
                                let text = format!(
                                    "{} — {}",
                                    s.short.get(self.lang),
                                    s.title.get(self.lang)
                                );
                                if ui.selectable_label(on, RichText::new(text).size(12.0)).clicked() {
                                    ask.recipe.source = s.id;
                                    picked = true;
                                }
                            }
                        });
                    }
                    Chip::Condition => {
                        ui.label(RichText::new(&condition_head).size(11.0).color(muted));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&threshold_lbl).size(11.5).color(muted));
                            ui.add(egui::DragValue::new(&mut ask.number).speed(1.0));
                        });
                        ui.add_space(6.0);
                        let n = ask.number;
                        for (text, condition) in [
                            (format!("{above_lbl} {n}"), Condition::Above(n)),
                            (format!("{below_lbl} {n}"), Condition::Below(n)),
                            (active_lbl.clone(), Condition::Active),
                        ] {
                            if ui
                                .add(
                                    Button::new(RichText::new(text).size(12.0))
                                        .min_size(vec2(ui.available_width(), 26.0))
                                        .corner_radius(crate::style::RADIUS),
                                )
                                .clicked()
                            {
                                ask.recipe.condition = Some(condition);
                                picked = true;
                            }
                        }
                    }
                    Chip::Sink => {
                        ui.label(RichText::new(&sink_head).size(11.0).color(muted));
                        ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for s in SINKS {
                                let on = ask.recipe.sink == s.id;
                                let text = s.title.get(self.lang);
                                let verb = crate::compose::behavior_for(s.id, action);
                                if ui
                                    .selectable_label(
                                        on,
                                        RichText::new(format!("{text} · {}", verb.label().get(self.lang)))
                                            .size(12.0),
                                    )
                                    .clicked()
                                {
                                    ask.recipe.sink = s.id;
                                    ask.recipe.behavior = verb;
                                    picked = true;
                                }
                            }
                        });
                    }
                }
                let _ = fg;
            });
        if picked {
            let recipe = ask.recipe;
            self.recipe_ask = None;
            self.apply_recipe(recipe);
        } else if close {
            self.recipe_ask = None;
        }
    }

    /// A row of jumper colours. `None` is the automatic net colour.
    fn colour_swatches(&self, ui: &mut egui::Ui, current: Option<u8>) -> Option<Option<u8>> {
        let mut picked = None;
        ui.horizontal(|ui| {
            if ui
                .selectable_label(current.is_none(), "A")
                .on_hover_text(self.t("wire.auto"))
                .clicked()
            {
                picked = Some(None);
            }
            for (i, colour) in crate::scene::WIRE_PALETTE.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::click());
                ui.painter().rect_filled(rect, CornerRadius::same(3), self.c32(*colour));
                if current == Some(i as u8) {
                    ui.painter().rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.5, Color32::WHITE), StrokeKind::Outside);
                }
                if resp.clicked() {
                    picked = Some(Some(i as u8));
                }
            }
        });
        picked
    }

    /// The property inspector: reference, value and rotation of a selected
    /// part, or the net class and hand bends of the active cable.
    fn value_window(
        &mut self,
        ctx: &egui::Context,
        _accent: Color32,
        fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
    ) {
        if self.mode != Mode::Manual || self.help {
            return;
        }
        let part = self.draft.selected_part().filter(|&i| {
            self.draft.circuit.parts.get(i).is_some_and(|p| p.active)
        });
        let wire = self.active_wire.filter(|&i| {
            self.draft.circuit.wires.get(i).is_some_and(|w| w.active)
        });
        if part.is_none() && wire.is_none() {
            return;
        }
        let key = format!("{part:?}/{wire:?}");
        if self.inspect_for != key {
            self.inspect_for = key.clone();
            self.inspect_label = part
                .map(|i| self.draft.circuit.parts[i].label.clone())
                .unwrap_or_default();
            self.inspect_value = part
                .map(|i| self.draft.circuit.parts[i].value.clone())
                .unwrap_or_default();
        }
        if let Some(w) = wire {
            let nets = self.draft.circuit.nets();
            let pin = [self.draft.circuit.wires[w].a, self.draft.circuit.wires[w].b]
                .into_iter()
                .find_map(|e| match e {
                    crate::model::End::Pin(p) => Some(p),
                    _ => None,
                });
            self.inspect_net = pin
                .map(|p| self.draft.circuit.net_name(&nets, nets.root(p)))
                .unwrap_or_default();
        }
        let current = part
            .map(|i| self.draft.circuit.parts[i].value.clone())
            .unwrap_or_default();
        let choices: &[manual::Variant] = part
            .map(|i| manual::variants_for(self.draft.circuit.parts[i].kind))
            .unwrap_or(&[]);
        let via_count = wire
            .map(|i| self.draft.circuit.wires[i].via.len())
            .unwrap_or(0);
        let wire_class = wire.map(|i| self.draft.circuit.wires[i].class);
        let title = if part.is_some() {
            self.t("inspect.title")
        } else {
            self.t("inspect.wire")
        };
        let class_key = |c: NetClass| match c {
            NetClass::Vcc => "class.vcc",
            NetClass::Gnd => "class.gnd",
            NetClass::Signal => "class.signal",
            NetClass::Analog => "class.analog",
        };
        let mut action: Option<InspectAction> = None;
        egui::Window::new(title)
            .id(egui::Id::unique("value-window"))
            .anchor(Align2::LEFT_BOTTOM, [520.0, -64.0])
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .frame(
                Frame::NONE
                    .fill(panel)
                    .stroke(Stroke::new(1.0, muted.gamma_multiply(0.4)))
                    .corner_radius(radius)
                    .inner_margin(Margin::same(10)),
            )
            .show(ctx, |ui| {
                ui.set_width(228.0);
                if let Some(i) = part {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{i}")).size(10.5).color(muted));
                        ui.label(RichText::new(self.t("variant.hint")).size(10.5).color(muted));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.t("inspect.label")).size(11.0).color(muted));
                        let r = ui.add(
                            TextEdit::singleline(&mut self.inspect_label).desired_width(140.0),
                        );
                        if r.lost_focus() {
                            action = Some(InspectAction::Label(self.inspect_label.clone()));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.t("inspect.value")).size(11.0).color(muted));
                        let r = ui.add(
                            TextEdit::singleline(&mut self.inspect_value).desired_width(140.0),
                        );
                        if r.lost_focus() {
                            action = Some(InspectAction::Value(self.inspect_value.clone()));
                        }
                    });
                    ui.add_space(3.0);
                    if ui.button(RichText::new(self.t("inspect.rotate")).size(11.0)).clicked() {
                        action = Some(InspectAction::Rotate);
                    }
                    if !choices.is_empty() {
                        egui::ComboBox::from_id_salt("variant-pick")
                            .selected_text(RichText::new(current.clone()).size(12.0).color(fg))
                            .width(208.0)
                            .show_ui(ui, |ui| {
                                for v in choices {
                                    let on = v.value == current;
                                    if ui.selectable_label(on, v.value).clicked() && !on {
                                        action = Some(InspectAction::Variant(
                                            v.value.to_string(),
                                            v.kind,
                                        ));
                                    }
                                }
                            });
                    }
                } else if let Some(w) = wire {
                    ui.label(RichText::new(format!("{w}")).size(10.5).color(muted));
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.t("inspect.class")).size(11.0).color(muted));
                        egui::ComboBox::from_id_salt("wire-class-pick")
                            .selected_text(RichText::new(self.t(class_key(wire_class.unwrap()))).size(12.0).color(fg))
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                for class in [
                                    NetClass::Gnd,
                                    NetClass::Vcc,
                                    NetClass::Signal,
                                    NetClass::Analog,
                                ] {
                                    let label = self.t(class_key(class));
                                    if ui.selectable_label(wire_class == Some(class), label).clicked()
                                        && wire_class != Some(class)
                                    {
                                        action = Some(InspectAction::WireClass(class));
                                    }
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.t("inspect.net")).size(11.0).color(muted));
                        let r = ui.add(
                            TextEdit::singleline(&mut self.inspect_net).desired_width(140.0),
                        );
                        if r.lost_focus() {
                            action = Some(InspectAction::NetName(self.inspect_net.clone()));
                        }
                    });
                    ui.label(RichText::new(self.t("wire.colour")).size(11.0).color(muted));
                    if let Some(colour) = self.colour_swatches(
                        ui,
                        wire.and_then(|i| self.draft.circuit.wires[i].colour),
                    ) {
                        action = Some(InspectAction::WireColour(colour));
                    }
                    ui.label(
                        RichText::new(format!("{}: {}", self.t("inspect.vias"), via_count))
                            .size(11.0)
                            .color(muted),
                    );
                    if via_count > 0
                        && ui.button(RichText::new(self.t("inspect.clear_vias")).size(11.0)).clicked()
                    {
                        action = Some(InspectAction::ClearVias);
                    }
                }
            });
        let done = match action {
            Some(InspectAction::Label(text)) => part.is_some_and(|i| self.draft.set_label(i, &text)),
            Some(InspectAction::Value(text)) => part.is_some_and(|i| self.draft.set_value(i, &text)),
            Some(InspectAction::Variant(value, kind)) => {
                part.is_some_and(|i| self.draft.set_variant(i, &value, kind))
            }
            Some(InspectAction::Rotate) => self.draft.rotate_selected(1),
            Some(InspectAction::WireClass(class)) => {
                wire.is_some_and(|i| self.draft.set_wire_class(i, class))
            }
            Some(InspectAction::NetName(name)) => {
                wire.is_some_and(|i| self.draft.set_net_name(i, &name))
            }
            Some(InspectAction::WireColour(colour)) => {
                wire.is_some_and(|i| self.draft.set_wire_colour(i, colour))
            }
            Some(InspectAction::ClearVias) => wire.is_some_and(|i| self.draft.clear_vias(i)),
            None => false,
        };
        if done {
            self.inspect_for.clear();
            self.refresh_manual();
        }
    }

    /// The easter egg: a small flat button next to "?" opens the photo with
    /// the author's nick. The JPEG is compiled in and stripped of every tag,
    /// so it carries no address, date or camera model.
    fn author_window(
        &mut self,
        ctx: &egui::Context,
        accent: Color32,
        fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
    ) {
        if !self.about {
            return;
        }
        if self.about_tex.is_none() {
            let bytes = include_bytes!("../assets/author.jpg");
            if let Ok(img) = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                self.about_tex = Some(ctx.load_texture("author", color, egui::TextureOptions::LINEAR));
            }
        }
        let title = self.t("about.title");
        let line = self.t("about.line");
        let close = self.t("about.close");
        let tex = self.about_tex.clone();
        let mut open = true;
        let mut close_clicked = false;
        egui::Window::new(title)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .frame(
                Frame::NONE
                    .fill(panel)
                    .stroke(Stroke::new(1.0, accent.gamma_multiply(0.8)))
                    .corner_radius(radius)
                    .inner_margin(Margin::same(crate::style::PAD as i8)),
            )
            .show(ctx, |ui| {
                ui.set_max_width(260.0);
                if let Some(tex) = &tex {
                    ui.add(
                        egui::Image::from_texture(tex)
                            .max_size(vec2(240.0, 240.0))
                            .corner_radius(radius),
                    );
                }
                ui.add_space(10.0);
                ui.label(RichText::new("ElMak").size(20.0).strong().color(fg));
                ui.label(RichText::new(line).size(12.0).color(muted));
                ui.add_space(12.0);
                if ui
                    .add(
                        Button::new(RichText::new(close).size(12.0))
                            .min_size(vec2(ui.available_width(), 28.0))
                            .corner_radius(radius),
                    )
                    .clicked()
                {
                    close_clicked = true;
                }
            });
        if close_clicked || !open {
            self.about = false;
        }
    }

    fn overlays(
        &mut self,
        ctx: &egui::Context,
        accent: Color32,
        fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
    ) {
        // A task from the last session: continue it or start clean. The
        // draft is written on every plan, so closing the window is not work
        // lost any more.
        if let Some((task, _manual)) = self.resume.clone() {
            let screen = ctx.content_rect();
            let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("resume-dim"));
            ctx.layer_painter(layer)
                .rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(DIM_ALPHA));
            let title = self.t("resume.title");
            let body = self.t("resume.body");
            let yes = self.t("resume.yes");
            let no = self.t("resume.no");
            let mut resume = false;
            let mut fresh = false;
            egui::Window::new(title.clone())
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .frame(
                    Frame::NONE
                        .fill(panel)
                        .stroke(Stroke::new(1.0, accent.gamma_multiply(0.6)))
                        .corner_radius(radius)
                        .inner_margin(Margin::same(18)),
                )
                .show(ctx, |ui| {
                    ui.set_width(400.0);
                    ui.label(RichText::new(&title).size(16.0).strong().color(accent));
                    ui.add_space(6.0);
                    ui.label(RichText::new(&body).size(12.0).color(muted));
                    ui.add_space(8.0);
                    Frame::NONE
                        .fill(ui.visuals().extreme_bg_color)
                        .corner_radius(crate::style::RADIUS)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            let short = if task.chars().count() > 180 {
                                format!("{}…", task.chars().take(180).collect::<String>())
                            } else {
                                task.clone()
                            };
                            ui.label(RichText::new(short).size(12.5).color(fg));
                        });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                Button::new(RichText::new(&yes).strong().size(12.5))
                                    .fill(accent)
                                    .corner_radius(crate::style::RADIUS)
                                    .min_size(vec2(150.0, 30.0)),
                            )
                            .clicked()
                        {
                            resume = true;
                        }
                        if ui
                            .add(
                                Button::new(RichText::new(&no).size(12.5))
                                    .corner_radius(crate::style::RADIUS)
                                    .min_size(vec2(150.0, 30.0)),
                            )
                            .clicked()
                        {
                            fresh = true;
                        }
                    });
                });
            if resume {
                self.resume_task();
            } else if fresh {
                self.start_fresh();
            }
        }

        if self.help {
            let screen = ctx.content_rect();
            let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("help-dim"));
            ctx.layer_painter(layer)
                .rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(DIM_ALPHA));
            let keys: [(&str, &str); 20] = [
                ("Ctrl+Enter", "help.plan"),
                ("Ctrl+S", "export.title"),
                ("Ctrl+E", "help.export"),
                ("Ctrl+Z", "manual.undo"),
                ("Ctrl+Shift+Z", "manual.redo"),
                ("Ctrl+A", "help.select_all"),
                ("Ctrl+C/V/X", "help.clipboard"),
                ("Ctrl+D", "help.duplicate"),
                ("Shift+click", "help.multi"),
                ("← ↑ → ↓", "help.nudge"),
                ("1 / 2 / 3", "help.tools"),
                ("G", "help.grid"),
                ("R", "manual.rotate_part"),
                ("Shift+R", "manual.rotate_board"),
                ("Delete", "manual.delete_hint"),
                ("F", "help.fit"),
                ("Ctrl+B", "view.narrow"),
                ("?", "help.title"),
                ("Q", "help.quit"),
                ("Esc", "help.close"),
            ];
            egui::Window::new(self.t("help.title"))
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .frame(Frame::NONE.fill(panel).corner_radius(radius).inner_margin(Margin::same(18)))
                .show(ctx, |ui| {
                    ui.set_width(440.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.t("help.title")).size(16.0).strong().color(accent));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(self.t("help.close")).clicked() {
                                self.help = false;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    for (k, key) in keys {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(k).monospace().size(12.0).color(accent));
                            ui.label(RichText::new(self.t(key)).size(12.0).color(muted));
                        });
                    }
                });
        }

        if self.quit_ask {
            egui::Window::new(self.t("quit.title"))
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .frame(Frame::NONE.fill(panel).corner_radius(radius).inner_margin(Margin::same(18)))
                .show(ctx, |ui| {
                    ui.set_width(340.0);
                    ui.label(RichText::new(self.t("quit.body")).size(12.5).color(muted));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(self.t("quit.no")).clicked() {
                            self.quit_ask = false;
                        }
                        if ui
                            .add(Button::new(RichText::new(self.t("quit.yes"))))
                            .clicked()
                        {
                            self.quit_ask = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
        }
    }
}

/// Paint the scene in window coordinates: `origin` is the top-left of the
/// drawing, `scale` is pixels per millimetre.
/// Window pixels to bench millimetres. `origin` is where the scene's top-left
/// corner lands on screen; `scene_origin` is the bench point that corner
/// stands for, because the drawing is shifted to its own frame.
fn screen_to_bench(p: Pos2, origin: Pos2, scale: f32, scene_origin: Pt) -> Pt {
    Pt::new(
        (p.x - origin.x) / scale + scene_origin.x,
        (p.y - origin.y) / scale + scene_origin.y,
    )
}

/// The inverse of [`screen_to_bench`].
fn bench_to_screen(p: Pt, origin: Pos2, scale: f32, scene_origin: Pt) -> Pos2 {
    origin + vec2((p.x - scene_origin.x) * scale, (p.y - scene_origin.y) * scale)
}

/// Ring the parts a failed check is talking about and dot the pins, without
/// redrawing the bench. The marks sit on top of the scene and are clipped to
/// the canvas, so a zoomed-in view does not paint over the side panels.
fn paint_focus(
    painter: &egui::Painter,
    c: &Circuit,
    f: &CheckFocus,
    origin: Pos2,
    scale: f32,
    scene_origin: Pt,
    clip: egui::Rect,
) {
    let col = match f.sev {
        Sev::Pass => Color32::from_rgb(0x3E, 0x9E, 0x5A),
        Sev::Warn => Color32::from_rgb(0xD8, 0xA3, 0x2E),
        Sev::Fail => Color32::from_rgb(0xD2, 0x4B, 0x45),
    };
    let p = painter.with_clip_rect(clip);
    for &i in &f.focus.parts {
        let Some(part) = c.parts.get(i) else { continue };
        let r = c.part_rect(part);
        let a = bench_to_screen(Pt::new(r.x, r.y), origin, scale, scene_origin);
        let b = bench_to_screen(Pt::new(r.right(), r.bottom()), origin, scale, scene_origin);
        let rect = egui::Rect::from_min_max(a.min(b), a.max(b)).expand(4.0);
        p.rect_filled(rect, 6.0, col.gamma_multiply(0.12));
        p.rect_stroke(rect, 6.0, Stroke::new(2.0, col), StrokeKind::Outside);
    }
    for &i in &f.focus.pins {
        let Some(pin) = c.pins.get(i) else { continue };
        let at = bench_to_screen(pin.pos, origin, scale, scene_origin);
        p.circle_filled(at, 4.5, col.gamma_multiply(0.35));
        p.circle_stroke(at, 7.0, Stroke::new(1.8, col));
    }
}

/// egui's built-in fonts have no ✓ or ↶, which the checks and the undo
/// buttons need. DejaVu covers those, plus Cyrillic.
/// A tab is a word with an accent rule under it: no box, no fill, no
/// rounding. It is the quietest way to switch a pane and it keeps the window
/// looking like one slab rather than a pile of buttons.
/// Sub-folders of `dir`, hidden ones skipped, sorted the way a file manager
/// lists them.
fn folders_in(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.starts_with('.'))
        .collect();
    out.sort_by_key(|n| n.to_lowercase());
    out
}

/// Add an extension unless the name already ends with it, so typing
/// `circuit.png` does not produce `circuit.png.png`.
fn with_ext(path: &Path, ext: &str) -> PathBuf {
    let has = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(ext))
        .unwrap_or(false);
    if has {
        path.to_path_buf()
    } else {
        path.with_extension(ext)
    }
}

fn tab_button(
    ui: &mut egui::Ui,
    label: &str,
    on: bool,
    size: f32,
    accent: Color32,
    fg: Color32,
    muted: Color32,
) -> egui::Response {
    let font = FontId::proportional(size);
    let probe = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), fg);
    let (rect, resp) = ui.allocate_exact_size(probe.size() + vec2(22.0, 14.0), Sense::click());
    let colour = if on {
        accent
    } else if resp.hovered() {
        fg
    } else {
        muted
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, colour);
    let painter = ui.painter();
    if resp.hovered() && !on {
        painter.rect_filled(rect, crate::style::RADIUS, Color32::from_white_alpha(6));
    }
    painter.galley(
        (rect.center() - galley.size() / 2.0).round(),
        galley,
        colour,
    );
    if on {
        painter.line_segment(
            [
                pos2(rect.left() + 5.0, rect.bottom() - 1.0),
                pos2(rect.right() - 5.0, rect.bottom() - 1.0),
            ],
            Stroke::new(2.0, accent),
        );
    }
    resp
}

/// One square palette cell: an icon in its own slot and a one-line name.
/// Painted by hand so the active state is a flat accent border, not a bevel.
fn palette_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    on: bool,
    width: f32,
    accent: Color32,
    fg: Color32,
    muted: Color32,
) -> egui::Response {
    let height = crate::style::CONTROL_H;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let panel = ui.visuals().panel_fill;
    let fill = if on {
        panel.lerp_to_gamma(accent, 0.22)
    } else if resp.hovered() {
        panel.lerp_to_gamma(Color32::WHITE, 0.05)
    } else {
        Color32::TRANSPARENT
    };
    let painter = ui.painter();
    painter.rect_filled(rect, crate::style::RADIUS, fill);
    let border = if on {
        accent
    } else if resp.hovered() {
        muted
    } else {
        muted.gamma_multiply(0.45)
    };
    painter.rect_stroke(
        rect,
        crate::style::RADIUS,
        Stroke::new(if on { 2.0 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    let icon_rect = egui::Rect::from_min_size(
        rect.min + vec2(4.0, (height - crate::style::ICON) / 2.0),
        vec2(crate::style::ICON, crate::style::ICON),
    );
    icons::paint(painter, icon_rect, icon, accent);
    let text_x = icon_rect.right() + 5.0;
    painter.with_clip_rect(rect.shrink(2.0)).text(
        pos2(text_x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(crate::style::TEXT_SMALL),
        fg,
    );
    resp
}

/// A card of the start gallery: pictogram, phrase, one click.
fn sample_card(
    ui: &mut egui::Ui,
    sample: &Sample,
    width: f32,
    lang: Lang,
    accent: Color32,
    fg: Color32,
    muted: Color32,
) -> egui::Response {
    let text = sample.task.get(lang);
    let galley = ui
        .painter()
        .layout(text.to_owned(), FontId::proportional(11.0), fg, width - 16.0);
    let height = (galley.size().y + 36.0).max(52.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let fill = if resp.hovered() {
        ui.visuals().panel_fill.lerp_to_gamma(Color32::WHITE, 0.05)
    } else {
        Color32::TRANSPARENT
    };
    let border = if resp.hovered() {
        accent
    } else {
        muted.gamma_multiply(0.45)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, crate::style::RADIUS, fill);
    painter.rect_stroke(
        rect,
        crate::style::RADIUS,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    let icon_rect =
        egui::Rect::from_min_size(rect.min + vec2(8.0, 8.0), vec2(18.0, 18.0));
    icons::paint(painter, icon_rect, sample.icon, accent);
    painter.galley(pos2(rect.left() + 8.0, rect.top() + 30.0), galley, fg);
    resp
}

/// The three chips of a composition: sensor, condition, output.
fn recipe_chips(c: Composition, lang: Lang) -> Vec<(Chip, String, String)> {
    let source = spec::source(c.source);
    let sink = spec::sink(c.sink);
    vec![
        (
            Chip::Source,
            source.short.get(lang).to_owned(),
            L::new("Источник: изменить", "Source: change")
                .get(lang)
                .to_owned(),
        ),
        (
            Chip::Condition,
            condition_text(c, lang),
            L::new("Условие: изменить", "Condition: change")
                .get(lang)
                .to_owned(),
        ),
        (
            Chip::Sink,
            sink.title.get(lang).to_owned(),
            format!(
                "{}: {}",
                L::new("Приёмник", "Output").get(lang),
                c.behavior.label().get(lang)
            ),
        ),
    ]
}

/// The threshold the source falls back to when the sentence named none.
fn default_condition(c: Composition) -> Condition {
    spec::source(c.source)
        .variants
        .first()
        .map(|v| v.condition)
        .unwrap_or(Condition::Active)
}

/// The number the condition picker starts with.
fn recipe_number(c: Composition) -> f32 {
    match c.condition.unwrap_or_else(|| default_condition(c)) {
        Condition::Above(n)
        | Condition::Below(n)
        | Condition::AboveBand { on: n, .. }
        | Condition::BelowBand { on: n, .. }
        | Condition::Between { low: n, .. } => n,
        Condition::Active => 20.0,
    }
}

/// A threshold as a chip would say it: "выше 30", "есть сигнал", "20…30".
fn condition_text(c: Composition, lang: Lang) -> String {
    if c.behavior == Behavior::Show {
        return L::new("показание", "the reading").get(lang).to_owned();
    }
    let above = L::new("выше", "above").get(lang);
    let below = L::new("ниже", "below").get(lang);
    let back = L::new("возврат", "return").get(lang);
    match c.condition.unwrap_or_else(|| default_condition(c)) {
        Condition::Above(n) => format!("{above} {n}"),
        Condition::Below(n) => format!("{below} {n}"),
        Condition::Active => L::new("есть сигнал", "signal present").get(lang).to_owned(),
        Condition::AboveBand { on, off } => format!("{above} {on} · {back} {off}"),
        Condition::BelowBand { on, off } => format!("{below} {on} · {back} {off}"),
        Condition::Between { low, high } => format!("{low}…{high}"),
    }
}

/// Why a part is on the bench, in a few words: the tooltip that answers "what
/// is this doing here" without another panel.
fn part_why(kind: crate::model::Kind, lang: Lang) -> &'static str {
    use crate::model::Kind;
    let (ru, en) = match kind {
        Kind::Arduino => ("мозг схемы", "the brain of the circuit"),
        Kind::Resistor => ("ограничивает ток", "limits the current"),
        Kind::Led => ("показывает результат", "shows the result"),
        Kind::Diode => ("не пускает ток назад", "blocks the reverse current"),
        Kind::Nmos | Kind::Npn => ("ключает нагрузку", "switches the load"),
        Kind::Button | Kind::Reed => ("ввод от человека", "a human input"),
        Kind::Motor | Kind::Servo | Kind::Buzzer | Kind::Speaker | Kind::Relay | Kind::Peltier => {
            ("исполнитель: делает то, что просили", "the output: it does the work")
        }
        Kind::Pot => ("регулятор", "a knob"),
        Kind::Ldr => ("измеряет свет", "reads the light"),
        Kind::Tmp36 => ("измеряет температуру", "reads the temperature"),
        Kind::Capacitor | Kind::Electrolytic => ("сглаживает питание", "smooths the supply"),
        Kind::Module => ("готовая плата: датчик или экран", "a ready board: a sensor or a screen"),
        _ => ("деталь схемы", "a part of the circuit"),
    };
    if lang == Lang::Ru {
        ru
    } else {
        en
    }
}

/// The smallest part whose footprint is under the pointer.
fn part_under(c: &Circuit, at: Pt) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, p) in c.parts_id() {
        let r = c.part_rect(p);
        if r.inflate(0.8).contains(at) {
            let area = r.w * r.h;
            if best.map(|(_, a)| area < a).unwrap_or(true) {
                best = Some((i, area));
            }
        }
    }
    best.map(|(i, _)| i)
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "dejavu".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/DejaVuSans.ttf"
        ))),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("dejavu".to_owned());
    }
    ctx.set_fonts(fonts);
}

/// Text goes through the font atlas, which keeps a raster of every distinct
/// size it is asked for. A smoothly zoomed canvas would ask for a new size on
/// every wheel notch and grow the atlas without bound (hundreds of megabytes
/// after a minute of zooming), so sizes snap to a short geometric ladder.
fn text_size(raw: f32) -> f32 {
    const LADDER: [f32; 30] = [
        3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 12.0, 14.0, 16.0, 18.0, 21.0, 24.0, 28.0,
        32.0, 37.0, 43.0, 50.0, 58.0, 67.0, 78.0, 90.0, 105.0, 122.0, 142.0, 165.0, 192.0,
        224.0, 260.0,
    ];
    let raw = raw.clamp(3.0, 260.0);
    LADDER
        .iter()
        .copied()
        .find(|&size| size >= raw)
        .unwrap_or(260.0)
}

fn paint_scene(painter: &egui::Painter, s: &Scene, origin: Pos2, scale: f32, clip: egui::Rect) {
    let at = |p: &Pt| origin + vec2(p.x * scale, p.y * scale);
    let c = |c: Rgba| Color32::from_rgba_unmultiplied(c.0, c.1, c.2, c.3);
    let clip_painter = painter.with_clip_rect(clip);
    for prim in &s.prims {
        match prim {
            Prim::Rect { r, radius, fill, stroke } => {
                let rect = egui::Rect::from_min_size(at(&Pt::new(r.x, r.y)), vec2(r.w * scale, r.h * scale));
                let cr = CornerRadius::same((*radius * scale).clamp(0.0, 255.0) as u8);
                clip_painter.rect_filled(rect, cr, c(*fill));
                if let Some((sc, w)) = stroke {
                    clip_painter.rect_stroke(rect, cr, Stroke::new(w * scale, c(*sc)), egui::StrokeKind::Inside);
                }
            }
            Prim::Circle { c: p, r, fill, stroke } => {
                let center = at(p);
                clip_painter.circle_filled(center, r * scale, c(*fill));
                if let Some((sc, w)) = stroke {
                    clip_painter.circle_stroke(center, r * scale, Stroke::new(w * scale, c(*sc)));
                }
            }
            Prim::Poly { pts, w, color, closed } => {
                let points: Vec<Pos2> = pts.iter().map(at).collect();
                if points.len() < 2 {
                    continue;
                }
                if *closed {
                    clip_painter.add(egui::Shape::convex_polygon(points, c(*color), Stroke::NONE));
                } else {
                    clip_painter.add(egui::Shape::line(points, Stroke::new((w * scale).max(0.5), c(*color))));
                }
            }
            Prim::Text { p, size, text, color, anchor, bold, angle } => {
                let font = FontId::proportional(text_size(size * scale));
                let galley = clip_painter.layout_no_wrap(text.clone(), font, c(*color));
                let s_sz = galley.size();
                let off = match anchor {
                    Anchor::Start => vec2(0.0, -s_sz.y / 2.0),
                    Anchor::Center => vec2(-s_sz.x / 2.0, -s_sz.y / 2.0),
                    Anchor::End => vec2(-s_sz.x, -s_sz.y / 2.0),
                };
                let base = at(p);
                let mut shape = egui::epaint::TextShape::new(base + off, galley, c(*color));
                if *angle != 0.0 {
                    let rad = angle.to_radians();
                    let (sin, cos) = (rad.sin(), rad.cos());
                    let rotated = vec2(off.x * cos - off.y * sin, off.x * sin + off.y * cos);
                    shape.pos = base + rotated;
                    shape.angle = rad;
                }
                if *bold {
                    clip_painter.add(shape.clone());
                    let mut second = shape;
                    second.pos += vec2(0.45, 0.0);
                    clip_painter.add(second);
                } else {
                    clip_painter.add(shape);
                }
            }
        }
    }
}

pub fn run() -> Result<(), String> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_app_id("volta")
        .with_title("volta")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1000.0, 660.0]);
    // The demo hooks are for screenshots and for driving the window from a
    // test; a normal start ignores both.
    if std::env::var("VOLTA_FULLSCREEN").is_ok() {
        viewport = viewport.with_fullscreen(true);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "volta",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc).demo()))),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::uno_spot;
    use crate::scene::draw;

    /// Clicking is only honest if a pin drawn at some pixel maps back to that
    /// same pin in the circuit's own coordinates.
    #[test]
    fn a_click_on_a_pin_lands_on_that_pin() {
        let mut circuit = Circuit::new();
        circuit.add_arduino(uno_spot());
        let scene = draw(&circuit, &Theme::default(), &scene::Opts::default());
        for (i, pin) in circuit.pins_id() {
            for (origin, scale) in [
                (Pos2::new(0.0, 0.0), 2.0),
                (Pos2::new(120.0, 300.0), 0.7),
                (Pos2::new(-40.0, 15.0), 5.5),
            ] {
                let px = bench_to_screen(pin.pos, origin, scale, scene.origin);
                let back = screen_to_bench(px, origin, scale, scene.origin);
                assert!(
                    back.dist(pin.pos) < 0.01,
                    "pin {i} drifted by {:.3} mm",
                    back.dist(pin.pos)
                );
            }
        }
    }

    /// The mapping has to respect the scene's shift; that is what put clicks a
    /// few millimetres away from the cursor before.
    /// The font atlas is only bounded if the canvas asks for a bounded set of
    /// text sizes, so zooming must not mint a new size on every notch.
    #[test]
    fn zoomed_text_sizes_come_from_a_short_ladder() {
        let mut seen = std::collections::BTreeSet::new();
        let mut zoom = 0.25f32;
        while zoom < 12.0 {
            seen.insert(text_size(1.9 * zoom).to_bits());
            zoom *= 1.001;
        }
        assert!(seen.len() < 40, "{} distinct sizes", seen.len());
        // Snapping stays close enough that labels keep their place.
        for raw in [3.0f32, 7.3, 19.0, 44.4, 133.0, 250.0] {
            let got = text_size(raw);
            assert!(got >= raw * 0.75 && got <= raw * 1.35, "{raw} -> {got}");
        }
        assert_eq!(text_size(0.1), 3.0);
        assert_eq!(text_size(1e6), 260.0);
    }

    #[test]
    fn the_scene_shift_is_part_of_the_mapping() {
        let mut circuit = Circuit::new();
        circuit.add_arduino(uno_spot());
        let scene = draw(&circuit, &Theme::default(), &scene::Opts::default());
        assert!(scene.origin.x != 0.0 || scene.origin.y != 0.0, "no shift to test");
        let origin = Pos2::ZERO;
        let scale = 3.0;
        let local = Pt::new(10.0, 20.0);
        let px = origin + vec2(local.x * scale, local.y * scale);
        let back = screen_to_bench(px, origin, scale, scene.origin);
        assert!(back.dist(scene.to_bench(local)) < 0.01);
        assert!(back.dist(local) > 0.5, "the shift was ignored");
    }

    #[test]
    fn extensions_are_added_once() {
        assert_eq!(
            with_ext(Path::new("/tmp/circuit"), "png"),
            PathBuf::from("/tmp/circuit.png")
        );
        assert_eq!(
            with_ext(Path::new("/tmp/circuit.png"), "png"),
            PathBuf::from("/tmp/circuit.png")
        );
        assert_eq!(
            with_ext(Path::new("/tmp/circuit.PNG"), "png"),
            PathBuf::from("/tmp/circuit.PNG")
        );
        assert_eq!(
            with_ext(Path::new("/tmp/my.circuit"), "svg"),
            PathBuf::from("/tmp/my.svg")
        );
    }

    #[test]
    fn the_folder_list_skips_hidden_entries() {
        let dir = std::env::temp_dir().join("volta-folders");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Desktop")).unwrap();
        std::fs::create_dir_all(dir.join("alpha")).unwrap();
        std::fs::create_dir_all(dir.join(".cache")).unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let names = folders_in(&dir);
        assert_eq!(names, vec!["alpha", "Desktop"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
