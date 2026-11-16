//! SVG output.
//!
//! The export is a real vector file rather than a screen grab, so a 4K version
//! stays crisp. Colours and geometry come from the same scene the window
//! paints, which is what keeps the screenshot honest.

use crate::scene::{Anchor, Prim, Rgba, Scene};

fn col(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

fn opacity(c: Rgba) -> f32 {
    c.3 as f32 / 255.0
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn n(v: f32) -> String {
    let mut s = format!("{v:.2}");
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

pub fn render(s: &Scene, scale: f32) -> String {
    let mut out = String::with_capacity(64 * 1024);
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        n(s.w * scale),
        n(s.h * scale),
        n(s.w),
        n(s.h)
    ));
    out.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{}\" fill-opacity=\"{}\"/>\n",
        n(s.w),
        n(s.h),
        col(s.bg),
        n(opacity(s.bg))
    ));
    for p in &s.prims {
        match p {
            Prim::Rect {
                r,
                radius,
                fill,
                stroke,
            } => {
                out.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" fill-opacity=\"{}\"",
                    n(r.x),
                    n(r.y),
                    n(r.w),
                    n(r.h),
                    n(*radius),
                    col(*fill),
                    n(opacity(*fill))
                ));
                if let Some((c, w)) = stroke {
                    out.push_str(&format!(" stroke=\"{}\" stroke-width=\"{}\"", col(*c), n(*w)));
                }
                out.push_str("/>\n");
            }
            Prim::Circle { c, r, fill, stroke } => {
                out.push_str(&format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" fill-opacity=\"{}\"",
                    n(c.x),
                    n(c.y),
                    n(*r),
                    col(*fill),
                    n(opacity(*fill))
                ));
                if let Some((sc, w)) = stroke {
                    out.push_str(&format!(" stroke=\"{}\" stroke-width=\"{}\"", col(*sc), n(*w)));
                }
                out.push_str("/>\n");
            }
            Prim::Poly { pts, w, color, closed } => {
                let points: Vec<String> = pts.iter().map(|p| format!("{},{}", n(p.x), n(p.y))).collect();
                let tag = if *closed { "polygon" } else { "polyline" };
                out.push_str(&format!(
                    "<{tag} points=\"{}\" fill=\"{}\" fill-opacity=\"{}\" stroke=\"{}\" stroke-opacity=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n",
                    points.join(" "),
                    if *closed { col(*color) } else { "none".into() },
                    if *closed { n(opacity(*color)) } else { "0".into() },
                    col(*color),
                    n(opacity(*color)),
                    n(*w)
                ));
            }
            Prim::Text {
                p,
                size,
                text,
                color,
                anchor,
                bold,
                angle,
            } => {
                let a = match anchor {
                    Anchor::Start => "start",
                    Anchor::Center => "middle",
                    Anchor::End => "end",
                };
                let rot = if *angle == 0.0 {
                    String::new()
                } else {
                    format!(
                        " transform=\"rotate({} {} {})\"",
                        n(*angle),
                        n(p.x),
                        n(p.y)
                    )
                };
                out.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-size=\"{}\" font-family=\"Inter, Cantarell, 'Liberation Sans', 'DejaVu Sans', sans-serif\" font-weight=\"{}\" text-anchor=\"{}\" dominant-baseline=\"central\"{} fill=\"{}\" fill-opacity=\"{}\">{}</text>\n",
                    n(p.x),
                    n(p.y),
                    n(*size),
                    if *bold { 600 } else { 400 },
                    a,
                    rot,
                    col(*color),
                    n(opacity(*color)),
                    esc(text)
                ));
            }
        }
    }
    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Anchor, Prim, Rgba, Scene};
    use crate::model::Rect;
    use crate::model::Pt;

    fn sample() -> Scene {
        Scene {
            prims: vec![
                Prim::Rect { r: Rect::new(1.0, 2.0, 3.0, 4.0), radius: 0.5, fill: Rgba::rgb(1, 2, 3), stroke: None },
                Prim::Circle { c: Pt::new(5.0, 6.0), r: 2.0, fill: Rgba::rgb(9, 9, 9), stroke: Some((Rgba::rgb(0, 0, 0), 0.2)) },
                Prim::Poly { pts: vec![Pt::new(0.0, 0.0), Pt::new(1.0, 1.0)], w: 0.3, color: Rgba::rgb(4, 4, 4), closed: false },
                Prim::Text {
                    p: Pt::new(7.0, 8.0),
                    size: 1.2,
                    text: "a < b & c".into(),
                    color: Rgba::rgb(0, 0, 0),
                    anchor: Anchor::Center,
                    bold: true,
                    angle: 90.0,
                },
            ],
            owners: vec![],
            bg: Rgba::rgb(0xEF, 0xEE, 0xEB),
            w: 10.0,
            h: 12.0,
            origin: Pt::default(),
        }
    }

    #[test]
    fn the_svg_is_well_formed_and_escaped() {
        let out = render(&sample(), 1.0);
        assert!(out.starts_with("<svg "));
        assert!(out.trim_end().ends_with("</svg>"));
        assert!(!out.contains("NaN") && !out.contains("inf"), "numeric output broke");
        assert!(out.contains("a &lt; b &amp; c"), "text was not escaped");
        assert!(out.contains("rotate(90 7 8)"), "rotation missing");
        assert_eq!(out.matches("<text").count(), 1);
        assert_eq!(out.matches("<circle").count(), 1);
        assert_eq!(out.matches("<rect").count(), 2, "background plus the primitive");
    }

    #[test]
    fn scale_only_touches_the_pixel_size() {
        let small = render(&sample(), 1.0);
        let big = render(&sample(), 4.0);
        assert!(big.contains("viewBox=\"0 0 10 12\""));
        assert!(big.contains("width=\"40\""));
        assert_eq!(small.matches("<rect").count(), big.matches("<rect").count());
    }
}
