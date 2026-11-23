//! Checking GitHub for a newer release.
//!
//! The app solves tasks offline; this is the only request it makes. The check
//! runs on its own thread so the window never waits, fails silently, and is
//! asked for at most once a day unless the user starts it from About.
//!
//! Linux (and the other desktops) show the result in a small card inside the
//! main window. Windows shows a system notification; the same release is
//! opened from it.

use std::sync::mpsc::{self, Receiver};

/// The repository whose stable releases are checked.
pub const REPO: &str = "ElMakgit/volta";
/// How long a successful or failed automatic check is trusted.
pub const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
/// The release endpoint: GitHub returns the newest non-draft, non-prerelease
/// release, so a beta tag never tells stable users to upgrade.
const LATEST: &str = "https://api.github.com/repos/ElMakgit/volta/releases/latest";
const USER_AGENT: &str = "volta-update-check";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// The tag as published, for example `v0.9.0`.
    pub tag: String,
    /// The tag without the leading `v`, for comparisons and labels.
    pub version: String,
    /// The release page the card and the notification open.
    pub url: String,
}

#[derive(Debug)]
pub enum Outcome {
    Latest,
    Newer(Release),
    Failed(String),
}

/// Ask GitHub in the background. The caller owns the receiver and polls it
/// from the UI thread; no network call ever runs in the paint path.
pub fn spawn() -> Receiver<Outcome> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let outcome = match fetch_latest() {
            Ok(Some(release)) => Outcome::Newer(release),
            Ok(None) => Outcome::Latest,
            Err(why) => Outcome::Failed(why),
        };
        let _ = tx.send(outcome);
    });
    rx
}

/// Today's version, straight from the manifest the binary was built from.
pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A release is newer only when its numeric version is. A suffix such as
/// `-beta` is not an upgrade for a stable build, and a malformed tag is not
/// a reason to nag.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let a = version_key(latest);
    let b = version_key(current);
    for i in 0..a.len().max(b.len()) {
        let left = a.get(i).copied().unwrap_or(0);
        let right = b.get(i).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

fn version_key(text: &str) -> Vec<u64> {
    text.trim()
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

/// Read the fields the window needs, and build the release URL ourselves:
/// a URL from the response must never choose what the browser opens.
pub fn release_from_json(text: &str) -> Option<Release> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let tag = value.get("tag_name")?.as_str()?.trim();
    if tag.is_empty()
        || !tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return None;
    }
    let version = tag.trim_start_matches(['v', 'V']).to_string();
    if version.is_empty()
        || !version
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some(Release {
        tag: tag.to_string(),
        version,
        url: format!("https://github.com/{REPO}/releases/tag/{tag}"),
    })
}

fn fetch_latest() -> Result<Option<Release>, String> {
    let body = http_get(LATEST).ok_or_else(|| "network".to_string())?;
    let release = release_from_json(&body).ok_or_else(|| "json".to_string())?;
    if is_newer(&release.version, current()) {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(cmd).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(not(windows))]
fn http_get(url: &str) -> Option<String> {
    // Both tools are part of a normal desktop install. A user without them
    // simply gets no notice, which is better than a hard dependency.
    run(
        "curl",
        &[
            "-fsSL",
            "--max-time",
            "10",
            "-H",
            &format!("User-Agent: {USER_AGENT}"),
            url,
        ],
    )
    .or_else(|| {
        run(
            "wget",
            &[
                "-qO-",
                "--timeout=10",
                &format!("--header=User-Agent: {USER_AGENT}"),
                url,
            ],
        )
    })
}

#[cfg(windows)]
fn http_get(url: &str) -> Option<String> {
    // Windows 10 and 11 ship curl.exe; older or stripped installs fall back
    // to PowerShell, which is always present.
    run(
        "curl.exe",
        &[
            "-fsSL",
            "--max-time",
            "10",
            "-H",
            &format!("User-Agent: {USER_AGENT}"),
            url,
        ],
    )
    .or_else(|| {
        let script = format!(
            "(Invoke-WebRequest -UseBasicParsing -Uri '{url}' -Headers @{{'User-Agent'='{USER_AGENT}'}}).Content"
        );
        run(
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-Command", &script],
        )
    })
}

/// Open the release page in the desktop browser. The URL is built from a
/// checked tag, never from the sentence or the response body.
pub fn open_url(url: &str) -> bool {
    #[cfg(windows)]
    let child = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let child = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(windows, target_os = "macos")))]
    let child = std::process::Command::new("xdg-open").arg(url).spawn();
    child.is_ok()
}

/// One system notification on Windows. The toast opens the release page when
/// clicked; if WinRT is unavailable, the tray balloon still carries the text.
#[cfg(windows)]
pub fn notify_windows(release: &Release, lang: crate::text::Lang) -> bool {
    use crate::text::tr;
    use std::io::Write;

    let title = tr(lang, "Доступно обновление volta", "A volta update is available");
    let body = format!(
        "{} {} — {}",
        tr(lang, "Версия", "Version"),
        release.version,
        tr(lang, "нажмите, чтобы открыть релиз", "click to open the release")
    );
    let script = windows_script(title, &body, &release.url);
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = std::process::Command::new("powershell.exe");
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            "-",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return false,
    };
    if let Some(stdin) = child.stdin.as_mut() {
        if stdin.write_all(script.as_bytes()).is_err() {
            return false;
        }
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

#[cfg(windows)]
fn windows_script(title: &str, body: &str, url: &str) -> String {
    let xml_title = xml_escape(title);
    let xml_body = xml_escape(body);
    let xml_url = xml_escape(url);
    let ps_title = title.replace('\'', "''");
    let ps_body = body.replace('\'', "''");
    let ps_url = url.replace('\'', "''");
    format!(
        r#"$ErrorActionPreference = 'Stop'
$url = @'
{ps_url}
'@
try {{
  [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
  [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
  $xml = New-Object Windows.Data.Xml.Dom.XmlDocument
  $xml.LoadXml(@'
<toast activationType="protocol" launch="{xml_url}">
  <visual><binding template="ToastGeneric"><text>{xml_title}</text><text>{xml_body}</text></binding></visual>
</toast>
'@)
  $note = New-Object Windows.UI.Notifications.ToastNotification $xml
  [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('PowerShell').Show($note)
}} catch {{
  Add-Type -AssemblyName System.Windows.Forms
  Add-Type -AssemblyName System.Drawing
  $icon = New-Object System.Windows.Forms.NotifyIcon
  $icon.Icon = [System.Drawing.SystemIcons]::Information
  $icon.Visible = $true
  $icon.ShowBalloonTip(8000, @'
{ps_title}
'@, @'
{ps_body}
'@, [System.Windows.Forms.ToolTipIcon]::Info)
  Start-Sleep -Seconds 8
  $icon.Dispose()
}}
"#
    )
}

#[cfg(windows)]
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_numeric_newer_version_is_offered() {
        assert!(is_newer("0.9.0", "0.8.3"));
        assert!(is_newer("v0.8.4", "0.8.3"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(!is_newer("0.8.3", "0.8.3"));
        assert!(!is_newer("v0.8.2", "0.8.3"));
        assert!(!is_newer("0.8.3-beta", "0.8.3"));
        assert!(!is_newer("nightly", "0.8.3"));
    }

    #[test]
    fn the_release_page_is_built_from_a_safe_tag() {
        let json = r#"{
            "tag_name": "v0.9.0",
            "html_url": "https://example.com/elsewhere",
            "body": "notes"
        }"#;
        let release = release_from_json(json).expect("a release");
        assert_eq!(release.version, "0.9.0");
        assert_eq!(
            release.url,
            "https://github.com/ElMakgit/volta/releases/tag/v0.9.0"
        );
        // The response cannot point the browser at another site.
        assert!(!release.url.contains("example.com"));
    }

    #[test]
    fn a_malformed_release_is_ignored() {
        assert!(release_from_json("not json").is_none());
        assert!(release_from_json(r#"{"tag_name": "../evil"}"#).is_none());
        assert!(release_from_json(r#"{"tag_name": ""}"#).is_none());
        assert!(release_from_json(r#"{"tag_name": "release"}"#).is_none());
        assert!(release_from_json(r#"{"html_url": "x"}"#).is_none());
    }
}
