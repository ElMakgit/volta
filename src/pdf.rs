//! PDF export.
//!
//! A page is the same rasterised scene as the PNG, wrapped in a minimal PDF
//! with a JPEG image XObject. No print driver or external converter is needed.

use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, RgbaImage};

use crate::scene::Scene;

fn jpeg_bytes(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let mut out = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut out, 92);
    enc.encode_image(&rgb)
        .map_err(|e| format!("JPEG: {e}"))?;
    Ok(out)
}

pub fn pdf_from_scene(scene: &Scene, out: &Path, width_px: u32) -> Result<(), String> {
    let img = crate::png::image_from_scene(scene, width_px);
    let jpeg = jpeg_bytes(&img)?;
    let (w, h) = img.dimensions();
    let page_w = 900.0_f32;
    let page_h = page_w * h as f32 / w as f32;

    let stream = format!("q\n{page_w:.2} 0 0 {page_h:.2} 0 0 cm\n/Im0 Do\nQ\n");
    let mut objects: Vec<Vec<u8>> = Vec::new();
    objects.push(format!(
        "<< /Type /Catalog /Pages 2 0 R >>\n"
    ).into_bytes());
    objects.push(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>\n".to_vec());
    objects.push(
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_w:.2} {page_h:.2}] \
             /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>\n"
        )
        .into_bytes(),
    );
    let mut img_obj = format!(
        "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} \
         /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
        jpeg.len()
    )
    .into_bytes();
    img_obj.extend_from_slice(&jpeg);
    img_obj.extend_from_slice(b"\nendstream\n");
    objects.push(img_obj);
    objects.push(
        format!("<< /Length {} >>\nstream\n{}endstream\n", stream.len(), stream).into_bytes(),
    );

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        pdf.extend_from_slice(obj);
        pdf.extend_from_slice(b"endobj\n");
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets {
        pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    std::fs::write(out, pdf).map_err(|e| format!("{}: {e}", out.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Pt, Rect};
    use crate::scene::{Rgba, Scene};

    #[test]
    fn a_scene_becomes_a_pdf() {
        let dir = std::env::temp_dir().join(format!("volta-pdf-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("test.pdf");
        let scene = Scene {
            prims: vec![PrimRect()],
            owners: vec![],
            bg: Rgba::rgb(240, 240, 240),
            w: 10.0,
            h: 6.0,
            origin: Pt::new(0.0, 0.0),
        };
        pdf_from_scene(&scene, &out, 400).expect("writes a pdf");
        let data = std::fs::read(&out).expect("pdf exists");
        assert!(data.starts_with(b"%PDF-1.4"));
        assert!(data.windows(5).any(|w| w == b"%%EOF"));
        let _ = std::fs::remove_file(out);
    }

    fn PrimRect() -> crate::scene::Prim {
        crate::scene::Prim::Rect {
            r: Rect::new(1.0, 1.0, 8.0, 4.0),
            radius: 0.0,
            fill: Rgba::rgb(20, 60, 120),
            stroke: None,
        }
    }
}
