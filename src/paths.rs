//! Where the app keeps its files.
//!
//! Linux follows the XDG layout. Windows has no `$HOME`, so the same data
//! lives under `%APPDATA%` and `%LOCALAPPDATA%`. Keeping the rule in one place
//! means the config, the theme and the project list cannot drift apart.

use std::path::PathBuf;

/// The user's home directory, with a Windows fallback for a native launch
/// where `$HOME` is not set by a shell.
pub fn home() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(p) = std::env::var("USERPROFILE") {
            if !p.trim().is_empty() {
                return PathBuf::from(p);
            }
        }
        let drive = std::env::var("HOMEDRIVE").unwrap_or_default();
        let path = std::env::var("HOMEPATH").unwrap_or_default();
        if !drive.is_empty() || !path.is_empty() {
            return PathBuf::from(format!("{drive}{path}"));
        }
        PathBuf::from("C:\\")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
    }
}

/// Preferences: theme choice, wire style, export format, the task draft.
pub fn config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home().join("AppData/Roaming"));
        base.join("volta")
    }
    #[cfg(not(windows))]
    {
        home().join(".config/volta")
    }
}

/// Saved projects and the autosave bench. These are user data, not settings.
pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home().join("AppData/Local"));
        base.join("volta")
    }
    #[cfg(not(windows))]
    {
        if let Ok(x) = std::env::var("XDG_DATA_HOME") {
            if !x.trim().is_empty() {
                return PathBuf::from(x).join("volta");
            }
        }
        home().join(".local/share/volta")
    }
}
