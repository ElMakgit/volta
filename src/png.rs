//! PNG export.
//!
//! The scene is a flat list of primitives drawn for both the screen and the
//! SVG writer, so the PNG is rasterised from the same list. No external
//! `rsvg-convert` is needed: an export works on a clean machine with just the
//! binary, including Windows.

use std::path::{Path, PathBuf};

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};

use crate::scene::{Anchor, Prim, Rgba as SRgba, Scene};

const FONT: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");

/// Where the export dialog starts when no folder was chosen yet.
pub fn default_dir() -> PathBuf {
    crate::paths::home().join("Pictures").join("volta")
}

fn px(colour: SRgba) -> Rgba<u8> {
    Rgba([colour.0, colour.1, colour.2, colour.3])
}

fn blend(img: &mut RgbaImage, x: i32, y: i32, colour: Rgba<u8>) {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 {
        return;
    }
    let a = colour[3] as f32 / 255.0;
    if a <= 0.0 {
        return;
    }
    let old = *img.get_pixel(x as u32, y as u32);
    let f = |c: u8, b: u8| (c as f32 * a + b as f32 * (1.0 - a)).round() as u8;
    let mixed = Rgba([
        f(colour[0], old[0]),
        f(colour[1], old[1]),
        f(colour[2], old[2]),
        255,
    ]);
    img.put_pixel(x as u32, y as u32, mixed);
}

fn disc(img: &mut RgbaImage, cx: f32, cy: f32, radius: f32, colour: Rgba<u8>) {
    let r = radius.max(0.5);
    let x0 = (cx - r).floor() as i32;
    let x1 = (cx + r).ceil() as i32;
    let y0 = (cy - r).floor() as i32;
    let y1 = (cy + r).ceil() as i32;
    let r2 = r * r;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r2 {
                blend(img, x, y, colour);
            }
        }
    }
}

fn segment(img: &mut RgbaImage, ax: f32, ay: f32, bx: f32, by: f32, w: f32, colour: Rgba<u8>) {
    let len = ((bx - ax) * (bx - ax) + (by - ay) * (by - ay)).sqrt();
    let steps = (len / 0.7).ceil().max(1.0) as usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        disc(
            img,
            ax + (bx - ax) * t,
            ay + (by - ay) * t,
            w / 2.0,
            colour,
        );
    }
}

fn rect(img: &mut RgbaImage, x: f32, y: f32, w: f32, h: f32, colour: Rgba<u8>) {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = (x + w).ceil() as i32;
    let y1 = (y + h).ceil() as i32;
    for yy in y0..y1 {
        for xx in x0..x1 {
            blend(img, xx, yy, colour);
        }
    }
}

fn draw_text(
    img: &mut RgbaImage,
    string: &str,
    x: f32,
    y: f32,
    size_px: f32,
    bold: bool,
    anchor: Anchor,
    colour: Rgba<u8>,
) {
    let _ = bold;
    let Ok(font) = FontRef::try_from_slice(FONT) else { return };
    let scale = PxScale::from(size_px);
    let scaled = font.as_scaled(scale);
    let mut caret = 0.0;
    let advance: f32 = string.chars().map(|c| scaled.h_advance(font.glyph_id(c))).sum();
    let start = match anchor {
        Anchor::Start => x,
        Anchor::Center => x - advance / 2.0,
        Anchor::End => x - advance,
    };
    let baseline = y + size_px * 0.36;
    for ch in string.chars() {
        let id = font.glyph_id(ch);
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(start + caret, baseline));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            let ox = bounds.min.x as i32;
            let oy = bounds.min.y as i32;
            outline.draw(|gx, gy, coverage| {
                let mut c = colour;
                c.0[3] = (c.0[3] as f32 * coverage).round() as u8;
                blend(img, ox + gx as i32, oy + gy as i32, c);
            });
        }
        caret += scaled.h_advance(id);
    }
}

/// Rasterise a scene into a PNG. `width_px` fixes the horizontal resolution;
/// the height follows the scene aspect so nothing is stretched.
///
/// The scene has already moved its primitives to its own origin (see
/// `Scene::translate`), so their coordinates are scene millimetres. Using
/// `scene.origin` here as well would shift the picture a second time and crop
/// its right and bottom edges.
pub fn image_from_scene(scene: &Scene, width_px: u32) -> RgbaImage {
    let scale = width_px as f32 / scene.w.max(1e-3);
    let height = (scene.h * scale).round().max(1.0) as u32;
    let mut img = RgbaImage::from_pixel(width_px, height, px(scene.bg));

    for prim in &scene.prims {
        match prim {
            Prim::Rect { r, fill, stroke, .. } => {
                rect(
                    &mut img,
                    r.x * scale,
                    r.y * scale,
                    r.w * scale,
                    r.h * scale,
                    px(*fill),
                );
                if let Some((colour, w)) = stroke {
                    let (x, y) = (r.x * scale, r.y * scale);
                    let (wpx, hpx) = (r.w * scale, r.h * scale);
                    let s = px(*colour);
                    segment(&mut img, x, y, x + wpx, y, *w * scale, s);
                    segment(&mut img, x + wpx, y, x + wpx, y + hpx, *w * scale, s);
                    segment(&mut img, x + wpx, y + hpx, x, y + hpx, *w * scale, s);
                    segment(&mut img, x, y + hpx, x, y, *w * scale, s);
                }
            }
            Prim::Circle { c, r, fill, stroke } => {
                let (cx, cy) = (c.x * scale, c.y * scale);
                disc(&mut img, cx, cy, *r * scale, px(*fill));
                if let Some((colour, w)) = stroke {
                    let steps = (2.0 * std::f32::consts::PI * *r * scale / 1.0).ceil().max(8.0) as usize;
                    for i in 0..steps {
                        let a = i as f32 * std::f32::consts::TAU / steps as f32;
                        let b = (i + 1) as f32 * std::f32::consts::TAU / steps as f32;
                        segment(
                            &mut img,
                            cx + a.cos() * *r * scale,
                            cy + a.sin() * *r * scale,
                            cx + b.cos() * *r * scale,
                            cy + b.sin() * *r * scale,
                            *w * scale,
                            px(*colour),
                        );
                    }
                }
            }
            Prim::Poly { pts, w, color, closed } => {
                let mut it = pts.iter().peekable();
                while let Some(p) = it.next() {
                    let Some(q) = it.peek() else { break };
                    segment(
                        &mut img,
                        p.x * scale,
                        p.y * scale,
                        q.x * scale,
                        q.y * scale,
                        *w * scale,
                        px(*color),
                    );
                }
                if *closed && pts.len() > 2 {
                    let (a, b) = (pts.first().unwrap(), pts.last().unwrap());
                    segment(
                        &mut img,
                        a.x * scale,
                        a.y * scale,
                        b.x * scale,
                        b.y * scale,
                        *w * scale,
                        px(*color),
                    );
                }
            }
            Prim::Text { p, size, text, color, anchor, bold, angle } => {
                if angle.abs() > 1.0 {
                    continue;
                }
                draw_text(
                    &mut img,
                    text,
                    p.x * scale,
                    p.y * scale,
                    *size * scale,
                    *bold,
                    *anchor,
                    px(*color),
                );
            }
        }
    }
    img
}

pub fn png_from_scene(scene: &Scene, out: &Path, width_px: u32) -> Result<(), String> {
    image_from_scene(scene, width_px)
        .save(out)
        .map_err(|e| format!("{}: {e}", out.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Pt, Rect};
    use crate::scene::Rgba;

    #[test]
    fn a_scene_becomes_a_png() {
        let dir = std::env::temp_dir().join(format!("volta-png-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("test.png");
        let scene = Scene {
            prims: vec![
                Prim::Rect {
                    r: Rect::new(0.0, 0.0, 10.0, 8.0),
                    radius: 0.0,
                    fill: Rgba::rgb(10, 20, 30),
                    stroke: None,
                },
                Prim::Text {
                    p: Pt::new(5.0, 5.0),
                    size: 1.5,
                    text: "volta".into(),
                    color: Rgba::rgb(255, 255, 255),
                    anchor: Anchor::Center,
                    bold: false,
                    angle: 0.0,
                },
            ],
            owners: vec![],
            bg: Rgba::rgb(240, 240, 240),
            w: 10.0,
            h: 8.0,
            origin: Pt::new(0.0, 0.0),
        };
        png_from_scene(&scene, &out, 400).expect("writes a png");
        let data = std::fs::read(&out).expect("png exists");
        assert!(data.starts_with(&[0x89, b'P', b'N', b'G']));
        assert!(data.len() > 100);
        let _ = std::fs::remove_file(out);
    }

    /// Primitives already sit at the scene origin, so a non-zero origin must
    /// not shift them again: that used to crop the right and bottom edges.
    #[test]
    fn a_non_zero_origin_does_not_shift_the_picture() {
        let scene = Scene {
            prims: vec![Prim::Rect {
                r: Rect::new(1.0, 1.0, 2.0, 2.0),
                radius: 0.0,
                fill: Rgba::rgb(200, 10, 20),
                stroke: None,
            }],
            owners: vec![],
            bg: Rgba::rgb(240, 240, 240),
            w: 10.0,
            h: 10.0,
            origin: Pt::new(100.0, -50.0),
        };
        let img = image_from_scene(&scene, 100);
        assert_eq!(*img.get_pixel(15, 15), px(Rgba::rgb(200, 10, 20)));
        assert_eq!(*img.get_pixel(95, 95), px(Rgba::rgb(240, 240, 240)));
    }
}
