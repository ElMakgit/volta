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
use crate::model::{Circuit, Pt, WireStyle};
use crate::parse::{self, Parse};
use crate::scene::{self, Anchor, Prim, Rgba, Scene};
use crate::svg;
use crate::templates::{self, Built, Plan};
use crate::text::{Lang, Tr};
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
            SaveKind::Bundle => "bundle",
        }
    }
    fn default_name(self) -> &'static str {
        match self {
            SaveKind::Png => "circuit.png",
            SaveKind::Svg => "circuit.svg",
            SaveKind::Sketch => "sketch.ino",
            SaveKind::Csv => "components.csv",
            SaveKind::Bundle => "circuit",
        }
    }
}

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
    wire_style: WireStyle,
    /// The cable whose grips are shown: the one under the pointer or being
    /// bent by hand.
    active_wire: Option<usize>,
    /// Where the pointer last hovered, in bench millimetres. A drag starts a
    /// frame or two after the press, and this is the finger on the cable.
    hover_world: Option<Pt>,
    /// The bench point under the last press, kept until a drag consumes it.
    press_world: Option<Pt>,
    tab: Tab,
    labels: bool,
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
            wire_style: crate::config::load_wire_style(),
            active_wire: None,
            hover_world: None,
            press_world: None,
            tab: Tab::Circuit,
            labels: false,
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
            self.plan_now();
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

    fn build_now(&mut self) {
        self.error = None;
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
                frame: self.drag.frame,
                active_wire: self.active_wire,
                handles: self
                    .active_wire
                    .and_then(|i| self.draft.circuit.wires.get(i))
                    .map(|w| w.via.clone())
                    .unwrap_or_default(),
            },
        ));
        self.manual_report = Some(checks::run_circuit(&self.draft.circuit, self.lang, None));
    }

    fn edit_by_hand(&mut self) {
        let Some(built) = &self.built else { return };
        self.draft.adopt(built.circuit.clone());
        // The generated picture is replaced by the editable one right away.
        self.scene = None;
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

    fn export_png(&mut self) {
        self.ask_save(SaveKind::Png);
    }

    fn export_files(&mut self) {
        self.ask_save(SaveKind::Bundle);
    }

    fn save_sketch(&mut self) {
        self.ask_save(SaveKind::Sketch);
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
        let mut dir = crate::config::load_export_dir()
            .filter(|p| p.is_dir())
            .unwrap_or_else(crate::png::default_dir);
        if !dir.is_dir() {
            let _ = std::fs::create_dir_all(&dir);
        }
        if !dir.is_dir() {
            dir = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()));
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
                let src = svg::render(&sc, 1.0);
                crate::png::png_from_svg(&src, &out, 2400).map(|()| out)
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
        let (ctrl, shift, enter, s, e, z, q, f, r, esc, del, quest) = ctx.input(|i| {
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
            self.export_png();
        }
        if ctrl && e {
            self.export_files();
        }
        if ctrl && z {
            let changed = if shift { self.draft.redo() } else { self.draft.undo() };
            if changed {
                self.mode = Mode::Manual;
                self.hover = None;
                self.refresh_manual();
            }
        }
        if del && !typing && self.mode == Mode::Manual {
            if let Some(part) = self.draft.selected_part() {
                if self.draft.circuit.arduino != Some(part) {
                    self.draft.snapshot();
                    self.draft.circuit.remove_part(part);
                    self.draft.selected = None;
                    self.hover = None;
                    self.refresh_manual();
                }
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
            self.draft.pending = None;
            self.tool = Tool::Select;
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
                        ui.label(RichText::new(self.lang.code().to_uppercase()).size(12.0).color(muted));
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

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(bg).inner_margin(Margin::symmetric(12, 10)))
            .show(root, |ui| {
                self.center_panel(ctx, ui, accent, fg, muted, panel, radius);
            });

        self.overlays(ctx, accent, fg, muted, panel, radius);
        self.author_window(ctx, accent, fg, muted, panel, radius);
        self.value_window(ctx, accent, fg, muted, panel, radius);
        self.save_window(ctx, accent, fg, muted, panel, radius);
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
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for key in ["task.example1", "task.example2", "task.example3"] {
                let text = self.t(key);
                let short = if text.chars().count() > 32 {
                    format!("{}…", text.chars().take(32).collect::<String>())
                } else {
                    text
                };
                if ui
                    .add(
                        Button::new(RichText::new(short).size(11.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(1.0, muted.gamma_multiply(0.6)))
                            .corner_radius(crate::style::RADIUS),
                    )
                    .on_hover_text(self.t(key))
                    .clicked()
                {
                    self.task = self.t(key);
                    self.plan_now();
                }
            }
        });
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

        // What the parser actually heard. Without it a wrong circuit looks
        // like magic; with it the user sees which word was taken how.
        if let Some(p) = &self.parsed {
            if !p.understood.is_empty() {
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

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.t("wire.style")).size(11.5).color(muted));
            for (style, key) in [
                (WireStyle::Arc, "wire.arc"),
                (WireStyle::Straight, "wire.straight"),
                (WireStyle::Ortho, "wire.ortho"),
            ] {
                let on = self.wire_style == style;
                if ui
                    .add(
                        Button::new(RichText::new(self.t(key)).size(11.5))
                            .fill(if on { accent.gamma_multiply(0.85) } else { Color32::TRANSPARENT })
                            .stroke(Stroke::new(1.0, muted.gamma_multiply(0.5)))
                            .corner_radius(crate::style::RADIUS),
                    )
                    .clicked()
                {
                    self.set_wire_style(style);
                    self.refresh_manual();
                }
            }
        });

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
        ui.label(RichText::new(self.t("bom.title")).size(13.0).strong().color(fg));
        ui.add_space(4.0);
        for (value, kind, count) in &bom {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{count}")).size(12.0).color(accent));
                ui.label(RichText::new(value).size(12.0).color(fg));
                ui.label(RichText::new(kind).size(11.0).color(muted));
            });
        }

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(8.0);
        let full = vec2(ui.available_width(), 30.0);
        if ui
            .add(
                Button::new(RichText::new(self.t("canvas.screenshot")).size(12.5))
                    .min_size(full)
                    .corner_radius(crate::style::RADIUS),
            )
            .clicked()
        {
            self.export_png();
        }
        ui.horizontal(|ui| {
            let half = (ui.available_width() - 6.0) / 2.0;
            if ui
                .add(Button::new(RichText::new(self.t("canvas.export")).size(12.0)).min_size(vec2(half, 28.0)))
                .clicked()
            {
                self.export_files();
            }
            if self.mode == Mode::Ai
                && ui
                    .add(
                        Button::new(RichText::new(self.t("code.save")).size(12.0))
                            .min_size(vec2(ui.available_width(), 28.0)),
                    )
                    .clicked()
            {
                self.save_sketch();
            }
        });

        if self.mode == Mode::Ai && self.built.is_some() {
            ui.add_space(8.0);
            // The way out of the generated circuit: keep it, fix it by hand.
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
                    let fit = Button::new(RichText::new("⤢").size(13.0)).min_size(vec2(26.0, 22.0));
                    if ui.add(fit).on_hover_text(self.t("canvas.fit")).clicked() {
                        self.view = View::default();
                    }
                    let (icon, next, mode_key) = match self.wire_style {
                        WireStyle::Arc => ("∿", WireStyle::Straight, "wire.arc"),
                        WireStyle::Straight => ("╱", WireStyle::Ortho, "wire.straight"),
                        WireStyle::Ortho => ("⌐", WireStyle::Arc, "wire.ortho"),
                    };
                    let wire = Button::new(RichText::new(icon).size(13.0))
                        .fill(if self.wire_style == WireStyle::Straight {
                            accent.gamma_multiply(0.8)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .min_size(vec2(26.0, 22.0));
                    let hint = format!("{}: {}", self.t("wire.style"), self.t(mode_key));
                    if ui.add(wire).on_hover_text(hint).clicked() {
                        self.set_wire_style(next);
                        match self.mode {
                            Mode::Ai => self.refresh_ai(),
                            Mode::Manual => self.refresh_manual(),
                        }
                    }
                    let mut labels = self.labels;
                    let tag = Button::new(RichText::new("Aa").size(12.0))
                        .fill(if self.labels {
                            accent.gamma_multiply(0.8)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .min_size(vec2(28.0, 22.0));
                    if ui
                        .add(tag)
                        .on_hover_text(self.t("canvas.labels"))
                        .clicked()
                    {
                        labels = !labels;
                    }
                    // One small button for the sheet colour: sun for the
                    // white Tinkercad canvas, moon for the black one.
                    let dark = self.bench_dark;
                    let bg = Button::new(RichText::new(if dark { "☾" } else { "☀" }).size(12.5))
                        .fill(if dark {
                            accent.gamma_multiply(0.8)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .min_size(vec2(26.0, 22.0));
                    if ui
                        .add(bg)
                        .on_hover_text(self.t(if dark { "bg.black" } else { "bg.white" }))
                        .clicked()
                    {
                        self.toggle_bench();
                    }
                    ui.label(
                        RichText::new(format!("{}%", (self.view.zoom * 100.0).round() as i32))
                            .size(11.0)
                            .color(muted),
                    );
                    if labels != self.labels {
                        self.labels = labels;
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
            if self.draft.circuit.reseat_part(part, step.0, step.1) {
                self.drag.applied = (dcol, drow);
                self.drag.moved = true;
                self.refresh_manual();
            }
        } else {
            let to = Pt::new(self.drag.at.x + dx, self.drag.at.y + dy);
            if self.draft.circuit.move_part(part, to) {
                self.drag.moved = true;
                self.refresh_manual();
            }
        }
    }

    /// Start a drag: a cable bend if the pointer is on one, otherwise a part.
    /// A cable is only a couple of millimetres wide, so a hit on it is meant
    /// for it even when it runs across a part's body.
    fn grab_at(&mut self, at: Pt, frame: crate::model::Rect) -> bool {
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
        self.draft.selected = Some(part);
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

        // The scene is drawn shifted to its own origin, so screen pixels have
        // to be mapped back through that shift before they mean anything to the
        // circuit.
        let to_world = |p: Pos2| screen_to_bench(p, origin, scale, scene.origin);
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
                    self.grab_at(at, frame);
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
        }
        if resp.secondary_clicked() {
            hit.right = resp.interact_pointer_pos().map(to_world);
        }
        if resp.hovered() {
            if let Some(p) = resp.hover_pos() {
                hit.hover = Some(to_world(p));
            }
        }
        if let Some(w) = hit.hover {
            let radius = 9.0 / scale;
            let mut best = (radius, None);
            for (i, pin) in circuit.pins.iter().enumerate() {
                let d = pin.pos.dist(Pt::new(w.x, w.y));
                if d < best.0 {
                    best = (d, Some(i));
                }
            }
            hit.pin = best.1;
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
                if end == crate::model::End::Hole(crate::model::Hole::OffBoard) && self.draft.pending.is_none() {
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
                let picked = self.draft.part_at(at);
                if self.draft.selected != picked {
                    self.draft.selected = picked;
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
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} {}", r.passed(), self.t("checks.passed")))
                    .size(12.0)
                    .color(accent),
            );
            if r.failed() > 0 {
                ui.label(
                    RichText::new(format!("{} {}", r.failed(), self.t("checks.failed")))
                        .size(12.0)
                        .color(Color32::from_rgb(0xD2, 0x4B, 0x45)),
                );
            }
        });
        ui.add_space(6.0);
        Frame::NONE
            .fill(panel)
            .corner_radius(radius)
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for c in &r.checks {
                        let (mark, col) = match c.sev {
                            Sev::Pass => ("✓", accent),
                            Sev::Warn => ("!", Color32::from_rgb(0xD8, 0xA3, 0x2E)),
                            Sev::Fail => ("×", Color32::from_rgb(0xD2, 0x4B, 0x45)),
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(mark).size(13.0).color(col));
                            ui.label(
                                RichText::new(format!("{:<22}", c.name.get(self.lang)))
                                    .size(12.0)
                                    .color(fg),
                            );
                            ui.label(RichText::new(&c.detail).size(11.5).color(muted));
                        });
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
            .rect_filled(ctx.content_rect(), CornerRadius::ZERO, Color32::from_black_alpha(150));

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

    fn value_window(
        &mut self,
        ctx: &egui::Context,
        accent: Color32,
        fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
    ) {
        if self.mode != Mode::Manual || self.help {
            return;
        }
        let Some(part) = self.draft.selected_part() else { return };
        let choices = manual::variants_for(self.draft.circuit.parts[part].kind);
        if choices.is_empty() {
            return;
        }
        let current = self.draft.circuit.parts[part].value.clone();
        let label = self.draft.circuit.parts[part].label.clone();
        let title = self.t("variant.title");
        let mut pick: Option<(usize, &'static str, crate::model::Kind)> = None;
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
                ui.set_width(196.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).size(12.0).strong().color(accent));
                    ui.label(RichText::new(self.t("variant.hint")).size(10.5).color(muted));
                });
                ui.add_space(4.0);
                egui::ComboBox::from_id_salt("variant-pick")
                    .selected_text(RichText::new(current.clone()).size(12.0).color(fg))
                    .width(176.0)
                    .show_ui(ui, |ui| {
                        for v in choices {
                            let on = v.value == current;
                            if ui.selectable_label(on, v.value).clicked() && !on {
                                pick = Some((part, v.value, v.kind));
                            }
                        }
                    });
            });
        if let Some((part, value, kind)) = pick {
            if self.draft.set_variant(part, value, kind) {
                self.refresh_manual();
            }
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
        _fg: Color32,
        muted: Color32,
        panel: Color32,
        radius: CornerRadius,
    ) {
        if self.help {
            let screen = ctx.content_rect();
            let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("help-dim"));
            ctx.layer_painter(layer)
                .rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(160));
            let keys: [(&str, &str); 13] = [
                ("Ctrl+Enter", "help.plan"),
                ("Ctrl+S", "help.screen"),
                ("Ctrl+E", "help.export"),
                ("Ctrl+Z", "manual.undo"),
                ("Ctrl+Shift+Z", "manual.redo"),
                ("R", "manual.rotate_part"),
                ("Shift+R", "manual.rotate_board"),
                ("Delete", "manual.delete_hint"),
                ("F", "help.fit"),
                ("?", "help.title"),
                ("Q", "help.quit"),
                ("Esc", "help.close"),
                ("—", "help.build"),
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
                        if key == "help.build" {
                            continue;
                        }
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
        for (i, pin) in circuit.pins.iter().enumerate() {
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
