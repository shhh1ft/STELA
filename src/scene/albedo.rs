use image::RgbaImage;
use std::collections::HashMap;
use std::path::Path;

/// CPU albedo images for software rasterization.
#[derive(Default)]
pub struct AlbedoCache {
    images: HashMap<String, RgbaImage>,
}

impl AlbedoCache {
    pub fn get(&mut self, path: &str) -> Option<&RgbaImage> {
        if !self.images.contains_key(path) {
            if let Ok(img) = image::open(Path::new(path)) {
                self.images.insert(path.to_string(), img.to_rgba8());
            } else {
                return None;
            }
        }
        self.images.get(path)
    }

    pub fn sample(img: &RgbaImage, u: f32, v: f32) -> [f32; 3] {
        let w = img.width().max(1);
        let h = img.height().max(1);
        // Repeat wrap
        let mut uf = u.fract();
        let mut vf = v.fract();
        if uf < 0.0 {
            uf += 1.0;
        }
        if vf < 0.0 {
            vf += 1.0;
        }
        let x = ((uf * w as f32) as u32).min(w - 1);
        let y = ((vf * h as f32) as u32).min(h - 1);
        let p = img.get_pixel(x, y).0;
        [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]
    }

}
