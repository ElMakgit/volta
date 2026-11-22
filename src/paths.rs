//! Where the app keeps its files.
//!
//! Linux follows the XDG layout. Windows has no `$HOME`, so the same data
//! lives under `%APPDATA%` and `%LOCALAPPDATA%`. Keeping the rule in one place
//! means the config, the theme and the project list cannot drift apart.

use std::path::{Path, PathBuf};

/// Write `body` to `path` through a temporary file in the same directory.
/// A crash halfway through then leaves the old file intact instead of a
/// truncated one: the rename is atomic on the same filesystem.
pub fn write_atomic(path: &Path, body: &str) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "volta.tmp".into());
    let tmp = dir.join(format!(".{name}.tmp"));
    let write = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(body.as_bytes())?;
        file.sync_all()
    })();
    if let Err(e) = write {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

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
