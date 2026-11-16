//! PNG export.
//!
//! The SVG is rasterised by `rsvg-convert` instead of a hand-written rasteriser:
//! text shaping and anti-aliasing come out right, and the tool is part of the
//! same librsvg package Omarchy already ships.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn have_tool() -> bool {
    Command::new("rsvg-convert")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Rasterise the SVG next to the target, then remove the temporary file.
pub fn png_from_svg(svg: &str, out: &Path, width_px: u32) -> Result<(), String> {
    let tmp = out.with_extension("tmp.svg");
    std::fs::write(&tmp, svg).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let result = Command::new("rsvg-convert")
        .arg("-w")
        .arg(width_px.to_string())
        .arg("-o")
        .arg(out)
        .arg(&tmp)
        .output();
    let _ = std::fs::remove_file(&tmp);
    match result {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(format!(
            "rsvg-convert: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        )),
        Err(e) => Err(format!(
            "rsvg-convert not available: {e}. Install librsvg and try again."
        )),
    }
}

pub fn default_dir() -> PathBuf {
    crate::paths::home().join("Pictures").join("volta")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Anchor, Prim, Rgba, Scene};
    use crate::model::{Pt, Rect};

    #[test]
    fn the_screenshot_round_trips_through_librsvg() {
        if !have_tool() {
            eprintln!("rsvg-convert is missing, skipping");
            return;
        }
        let scene = Scene {
            prims: vec![
                Prim::Rect {
                    r: Rect::new(1.0, 1.0, 8.0, 6.0),
                    radius: 0.4,
                    fill: Rgba::rgb(0x2A, 0x75, 0xB8),
                    stroke: Some((Rgba::rgb(0, 0, 0), 0.2)),
                },
                Prim::Text {
                    p: Pt::new(5.0, 4.0),
                    size: 1.4,
                    text: "volta".into(),
                    color: Rgba::rgb(0xFF, 0xFF, 0xFF),
                    anchor: Anchor::Center,
                    bold: true,
                    angle: 0.0,
                },
            ],
            bg: Rgba::rgb(0xEF, 0xEE, 0xEB),
            w: 10.0,
            h: 8.0,
            origin: Pt::default(),
        };
        let dir = std::env::temp_dir().join("volta-png-test");
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("shot.png");
        let _ = std::fs::remove_file(&out);
        let svg = crate::svg::render(&scene, 1.0);
        png_from_svg(&svg, &out, 600).expect("rasterising failed");
        let bytes = std::fs::read(&out).expect("no PNG written");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
        assert!(bytes.len() > 2000, "the PNG is suspiciously small: {}", bytes.len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
