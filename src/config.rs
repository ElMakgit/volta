//! Small user preferences.
//!
//! Everything here survives a restart but stays out of the circuit: the theme
//! follows the desktop, and the wire routing is the one drawing choice the user
//! makes. Kept in `~/.config/volta/config.json` as plain JSON so the file is
//! readable and editable by hand.

use std::path::{Path, PathBuf};

use crate::model::WireStyle;

pub fn config_dir() -> PathBuf {
    crate::paths::config_dir()
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn load_wire_style() -> WireStyle {
    load_wire_style_at(&config_path())
}

pub fn load_wire_style_at(path: &Path) -> WireStyle {
    let Ok(text) = std::fs::read_to_string(path) else {
        return WireStyle::Ortho;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return WireStyle::Ortho;
    };
    match value.get("wire_style").and_then(|v| v.as_str()) {
        Some("straight") => WireStyle::Straight,
        Some("arc") => WireStyle::Arc,
        _ => WireStyle::Ortho,
    }
}

pub fn save_wire_style(style: WireStyle) {
    let _ = save_wire_style_at(&config_path(), style);
}

pub fn save_wire_style_at(path: &Path, style: WireStyle) -> std::io::Result<()> {
    let name = match style {
        WireStyle::Arc => "arc",
        WireStyle::Straight => "straight",
        WireStyle::Ortho => "ortho",
    };
    put(path, "wire_style", serde_json::Value::from(name))
}

/// The circuit background the user last picked: `false` is the white
/// Tinkercad-like sheet, which is also the default.
pub fn load_bench_dark() -> bool {
    load_bench_dark_at(&config_path())
}

pub fn load_bench_dark_at(path: &Path) -> bool {
    read(path)
        .get("bench_dark")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn save_bench_dark(dark: bool) {
    let _ = save_bench_dark_at(&config_path(), dark);
}

pub fn save_bench_dark_at(path: &Path, dark: bool) -> std::io::Result<()> {
    put(path, "bench_dark", serde_json::Value::from(dark))
}

/// Where the last export went, so the save window opens there next time.
pub fn load_export_dir() -> Option<PathBuf> {
    load_export_dir_at(&config_path())
}

pub fn load_export_dir_at(path: &Path) -> Option<PathBuf> {
    read(path)
        .get("export_dir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
}

pub fn save_export_dir(dir: &Path) {
    let _ = save_export_dir_at(&config_path(), dir);
}

pub fn save_export_dir_at(path: &Path, dir: &Path) -> std::io::Result<()> {
    let text = dir.to_string_lossy().to_string();
    put(path, "export_dir", serde_json::Value::from(text))
}

/// The format of the last export, so one button can repeat what the user did
/// last time instead of asking again.
pub fn load_export_kind() -> Option<String> {
    load_export_kind_at(&config_path())
}

pub fn load_export_kind_at(path: &Path) -> Option<String> {
    read(path)
        .get("export_kind")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
}

pub fn save_export_kind(key: &str) {
    let _ = save_export_kind_at(&config_path(), key);
}

pub fn save_export_kind_at(path: &Path, key: &str) -> std::io::Result<()> {
    put(path, "export_kind", serde_json::Value::from(key))
}

/// Whether the window may ask GitHub for a new release on start. The check is
/// the app's only network request, so the user can turn it off for good.
pub fn load_update_enabled() -> bool {
    load_update_enabled_at(&config_path())
}

pub fn load_update_enabled_at(path: &Path) -> bool {
    read(path)
        .get("update_auto")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

pub fn save_update_enabled(enabled: bool) {
    let _ = save_update_enabled_at(&config_path(), enabled);
}

pub fn save_update_enabled_at(path: &Path, enabled: bool) -> std::io::Result<()> {
    put(path, "update_auto", serde_json::Value::from(enabled))
}

/// When the last automatic check finished, as a Unix timestamp. A failed
/// check counts too: an offline laptop must not retry on every launch.
pub fn load_update_checked() -> Option<u64> {
    load_update_checked_at(&config_path())
}

pub fn load_update_checked_at(path: &Path) -> Option<u64> {
    read(path).get("update_checked").and_then(|v| v.as_u64())
}

pub fn save_update_checked(secs: u64) {
    let _ = save_update_checked_at(&config_path(), secs);
}

pub fn save_update_checked_at(path: &Path, secs: u64) -> std::io::Result<()> {
    put(path, "update_checked", serde_json::Value::from(secs))
}

/// A version the user asked not to hear about again. A later release has a
/// different number and is offered normally.
pub fn load_update_skipped() -> Option<String> {
    load_update_skipped_at(&config_path())
}

pub fn load_update_skipped_at(path: &Path) -> Option<String> {
    read(path)
        .get("update_skip")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
}

pub fn save_update_skipped(version: &str) {
    let _ = save_update_skipped_at(&config_path(), version);
}

pub fn save_update_skipped_at(path: &Path, version: &str) -> std::io::Result<()> {
    put(path, "update_skip", serde_json::Value::from(version))
}

/// The language the user picked by hand, if any. `None` means the app has
/// never been switched and should follow the desktop's own language.
pub fn load_lang() -> Option<String> {
    load_lang_at(&config_path())
}

pub fn load_lang_at(path: &Path) -> Option<String> {
    read(path)
        .get("lang")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
}

pub fn save_lang(tag: &str) {
    let _ = save_lang_at(&config_path(), tag);
}

pub fn save_lang_at(path: &Path, tag: &str) -> std::io::Result<()> {
    put(path, "lang", serde_json::Value::from(tag))
}

/// The language the app starts in: the user's own pick, or the desktop's tag
/// until they make one. A manual choice has to win, otherwise the theme poll
/// would switch the interface back on every frame.
pub fn lang() -> crate::text::Lang {
    lang_from(&load_lang(), &crate::theme::language_tag())
}

/// Split out so the priority rule can be tested without a config file.
pub fn lang_from(picked: &Option<String>, desktop: &str) -> crate::text::Lang {
    crate::text::Lang::from_tag(picked.as_deref().unwrap_or(desktop))
}

/// The task left unfinished last time. An empty task is no draft at all.
pub fn load_draft() -> Option<(String, bool)> {
    load_draft_at(&config_path())
}

pub fn load_draft_at(path: &Path) -> Option<(String, bool)> {
    let doc = read(path);
    let task = doc.get("draft").and_then(|d| d.get("task"))?.as_str()?;
    if task.trim().is_empty() {
        return None;
    }
    let manual = doc
        .get("draft")
        .and_then(|d| d.get("mode"))
        .and_then(|m| m.as_str())
        .map(|m| m == "manual")
        .unwrap_or(false);
    Some((task.to_owned(), manual))
}

pub fn save_draft(task: &str, manual: bool) {
    let _ = save_draft_at(&config_path(), task, manual);
}

pub fn save_draft_at(path: &Path, task: &str, manual: bool) -> std::io::Result<()> {
    let draft = serde_json::json!({
        "task": task,
        "mode": if manual { "manual" } else { "ai" },
    });
    put(path, "draft", draft)
}

pub fn clear_draft() {
    let _ = clear_draft_at(&config_path());
}

pub fn clear_draft_at(path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut doc = read(path);
    if let Some(obj) = doc.as_object_mut() {
        obj.remove("draft");
    }
    let mut body = serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".into());
    body.push('\n');
    crate::paths::write_atomic(path, &body)
}

fn read(path: &Path) -> serde_json::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Set one key and keep the rest, so saving the background does not forget
/// the wire style and vice versa.
fn put(path: &Path, key: &str, value: serde_json::Value) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut doc = read(path);
    if let Some(obj) = doc.as_object_mut() {
        obj.insert(key.to_string(), value);
    }
    let mut body = serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".into());
    body.push('\n');
    crate::paths::write_atomic(path, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("volta-config-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config.json")
    }

    #[test]
    fn the_routing_round_trips() {
        let path = scratch("roundtrip");
        assert_eq!(load_wire_style_at(&path), WireStyle::Ortho, "missing file");
        save_wire_style_at(&path, WireStyle::Straight).unwrap();
        assert_eq!(load_wire_style_at(&path), WireStyle::Straight);
        save_wire_style_at(&path, WireStyle::Arc).unwrap();
        assert_eq!(load_wire_style_at(&path), WireStyle::Arc);
        save_wire_style_at(&path, WireStyle::Ortho).unwrap();
        assert_eq!(load_wire_style_at(&path), WireStyle::Ortho);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_background_and_the_folder_survive_a_save() {
        let path = scratch("prefs");
        assert!(!load_bench_dark_at(&path));
        assert!(load_export_dir_at(&path).is_none());
        save_bench_dark_at(&path, true).unwrap();
        save_wire_style_at(&path, WireStyle::Arc).unwrap();
        save_export_dir_at(&path, Path::new("/tmp/somewhere")).unwrap();
        // All keys live in the same file: writing one keeps the others.
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"bench_dark\": true"));
        assert!(text.contains("\"wire_style\": \"arc\""));
        assert_eq!(load_wire_style_at(&path), WireStyle::Arc);
        assert!(load_bench_dark_at(&path));
        assert_eq!(load_export_dir_at(&path).as_deref(), Some(Path::new("/tmp/somewhere")));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_last_export_and_the_draft_survive_a_save() {
        let path = scratch("draft");
        assert!(load_export_kind_at(&path).is_none());
        assert!(load_draft_at(&path).is_none());
        save_export_kind_at(&path, "png").unwrap();
        save_draft_at(&path, "температура выше 30 серво", true).unwrap();
        assert_eq!(load_export_kind_at(&path).as_deref(), Some("png"));
        assert_eq!(
            load_draft_at(&path),
            Some(("температура выше 30 серво".into(), true))
        );
        // One key does not forget the others.
        assert_eq!(load_wire_style_at(&path), WireStyle::Ortho);
        clear_draft_at(&path).unwrap();
        assert!(load_draft_at(&path).is_none());
        assert_eq!(load_export_kind_at(&path).as_deref(), Some("png"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_blank_task_is_not_a_draft() {
        let path = scratch("blank-draft");
        save_draft_at(&path, "   ", false).unwrap();
        assert!(load_draft_at(&path).is_none());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// A language picked by hand outranks the desktop tag, and it survives a
    /// restart. Without a pick the desktop decides.
    #[test]
    fn a_picked_language_wins_over_the_desktop() {
        use crate::text::Lang;
        let path = scratch("lang");
        assert!(load_lang_at(&path).is_none());
        assert_eq!(lang_from(&None, "ru"), Lang::Ru);
        assert_eq!(lang_from(&None, "en_GB"), Lang::En);
        save_lang_at(&path, "en").unwrap();
        assert_eq!(load_lang_at(&path).as_deref(), Some("en"));
        assert_eq!(lang_from(&load_lang_at(&path), "ru"), Lang::En);
        // One key does not forget the others.
        save_wire_style_at(&path, WireStyle::Arc).unwrap();
        assert_eq!(load_lang_at(&path).as_deref(), Some("en"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_update_check_remembers_its_schedule() {
        let path = scratch("update");
        assert!(load_update_enabled_at(&path), "automatic checks are on by default");
        assert!(load_update_checked_at(&path).is_none());
        assert!(load_update_skipped_at(&path).is_none());
        save_update_enabled_at(&path, false).unwrap();
        save_update_checked_at(&path, 1_700_000_000).unwrap();
        save_update_skipped_at(&path, "0.9.0").unwrap();
        assert!(!load_update_enabled_at(&path));
        assert_eq!(load_update_checked_at(&path), Some(1_700_000_000));
        assert_eq!(load_update_skipped_at(&path).as_deref(), Some("0.9.0"));
        // The other preferences live in the same file and stay put.
        save_lang_at(&path, "en").unwrap();
        assert_eq!(load_update_skipped_at(&path).as_deref(), Some("0.9.0"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_broken_file_falls_back_to_arcs() {
        let path = scratch("broken");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load_wire_style_at(&path), WireStyle::Ortho);
        std::fs::write(&path, "{\"wire_style\": 42}").unwrap();
        assert_eq!(load_wire_style_at(&path), WireStyle::Ortho);
        std::fs::write(&path, "{\"wire_style\": \"zigzag\"}").unwrap();
        assert_eq!(load_wire_style_at(&path), WireStyle::Ortho);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
