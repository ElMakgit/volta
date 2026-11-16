//! Following the desktop.
//!
//! Omarchy rewrites the theme directory on every switch (files are replaced,
//! sometimes symlinked), so the watch re-arms itself: we compare the current
//! `theme.name` and the mtime of `colors.toml` instead of holding on to a file
//! handle.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::scene::Rgba;

fn home() -> PathBuf {
    crate::paths::home()
}

pub fn strip_dir() -> PathBuf {
    home().join(".local/state/omarchy/current/theme")
}

pub fn state_dir() -> PathBuf {
    home().join(".local/state/omarchy/current")
}

pub fn config_dir() -> PathBuf {
    home().join(".config/omarchy")
}

pub struct Theme {
    pub accent: Rgba,
    pub bg: Rgba,
    pub fg: Rgba,
    pub muted: Rgba,
    pub panel: Rgba,
    theme_dir: PathBuf,
    state: PathBuf,
    stamp: (String, Option<SystemTime>),
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: Rgba::rgb(0xFF, 0xD6, 0x0A),
            bg: Rgba::rgb(0x14, 0x14, 0x14),
            fg: Rgba::rgb(0xFC, 0xFC, 0xFC),
            muted: Rgba::rgb(0x80, 0x80, 0x80),
            panel: Rgba::rgb(0x1c, 0x1c, 0x1c),
            theme_dir: strip_dir(),
            state: state_dir(),
            stamp: (String::new(), None),
        }
    }
}

impl Theme {
    pub fn load() -> Self {
        Self::load_at(&strip_dir(), &state_dir())
    }

    pub fn load_at(theme_dir: &Path, state: &Path) -> Self {
        let mut t = Theme {
            theme_dir: theme_dir.to_path_buf(),
            state: state.to_path_buf(),
            ..Default::default()
        };
        t.refresh();
        t
    }

    fn stamp_now(&self) -> (String, Option<SystemTime>) {
        let name = std::fs::read_to_string(self.state.join("theme.name")).unwrap_or_default();
        let mtime = std::fs::metadata(self.theme_dir.join("colors.toml"))
            .and_then(|m| m.modified())
            .ok();
        (name.trim().to_string(), mtime)
    }

    /// Re-read the theme if the desktop switched. Returns true on change.
    pub fn poll(&mut self) -> bool {
        let now = self.stamp_now();
        if now == self.stamp {
            return false;
        }
        self.stamp = now;
        self.refresh_values();
        true
    }

    fn refresh(&mut self) {
        self.stamp = self.stamp_now();
        self.refresh_values();
    }

    fn refresh_values(&mut self) {
        let src = std::fs::read_to_string(self.theme_dir.join("colors.toml")).unwrap_or_default();
        let mut accent = None;
        let mut bg = None;
        let mut fg = None;
        let mut muted = None;
        for line in src.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim().trim_matches('"');
            let Some(col) = Rgba::from_hex(value) else { continue };
            match key.trim() {
                "accent" => accent = Some(col),
                "background" => bg = Some(col),
                "foreground" => fg = Some(col),
                "muted" => muted = Some(col),
                _ => {}
            }
        }
        if let Some(a) = accent {
            self.accent = a;
        }
        if let Some(b) = bg {
            self.bg = b;
            self.panel = b.lighten(0.07);
        }
        if let Some(f) = fg {
            self.fg = f;
        }
        if let Some(m) = muted {
            self.muted = m.lighten(0.35);
        }
    }
}

/// The desktop's language tag, e.g. `ru` or `pt-BR`.
pub fn language_tag() -> String {
    std::fs::read_to_string(config_dir().join("language"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "en".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_theme_falls_back_to_omarchy_yellow() {
        let t = Theme::load_at(Path::new("/nonexistent/theme"), Path::new("/nonexistent/state"));
        assert_eq!(t.accent, Rgba::rgb(0xFF, 0xD6, 0x0A));
    }

    #[test]
    fn theme_colours_are_parsed() {
        let dir = std::env::temp_dir().join("volta-theme-test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("colors.toml"),
            "accent = \"#123456\"\nbackground = \"#010203\"\nforeground = \"#ffffff\"\n",
        )
        .unwrap();
        let t = Theme::load_at(&dir, Path::new("/nonexistent/state"));
        assert_eq!(t.accent, Rgba::rgb(0x12, 0x34, 0x56));
        assert_eq!(t.bg, Rgba::rgb(0x01, 0x02, 0x03));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
