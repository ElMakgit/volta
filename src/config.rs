//! Small user preferences.
//!
//! Everything here survives a restart but stays out of the circuit: the theme
//! follows the desktop, and the wire routing is the one drawing choice the user
//! makes. Kept in `~/.config/volta/config.json` as plain JSON so the file is
//! readable and editable by hand.

use std::path::{Path, PathBuf};

use crate::model::WireStyle;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
}

pub fn config_dir() -> PathBuf {
    home().join(".config/volta")
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
    std::fs::write(path, body)
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
