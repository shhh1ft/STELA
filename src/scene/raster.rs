use crate::scene::albedo::AlbedoCache;
use crate::scene::mesh::TriangleMesh;
use egui::{Color32, ColorImage};
use glam::{Mat4, Vec3, Vec4Swizzles};

/// Software framebuffer with a real Z-buffer — fixes see-through / wrong overlaps.
pub struct FrameBuffer {
    pub width: usize,
    pub height: usize,
    color: Vec<Color32>,
    depth: Vec<f32>,
}

impl FrameBuffer {
    pub fn new(width: usize, height: usize, clear: Color32) -> Self {
        let n = width.max(1) * height.max(1);
        Self {
            width: width.max(1),
            height: height.max(1),
            color: vec![clear; n],
            depth: vec![f32::INFINITY; n],
        }
    }

    pub fn to_color_image(&self) -> ColorImage {
        ColorImage {
            size: [self.width, self.height],
            pixels: self.color.clone(),
        }
    }

    /// 2×2 box filter downsample for SSAA.
    pub fn downsample_2x(&self) -> ColorImage {
        let w = self.width / 2;
        let h = self.height / 2;
        let mut pixels = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let mut r = 0u32;
                let mut g = 0u32;
                let mut b = 0u32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let c = self.color[(y * 2 + dy) * self.width + (x * 2 + dx)];
                        r += c.r() as u32;
                        g += c.g() as u32;
                        b += c.b() as u32;
                    }
                }
                pixels.push(Color32::from_rgb(
                    (r / 4) as u8,
                    (g / 4) as u8,
                    (b / 4) as u8,
                ));
            }
        }
        ColorImage { size: [w, h], pixels }
    }

    #[inline]
    fn put(&mut self, x: i32, y: i32, z: f32, c: Color32) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let idx = y as usize * self.width + x as usize;
        if z < self.depth[idx] {
            self.depth[idx] = z;
            self.color[idx] = c;
        }
    }
}

#[derive(Clone, Copy)]
struct Vert {
    x: f32,
    y: f32,
    z: f32,
    r: f32,
    g: f32,
    b: f32,
    u: f32,
    v: f32,
}

fn project_vert(
    vp: Mat4,
    w: f32,
    h: f32,
    p: Vec3,
    shade: Vec3,
    uv: [f32; 2],
) -> Option<Vert> {
    let clip = vp * p.extend(1.0);
    if clip.w <= 1e-5 {
        return None;
    }
    let ndc = clip.xyz() / clip.w;
    if ndc.z < -1.2 || ndc.z > 1.2 {
        return None;
    }
    Some(Vert {
        x: (ndc.x * 0.5 + 0.5) * w,
        y: (1.0 - (ndc.y * 0.5 + 0.5)) * h,
        z: ndc.z,
        r: shade.x,
        g: shade.y,
        b: shade.z,
        u: uv[0],
        v: uv[1],
    })
}

fn shade_normal(n: Vec3, light: Vec3, fill: Vec3, base: [f32; 3], ambient: f32) -> Vec3 {
    let ndl = n.dot(light).max(0.0);
    let fill_l = n.dot(fill).max(0.0) * 0.35;
    let s = (ambient + (1.0 - ambient) * (ndl + fill_l).min(1.0)).clamp(0.12, 1.0);
    // Soft specular for polished stone / painted metal
    let half = (light + Vec3::new(0.0, 0.4, 0.9).normalize()).normalize();
    let spec = n.dot(half).max(0.0).powf(48.0) * 0.18;
    Vec3::new(
        (base[0] * s + spec).min(1.0),
        (base[1] * s + spec).min(1.0),
        (base[2] * s + spec).min(1.0),
    )
}

/// Lighting scale only (texture multiplies later).
fn shade_factor(n: Vec3, light: Vec3, fill: Vec3, ambient: f32) -> f32 {
    let ndl = n.dot(light).max(0.0);
    let fill_l = n.dot(fill).max(0.0) * 0.35;
    (ambient + (1.0 - ambient) * (ndl + fill_l).min(1.0)).clamp(0.12, 1.0)
}

#[inline]
fn edge(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32) -> f32 {
    (cx - ax) * (by - ay) - (cy - ay) * (bx - ax)
}

fn fill_triangle(fb: &mut FrameBuffer, v0: Vert, v1: Vert, v2: Vert, albedo: Option<&image::RgbaImage>) {
    let min_x = v0.x.min(v1.x).min(v2.x).floor().max(0.0) as i32;
    let max_x = v0
        .x
        .max(v1.x)
        .max(v2.x)
        .ceil()
        .min((fb.width - 1) as f32) as i32;
    let min_y = v0.y.min(v1.y).min(v2.y).floor().max(0.0) as i32;
    let max_y = v0
        .y
        .max(v1.y)
        .max(v2.y)
        .ceil()
        .min((fb.height - 1) as f32) as i32;

    let area = edge(v0.x, v0.y, v1.x, v1.y, v2.x, v2.y);
    if area.abs() < 1e-5 {
        return;
    }
    let inv_area = 1.0 / area;

    for y in min_y..=max_y {
        let py = y as f32 + 0.5;
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let w0 = edge(v1.x, v1.y, v2.x, v2.y, px, py) * inv_area;
            let w1 = edge(v2.x, v2.y, v0.x, v0.y, px, py) * inv_area;
            let w2 = edge(v0.x, v0.y, v1.x, v1.y, px, py) * inv_area;
            if w0 < -1e-5 || w1 < -1e-5 || w2 < -1e-5 {
                continue;
            }
            let z = w0 * v0.z + w1 * v1.z + w2 * v2.z;
            let mut r = (w0 * v0.r + w1 * v1.r + w2 * v2.r).clamp(0.0, 1.0);
            let mut g = (w0 * v0.g + w1 * v1.g + w2 * v2.g).clamp(0.0, 1.0);
            let mut b = (w0 * v0.b + w1 * v1.b + w2 * v2.b).clamp(0.0, 1.0);
            if let Some(img) = albedo {
                let u = w0 * v0.u + w1 * v1.u + w2 * v2.u;
                let v = w0 * v0.v + w1 * v1.v + w2 * v2.v;
                let tex = AlbedoCache::sample(img, u, v);
                // v0/v1/v2.r hold lighting scale when textured
                let s = (w0 * v0.r + w1 * v1.r + w2 * v2.r).clamp(0.12, 1.0);
                r = (tex[0] * s).min(1.0);
                g = (tex[1] * s).min(1.0);
                b = (tex[2] * s).min(1.0);
            }
            fb.put(
                x,
                y,
                z,
                Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8),
            );
        }
    }
}

pub struct RasterStats {
    pub triangles: usize,
    pub drawn: usize,
}

/// Rasterize opaque scene. `ssaa2` renders at 2× and downsamples (smoother edges).
/// `plot_w_m` / `plot_l_m` — размер участка в метрах (ширина X, длина Z).
pub fn rasterize_scene(
    meshes: &[TriangleMesh],
    vp: Mat4,
    eye: Vec3,
    width: usize,
    height: usize,
    ground: bool,
    ssaa2: bool,
    plot_w_m: f32,
    plot_l_m: f32,
    plot_texture: Option<&str>,
) -> (ColorImage, RasterStats) {
    let (rw, rh) = if ssaa2 {
        (width * 2, height * 2)
    } else {
        (width, height)
    };

    let clear = Color32::from_rgb(28, 31, 36);
    let mut fb = FrameBuffer::new(rw, rh, clear);
    let w = rw as f32;
    let h = rh as f32;
    let light = Vec3::new(0.45, 0.92, 0.35).normalize();
    let fill = Vec3::new(-0.35, 0.2, -0.55).normalize();

    if ground {
        let ground_img = plot_texture.and_then(|p| image::open(p).ok().map(|i| i.to_rgba8()));
        rasterize_ground(
            &mut fb,
            vp,
            w,
            h,
            light,
            fill,
            plot_w_m,
            plot_l_m,
            ground_img.as_ref(),
        );
    }

    let mut stats = RasterStats {
        triangles: 0,
        drawn: 0,
    };

    let mut albedo_cache = AlbedoCache::default();

    for mesh in meshes {
        let has_normals = mesh.normals.len() == mesh.positions.len();
        let has_vcol = mesh
            .vertex_colors
            .as_ref()
            .map(|c| c.len() == mesh.positions.len())
            .unwrap_or(false);
        let has_uv = mesh
            .uvs
            .as_ref()
            .map(|u| u.len() == mesh.positions.len())
            .unwrap_or(false);
        let tex_path = mesh.texture_path.clone();
        if let Some(ref p) = tex_path {
            let _ = albedo_cache.get(p);
        }

        for tri in mesh.indices.chunks_exact(3) {
            stats.triangles += 1;
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;
            if i0 >= mesh.positions.len()
                || i1 >= mesh.positions.len()
                || i2 >= mesh.positions.len()
            {
                continue;
            }
            let p0 = mesh.positions[i0];
            let p1 = mesh.positions[i1];
            let p2 = mesh.positions[i2];

            let mut face_n = (p1 - p0).cross(p2 - p0);
            if face_n.length_squared() < 1e-16 {
                continue;
            }
            face_n = face_n.normalize();
            let center = (p0 + p1 + p2) / 3.0;
            let view = (eye - center).normalize_or_zero();
            let mut facing = face_n.dot(view);

            if facing <= 0.0 {
                if mesh.double_sided {
                    face_n = -face_n;
                    facing = -facing;
                } else {
                    continue;
                }
            }

            let align = |n: Vec3| {
                if n.dot(face_n) < 0.0 {
                    -n
                } else {
                    n
                }
            };

            let n0 = if has_normals {
                align(mesh.normals[i0]).normalize_or_zero()
            } else {
                face_n
            };
            let n1 = if has_normals {
                align(mesh.normals[i1]).normalize_or_zero()
            } else {
                face_n
            };
            let n2 = if has_normals {
                align(mesh.normals[i2]).normalize_or_zero()
            } else {
                face_n
            };

            let uv0 = if has_uv {
                mesh.uvs.as_ref().unwrap()[i0]
            } else {
                [0.0, 0.0]
            };
            let uv1 = if has_uv {
                mesh.uvs.as_ref().unwrap()[i1]
            } else {
                [0.0, 0.0]
            };
            let uv2 = if has_uv {
                mesh.uvs.as_ref().unwrap()[i2]
            } else {
                [0.0, 0.0]
            };

            let use_tex = tex_path.is_some() && has_uv;

            let (s0, s1, s2) = if mesh.flat_shade {
                // Engraving: floor and walls share one flat color (no lighting split)
                let c = mesh.color;
                let v = Vec3::new(c[0], c[1], c[2]);
                (v, v, v)
            } else if use_tex {
                let f0 = shade_factor(n0, light, fill, 0.30);
                let f1 = shade_factor(n1, light, fill, 0.30);
                let f2 = shade_factor(n2, light, fill, 0.30);
                (Vec3::splat(f0), Vec3::splat(f1), Vec3::splat(f2))
            } else {
                let c0 = if has_vcol {
                    mesh.vertex_colors.as_ref().unwrap()[i0]
                } else {
                    mesh.color
                };
                let c1 = if has_vcol {
                    mesh.vertex_colors.as_ref().unwrap()[i1]
                } else {
                    mesh.color
                };
                let c2 = if has_vcol {
                    mesh.vertex_colors.as_ref().unwrap()[i2]
                } else {
                    mesh.color
                };
                (
                    shade_normal(n0, light, fill, c0, 0.30),
                    shade_normal(n1, light, fill, c1, 0.30),
                    shade_normal(n2, light, fill, c2, 0.30),
                )
            };

            let Some(v0) = project_vert(vp, w, h, p0, s0, uv0) else {
                continue;
            };
            let Some(v1) = project_vert(vp, w, h, p1, s1, uv1) else {
                continue;
            };
            let Some(v2) = project_vert(vp, w, h, p2, s2, uv2) else {
                continue;
            };

            let albedo = if use_tex {
                tex_path
                    .as_deref()
                    .and_then(|p| albedo_cache.get(p))
            } else {
                None
            };
            fill_triangle(&mut fb, v0, v1, v2, albedo);
            stats.drawn += 1;
        }
    }

    let image = if ssaa2 {
        fb.downsample_2x()
    } else {
        fb.to_color_image()
    };
    (image, stats)
}

fn rasterize_ground(
    fb: &mut FrameBuffer,
    vp: Mat4,
    w: f32,
    h: f32,
    light: Vec3,
    fill: Vec3,
    plot_w_m: f32,
    plot_l_m: f32,
    albedo: Option<&image::RgbaImage>,
) {
    use crate::scene::clip::DEFAULT_TEXTURE_TILE_M;

    let n = Vec3::Y;
    let half_x = (plot_w_m * 0.5).max(0.25);
    let half_z = (plot_l_m * 0.5).max(0.25);
    let tile = DEFAULT_TEXTURE_TILE_M;

    let corners = [
        Vec3::new(-half_x, 0.0, -half_z),
        Vec3::new(half_x, 0.0, -half_z),
        Vec3::new(half_x, 0.0, half_z),
        Vec3::new(-half_x, 0.0, half_z),
    ];
    let uvs = [
        [-half_x / tile, -half_z / tile],
        [half_x / tile, -half_z / tile],
        [half_x / tile, half_z / tile],
        [-half_x / tile, half_z / tile],
    ];

    let verts: Vec<Option<Vert>> = if albedo.is_some() {
        let s = shade_factor(n, light, fill, 0.50);
        let shade = Vec3::splat(s);
        corners
            .iter()
            .zip(uvs.iter())
            .map(|(p, uv)| project_vert(vp, w, h, *p, shade, *uv))
            .collect()
    } else {
        let col = [0.22, 0.28, 0.24];
        let s = shade_normal(n, light, fill, col, 0.50);
        corners
            .iter()
            .map(|p| project_vert(vp, w, h, *p, s, [0.0, 0.0]))
            .collect()
    };
    if let (Some(a), Some(b), Some(c), Some(d)) = (verts[0], verts[1], verts[2], verts[3]) {
        fill_triangle(fb, a, b, c, albedo);
        fill_triangle(fb, a, c, d, albedo);
    }

    // Border of the plot
    let border = shade_normal(n, light, fill, [0.55, 0.50, 0.35], 0.6);
    let bl = Vec3::new(-half_x, 0.002, -half_z);
    let br = Vec3::new(half_x, 0.002, -half_z);
    let tr = Vec3::new(half_x, 0.002, half_z);
    let tl = Vec3::new(-half_x, 0.002, half_z);
    stroke_ground_line(fb, vp, w, h, bl, br, border);
    stroke_ground_line(fb, vp, w, h, br, tr, border);
    stroke_ground_line(fb, vp, w, h, tr, tl, border);
    stroke_ground_line(fb, vp, w, h, tl, bl, border);

    // Grid inside plot (0.5 m step)
    let line_col = shade_normal(n, light, fill, [0.34, 0.40, 0.36], 0.55);
    let axis_col = shade_normal(n, light, fill, [0.48, 0.52, 0.42], 0.55);
    let step = 0.5_f32;
    let nx = (half_x / step).ceil() as i32;
    let nz = (half_z / step).ceil() as i32;
    for i in -nx..=nx {
        let t = i as f32 * step;
        if t.abs() > half_x + 0.01 {
            continue;
        }
        let c = if i == 0 { axis_col } else { line_col };
        stroke_ground_line(
            fb,
            vp,
            w,
            h,
            Vec3::new(t, 0.001, -half_z),
            Vec3::new(t, 0.001, half_z),
            c,
        );
    }
    for i in -nz..=nz {
        let t = i as f32 * step;
        if t.abs() > half_z + 0.01 {
            continue;
        }
        let c = if i == 0 { axis_col } else { line_col };
        stroke_ground_line(
            fb,
            vp,
            w,
            h,
            Vec3::new(-half_x, 0.001, t),
            Vec3::new(half_x, 0.001, t),
            c,
        );
    }
}

fn stroke_ground_line(
    fb: &mut FrameBuffer,
    vp: Mat4,
    w: f32,
    h: f32,
    a: Vec3,
    b: Vec3,
    shade: Vec3,
) {
    let Some(va) = project_vert(vp, w, h, a, shade, [0.0, 0.0]) else {
        return;
    };
    let Some(vb) = project_vert(vp, w, h, b, shade, [0.0, 0.0]) else {
        return;
    };
    let steps = ((va.x - vb.x).hypot(va.y - vb.y)).ceil().max(1.0) as i32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (va.x + (vb.x - va.x) * t).round() as i32;
        let y = (va.y + (vb.y - va.y) * t).round() as i32;
        let z = va.z + (vb.z - va.z) * t;
        let c = Color32::from_rgb(
            (shade.x * 255.0) as u8,
            (shade.y * 255.0) as u8,
            (shade.z * 255.0) as u8,
        );
        fb.put(x, y, z - 0.0001, c);
        fb.put(x + 1, y, z - 0.0001, c);
    }
}
