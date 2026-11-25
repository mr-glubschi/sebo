//! Bilder von fremden Seiten verkleinern und neu kodieren.
//! Originaldateien werden nie gespeichert oder ausgeliefert.

use std::io::Cursor;

use image::{
    DynamicImage, ImageFormat, ImageReader, Limits, codecs::jpeg::JpegEncoder, imageops::FilterType,
};

pub const OG_MAX_WIDTH: u32 = 600;
pub const ICON_SIZE: u32 = 64;
/// Kleinere Vorschaubilder sind meist Zählpixel oder Platzhalter.
const OG_MIN_SIZE: u32 = 50;
const MAX_DIMENSION: u32 = 10_000;
const MAX_ALLOC: u64 = 64 * 1024 * 1024;
/// Größere SVG-Favicons sind ungewöhnlich und teuer zu rendern.
const MAX_SVG_BYTES: usize = 256 * 1024;

fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    if reader.format().is_none() {
        return Err("unbekanntes Bildformat".into());
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    reader.decode().map_err(|error| error.to_string())
}

/// Vorschaubild: höchstens 600 px breit, als JPEG.
pub fn process_og(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = decode(bytes)?;
    if image.width() < OG_MIN_SIZE || image.height() < OG_MIN_SIZE {
        return Err("Bild zu klein".into());
    }
    let image = if image.width() > OG_MAX_WIDTH {
        image.resize(OG_MAX_WIDTH, MAX_DIMENSION, FilterType::Triangle)
    } else {
        image
    };
    let rgb = flatten_on_white(&image);
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 80)
        .encode_image(&rgb)
        .map_err(|error| error.to_string())?;
    Ok(out)
}

/// Favicon: in 64×64 px eingepasst, als PNG. SVG wird gerendert.
pub fn process_icon(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if looks_like_svg(bytes) {
        return render_svg(bytes);
    }
    let image = decode(bytes)?;
    let image = if image.width() > ICON_SIZE || image.height() > ICON_SIZE {
        image.resize(ICON_SIZE, ICON_SIZE, FilterType::Lanczos3)
    } else {
        image
    };
    let mut out = Cursor::new(Vec::new());
    image
        .to_rgba8()
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(out.into_inner())
}

fn flatten_on_white(image: &DynamicImage) -> image::RgbImage {
    let rgba = image.to_rgba8();
    let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
    for (target, source) in rgb.pixels_mut().zip(rgba.pixels()) {
        let alpha = source[3] as u32;
        for channel in 0..3 {
            target[channel] = ((source[channel] as u32 * alpha + 255 * (255 - alpha)) / 255) as u8;
        }
    }
    rgb
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let start = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
    start.contains("<svg")
}

/// SVG ohne externe Ressourcen und ohne Text rendern.
fn render_svg(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use resvg::{tiny_skia, usvg};

    if bytes.len() > MAX_SVG_BYTES {
        return Err("SVG zu groß".into());
    }

    // Keine Dateien und keine eingebetteten Bilder laden.
    let options = usvg::Options {
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|error| error.to_string())?;

    let size = tree.size();
    let scale = ICON_SIZE as f32 / size.width().max(size.height());
    if !scale.is_finite() || scale <= 0.0 {
        return Err("ungültige SVG-Größe".into());
    }
    let mut pixmap = tiny_skia::Pixmap::new(ICON_SIZE, ICON_SIZE).ok_or("Pixmap")?;
    let offset_x = (ICON_SIZE as f32 - size.width() * scale) / 2.0;
    let offset_y = (ICON_SIZE as f32 - size.height() * scale) / 2.0;
    let transform =
        tiny_skia::Transform::from_scale(scale, scale).post_translate(offset_x, offset_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn dimensions(bytes: &[u8]) -> (u32, u32) {
        let image = image::load_from_memory(bytes).unwrap();
        (image.width(), image.height())
    }

    #[test]
    fn vorschaubild_wird_verkleinert() {
        let out = process_og(&png(1200, 630)).unwrap();
        assert_eq!(&out[..2], &[0xFF, 0xD8], "JPEG");
        assert_eq!(dimensions(&out), (600, 315));
        // Kleine Bilder bleiben, winzige werden abgelehnt.
        assert_eq!(dimensions(&process_og(&png(300, 200)).unwrap()), (300, 200));
        assert!(process_og(&png(1, 1)).is_err());
    }

    #[test]
    fn favicon_wird_eingepasst() {
        let out = process_icon(&png(180, 90)).unwrap();
        assert_eq!(&out[..4], b"\x89PNG");
        assert_eq!(dimensions(&out), (64, 32));
        assert_eq!(dimensions(&process_icon(&png(16, 16)).unwrap()), (16, 16));
    }

    #[test]
    fn svg_favicon_wird_gerendert() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
            <rect width="10" height="10" fill="red"/>
            <image href="file:///etc/passwd" width="10" height="10"/></svg>"#;
        let out = process_icon(svg).unwrap();
        assert_eq!(dimensions(&out), (64, 64));
    }

    #[test]
    fn kein_bild() {
        assert!(process_og(b"<html>keine Bilddatei</html>").is_err());
        assert!(process_icon(b"\x00\x01\x02").is_err());
    }
}
