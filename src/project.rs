//! Saved projects.
//!
//! A project is the whole bench in one readable JSON file: the sentence, the
//! chosen option, the hand-drawn circuit and the camera. It is deliberately
//! plain JSON with a version tag, so a file made today can still be read after
//! the planner changes, and a broken file is reported instead of crashing.
//!
//! The manual circuit is stored pin by pin. Rebuilding it from scratch would
//! lose the exact hole a part sits in, the bends the user pulled into a cable
//! and the order of the parts, which is precisely the work worth keeping.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use crate::model::*;
use crate::spec::{Behavior, Composition, Condition};

/// Bumped when the shape of the file changes incompatibly.
pub const VERSION: u64 = 1;

/// The file-name suffix that marks a project in the projects folder.
pub const SUFFIX: &str = ".volta.json";

/// A project as the UI sees it.
#[derive(Clone)]
pub struct Project {
    pub name: String,
    pub task: String,
    pub manual: bool,
    /// Which of the offered part sets was chosen in AI mode.
    pub chosen: usize,
    /// A recipe the user corrected by hand, if any.
    pub recipe: Option<Composition>,
    /// The hand-drawn circuit. AI projects rebuild theirs from the sentence.
    pub circuit: Option<Circuit>,
    pub view: ViewData,
    /// Unix seconds; zero means "stamp it on save".
    pub saved: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewData {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
}

impl Default for ViewData {
    fn default() -> Self {
        Self { zoom: 1.0, pan_x: 0.0, pan_y: 0.0 }
    }
}

/// One row of the projects window.
#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub manual: bool,
    pub parts: usize,
    pub saved: i64,
    pub broken: bool,
}

pub fn dir() -> PathBuf {
    crate::paths::data_dir().join("projects")
}

pub fn autosave_path() -> PathBuf {
    dir().join(format!("autosave{SUFFIX}"))
}

pub fn is_autosave(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == format!("autosave{SUFFIX}"))
        .unwrap_or(false)
}

/// A safe file name from a human name. Cyrillic is allowed; characters that
/// Windows forbids are replaced, and spaces become dashes.
pub fn file_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars().take(64) {
        if ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('-');
        }
    }
    let out = out.trim_matches('.').trim_matches('-').to_string();
    let mut stem = if out.is_empty() { "проект".to_string() } else { out };
    // `autosave.volta.json` belongs to the automatic copy; a user project
    // with that name gets a prefix instead of overwriting it.
    if stem.eq_ignore_ascii_case("autosave") {
        stem = format!("проект-{stem}");
    }
    format!("{stem}{SUFFIX}")
}

pub fn save(path: &Path, p: &Project) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let mut body = serde_json::to_string_pretty(&to_json(p))
        .map_err(|e| e.to_string())?;
    body.push('\n');
    std::fs::write(path, body).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn load(path: &Path) -> Result<Project, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    from_json(&text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn list() -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(dir()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in rd.flatten() {
        let path = item.path();
        let Some(file) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if !file.ends_with(SUFFIX) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        let stored = v.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let fallback = file.trim_end_matches(SUFFIX);
        let name = if stored.trim().is_empty() { fallback.to_string() } else { stored.to_string() };
        let manual = v.get("manual").and_then(|x| x.as_bool()).unwrap_or(false);
        let saved = v.get("saved").and_then(|x| x.as_i64()).unwrap_or(0);
        let circuit = v.get("circuit");
        let parts_all = circuit
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        // The Uno is a part too, but the list means the things on the bench.
        let has_uno = circuit
            .and_then(|c| c.get("arduino"))
            .map(|a| !a.is_null())
            .unwrap_or(false);
        let parts = parts_all.saturating_sub(usize::from(has_uno));
        let broken = from_json(&text).is_err();
        out.push(Entry { path, name, manual, parts, saved, broken });
    }
    // Newest first. A file with no timestamp goes to the bottom.
    out.sort_by(|a, b| b.saved.cmp(&a.saved));
    out
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn date_text(saved: i64) -> String {
    if saved <= 0 {
        return String::new();
    }
    chrono::DateTime::from_timestamp(saved, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%d.%m.%Y %H:%M").to_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------- JSON out

pub fn to_json(p: &Project) -> Value {
    let mut doc = json!({
        "kind": "volta-project",
        "version": VERSION,
        "name": p.name,
        "task": p.task,
        "manual": p.manual,
        "chosen": p.chosen,
        "saved": if p.saved <= 0 { now() } else { p.saved },
        "view": {
            "zoom": p.view.zoom,
            "pan_x": p.view.pan_x,
            "pan_y": p.view.pan_y,
        },
    });
    if let Some(recipe) = p.recipe {
        doc["recipe"] = recipe_json(recipe);
    }
    if let Some(circuit) = &p.circuit {
        doc["circuit"] = circuit_json(circuit);
    }
    doc
}

fn circuit_json(c: &Circuit) -> Value {
    let parts: Vec<Value> = c
        .parts
        .iter()
        .map(|p| {
            let internal: Vec<Value> = p
                .internal
                .iter()
                .map(|(a, b)| json!([a, b]))
                .collect();
            json!({
                "kind": kind_name(p.kind),
                "label": p.label,
                "value": p.value,
                "at": [p.at.x, p.at.y],
                "rot": p.rot,
                "pins": p.pins,
                "internal": internal,
                "active": p.active,
            })
        })
        .collect();
    let pins: Vec<Value> = c
        .pins
        .iter()
        .map(|p| {
            json!({
                "part": p.part,
                "name": p.name,
                "pos": [p.pos.x, p.pos.y],
                "kind": pin_kind_name(p.kind),
                "hole": hole_json(p.hole),
                "active": p.active,
            })
        })
        .collect();
    let wires: Vec<Value> = c
        .wires
        .iter()
        .map(|w| {
            let via: Vec<Value> = w.via.iter().map(|p| json!([p.x, p.y])).collect();
            json!({
                "a": end_json(w.a),
                "b": end_json(w.b),
                "class": net_class_name(w.class),
                "lift": w.lift,
                "via": via,
                "colour": w.colour,
                "active": w.active,
            })
        })
        .collect();
    json!({
        "board": {
            "at": [c.board.at.x, c.board.at.y],
            "cols": c.board.cols,
            "rot": c.board.rot,
        },
        "bare": c.bare,
        "arduino": c.arduino,
        "parts": parts,
        "pins": pins,
        "wires": wires,
        "net_names": c
            .net_names
            .iter()
            .map(|(k, v)| json!([k, v]))
            .collect::<Vec<_>>(),
    })
}

fn recipe_json(c: Composition) -> Value {
    json!({
        "source": c.source as usize,
        "sink": c.sink as usize,
        "behavior": behavior_name(c.behavior),
        "condition": c.condition.map(condition_json),
        "screens": c.screens,
    })
}

fn condition_json(c: Condition) -> Value {
    match c {
        Condition::Above(n) => json!({ "above": n }),
        Condition::Below(n) => json!({ "below": n }),
        Condition::Active => json!({ "active": true }),
        Condition::AboveBand { on, off } => json!({ "above_band": [on, off] }),
        Condition::BelowBand { on, off } => json!({ "below_band": [on, off] }),
        Condition::Between { low, high } => json!({ "between": [low, high] }),
    }
}

fn end_json(e: End) -> Value {
    match e {
        End::Pin(i) => json!({ "pin": i }),
        End::Hole(h) => json!({ "hole": hole_json(h) }),
    }
}

fn hole_json(h: Hole) -> Value {
    match h {
        Hole::Col(col, row) => json!({ "col": [col, row.idx()] }),
        Hole::Rail(rail, half, idx) => json!({
            "rail": [rail_name(rail), rail_half_name(half), idx]
        }),
        Hole::OffBoard => json!("off"),
    }
}

// ---------------------------------------------------------------- JSON in

pub fn from_json(text: &str) -> Result<Project, String> {
    let doc: Value = serde_json::from_str(text).map_err(|e| format!("JSON: {e}"))?;
    let obj = doc.as_object().ok_or("the file is not a JSON object")?;
    if let Some(version) = obj.get("version").and_then(|v| v.as_u64()) {
        if version > VERSION {
            return Err(format!("project version {version} is newer than {VERSION}"));
        }
    }
    let task = obj
        .get("task")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let manual = obj.get("manual").and_then(|v| v.as_bool()).unwrap_or(false);
    let chosen = obj
        .get("chosen")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .min(16) as usize;
    let saved = obj.get("saved").and_then(|v| v.as_i64()).unwrap_or(0);
    let view = obj
        .get("view")
        .and_then(|v| v.as_object())
        .map(|v| ViewData {
            zoom: number(v.get("zoom")).unwrap_or(1.0).clamp(0.35, 12.0),
            pan_x: number(v.get("pan_x")).unwrap_or(0.0),
            pan_y: number(v.get("pan_y")).unwrap_or(0.0),
        })
        .unwrap_or_default();
    let recipe = obj
        .get("recipe")
        .filter(|v| !v.is_null())
        .map(recipe_from)
        .transpose()?;
    let circuit = obj
        .get("circuit")
        .filter(|v| !v.is_null())
        .map(circuit_from)
        .transpose()?;
    if manual && circuit.is_none() {
        return Err("a manual project has no circuit".into());
    }
    Ok(Project { name, task, manual, chosen, recipe, circuit, view, saved })
}

fn recipe_from(v: &Value) -> Result<Composition, String> {
    let obj = v.as_object().ok_or("the recipe is not an object")?;
    let source_i = obj
        .get("source")
        .and_then(|x| x.as_u64())
        .ok_or("the recipe has no source")? as usize;
    let sink_i = obj
        .get("sink")
        .and_then(|x| x.as_u64())
        .ok_or("the recipe has no sink")? as usize;
    let source = crate::spec::SOURCES
        .get(source_i)
        .ok_or("unknown source")?
        .id;
    let sink = crate::spec::SINKS.get(sink_i).ok_or("unknown sink")?.id;
    let behavior = obj
        .get("behavior")
        .and_then(|x| x.as_str())
        .and_then(behavior_from)
        .ok_or("unknown behaviour")?;
    let condition = obj
        .get("condition")
        .filter(|x| !x.is_null())
        .map(condition_from)
        .transpose()?;
    let screens = obj
        .get("screens")
        .and_then(|x| x.as_u64())
        .unwrap_or(1) as usize;
    Ok(Composition { source, sink, behavior, condition, screens })
}

fn condition_from(v: &Value) -> Result<Condition, String> {
    let obj = v.as_object().ok_or("the condition is not an object")?;
    if let Some(n) = number(obj.get("above")) {
        return Ok(Condition::Above(n));
    }
    if let Some(n) = number(obj.get("below")) {
        return Ok(Condition::Below(n));
    }
    if obj.get("active").and_then(|x| x.as_bool()).unwrap_or(false) {
        return Ok(Condition::Active);
    }
    if let Some(pair) = pair(obj.get("above_band")) {
        return Ok(Condition::AboveBand { on: pair.0, off: pair.1 });
    }
    if let Some(pair) = pair(obj.get("below_band")) {
        return Ok(Condition::BelowBand { on: pair.0, off: pair.1 });
    }
    if let Some(pair) = pair(obj.get("between")) {
        return Ok(Condition::Between { low: pair.0, high: pair.1 });
    }
    Err("unknown condition".into())
}

fn circuit_from(v: &Value) -> Result<Circuit, String> {
    let obj = v.as_object().ok_or("the circuit is not an object")?;
    let board_obj = obj
        .get("board")
        .and_then(|b| b.as_object())
        .ok_or("the circuit has no board")?;
    let board_at = point(board_obj.get("at")).ok_or("bad board position")?;
    let cols = number(board_obj.get("cols")).unwrap_or(30.0).clamp(1.0, 64.0) as u8;
    let rot = number(board_obj.get("rot")).unwrap_or(0.0) as i32;
    let board = Board { at: board_at, cols, rot: rot.rem_euclid(4) as u8 };

    let parts_v = obj
        .get("parts")
        .and_then(|p| p.as_array())
        .ok_or("the circuit has no parts")?;
    if parts_v.len() > 2000 {
        return Err("too many parts".into());
    }
    let mut parts = Vec::with_capacity(parts_v.len());
    for pv in parts_v {
        parts.push(part_from(pv)?);
    }

    let pins_v = obj
        .get("pins")
        .and_then(|p| p.as_array())
        .ok_or("the circuit has no pins")?;
    if pins_v.len() > 20000 {
        return Err("too many pins".into());
    }
    let mut pins = Vec::with_capacity(pins_v.len());
    for pv in pins_v {
        let pin = pin_from(pv, &parts)?;
        if let Hole::Col(col, _) = pin.hole {
            if col >= cols {
                return Err("a pin is outside the board".into());
            }
        }
        pins.push(pin);
    }
    // Every part must own exactly the pins it lists, and no pin may point at
    // a part that is not there.
    for (i, part) in parts.iter().enumerate() {
        if part.pins.len() > 256 {
            return Err("a part has too many pins".into());
        }
        for &p in &part.pins {
            if p >= pins.len() || pins[p].part != i {
                return Err("a part and its pins disagree".into());
            }
        }
    }

    let wires_v = obj
        .get("wires")
        .and_then(|w| w.as_array())
        .ok_or("the circuit has no wires")?;
    if wires_v.len() > 20000 {
        return Err("too many wires".into());
    }
    let mut wires = Vec::with_capacity(wires_v.len());
    for wv in wires_v {
        wires.push(wire_from(wv, pins.len(), cols)?);
    }

    let arduino = match obj.get("arduino") {
        Some(Value::Null) | None => None,
        Some(v) => {
            let i = v.as_u64().ok_or("bad Arduino index")? as usize;
            if i >= parts.len() {
                return Err("the Arduino index is outside the parts".into());
            }
            Some(i)
        }
    };
    let bare = obj.get("bare").and_then(|b| b.as_bool()).unwrap_or(false);
    let mut net_names = std::collections::BTreeMap::new();
    if let Some(list) = obj.get("net_names").and_then(|n| n.as_array()) {
        for pair in list.iter().take(2000) {
            let Some(pair) = pair.as_array() else { continue };
            if pair.len() != 2 {
                continue;
            }
            let (Some(key), Some(name)) = (pair[0].as_str(), pair[1].as_str()) else { continue };
            if key.len() <= 256 && name.chars().count() <= 32 {
                net_names.insert(key.to_string(), name.to_string());
            }
        }
    }

    Ok(Circuit {
        board,
        parts,
        pins,
        wires,
        arduino,
        notes: Vec::new(),
        bare,
        net_names,
        ..Default::default()
    })
}

fn part_from(v: &Value) -> Result<Part, String> {
    let obj = v.as_object().ok_or("a part is not an object")?;
    let kind = obj
        .get("kind")
        .and_then(|k| k.as_str())
        .and_then(kind_from)
        .ok_or("unknown part kind")?;
    let label = text_field(obj, "label")?;
    let value = text_field(obj, "value")?;
    let at = point(obj.get("at")).ok_or("bad part position")?;
    let rot = number(obj.get("rot")).unwrap_or(0.0) as i32;
    let pins_v = obj
        .get("pins")
        .and_then(|p| p.as_array())
        .ok_or("a part has no pin list")?;
    let mut pins = Vec::with_capacity(pins_v.len());
    for p in pins_v {
        pins.push(usize_of(p).ok_or("bad pin index")?);
    }
    let mut internal = Vec::new();
    if let Some(list) = obj.get("internal").and_then(|i| i.as_array()) {
        for pair in list.iter().take(64) {
            let Some(arr) = pair.as_array() else { return Err("bad internal pair".into()) };
            if arr.len() != 2 {
                return Err("bad internal pair".into());
            }
            internal.push((
                usize_of(&arr[0]).ok_or("bad internal pin")?,
                usize_of(&arr[1]).ok_or("bad internal pin")?,
            ));
        }
    }
    Ok(Part {
        kind,
        label,
        value,
        at,
        pins,
        internal,
        rot: rot.rem_euclid(4) as u8,
        active: obj.get("active").and_then(|a| a.as_bool()).unwrap_or(true),
    })
}

fn pin_from(v: &Value, parts: &[Part]) -> Result<Pin, String> {
    let obj = v.as_object().ok_or("a pin is not an object")?;
    let part = obj
        .get("part")
        .and_then(usize_of)
        .ok_or("a pin has no part")?;
    if part >= parts.len() {
        return Err("a pin points outside the parts".into());
    }
    let name = text_field(obj, "name")?;
    if name.chars().count() > 40 {
        return Err("a pin name is too long".into());
    }
    let pos = point(obj.get("pos")).ok_or("bad pin position")?;
    let kind = obj
        .get("kind")
        .and_then(|k| k.as_str())
        .and_then(pin_kind_from)
        .ok_or("unknown pin kind")?;
    let hole = obj
        .get("hole")
        .and_then(hole_from)
        .ok_or("bad pin hole")?;
    Ok(Pin {
        part,
        name: intern(&name),
        pos,
        hole,
        kind,
        active: obj.get("active").and_then(|a| a.as_bool()).unwrap_or(true),
    })
}

fn wire_from(v: &Value, pins: usize, cols: u8) -> Result<Wire, String> {
    let obj = v.as_object().ok_or("a wire is not an object")?;
    let a = end_from(obj.get("a"), pins, cols).ok_or("bad first wire end")?;
    let b = end_from(obj.get("b"), pins, cols).ok_or("bad second wire end")?;
    let class = obj
        .get("class")
        .and_then(|c| c.as_str())
        .and_then(net_class_from)
        .unwrap_or(NetClass::Signal);
    let lift = number(obj.get("lift")).unwrap_or(0.0).clamp(0.0, 50.0);
    let mut via = Vec::new();
    if let Some(list) = obj.get("via").and_then(|x| x.as_array()) {
        for p in list.iter().take(64) {
            via.push(point(Some(p)).ok_or("bad wire bend")?);
        }
    }
    Ok(Wire {
        a,
        b,
        class,
        lift,
        via,
        colour: obj.get("colour").and_then(|c| c.as_u64()).and_then(|c| u8::try_from(c).ok()),
        active: obj.get("active").and_then(|a| a.as_bool()).unwrap_or(true),
    })
}

fn end_from(v: Option<&Value>, pins: usize, cols: u8) -> Option<End> {
    let obj = v?.as_object()?;
    if let Some(i) = obj.get("pin").and_then(usize_of) {
        return (i < pins).then_some(End::Pin(i));
    }
    let hole = obj.get("hole").and_then(hole_from)?;
    if let Hole::Col(col, _) = hole {
        if col >= cols {
            return None;
        }
    }
    Some(End::Hole(hole))
}

fn hole_from(v: &Value) -> Option<Hole> {
    if v.as_str() == Some("off") {
        return Some(Hole::OffBoard);
    }
    let obj = v.as_object()?;
    if let Some(arr) = obj.get("col").and_then(|c| c.as_array()) {
        if arr.len() != 2 {
            return None;
        }
        let col = u8_of(&arr[0])?;
        let row = Row::from_idx(u8_of(&arr[1])?);
        return Some(Hole::Col(col, row));
    }
    let arr = obj.get("rail")?.as_array()?;
    if arr.len() != 3 {
        return None;
    }
    let rail = rail_from(arr[0].as_str()?)?;
    let half = rail_half_from(arr[1].as_str()?)?;
    let idx = u8_of(&arr[2])?;
    if idx >= Board::RAIL_HOLES {
        return None;
    }
    Some(Hole::Rail(rail, half, idx))
}

// ---------------------------------------------------------------- names

fn text_field(obj: &serde_json::Map<String, Value>, key: &str) -> Result<String, String> {
    let text = obj
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("a part has no {key}"))?;
    if text.chars().count() > 200 {
        return Err(format!("a part {key} is too long"));
    }
    Ok(text.to_string())
}

fn number(v: Option<&Value>) -> Option<f32> {
    v?.as_f64().map(|x| x as f32).filter(|x| x.is_finite())
}

fn usize_of(v: &Value) -> Option<usize> {
    v.as_u64().map(|x| x as usize)
}

fn u8_of(v: &Value) -> Option<u8> {
    v.as_u64().and_then(|x| u8::try_from(x).ok())
}

fn point(v: Option<&Value>) -> Option<Pt> {
    let arr = v?.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    Some(Pt::new(number(Some(&arr[0]))?, number(Some(&arr[1]))?))
}

fn pair(v: Option<&Value>) -> Option<(f32, f32)> {
    let arr = v?.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    Some((number(Some(&arr[0]))?, number(Some(&arr[1]))?))
}

/// Pin names are `&'static str` throughout the drawing code. A loaded name is
/// interned once, so opening the same project repeatedly does not grow the
/// process.
fn intern(s: &str) -> &'static str {
    static POOL: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let pool = POOL.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut set) = pool.lock() {
        if let Some(found) = set.get(s) {
            return found;
        }
        let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
        set.insert(leaked);
        leaked
    } else {
        Box::leak(s.to_owned().into_boxed_str())
    }
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Arduino => "Arduino",
        Kind::Resistor => "Resistor",
        Kind::Led => "Led",
        Kind::Diode => "Diode",
        Kind::Nmos => "Nmos",
        Kind::Npn => "Npn",
        Kind::Button => "Button",
        Kind::Motor => "Motor",
        Kind::Servo => "Servo",
        Kind::Lamp => "Lamp",
        Kind::Buzzer => "Buzzer",
        Kind::Speaker => "Speaker",
        Kind::Pot => "Pot",
        Kind::Ldr => "Ldr",
        Kind::Tmp36 => "Tmp36",
        Kind::Relay => "Relay",
        Kind::Capacitor => "Capacitor",
        Kind::Electrolytic => "Electrolytic",
        Kind::Reed => "Reed",
        Kind::Thermistor => "Thermistor",
        Kind::Peltier => "Peltier",
        Kind::Module => "Module",
    }
}

fn kind_from(s: &str) -> Option<Kind> {
    Some(match s {
        "Arduino" => Kind::Arduino,
        "Resistor" => Kind::Resistor,
        "Led" => Kind::Led,
        "Diode" => Kind::Diode,
        "Nmos" => Kind::Nmos,
        "Npn" => Kind::Npn,
        "Button" => Kind::Button,
        "Motor" => Kind::Motor,
        "Servo" => Kind::Servo,
        "Lamp" => Kind::Lamp,
        "Buzzer" => Kind::Buzzer,
        "Speaker" => Kind::Speaker,
        "Pot" => Kind::Pot,
        "Ldr" => Kind::Ldr,
        "Tmp36" => Kind::Tmp36,
        "Relay" => Kind::Relay,
        "Capacitor" => Kind::Capacitor,
        "Electrolytic" => Kind::Electrolytic,
        "Reed" => Kind::Reed,
        "Thermistor" => Kind::Thermistor,
        "Peltier" => Kind::Peltier,
        "Module" => Kind::Module,
        _ => return None,
    })
}

fn pin_kind_name(k: PinKind) -> &'static str {
    match k {
        PinKind::Power => "Power",
        PinKind::Ground => "Ground",
        PinKind::Signal => "Signal",
        PinKind::Analog => "Analog",
        PinKind::Passive => "Passive",
    }
}

fn pin_kind_from(s: &str) -> Option<PinKind> {
    Some(match s {
        "Power" => PinKind::Power,
        "Ground" => PinKind::Ground,
        "Signal" => PinKind::Signal,
        "Analog" => PinKind::Analog,
        "Passive" => PinKind::Passive,
        _ => return None,
    })
}

fn net_class_name(c: NetClass) -> &'static str {
    match c {
        NetClass::Vcc => "Vcc",
        NetClass::Gnd => "Gnd",
        NetClass::Signal => "Signal",
        NetClass::Analog => "Analog",
    }
}

fn net_class_from(s: &str) -> Option<NetClass> {
    Some(match s {
        "Vcc" => NetClass::Vcc,
        "Gnd" => NetClass::Gnd,
        "Signal" => NetClass::Signal,
        "Analog" => NetClass::Analog,
        _ => return None,
    })
}

fn rail_name(r: Rail) -> &'static str {
    match r {
        Rail::TopPlus => "top_plus",
        Rail::TopMinus => "top_minus",
        Rail::BotPlus => "bot_plus",
        Rail::BotMinus => "bot_minus",
    }
}

fn rail_from(s: &str) -> Option<Rail> {
    Some(match s {
        "top_plus" => Rail::TopPlus,
        "top_minus" => Rail::TopMinus,
        "bot_plus" => Rail::BotPlus,
        "bot_minus" => Rail::BotMinus,
        _ => return None,
    })
}

fn rail_half_name(h: RailHalf) -> &'static str {
    match h {
        RailHalf::Left => "left",
        RailHalf::Right => "right",
    }
}

fn rail_half_from(s: &str) -> Option<RailHalf> {
    Some(match s {
        "left" => RailHalf::Left,
        "right" => RailHalf::Right,
        _ => return None,
    })
}

fn behavior_name(b: Behavior) -> &'static str {
    match b {
        Behavior::Blink => "Blink",
        Behavior::Solid => "Solid",
        Behavior::Beep => "Beep",
        Behavior::Switch => "Switch",
        Behavior::Show => "Show",
        Behavior::Turn => "Turn",
        Behavior::Spin => "Spin",
        Behavior::Color => "Color",
    }
}

fn behavior_from(s: &str) -> Option<Behavior> {
    Some(match s {
        "Blink" => Behavior::Blink,
        "Solid" => Behavior::Solid,
        "Beep" => Behavior::Beep,
        "Switch" => Behavior::Switch,
        "Show" => Behavior::Show,
        "Turn" => Behavior::Turn,
        "Spin" => Behavior::Spin,
        "Color" => Behavior::Color,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::Lang;

    fn sample() -> Project {
        let parsed = crate::parse::parse("температура выше 30 включи мотор");
        let built = crate::templates::build(&parsed.intent, 0, Lang::Ru).unwrap();
        Project {
            name: "Теплица".into(),
            task: parsed.intent.raw.clone(),
            manual: false,
            chosen: 0,
            recipe: parsed.intent.recipe,
            circuit: Some(built.circuit),
            view: ViewData { zoom: 1.3, pan_x: 12.0, pan_y: -4.0 },
            saved: 1_700_000_000,
        }
    }

    #[test]
    fn a_built_circuit_round_trips_through_json() {
        let p = sample();
        let text = serde_json::to_string(&to_json(&p)).unwrap();
        let back = from_json(&text).unwrap();
        assert_eq!(back.name, p.name);
        assert_eq!(back.task, p.task);
        assert_eq!(back.manual, p.manual);
        assert_eq!(back.chosen, p.chosen);
        assert_eq!(back.view.zoom, p.view.zoom);
        assert_eq!(back.recipe.map(|r| r.sink), p.recipe.map(|r| r.sink));
        let a = p.circuit.unwrap();
        let b = back.circuit.unwrap();
        assert_eq!(a.parts.len(), b.parts.len());
        assert_eq!(a.pins.len(), b.pins.len());
        assert_eq!(a.wires.len(), b.wires.len());
        assert_eq!(a.board.cols, b.board.cols);
        assert_eq!(a.board.rot, b.board.rot);
        for (x, y) in a.parts.iter().zip(&b.parts) {
            assert_eq!(x.kind, y.kind);
            assert_eq!(x.label, y.label);
            assert_eq!(x.value, y.value);
            assert_eq!(x.at, y.at);
            assert_eq!(x.pins, y.pins);
            assert_eq!(x.internal, y.internal);
        }
        for (x, y) in a.pins.iter().zip(&b.pins) {
            assert_eq!(x.part, y.part);
            assert_eq!(x.name, y.name);
            assert_eq!(x.pos, y.pos);
            assert_eq!(x.hole, y.hole);
            assert_eq!(x.kind, y.kind);
        }
        for (x, y) in a.wires.iter().zip(&b.wires) {
            assert_eq!(x.a, y.a);
            assert_eq!(x.b, y.b);
            assert_eq!(x.class, y.class);
            assert_eq!(x.via, y.via);
        }
    }

    #[test]
    fn a_file_with_a_broken_pin_is_rejected() {
        let text = r#"{
            "version": 1,
            "task": "x",
            "circuit": {
                "board": {"at": [0, 0], "cols": 30, "rot": 0},
                "parts": [],
                "pins": [{"part": 7, "name": "A", "pos": [0, 0], "kind": "Passive", "hole": "off"}],
                "wires": []
            }
        }"#;
        assert!(from_json(text).is_err());
    }

    #[test]
    fn a_newer_version_is_refused() {
        let text = r#"{"version": 99, "task": "x"}"#;
        match from_json(text) {
            Err(e) => assert!(e.contains("newer")),
            Ok(_) => panic!("a newer version was accepted"),
        }
    }

    #[test]
    fn file_names_stay_safe_and_readable() {
        assert_eq!(file_name("Теплица"), "Теплица.volta.json");
        assert_eq!(file_name("газ / свет: 2"), "газ--свет-2.volta.json");
        assert_eq!(file_name("   "), "проект.volta.json");
        assert_eq!(file_name("autosave"), "проект-autosave.volta.json");
        assert_eq!(file_name("a<b>c|d?e*f"), "abcdef.volta.json");
        assert!(is_autosave(&autosave_path()));
    }

    #[test]
    fn a_project_survives_a_real_file() {
        let path = std::env::temp_dir().join(format!("volta-project-{}.volta.json", std::process::id()));
        let p = sample();
        save(&path, &p).unwrap();
        let back = load(&path).unwrap();
        assert_eq!(back.name, "Теплица");
        assert_eq!(back.circuit.unwrap().parts.len(), p.circuit.unwrap().parts.len());
        let _ = std::fs::remove_file(path);
    }
}
