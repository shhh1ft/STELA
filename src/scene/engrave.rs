//! 3D text engraving: font outlines → tessellated glyphs → carve into stele front.

use crate::model::{Decal, DecalKind, TextAlign, TextStyle};
use crate::scene::assemble::SteleFace;
use crate::scene::mesh::TriangleMesh;
use earcutr::earcut;
use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use glam::Vec3;
use std::path::Path;

const FRONT_EPS: f32 = 0.008;

const SYSTEM_FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
    "C:\\Windows\\Fonts\\times.ttf",
];

/// Resolve font file for a text style (library path or system fallback).
pub fn resolve_font_path(style: &TextStyle) -> Option<String> {
    if let Some(p) = &style.font_path {
        if Path::new(p).exists() {
            return Some(p.clone());
        }
    }
    for c in SYSTEM_FONT_CANDIDATES {
        if Path::new(c).exists() {
            return Some((*c).to_string());
        }
    }
    None
}

/// Apply all visible text engravings on stele meshes for one face.
pub fn engrave_stele_texts(
    meshes: &mut Vec<TriangleMesh>,
    face: &SteleFace,
    decals: &[Decal],
    resolve_font: &dyn Fn(&TextStyle) -> Option<String>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    for decal in decals {
        if !decal.visible {
            continue;
        }
        let DecalKind::Text {
            content,
            style,
            font_scale,
        } = &decal.kind
        else {
            continue;
        };
        if content.trim().is_empty() {
            continue;
        }
        let Some(font_path) = resolve_font(style) else {
            warnings.push(format!(
                "Эскиз «{}»: шрифт не найден — гравировка пропущена.",
                decal.name
            ));
            continue;
        };
        match carve_text(
            meshes,
            face,
            decal,
            content,
            style,
            *font_scale,
            Path::new(&font_path),
        ) {
            Ok(()) => {}
            Err(e) => warnings.push(format!("Эскиз «{}»: {e}", decal.name)),
        }
    }
    warnings
}

fn carve_text(
    meshes: &mut Vec<TriangleMesh>,
    face: &SteleFace,
    decal: &Decal,
    content: &str,
    style: &TextStyle,
    legacy_scale: f32,
    font_path: &Path,
) -> Result<(), String> {
    let depth = (style.depth_mm / 1000.0).clamp(0.001, 0.02);
    let letter_m = if style.letter_cm > 0.05 {
        style.letter_cm / 100.0
    } else {
        // legacy: relative to decal band
        (face.size_m.y * decal.height * legacy_scale).max(0.01)
    };

    let glyphs_uv = layout_text_uv(face, decal, content, style, letter_m, font_path)?;
    if glyphs_uv.is_empty() {
        return Err("не удалось построить контуры букв".into());
    }

    let glyph_mp = contours_to_multipolygon(&glyphs_uv);
    if glyph_mp.0.is_empty() {
        return Err("пустые контуры букв".into());
    }

    let color = style.color;
    let (_, _, _, front_n) = face_basis(face);
    let mut cavity = TriangleMesh {
        positions: vec![],
        normals: vec![],
        indices: vec![],
        color,
        vertex_colors: None,
        uvs: None,
        // Floors + walls must stay visible from the front
        double_sided: true,
        texture_path: None,
        flat_shade: true, // same color on floor and walls
    };

    // Solid letter fill (bottom of the engraving) — full glyph shapes, facing the viewer
    append_multipolygon_mesh(
        &glyph_mp,
        face,
        depth,
        front_n,
        color,
        &mut cavity.positions,
        &mut cavity.normals,
        &mut cavity.indices,
        false,
    );
    // Side walls of the carved letters
    add_glyph_walls(face, &glyph_mp, depth, &mut cavity);

    // Cut letter holes out of the stele front surface
    for mesh in meshes.iter_mut() {
        carve_mesh_front(mesh, face, &glyph_mp, depth);
    }

    if !cavity.indices.is_empty() {
        // Keep authored normals (outward floors); only fill gaps
        if cavity.normals.len() != cavity.positions.len() {
            cavity.recompute_normals();
        }
        meshes.push(cavity);
    }
    Ok(())
}

pub(crate) fn face_basis(face: &SteleFace) -> (Vec3, Vec3, Vec3, Vec3) {
    let origin = face.corners[0];
    let right = face.corners[1] - face.corners[0];
    let up = face.corners[3] - face.corners[0];
    let normal = right.cross(up).normalize_or_zero();
    (origin, right, up, normal)
}

fn world_to_uv(face: &SteleFace, p: Vec3) -> [f32; 2] {
    let (origin, right, up, _) = face_basis(face);
    let d = p - origin;
    let rw = right.length().max(1e-6);
    let uh = up.length().max(1e-6);
    let u = d.dot(right / rw) / rw;
    let v = d.dot(up / uh) / uh;
    [u, v]
}

pub(crate) fn uv_to_world(face: &SteleFace, u: f32, v: f32, inset: f32) -> Vec3 {
    let (origin, right, up, normal) = face_basis(face);
    origin + right * u + up * v - normal * inset
}

fn dist_to_front(face: &SteleFace, p: Vec3) -> f32 {
    let (origin, _, _, normal) = face_basis(face);
    (p - origin).dot(normal)
}

/// Layout text into UV-space contours (outer + holes), face UV 0…1.
fn layout_text_uv(
    face: &SteleFace,
    decal: &Decal,
    content: &str,
    style: &TextStyle,
    letter_m: f32,
    font_path: &Path,
) -> Result<Vec<Vec<[f32; 2]>>, String> {
    let font_data = std::fs::read(font_path).map_err(|e| format!("шрифт: {e}"))?;
    let face_ttf =
        ttf_parser::Face::parse(&font_data, 0).map_err(|e| format!("разбор TTF: {e:?}"))?;

    let units = face_ttf.units_per_em() as f32;
    let scale = letter_m / units; // world metres per font unit
    let face_w = face.size_m.x.max(1e-4);
    let face_h = face.size_m.y.max(1e-4);

    // Decal band in UV
    let band_u0 = (decal.u - decal.width * 0.5).clamp(0.0, 1.0);
    let band_u1 = (decal.u + decal.width * 0.5).clamp(0.0, 1.0);
    let band_v0 = (decal.v - decal.height * 0.5).clamp(0.0, 1.0);
    let band_v1 = (decal.v + decal.height * 0.5).clamp(0.0, 1.0);
    let band_w_m = (band_u1 - band_u0) * face_w;
    let band_h_m = (band_v1 - band_v0) * face_h;

    let lines: Vec<&str> = content.lines().collect();
    let line_step_m = letter_m * style.line_gap.max(1.0);
    let total_h = if lines.is_empty() {
        0.0
    } else {
        letter_m + (lines.len() as f32 - 1.0) * line_step_m
    };

    // Fit if text taller than band
    let fit = if total_h > band_h_m * 0.98 && total_h > 1e-6 {
        (band_h_m * 0.98) / total_h
    } else {
        1.0
    };
    let scale = scale * fit;
    let letter_m = letter_m * fit;
    let line_step_m = line_step_m * fit;
    let total_h = total_h * fit;

    let mut all_contours: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut y_top_m = (band_h_m + total_h) * 0.5; // local metres from band bottom

    for line in &lines {
        let line = line.trim_end();
        let (line_contours, line_w_m) =
            build_line_contours(&face_ttf, line, scale, style.bold, style.italic)?;
        let x_off = match style.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => (band_w_m - line_w_m) * 0.5,
            TextAlign::Right => band_w_m - line_w_m,
        };
        let baseline_m = y_top_m - letter_m;

        for contour in line_contours {
            let uv_contour: Vec<[f32; 2]> = contour
                .into_iter()
                .map(|[x, y]| {
                    let lx = x_off + x;
                    let ly = baseline_m + y;
                    [
                        band_u0 + (lx / face_w).clamp(-0.5, 1.5),
                        band_v0 + (ly / face_h).clamp(-0.5, 1.5),
                    ]
                })
                .collect();
            if uv_contour.len() >= 3 {
                all_contours.push(uv_contour);
            }
        }
        y_top_m -= line_step_m;
    }

    Ok(all_contours)
}

pub(crate) fn build_line_contours(
    face: &ttf_parser::Face<'_>,
    text: &str,
    scale: f32,
    bold: bool,
    italic: bool,
) -> Result<(Vec<Vec<[f32; 2]>>, f32), String> {
    let mut contours = Vec::new();
    let mut pen_x = 0.0_f32;
    let italic_shear = if italic { 0.22 } else { 0.0 };
    let bold_inflate = if bold { scale * face.units_per_em() as f32 * 0.03 } else { 0.0 };

    for ch in text.chars() {
        if ch == ' ' {
            let adv = face
                .glyph_hor_advance(face.glyph_index(' ').unwrap_or_default())
                .unwrap_or(face.units_per_em() / 3) as f32
                * scale;
            pen_x += adv;
            continue;
        }
        let Some(gid) = face.glyph_index(ch) else {
            continue;
        };
        let mut builder = OutlineCollector::default();
        let _ = face.outline_glyph(gid, &mut builder);
        let adv = face.glyph_hor_advance(gid).unwrap_or(0) as f32 * scale;

        for mut c in builder.finish() {
            for p in &mut c {
                let mut x = p[0] * scale + pen_x;
                let y = p[1] * scale;
                x += y * italic_shear;
                p[0] = x;
                p[1] = y;
            }
            if bold_inflate > 0.0 {
                c = inflate_contour(&c, bold_inflate);
            }
            if c.len() >= 3 {
                contours.push(c);
            }
        }
        pen_x += adv;
    }
    Ok((contours, pen_x))
}

#[derive(Default)]
struct OutlineCollector {
    contours: Vec<Vec<[f32; 2]>>,
    current: Vec<[f32; 2]>,
}

impl OutlineCollector {
    fn finish(mut self) -> Vec<Vec<[f32; 2]>> {
        if self.current.len() >= 3 {
            self.contours.push(std::mem::take(&mut self.current));
        }
        self.contours
    }
}

impl ttf_parser::OutlineBuilder for OutlineCollector {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.current.len() >= 3 {
            self.contours.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
        self.current.push([x, y]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.current.push([x, y]);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p0 = *self.current.last().unwrap_or(&[0.0, 0.0]);
        // 4-segment quadratic approximation
        for i in 1..=4 {
            let t = i as f32 / 4.0;
            let u = 1.0 - t;
            let px = u * u * p0[0] + 2.0 * u * t * x1 + t * t * x;
            let py = u * u * p0[1] + 2.0 * u * t * y1 + t * t * y;
            self.current.push([px, py]);
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p0 = *self.current.last().unwrap_or(&[0.0, 0.0]);
        for i in 1..=5 {
            let t = i as f32 / 5.0;
            let u = 1.0 - t;
            let px = u * u * u * p0[0]
                + 3.0 * u * u * t * x1
                + 3.0 * u * t * t * x2
                + t * t * t * x;
            let py = u * u * u * p0[1]
                + 3.0 * u * u * t * y1
                + 3.0 * u * t * t * y2
                + t * t * t * y;
            self.current.push([px, py]);
        }
    }

    fn close(&mut self) {
        if let (Some(first), Some(last)) = (self.current.first().copied(), self.current.last().copied())
        {
            if (first[0] - last[0]).abs() > 1e-4 || (first[1] - last[1]).abs() > 1e-4 {
                self.current.push(first);
            }
        }
        if self.current.len() >= 3 {
            self.contours.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }
}

fn inflate_contour(c: &[[f32; 2]], amount: f32) -> Vec<[f32; 2]> {
    let n = c.len();
    if n < 3 {
        return c.to_vec();
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let prev = c[(i + n - 1) % n];
        let cur = c[i];
        let next = c[(i + 1) % n];
        let e1 = [cur[0] - prev[0], cur[1] - prev[1]];
        let e2 = [next[0] - cur[0], next[1] - cur[1]];
        let n1 = {
            let l = (e1[0] * e1[0] + e1[1] * e1[1]).sqrt().max(1e-6);
            [-e1[1] / l, e1[0] / l]
        };
        let n2 = {
            let l = (e2[0] * e2[0] + e2[1] * e2[1]).sqrt().max(1e-6);
            [-e2[1] / l, e2[0] / l]
        };
        let mut nx = n1[0] + n2[0];
        let mut ny = n1[1] + n2[1];
        let nl = (nx * nx + ny * ny).sqrt().max(1e-6);
        nx /= nl;
        ny /= nl;
        out.push([cur[0] + nx * amount, cur[1] + ny * amount]);
    }
    out
}

fn ring_signed_area(ring: &[Coord<f64>]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut a = 0.0;
    for i in 0..ring.len().saturating_sub(1) {
        a += ring[i].x * ring[i + 1].y - ring[i + 1].x * ring[i].y;
    }
    a * 0.5
}

fn point_in_ring(p: Coord<f64>, ring: &[Coord<f64>]) -> bool {
    // Ray casting
    let mut inside = false;
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let pi = ring[i];
        let pj = ring[j];
        if ((pi.y > p.y) != (pj.y > p.y))
            && (p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y + 1e-30) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn contour_to_ring(c: &[[f32; 2]]) -> Option<Vec<Coord<f64>>> {
    if c.len() < 3 {
        return None;
    }
    let mut ring: Vec<Coord<f64>> = c
        .iter()
        .map(|p| Coord {
            x: p[0] as f64,
            y: p[1] as f64,
        })
        .collect();
    if let (Some(first), Some(last)) = (ring.first().copied(), ring.last().copied()) {
        if first != last {
            ring.push(first);
        }
    }
    if ring.len() < 4 {
        return None;
    }
    Some(ring)
}

/// Build polygons with proper holes (counters in О, Ф, 6, б…).
/// TrueType contours: opposite winding for outer vs inner; we nest by containment.
pub(crate) fn contours_to_multipolygon(contours: &[Vec<[f32; 2]>]) -> MultiPolygon<f64> {
    let mut rings: Vec<Vec<Coord<f64>>> = contours.iter().filter_map(|c| contour_to_ring(c)).collect();
    if rings.is_empty() {
        return MultiPolygon(vec![]);
    }

    // Largest area first — outers before their holes
    rings.sort_by(|a, b| {
        ring_signed_area(b)
            .abs()
            .partial_cmp(&ring_signed_area(a).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Pair each ring with whether it's a hole candidate (nested inside another)
    let mut used = vec![false; rings.len()];
    let mut polys: Vec<Polygon<f64>> = Vec::new();

    for i in 0..rings.len() {
        if used[i] {
            continue;
        }
        // This is an exterior (largest remaining / not inside another unused outer)
        let mut exterior = rings[i].clone();
        // geo expects CCW exterior
        if ring_signed_area(&exterior) < 0.0 {
            exterior.reverse();
        }
        used[i] = true;

        let mut holes: Vec<LineString<f64>> = Vec::new();
        let ext_sample = exterior[0];
        for j in (i + 1)..rings.len() {
            if used[j] {
                continue;
            }
            let sample = rings[j][rings[j].len() / 2];
            // Hole if this ring sits inside the exterior
            if point_in_ring(sample, &exterior) && !point_in_ring(ext_sample, &rings[j]) {
                let mut hole = rings[j].clone();
                // holes should be CW (opposite of exterior)
                if ring_signed_area(&hole) > 0.0 {
                    hole.reverse();
                }
                holes.push(LineString(hole));
                used[j] = true;
            }
        }
        polys.push(Polygon::new(LineString(exterior), holes));
    }

    // Merge separate glyphs with union (outers+holes already correct per letter)
    if polys.is_empty() {
        return MultiPolygon(vec![]);
    }
    let mut acc = MultiPolygon(vec![polys[0].clone()]);
    for p in polys.into_iter().skip(1) {
        acc = acc.union(&MultiPolygon(vec![p]));
    }
    acc
}

pub(crate) fn carve_mesh_front(
    mesh: &mut TriangleMesh,
    face: &SteleFace,
    glyphs: &MultiPolygon<f64>,
    _depth: f32,
) {
    let old_pos = std::mem::take(&mut mesh.positions);
    let old_idx = std::mem::take(&mut mesh.indices);
    let old_n = std::mem::take(&mut mesh.normals);
    let old_uv = mesh.uvs.take();
    let old_vc = mesh.vertex_colors.take();
    let has_n = old_n.len() == old_pos.len();
    let has_uv = old_uv
        .as_ref()
        .map(|u| u.len() == old_pos.len())
        .unwrap_or(false);
    let has_vc = old_vc
        .as_ref()
        .map(|c| c.len() == old_pos.len())
        .unwrap_or(false);

    let mut new_pos = Vec::new();
    let mut new_n = Vec::new();
    let mut new_uv: Vec<[f32; 2]> = Vec::new();
    let mut new_vc: Vec<[f32; 3]> = Vec::new();
    let mut new_idx = Vec::new();

    let push = |pos: &mut Vec<Vec3>,
                ns: &mut Vec<Vec3>,
                uvs: &mut Vec<[f32; 2]>,
                vcs: &mut Vec<[f32; 3]>,
                p: Vec3,
                n: Vec3,
                uv: [f32; 2],
                vc: [f32; 3]|
     -> u32 {
        let i = pos.len() as u32;
        pos.push(p);
        ns.push(n);
        if has_uv {
            uvs.push(uv);
        }
        if has_vc {
            vcs.push(vc);
        }
        i
    };

    for tri in old_idx.chunks_exact(3) {
        let ia = tri[0] as usize;
        let ib = tri[1] as usize;
        let ic = tri[2] as usize;
        if ia >= old_pos.len() || ib >= old_pos.len() || ic >= old_pos.len() {
            continue;
        }
        let pts = [old_pos[ia], old_pos[ib], old_pos[ic]];
        let center = (pts[0] + pts[1] + pts[2]) / 3.0;
        let d0 = dist_to_front(face, pts[0]).abs();
        let d1 = dist_to_front(face, pts[1]).abs();
        let d2 = dist_to_front(face, pts[2]).abs();
        let on_front = d0.min(d1).min(d2) < FRONT_EPS && dist_to_front(face, center).abs() < FRONT_EPS * 2.0;

        let n_face = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalize_or_zero();
        let (_, _, _, front_n) = face_basis(face);
        let facing_out = n_face.dot(front_n) > 0.2;

        if !on_front || !facing_out {
            // Keep triangle as-is
            let uv_a = old_uv.as_ref().and_then(|u| u.get(ia).copied()).unwrap_or([0.0, 0.0]);
            let uv_b = old_uv.as_ref().and_then(|u| u.get(ib).copied()).unwrap_or([0.0, 0.0]);
            let uv_c = old_uv.as_ref().and_then(|u| u.get(ic).copied()).unwrap_or([0.0, 0.0]);
            let vc_a = old_vc.as_ref().and_then(|c| c.get(ia).copied()).unwrap_or(mesh.color);
            let vc_b = old_vc.as_ref().and_then(|c| c.get(ib).copied()).unwrap_or(mesh.color);
            let vc_c = old_vc.as_ref().and_then(|c| c.get(ic).copied()).unwrap_or(mesh.color);
            let na = if has_n { old_n[ia] } else { n_face };
            let nb = if has_n { old_n[ib] } else { n_face };
            let nc = if has_n { old_n[ic] } else { n_face };
            let i0 = push(&mut new_pos, &mut new_n, &mut new_uv, &mut new_vc, pts[0], na, uv_a, vc_a);
            let i1 = push(&mut new_pos, &mut new_n, &mut new_uv, &mut new_vc, pts[1], nb, uv_b, vc_b);
            let i2 = push(&mut new_pos, &mut new_n, &mut new_uv, &mut new_vc, pts[2], nc, uv_c, vc_c);
            new_idx.extend_from_slice(&[i0, i1, i2]);
            continue;
        }

        // Front triangle → 2D polygon in UV, difference with glyphs
        let uvs_tri = [
            world_to_uv(face, pts[0]),
            world_to_uv(face, pts[1]),
            world_to_uv(face, pts[2]),
        ];
        let tri_poly = {
            let ring = vec![
                Coord {
                    x: uvs_tri[0][0] as f64,
                    y: uvs_tri[0][1] as f64,
                },
                Coord {
                    x: uvs_tri[1][0] as f64,
                    y: uvs_tri[1][1] as f64,
                },
                Coord {
                    x: uvs_tri[2][0] as f64,
                    y: uvs_tri[2][1] as f64,
                },
                Coord {
                    x: uvs_tri[0][0] as f64,
                    y: uvs_tri[0][1] as f64,
                },
            ];
            MultiPolygon(vec![Polygon::new(LineString(ring), vec![])])
        };

        let remaining = tri_poly.difference(glyphs);

        // Surface remainder (holes where letters are carved)
        append_multipolygon_mesh(
            &remaining,
            face,
            0.0,
            front_n,
            mesh.color,
            &mut new_pos,
            &mut new_n,
            &mut new_idx,
            false,
        );
    }

    mesh.positions = new_pos;
    mesh.normals = new_n;
    mesh.indices = new_idx;
    mesh.uvs = if has_uv { Some(new_uv) } else { None };
    mesh.vertex_colors = if has_vc { Some(new_vc) } else { None };
    if mesh.normals.len() != mesh.positions.len() {
        mesh.recompute_normals();
    }
}

pub(crate) fn append_multipolygon_mesh(
    mp: &MultiPolygon<f64>,
    face: &SteleFace,
    inset: f32,
    normal: Vec3,
    _color: [f32; 3],
    positions: &mut Vec<Vec3>,
    normals: &mut Vec<Vec3>,
    indices: &mut Vec<u32>,
    flip_winding: bool,
) {
    for poly in &mp.0 {
        let exterior: Vec<[f64; 2]> = poly
            .exterior()
            .coords()
            .map(|c| [c.x, c.y])
            .collect();
        if exterior.len() < 4 {
            continue;
        }
        // Drop closing duplicate for earcut
        let mut flat: Vec<f64> = Vec::new();
        let mut hole_indices: Vec<usize> = Vec::new();
        let ext_len = exterior.len() - 1;
        for p in exterior.iter().take(ext_len) {
            flat.push(p[0]);
            flat.push(p[1]);
        }
        for hole in poly.interiors() {
            hole_indices.push(flat.len() / 2);
            let h: Vec<[f64; 2]> = hole.coords().map(|c| [c.x, c.y]).collect();
            let hlen = h.len().saturating_sub(1);
            for p in h.iter().take(hlen) {
                flat.push(p[0]);
                flat.push(p[1]);
            }
        }
        let Ok(tris) = earcut(&flat, &hole_indices, 2) else {
            continue;
        };
        let base = positions.len() as u32;
        let nverts = flat.len() / 2;
        for i in 0..nverts {
            let u = flat[i * 2] as f32;
            let v = flat[i * 2 + 1] as f32;
            positions.push(uv_to_world(face, u, v, inset));
            normals.push(normal);
        }
        for t in tris.chunks_exact(3) {
            let a = base + t[0] as u32;
            let b = base + t[1] as u32;
            let c = base + t[2] as u32;
            if flip_winding {
                indices.extend_from_slice(&[a, c, b]);
            } else {
                indices.extend_from_slice(&[a, b, c]);
            }
        }
    }
}

pub(crate) fn add_glyph_walls(face: &SteleFace, glyphs: &MultiPolygon<f64>, depth: f32, cavity: &mut TriangleMesh) {
    let (_, _, _, front_n) = face_basis(face);
    for poly in &glyphs.0 {
        let rings: Vec<&LineString<f64>> = std::iter::once(poly.exterior())
            .chain(poly.interiors().iter())
            .collect();
        for ring in rings {
            let coords: Vec<[f32; 2]> = ring
                .coords()
                .map(|c| [c.x as f32, c.y as f32])
                .collect();
            if coords.len() < 2 {
                continue;
            }
            for i in 0..coords.len().saturating_sub(1) {
                let a = coords[i];
                let b = coords[i + 1];
                let p0 = uv_to_world(face, a[0], a[1], 0.0);
                let p1 = uv_to_world(face, b[0], b[1], 0.0);
                let p2 = uv_to_world(face, b[0], b[1], depth);
                let p3 = uv_to_world(face, a[0], a[1], depth);
                let edge = p1 - p0;
                let wall_n = front_n.cross(edge).normalize_or_zero();
                // Face inward (into cavity)
                let mid = (p0 + p1 + p2 + p3) * 0.25;
                let to_center = {
                    // Approximate: from wall mid toward glyph interior using 2D inset
                    let c2 = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
                    let inward_uv = [c2[0] - wall_n.dot(face_basis(face).1), c2[1]];
                    let _ = inward_uv;
                    mid
                };
                let mut wn = wall_n;
                // Orient wall normal away from front_n × edge pointing outside letter:
                // check against polygon contains slightly offset point
                let test = Coord {
                    x: ((a[0] + b[0]) * 0.5 + (-(b[1] - a[1])) * 0.0005) as f64,
                    y: ((a[1] + b[1]) * 0.5 + ((b[0] - a[0])) * 0.0005) as f64,
                };
                use geo::Contains;
                if glyphs.contains(&test) {
                    wn = -wn;
                }
                let _ = to_center;
                let base = cavity.positions.len() as u32;
                for (p, n) in [(p0, wn), (p1, wn), (p2, wn), (p3, wn)] {
                    cavity.positions.push(p);
                    cavity.normals.push(n);
                }
                // Winding so the wall faces into the cavity (visible from outside looking in)
                cavity
                    .indices
                    .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }
    // Ensure wall normals face somewhat toward the viewer / cavity interior
    let _ = front_n;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::create_project;
    use crate::scene::assemble::assemble_scene;

    #[test]
    fn letter_with_counter_keeps_hole() {
        let path = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        if !Path::new(path).exists() {
            return;
        }
        let data = std::fs::read(path).unwrap();
        let face = ttf_parser::Face::parse(&data, 0).unwrap();
        // Ф and 6 have enclosed counters
        for sample in ["Ф", "6", "б", "O"] {
            let (contours, _) = build_line_contours(&face, sample, 0.001, false, false).unwrap();
            assert!(contours.len() >= 2, "{sample} should have outer+hole contours, got {}", contours.len());
            let mp = contours_to_multipolygon(&contours);
            assert!(!mp.0.is_empty(), "{sample}: empty multipolygon");
            let has_hole = mp.0.iter().any(|p| !p.interiors().is_empty());
            assert!(has_hole, "{sample}: counter hole was filled (no interior rings)");
        }
    }

    #[test]
    fn outline_dejavu_letter_a() {
        let path = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        if !Path::new(path).exists() {
            return;
        }
        let data = std::fs::read(path).unwrap();
        let face = ttf_parser::Face::parse(&data, 0).unwrap();
        let (contours, w) = build_line_contours(&face, "АБВ", 0.001, false, false).unwrap();
        assert!(w > 0.0);
        assert!(!contours.is_empty());
    }

    #[test]
    fn assemble_with_text_does_not_panic() {
        let p = create_project("eng");
        let built = assemble_scene(&p);
        assert!(!built.meshes.is_empty());
    }

    #[test]
    fn carving_adds_cavity_geometry() {
        let path = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        if !Path::new(path).exists() {
            return;
        }
        let mut p = create_project("carve");
        for part in &mut p.parts {
            if part.role == crate::model::PartRole::Stele {
                if let Some(d) = part.decals.first_mut() {
                    if let DecalKind::Text { style, content, .. } = &mut d.kind {
                        *content = "ИВАНОВ".into();
                        style.font_path = Some(path.into());
                        style.letter_cm = 5.0;
                        style.depth_mm = 3.5;
                    }
                }
            }
        }
        let built = assemble_scene(&p);
        let tris: usize = built.meshes.iter().map(|m| m.indices.len() / 3).sum();
        assert!(tris > 12, "expected engraved geometry, tris={tris}");
        // Filled letter bottoms live in a cavity mesh without texture
        let cavity = built
            .meshes
            .iter()
            .find(|m| m.texture_path.is_none() && m.double_sided)
            .expect("cavity mesh with letter fill");
        assert!(
            cavity.indices.len() / 3 >= 8,
            "letter fill should have many tris, got {}",
            cavity.indices.len() / 3
        );
    }

    #[test]
    fn carving_keeps_stele_texture_uvs() {
        let path = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        if !Path::new(path).exists() {
            return;
        }
        let mut p = create_project("carve_tex");
        for part in &mut p.parts {
            if part.role == crate::model::PartRole::Stele {
                part.texture_path = Some("/tmp/fake_granite.png".into());
                if let Some(d) = part.decals.first_mut() {
                    if let DecalKind::Text { style, content, .. } = &mut d.kind {
                        *content = "ТЕСТ".into();
                        style.font_path = Some(path.into());
                        style.letter_cm = 4.0;
                    }
                }
            }
        }
        let built = assemble_scene(&p);
        let textured: Vec<_> = built
            .meshes
            .iter()
            .filter(|m| m.texture_path.is_some())
            .collect();
        assert!(!textured.is_empty(), "stele should keep texture_path after carve");
        for m in textured {
            let uvs = m.uvs.as_ref().expect("UVs after carve");
            assert_eq!(
                uvs.len(),
                m.positions.len(),
                "UV count must match verts so texture samples"
            );
        }
    }
}
