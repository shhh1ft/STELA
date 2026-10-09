//! Compose the medallion face image: background + person cutout + caption.

use crate::library::library_dir;
use crate::model::{PhotoSpec, TextAlign};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use image::{imageops, Rgba, RgbaImage};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

const CANVAS_W: u32 = 768;

/// Build (or reuse cached) composited medallion image; returns PNG path.
pub fn compose_medallion_image(photo_path: &str, spec: &PhotoSpec) -> Result<PathBuf, String> {
    let key = cache_key(photo_path, spec);
    let dir = library_dir().join("cache");
    let _ = std::fs::create_dir_all(&dir);
    let out = dir.join(format!("medallion_{key:016x}.png"));
    if out.exists() {
        return Ok(out);
    }
    let img = render(photo_path, spec)?;
    img.save(&out).map_err(|e| format!("кэш медальона: {e}"))?;
    Ok(out)
}

fn cache_key(photo_path: &str, spec: &PhotoSpec) -> u64 {
    let mut h = DefaultHasher::new();
    photo_path.hash(&mut h);
    if let Ok(meta) = std::fs::metadata(photo_path) {
        if let Ok(m) = meta.modified() {
            if let Ok(d) = m.duration_since(std::time::UNIX_EPOCH) {
                d.as_secs().hash(&mut h);
            }
        }
        meta.len().hash(&mut h);
    }
    if let Ok(s) = serde_json::to_string(spec) {
        s.hash(&mut h);
    }
    // Background file may change
    if let Some(bg) = &spec.background_path {
        if let Ok(meta) = std::fs::metadata(bg) {
            meta.len().hash(&mut h);
        }
    }
    h.finish()
}

fn render(photo_path: &str, spec: &PhotoSpec) -> Result<RgbaImage, String> {
    let aspect = (spec.height_cm / spec.width_cm.max(0.1)).clamp(0.3, 3.0);
    let w = CANVAS_W;
    let h = ((w as f32) * aspect).round().max(64.0) as u32;

    let mut canvas = background_layer(spec, w, h);

    // Portrait
    if Path::new(photo_path).exists() {
        let photo = image::open(photo_path).map_err(|e| format!("фото: {e}"))?.to_rgba8();
        let photo = if spec.remove_background {
            remove_background(&photo, spec.bg_tolerance)
        } else {
            photo
        };
        let photo = autocrop_alpha(&photo);
        // Scale: 1.0 = portrait height equals medallion height
        let target_h = (h as f32 * spec.photo_scale.clamp(0.2, 3.0)).max(8.0);
        let scale = target_h / photo.height().max(1) as f32;
        let target_w = (photo.width() as f32 * scale).max(1.0) as u32;
        let resized = imageops::resize(
            &photo,
            target_w,
            target_h as u32,
            imageops::FilterType::CatmullRom,
        );
        // Anchor: bottom of portrait at bottom of medallion (bust look) + offsets
        let x = (w as f32 - target_w as f32) * 0.5 + spec.photo_dx * w as f32 * 0.5;
        let y = h as f32 - target_h + spec.photo_dy * h as f32 * 0.5;
        imageops::overlay(&mut canvas, &resized, x.round() as i64, y.round() as i64);
    }

    // Caption
    if !spec.text.content.trim().is_empty() {
        draw_caption(&mut canvas, spec, w, h);
    }

    Ok(canvas)
}

fn background_layer(spec: &PhotoSpec, w: u32, h: u32) -> RgbaImage {
    if let Some(bg) = &spec.background_path {
        if let Ok(img) = image::open(bg) {
            let img = img.to_rgba8();
            // Cover-fit
            let sx = w as f32 / img.width().max(1) as f32;
            let sy = h as f32 / img.height().max(1) as f32;
            let s = sx.max(sy);
            let rw = (img.width() as f32 * s).ceil().max(1.0) as u32;
            let rh = (img.height() as f32 * s).ceil().max(1.0) as u32;
            let resized = imageops::resize(&img, rw, rh, imageops::FilterType::Triangle);
            let x = ((rw as i64 - w as i64) / 2).max(0) as u32;
            let y = ((rh as i64 - h as i64) / 2).max(0) as u32;
            let cropped = imageops::crop_imm(&resized, x, y, w.min(rw), h.min(rh)).to_image();
            let mut canvas = RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 255]));
            imageops::overlay(&mut canvas, &cropped, 0, 0);
            return canvas;
        }
    }
    // Default: soft studio gradient (dark top → lighter bottom), classic photoceramic look
    let mut canvas = RgbaImage::new(w, h);
    for y in 0..h {
        let t = y as f32 / h.max(1) as f32;
        let (r, g, b) = match spec.material {
            crate::model::MedallionMaterial::MetalCeramic => (
                (60.0 + 70.0 * t) as u8,
                (62.0 + 70.0 * t) as u8,
                (66.0 + 72.0 * t) as u8,
            ),
            _ => (
                (38.0 + 90.0 * t) as u8,
                (40.0 + 90.0 * t) as u8,
                (46.0 + 92.0 * t) as u8,
            ),
        };
        for x in 0..w {
            canvas.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }
    canvas
}

/// Flood-fill background removal from image borders by color similarity.
pub fn remove_background(img: &RgbaImage, tolerance: f32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w < 4 || h < 4 {
        return img.clone();
    }
    let tol = (tolerance.clamp(0.02, 1.0) * 441.67) as f32; // max RGB distance
    let mut mask = vec![false; (w * h) as usize]; // true = background
    let mut stack: Vec<(u32, u32)> = Vec::new();

    // Border seeds
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }

    // Reference: average border color (robust to gradients by local compare too)
    let mut sum = [0u64; 3];
    let mut n = 0u64;
    for &(x, y) in &stack {
        let p = img.get_pixel(x, y).0;
        sum[0] += p[0] as u64;
        sum[1] += p[1] as u64;
        sum[2] += p[2] as u64;
        n += 1;
    }
    let avg = [
        (sum[0] / n.max(1)) as f32,
        (sum[1] / n.max(1)) as f32,
        (sum[2] / n.max(1)) as f32,
    ];

    let dist = |a: [u8; 4], b: [f32; 3]| -> f32 {
        let dr = a[0] as f32 - b[0];
        let dg = a[1] as f32 - b[1];
        let db = a[2] as f32 - b[2];
        (dr * dr + dg * dg + db * db).sqrt()
    };

    while let Some((x, y)) = stack.pop() {
        let idx = (y * w + x) as usize;
        if mask[idx] {
            continue;
        }
        let p = img.get_pixel(x, y).0;
        if dist(p, avg) > tol {
            continue;
        }
        mask[idx] = true;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }

    // Soft edge: 3×3 alpha blur on the boundary
    let mut out = img.clone();
    for y in 0..h {
        for x in 0..w {
            let mut bg_count = 0u32;
            let mut total = 0u32;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    total += 1;
                    if mask[(ny as u32 * w + nx as u32) as usize] {
                        bg_count += 1;
                    }
                }
            }
            let alpha_keep = 1.0 - bg_count as f32 / total.max(1) as f32;
            let p = out.get_pixel_mut(x, y);
            p.0[3] = (p.0[3] as f32 * alpha_keep) as u8;
        }
    }
    out
}

fn autocrop_alpha(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0;
    let mut max_y = 0;
    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y).0[3] > 24 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if min_x >= max_x || min_y >= max_y {
        return img.clone();
    }
    imageops::crop_imm(img, min_x, min_y, max_x - min_x + 1, max_y - min_y + 1).to_image()
}

fn draw_caption(canvas: &mut RgbaImage, spec: &PhotoSpec, w: u32, h: u32) {
    let font_path = spec
        .text
        .font_path
        .clone()
        .or_else(|| crate::scene::engrave::resolve_font_path(&Default::default()));
    let Some(font_path) = font_path else {
        return;
    };
    let Ok(bytes) = std::fs::read(&font_path) else {
        return;
    };
    let Ok(font) = FontVec::try_from_vec(bytes) else {
        return;
    };

    let px_per_cm = h as f32 / spec.height_cm.max(1.0);
    let letter_px = (spec.text.letter_cm.max(0.3) * px_per_cm).clamp(8.0, h as f32 * 0.3);
    let scale = PxScale::from(letter_px * 1.35);
    let scaled = font.as_scaled(scale);
    let color = [
        (spec.text.color[0] * 255.0) as u8,
        (spec.text.color[1] * 255.0) as u8,
        (spec.text.color[2] * 255.0) as u8,
    ];

    let lines: Vec<&str> = spec.text.content.lines().collect();
    let line_h = scaled.height() * 1.1;
    let total_h = line_h * lines.len() as f32;
    // Caption sits in the lower band with a margin
    let margin = h as f32 * 0.035;
    let mut y = h as f32 - margin - total_h + scaled.ascent();

    for line in lines {
        let line_w: f32 = line
            .chars()
            .map(|c| scaled.h_advance(scaled.glyph_id(c)))
            .sum();
        let x0 = match spec.text.align {
            TextAlign::Left => margin,
            TextAlign::Center => (w as f32 - line_w) * 0.5,
            TextAlign::Right => w as f32 - margin - line_w,
        };
        // Shadow for legibility
        draw_line(canvas, &scaled, line, x0 + 1.5, y + 1.5, [0, 0, 0], 160);
        draw_line(canvas, &scaled, line, x0, y, color, 255);
        y += line_h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn border_flood_keeps_contrasting_center() {
        let mut img = RgbaImage::from_pixel(40, 40, Rgba([210, 210, 208, 255]));
        for y in 12..28 {
            for x in 12..28 {
                img.put_pixel(x, y, Rgba([24, 32, 48, 255]));
            }
        }
        let out = remove_background(&img, 0.15);
        assert!(out.get_pixel(20, 20).0[3] > 200);
        assert!(out.get_pixel(1, 1).0[3] < 20);
    }
}

fn draw_line<F: Font>(
    canvas: &mut RgbaImage,
    scaled: &ab_glyph::PxScaleFont<F>,
    text: &str,
    x0: f32,
    baseline: f32,
    color: [u8; 3],
    max_alpha: u8,
) {
    let (w, h) = canvas.dimensions();
    let mut x = x0;
    for c in text.chars() {
        let gid = scaled.glyph_id(c);
        let glyph = gid.with_scale_and_position(scaled.scale(), ab_glyph::point(x, baseline));
        if let Some(outlined) = scaled.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, cov| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                    return;
                }
                let a = (cov * max_alpha as f32) as u32;
                if a == 0 {
                    return;
                }
                let dst = canvas.get_pixel_mut(px as u32, py as u32);
                let inv = 255 - a;
                for i in 0..3 {
                    dst.0[i] = ((dst.0[i] as u32 * inv + color[i] as u32 * a) / 255) as u8;
                }
                dst.0[3] = 255;
            });
        }
        x += scaled.h_advance(gid);
    }
}
