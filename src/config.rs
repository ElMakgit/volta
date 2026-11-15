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
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let name = match style {
        WireStyle::Arc => "arc",
        WireStyle::Straight => "straight",
        WireStyle::Ortho => "ortho",
    };
    let body = format!("{{\n  \"wire_style\": \"{name}\"\n}}\n");
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
