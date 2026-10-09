use crate::model::{
    new_id, CatalogBackground, CatalogFont, CatalogModel, CatalogTexture, MedallionShape, PartRole,
    TextureCategory,
};
use crate::scene::import::load_model;
use crate::scene::mesh::TriangleMesh;
use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LibraryStore {
    pub items: Vec<CatalogModel>,
    #[serde(default)]
    pub textures: Vec<CatalogTexture>,
    #[serde(default)]
    pub texture_categories: Vec<TextureCategory>,
    #[serde(default)]
    pub fonts: Vec<CatalogFont>,
    /// Фоны медальонов: отдельный набор на каждую форму.
    #[serde(default)]
    pub backgrounds: Vec<CatalogBackground>,
}

pub fn library_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("stela").join("library");
    let _ = fs::create_dir_all(&dir);
    let _ = fs::create_dir_all(dir.join("models"));
    let _ = fs::create_dir_all(dir.join("previews"));
    let _ = fs::create_dir_all(dir.join("textures"));
    let _ = fs::create_dir_all(dir.join("texture_previews"));
    let _ = fs::create_dir_all(dir.join("fonts"));
    let _ = fs::create_dir_all(dir.join("backgrounds"));
    let _ = fs::create_dir_all(dir.join("background_previews"));
    let _ = fs::create_dir_all(dir.join("cache"));
    dir
}

fn catalog_path() -> PathBuf {
    library_dir().join("catalog.json")
}

pub fn load_library() -> LibraryStore {
    let path = catalog_path();
    let mut store = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => LibraryStore::default(),
    };
    seed_system_fonts(&mut store);
    store
}

/// Copy common system fonts into the library once if empty.
fn seed_system_fonts(store: &mut LibraryStore) {
    if !store.fonts.is_empty() {
        return;
    }
    let seeds: &[(&str, &str)] = &[
        (
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "DejaVu Sans",
        ),
        (
            "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
            "DejaVu Serif",
        ),
        (
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "Liberation Sans",
        ),
        (
            "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
            "Liberation Serif",
        ),
        ("C:\\Windows\\Fonts\\arial.ttf", "Arial"),
        ("C:\\Windows\\Fonts\\times.ttf", "Times New Roman"),
        ("C:\\Windows\\Fonts\\cour.ttf", "Courier New"),
    ];
    let mut added = false;
    for (path, name) in seeds {
        let p = Path::new(path);
        if !p.exists() {
            continue;
        }
        if let Ok(item) = add_font_from_file(store, p, (*name).to_string()) {
            let _ = item;
            added = true;
        }
    }
    if added {
        save_library(store);
    }
}

pub fn save_library(store: &LibraryStore) {
    let path = catalog_path();
    if let Ok(text) = serde_json::to_string_pretty(store) {
        let _ = fs::write(path, text);
    }
}

/// Copy a model into the library folder and register it.
pub fn add_model_from_file(
    store: &mut LibraryStore,
    source: &Path,
    name: String,
    role: PartRole,
) -> Result<CatalogModel, String> {
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("obj")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "obj" | "gltf" | "glb") {
        return Err("Нужен файл .obj / .gltf / .glb".into());
    }

    let id = new_id();
    let dest_name = format!("{id}.{ext}");
    let dest = library_dir().join("models").join(&dest_name);
    fs::copy(source, &dest).map_err(|e| format!("Копирование: {e}"))?;

    // Also copy sidecar .mtl if present (OBJ materials — optional)
    if ext == "obj" {
        let mtl = source.with_extension("mtl");
        if mtl.exists() {
            let _ = fs::copy(&mtl, dest.with_extension("mtl"));
        }
    }

    // Generate preview PNG
    let preview_path = library_dir().join("previews").join(format!("{id}.png"));
    let _ = generate_preview(&dest, &preview_path);

    let item = CatalogModel {
        id: id.clone(),
        name,
        role,
        file_path: dest.to_string_lossy().into_owned(),
        preview_path: Some(preview_path.to_string_lossy().into_owned()),
    };
    store.items.insert(0, item.clone());
    save_library(store);
    Ok(item)
}

pub fn remove_model(store: &mut LibraryStore, id: &str) -> Result<(), String> {
    let Some(item) = store.items.iter().find(|i| i.id == id).cloned() else {
        return Err("Модель не найдена".into());
    };
    store.items.retain(|i| i.id != id);
    save_library(store);
    let _ = fs::remove_file(&item.file_path);
    if let Some(p) = item.preview_path {
        let _ = fs::remove_file(p);
    }
    // MTL sidecar
    let mtl = Path::new(&item.file_path).with_extension("mtl");
    let _ = fs::remove_file(mtl);
    Ok(())
}

pub fn add_texture_category(store: &mut LibraryStore, name: String) -> TextureCategory {
    let cat = TextureCategory::new(name);
    store.texture_categories.push(cat.clone());
    save_library(store);
    cat
}

pub fn remove_texture_category(store: &mut LibraryStore, id: &str) -> Result<(), String> {
    if !store.texture_categories.iter().any(|c| c.id == id) {
        return Err("Категория не найдена".into());
    }
    store.texture_categories.retain(|c| c.id != id);
    for t in &mut store.textures {
        if t.category_id.as_deref() == Some(id) {
            t.category_id = None;
        }
    }
    save_library(store);
    Ok(())
}

/// Copy an image into the texture library.
pub fn add_texture_from_file(
    store: &mut LibraryStore,
    source: &Path,
    name: String,
    category_id: Option<String>,
) -> Result<CatalogTexture, String> {
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tga") {
        return Err("Нужен файл изображения (.png / .jpg / .webp…)".into());
    }

    let id = new_id();
    let dest_name = format!("{id}.{ext}");
    let dest = library_dir().join("textures").join(&dest_name);
    fs::copy(source, &dest).map_err(|e| format!("Копирование: {e}"))?;

    let preview_path = library_dir()
        .join("texture_previews")
        .join(format!("{id}.png"));
    let _ = generate_texture_preview(&dest, &preview_path);

    let item = CatalogTexture {
        id: id.clone(),
        name,
        category_id,
        file_path: dest.to_string_lossy().into_owned(),
        preview_path: Some(preview_path.to_string_lossy().into_owned()),
    };
    store.textures.insert(0, item.clone());
    save_library(store);
    Ok(item)
}

pub fn remove_texture(store: &mut LibraryStore, id: &str) -> Result<(), String> {
    let Some(item) = store.textures.iter().find(|i| i.id == id).cloned() else {
        return Err("Текстура не найдена".into());
    };
    store.textures.retain(|i| i.id != id);
    save_library(store);
    let _ = fs::remove_file(&item.file_path);
    if let Some(p) = item.preview_path {
        let _ = fs::remove_file(p);
    }
    Ok(())
}

pub fn add_font_from_file(
    store: &mut LibraryStore,
    source: &Path,
    name: String,
) -> Result<CatalogFont, String> {
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("ttf")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "ttf" | "otf") {
        return Err("Нужен файл шрифта .ttf / .otf".into());
    }
    // Validate parse
    let bytes = fs::read(source).map_err(|e| format!("Чтение шрифта: {e}"))?;
    ttf_parser::Face::parse(&bytes, 0).map_err(|e| format!("Шрифт повреждён: {e:?}"))?;

    let id = new_id();
    let dest_name = format!("{id}.{ext}");
    let dest = library_dir().join("fonts").join(&dest_name);
    fs::copy(source, &dest).map_err(|e| format!("Копирование: {e}"))?;

    let item = CatalogFont {
        id,
        name,
        file_path: dest.to_string_lossy().into_owned(),
    };
    store.fonts.insert(0, item.clone());
    save_library(store);
    Ok(item)
}

pub fn add_background_from_file(
    store: &mut LibraryStore,
    source: &Path,
    name: String,
    shape: MedallionShape,
) -> Result<CatalogBackground, String> {
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp") {
        return Err("Нужно изображение .png / .jpg / .webp".into());
    }
    image::open(source).map_err(|e| format!("Изображение: {e}"))?;

    let id = new_id();
    let dest_name = format!("{id}.{ext}");
    let dest = library_dir().join("backgrounds").join(&dest_name);
    fs::copy(source, &dest).map_err(|e| format!("Копирование: {e}"))?;

    let preview_path = library_dir()
        .join("background_previews")
        .join(format!("{id}.png"));
    let _ = generate_texture_preview(&dest, &preview_path);

    let item = CatalogBackground {
        id: id.clone(),
        name,
        shape,
        file_path: dest.to_string_lossy().into_owned(),
        preview_path: Some(preview_path.to_string_lossy().into_owned()),
    };
    store.backgrounds.insert(0, item.clone());
    save_library(store);
    Ok(item)
}

pub fn remove_background(store: &mut LibraryStore, id: &str) -> Result<(), String> {
    let Some(item) = store.backgrounds.iter().find(|i| i.id == id).cloned() else {
        return Err("Фон не найден".into());
    };
    store.backgrounds.retain(|i| i.id != id);
    save_library(store);
    let _ = fs::remove_file(&item.file_path);
    if let Some(p) = item.preview_path {
        let _ = fs::remove_file(p);
    }
    Ok(())
}

pub fn remove_font(store: &mut LibraryStore, id: &str) -> Result<(), String> {
    let Some(item) = store.fonts.iter().find(|i| i.id == id).cloned() else {
        return Err("Шрифт не найден".into());
    };
    store.fonts.retain(|i| i.id != id);
    save_library(store);
    let _ = fs::remove_file(&item.file_path);
    Ok(())
}

fn generate_preview(model_path: &Path, out_png: &Path) -> Result<(), String> {
    let meshes = load_model(model_path)?;
    let img = render_preview_image(&meshes, 160, 160);
    img.save(out_png).map_err(|e| e.to_string())?;
    Ok(())
}

fn generate_texture_preview(source: &Path, out_png: &Path) -> Result<(), String> {
    let img = image::open(source).map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(160, 160);
    thumb
        .to_rgba8()
        .save(out_png)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn render_preview_image(meshes: &[TriangleMesh], w: u32, h: u32) -> image::RgbaImage {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for m in meshes {
        for p in &m.positions {
            min = min.min(*p);
            max = max.max(*p);
        }
    }
    if !min.is_finite() {
        return image::RgbaImage::from_pixel(w, h, image::Rgba([40, 44, 52, 255]));
    }
    let center = (min + max) * 0.5;
    let extent = (max - min).max(Vec3::splat(1e-3));
    let scale = 1.6 / extent.max_element();

    // Simple orthographic-ish projection from front-right
    let mut zbuf = vec![f32::INFINITY; (w * h) as usize];
    let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([36, 40, 48, 255]));
    let light = Vec3::new(0.4, 0.85, 0.35).normalize();

    for mesh in meshes {
        for tri in mesh.indices.chunks_exact(3) {
            let pts = [
                mesh.positions[tri[0] as usize],
                mesh.positions[tri[1] as usize],
                mesh.positions[tri[2] as usize],
            ];
            let local: [Vec3; 3] = pts.map(|p| (p - center) * scale);
            let n = (local[1] - local[0])
                .cross(local[2] - local[0])
                .normalize_or_zero();
            let mut nn = n;
            if nn.z < 0.0 {
                nn = -nn;
            }
            let shade = (0.25 + 0.75 * nn.dot(light).max(0.0)).clamp(0.15, 1.0);
            let col = [
                (mesh.color[0] * shade * 255.0) as u8,
                (mesh.color[1] * shade * 255.0) as u8,
                (mesh.color[2] * shade * 255.0) as u8,
            ];

            let screen: [(f32, f32, f32); 3] = local.map(|p| {
                let x = (p.x * 0.85 + p.z * 0.25) * 0.45 + 0.5;
                let y = (-p.y * 0.9 + 0.05) * 0.45 + 0.5;
                let z = p.z * 0.3 + p.y * 0.1;
                (x * w as f32, y * h as f32, z)
            });
            fill_triangle(&mut img, &mut zbuf, w, h, screen, col);
        }
    }
    img
}

fn fill_triangle(
    img: &mut image::RgbaImage,
    zbuf: &mut [f32],
    w: u32,
    h: u32,
    pts: [(f32, f32, f32); 3],
    color: [u8; 3],
) {
    let min_x = pts[0].0.min(pts[1].0).min(pts[2].0).floor().max(0.0) as i32;
    let max_x = pts[0]
        .0
        .max(pts[1].0)
        .max(pts[2].0)
        .ceil()
        .min(w as f32 - 1.0) as i32;
    let min_y = pts[0].1.min(pts[1].1).min(pts[2].1).floor().max(0.0) as i32;
    let max_y = pts[0]
        .1
        .max(pts[1].1)
        .max(pts[2].1)
        .ceil()
        .min(h as f32 - 1.0) as i32;

    let area = edge(pts[0], pts[1], pts[2]);
    if area.abs() < 1e-4 {
        return;
    }

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = (x as f32 + 0.5, y as f32 + 0.5, 0.0);
            let w0 = edge(pts[1], pts[2], p) / area;
            let w1 = edge(pts[2], pts[0], p) / area;
            let w2 = edge(pts[0], pts[1], p) / area;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let z = w0 * pts[0].2 + w1 * pts[1].2 + w2 * pts[2].2;
            let idx = (y as u32 * w + x as u32) as usize;
            if z < zbuf[idx] {
                zbuf[idx] = z;
                img.put_pixel(
                    x as u32,
                    y as u32,
                    image::Rgba([color[0], color[1], color[2], 255]),
                );
            }
        }
    }
}

fn edge(a: (f32, f32, f32), b: (f32, f32, f32), c: (f32, f32, f32)) -> f32 {
    (c.0 - a.0) * (b.1 - a.1) - (c.1 - a.1) * (b.0 - a.0)
}

#[derive(Default)]
pub struct PreviewCache {
    textures: HashMap<String, TextureHandle>,
}

impl PreviewCache {
    pub fn get(&mut self, ctx: &egui::Context, item: &CatalogModel) -> Option<TextureHandle> {
        if let Some(t) = self.textures.get(&item.id) {
            return Some(t.clone());
        }
        let path = item.preview_path.as_ref()?;
        let dyn_img = image::open(path).ok()?;
        let rgba = dyn_img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels: Vec<Color32> = rgba
            .pixels()
            .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        let tex = ctx.load_texture(
            format!("lib-preview-{}", item.id),
            ColorImage { size, pixels },
            TextureOptions::LINEAR,
        );
        self.textures.insert(item.id.clone(), tex.clone());
        Some(tex)
    }

    pub fn get_texture(
        &mut self,
        ctx: &egui::Context,
        item: &CatalogTexture,
    ) -> Option<TextureHandle> {
        let key = format!("tex-{}", item.id);
        if let Some(t) = self.textures.get(&key) {
            return Some(t.clone());
        }
        let path = item
            .preview_path
            .as_ref()
            .or(Some(&item.file_path))?;
        let dyn_img = image::open(path).ok()?;
        let rgba = dyn_img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels: Vec<Color32> = rgba
            .pixels()
            .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        let tex = ctx.load_texture(
            format!("lib-tex-preview-{}", item.id),
            ColorImage { size, pixels },
            TextureOptions::LINEAR,
        );
        self.textures.insert(key, tex.clone());
        Some(tex)
    }

    pub fn get_background(
        &mut self,
        ctx: &egui::Context,
        item: &CatalogBackground,
    ) -> Option<TextureHandle> {
        let key = format!("bg-{}", item.id);
        if let Some(t) = self.textures.get(&key) {
            return Some(t.clone());
        }
        let path = item
            .preview_path
            .as_ref()
            .or(Some(&item.file_path))?;
        let dyn_img = image::open(path).ok()?;
        let rgba = dyn_img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels: Vec<Color32> = rgba
            .pixels()
            .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        let tex = ctx.load_texture(
            format!("lib-bg-preview-{}", item.id),
            ColorImage { size, pixels },
            TextureOptions::LINEAR,
        );
        self.textures.insert(key, tex.clone());
        Some(tex)
    }

    pub fn invalidate(&mut self, id: &str) {
        self.textures.remove(id);
        self.textures.remove(&format!("tex-{id}"));
        self.textures.remove(&format!("bg-{id}"));
    }
}
