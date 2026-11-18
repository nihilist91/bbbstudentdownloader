//! Minimal PDF writer: one page per image, each page embedding a JPEG (DCTDecode).
//! Replaces Pillow's multi-page save; ffmpeg has no PDF muxer, so the Python fallback never worked.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use image::{ExtendedColorType, ImageEncoder, RgbImage};

use crate::Result;

pub fn write_jpeg_pdf(pages: &[RgbImage], path: &Path, quality: u8) -> Result<()> {
    let n = pages.len();
    // Object layout: 1 = catalog, 2 = pages tree, then per page: page, content, image.
    let total_objects = 2 + 3 * n;
    let mut offsets = vec![0usize; total_objects + 1];
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();

    let mut write_obj = |out: &mut Vec<u8>, num: usize, dict: &str, stream: Option<&[u8]>| {
        offsets[num] = out.len();
        out.extend_from_slice(format!("{num} 0 obj\n{dict}\n").as_bytes());
        if let Some(data) = stream {
            out.extend_from_slice(b"stream\n");
            out.extend_from_slice(data);
            out.extend_from_slice(b"\nendstream\n");
        }
        out.extend_from_slice(b"endobj\n");
    };

    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + 3 * i)).collect();
    write_obj(&mut out, 1, "<< /Type /Catalog /Pages 2 0 R >>", None);
    write_obj(&mut out, 2, &format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), n), None);

    for (i, img) in pages.iter().enumerate() {
        let (w, h) = (img.width(), img.height());
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality)
            .write_image(img.as_raw(), w, h, ExtendedColorType::Rgb8)?;

        let page_num = 3 + 3 * i;
        let content_num = page_num + 1;
        let image_num = page_num + 2;
        let content = format!("q {w} 0 0 {h} 0 0 cm /Im{i} Do Q");

        write_obj(
            &mut out,
            page_num,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Contents {content_num} 0 R \
                 /Resources << /XObject << /Im{i} {image_num} 0 R >> >> >>"
            ),
            None,
        );
        write_obj(&mut out, content_num, &format!("<< /Length {} >>", content.len()), Some(content.as_bytes()));
        write_obj(
            &mut out,
            image_num,
            &format!(
                "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB \
                 /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>",
                jpeg.len()
            ),
            Some(&jpeg),
        );
    }

    let xref_pos = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", total_objects + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", total_objects + 1, xref_pos).as_bytes(),
    );

    let mut file = File::create(path)?;
    file.write_all(&out)?;
    Ok(())
}
