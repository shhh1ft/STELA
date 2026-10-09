//! Photo-ceramic medallions as real 3D plates: shape outline, extrusion,
//! niche carving into the stele, screws for metal-ceramic.

use crate::model::{Decal, DecalKind, MedallionMaterial, MedallionShape, MountType};
use crate::scene::assemble::{photo_plate_size_m, SteleFace};
use crate::scene::engrave::{
    add_glyph_walls, append_multipolygon_mesh, carve_mesh_front, contours_to_multipolygon,
    face_basis, uv_to_world,
};
use crate::scene::mesh::TriangleMesh;
use crate::scene::photo_compose::compose_medallion_image;
use earcutr::earcut;

/// Build medallion meshes for all photo decals on this face; carves niches into `stele`.
pub fn build_medallions(
    stele: &mut Vec<TriangleMesh>,
    face: &SteleFace,
    decals: &[Decal],
) -> (Vec<TriangleMesh>, Vec<String>) {
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    for decal in decals {
        if !decal.visible {
            continue;
        }
        let DecalKind::Photo { path, spec } = &decal.kind else {
            continue;
        };
        match build_one(stele, face, decal, path, spec) {
            Ok(meshes) => out.extend(meshes),
            Err(e) => warnings.push(format!("Медальон «{}»: {e}", decal.name)),
        }
    }
    (out, warnings)
}

fn build_one(
    stele: &mut Vec<TriangleMesh>,
    face: &SteleFace,
    decal: &Decal,
    photo_path: &str,
    spec: &crate::model::PhotoSpec,
) -> Result<Vec<TriangleMesh>, String> {
    let (w_m, h_m) = photo_plate_size_m(decal).ok_or_else(|| "Медальон без размера".to_string())?;
    let face_w = face.size_m.x.max(1e-4);
    let face_h = face.size_m.y.max(1e-4);
    let thick = (spec.material.thickness_mm() / 1000.0).max(0.0008);

    // Outline in UV space of the face (so carving and placement share coordinates)
    let cu = decal.u.clamp(0.0, 1.0);
    let cv = decal.v.clamp(0.0, 1.0);
    let outline_local = shape_outline(spec.shape, w_m, h_m);
    let outline_uv: Vec<[f32; 2]> = outline_local
        .iter()
        .map(|p| [cu + p[0] / face_w, cv + p[1] / face_h])
        .collect();

    // Mount geometry: where plate front/back sit relative to stone surface (inset>0 = into stone)
    let (front_inset, back_inset) = match spec.mount {
        MountType::Niche => (0.0, thick),
        MountType::SurfaceGlue | MountType::Screws => (-thick, 0.0),
    };

    if spec.mount == MountType::Niche {
        let mp = contours_to_multipolygon(&[outline_uv.clone()]);
        if !mp.0.is_empty() {
            for mesh in stele.iter_mut() {
                carve_mesh_front(mesh, face, &mp, thick);
            }
            // Niche walls in stone color (dark, unlit)
            let mut niche = TriangleMesh {
                positions: vec![],
                normals: vec![],
                indices: vec![],
                color: [0.18, 0.18, 0.19],
                vertex_colors: None,
                uvs: None,
                double_sided: true,
                texture_path: None,
                flat_shade: true,
            };
            let (_, _, _, front_n) = face_basis(face);
            append_multipolygon_mesh(
                &mp,
                face,
                thick,
                front_n,
                niche.color,
                &mut niche.positions,
                &mut niche.normals,
                &mut niche.indices,
                false,
            );
            add_glyph_walls(face, &mp, thick + 0.0005, &mut niche);
            if !niche.indices.is_empty() {
                stele.push(niche);
            }
        }
    }

    let composite = compose_medallion_image(photo_path, spec)
        .map(|p| p.to_string_lossy().into_owned())
        .ok();

    let mut meshes = Vec::new();

    // Front face (photo)
    let mut front = plate_cap(face, &outline_uv, front_inset, false, spec.material);
    front.texture_path = composite.clone();
    // UV: map outline bbox → 0..1 (image compositor uses the same aspect)
    let min_u = cu - (w_m * 0.5) / face_w;
    let max_u = cu + (w_m * 0.5) / face_w;
    let min_v = cv - (h_m * 0.5) / face_h;
    let max_v = cv + (h_m * 0.5) / face_h;
    let (origin, right, up, _) = face_basis(face);
    let rl = right.length().max(1e-6);
    let ul = up.length().max(1e-6);
    front.uvs = Some(
        front
            .positions
            .iter()
            .map(|p| {
                let d = *p - origin;
                let u = d.dot(right / rl) / rl;
                let v = d.dot(up / ul) / ul;
                [
                    ((u - min_u) / (max_u - min_u).max(1e-6)).clamp(0.0, 1.0),
                    (1.0 - (v - min_v) / (max_v - min_v).max(1e-6)).clamp(0.0, 1.0),
                ]
            })
            .collect(),
    );
    if composite.is_some() {
        // Printed face: show the composite at full color, not shaded by the rim tint.
        front.flat_shade = true;
        front.color = [1.0, 1.0, 1.0];
    } else {
        front.color = [0.35, 0.36, 0.40];
    }
    meshes.push(front);

    // Back cap
    let back = plate_cap(face, &outline_uv, back_inset, true, spec.material);
    meshes.push(back);

    // Rim
    meshes.push(plate_rim(
        face,
        &outline_uv,
        front_inset,
        back_inset,
        spec.material,
    ));

    // Screws for metal-ceramic
    if spec.mount == MountType::Screws {
        meshes.extend(screws(face, cu, cv, w_m, h_m, face_w, face_h, front_inset));
    }

    Ok(meshes)
}

/// Shape outline in metres, centered, CCW.
pub fn shape_outline(shape: MedallionShape, w: f32, h: f32) -> Vec<[f32; 2]> {
    let hw = w * 0.5;
    let hh = h * 0.5;
    match shape {
        MedallionShape::Oval => {
            let n = 72;
            (0..n)
                .map(|i| {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    [hw * a.cos(), hh * a.sin()]
                })
                .collect()
        }
        MedallionShape::Rect => {
            // Slightly rounded corners (3 mm)
            rounded_rect(hw, hh, (0.003_f32).min(hw * 0.2).min(hh * 0.2))
        }
        MedallionShape::Barrel => {
            // Rectangle with convex (bulged) sides on all four edges
            let bulge_x = hw * 0.10;
            let bulge_y = hh * 0.06;
            let seg = 16;
            let mut pts = Vec::new();
            // bottom edge: left → right, bulging downward
            for i in 0..=seg {
                let t = i as f32 / seg as f32;
                let x = -hw + 2.0 * hw * t;
                let s = (t * std::f32::consts::PI).sin();
                pts.push([x, -hh - bulge_y * s]);
            }
            // right edge: bottom → top
            for i in 1..=seg {
                let t = i as f32 / seg as f32;
                let y = -hh + 2.0 * hh * t;
                let s = (t * std::f32::consts::PI).sin();
                pts.push([hw + bulge_x * s, y]);
            }
            // top edge: right → left
            for i in 1..=seg {
                let t = i as f32 / seg as f32;
                let x = hw - 2.0 * hw * t;
                let s = (t * std::f32::consts::PI).sin();
                pts.push([x, hh + bulge_y * s]);
            }
            // left edge: top → bottom
            for i in 1..seg {
                let t = i as f32 / seg as f32;
                let y = hh - 2.0 * hh * t;
                let s = (t * std::f32::consts::PI).sin();
                pts.push([-hw - bulge_x * s, y]);
            }
            pts
        }
        MedallionShape::Arch => {
            // Rect with semicircular top (radius = half width)
            let r = hw;
            let straight_top = hh - r; // y where the arc starts
            let mut pts = vec![[-hw, -hh], [hw, -hh]];
            let n = 40;
            // Arc from the right shoulder (angle 0) up and over to the left (angle π).
            for i in 0..=n {
                let ang = std::f32::consts::PI * (i as f32 / n as f32);
                pts.push([r * ang.cos(), straight_top.max(-hh) + r * ang.sin()]);
            }
            // ensure left-bottom closure
            pts.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6);
            pts
        }
    }
}

fn rounded_rect(hw: f32, hh: f32, r: f32) -> Vec<[f32; 2]> {
    let r = r.max(0.0);
    if r < 1e-5 {
        return vec![[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]];
    }
    let n = 6;
    let mut pts = Vec::new();
    let corners = [
        (hw - r, -hh + r, -std::f32::consts::FRAC_PI_2),
        (hw - r, hh - r, 0.0),
        (-hw + r, hh - r, std::f32::consts::FRAC_PI_2),
        (-hw + r, -hh + r, std::f32::consts::PI),
    ];
    for (cx, cy, start) in corners {
        for i in 0..=n {
            let a = start + std::f32::consts::FRAC_PI_2 * i as f32 / n as f32;
            pts.push([cx + r * a.cos(), cy + r * a.sin()]);
        }
    }
    pts
}

fn plate_cap(
    face: &SteleFace,
    outline_uv: &[[f32; 2]],
    inset: f32,
    is_back: bool,
    material: MedallionMaterial,
) -> TriangleMesh {
    let (_, _, _, normal) = face_basis(face);
    let mut flat: Vec<f64> = Vec::with_capacity(outline_uv.len() * 2);
    for p in outline_uv {
        flat.push(p[0] as f64);
        flat.push(p[1] as f64);
    }
    let tris = earcut(&flat, &[], 2).unwrap_or_default();
    let mut positions = Vec::with_capacity(outline_uv.len());
    for p in outline_uv {
        positions.push(uv_to_world(face, p[0], p[1], inset));
    }
    let mut indices = Vec::with_capacity(tris.len());
    for t in tris.chunks_exact(3) {
        let (a, b, c) = (t[0] as u32, t[1] as u32, t[2] as u32);
        if is_back {
            indices.extend_from_slice(&[a, c, b]);
        } else {
            indices.extend_from_slice(&[a, b, c]);
        }
    }
    let n = if is_back { -normal } else { normal };
    TriangleMesh {
        normals: vec![n; positions.len()],
        positions,
        indices,
        color: material.edge_color(),
        vertex_colors: None,
        uvs: None,
        double_sided: true,
        texture_path: None,
        flat_shade: false,
    }
}

fn plate_rim(
    face: &SteleFace,
    outline_uv: &[[f32; 2]],
    front_inset: f32,
    back_inset: f32,
    material: MedallionMaterial,
) -> TriangleMesh {
    let (_, _, _, normal) = face_basis(face);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    let n = outline_uv.len();
    for i in 0..n {
        let a = outline_uv[i];
        let b = outline_uv[(i + 1) % n];
        let p0 = uv_to_world(face, a[0], a[1], front_inset);
        let p1 = uv_to_world(face, b[0], b[1], front_inset);
        let p2 = uv_to_world(face, b[0], b[1], back_inset);
        let p3 = uv_to_world(face, a[0], a[1], back_inset);
        let edge = p1 - p0;
        let wn = edge.cross(normal).normalize_or_zero();
        let base = positions.len() as u32;
        positions.extend_from_slice(&[p0, p1, p2, p3]);
        normals.extend_from_slice(&[wn, wn, wn, wn]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    TriangleMesh {
        positions,
        normals,
        indices,
        color: material.edge_color(),
        vertex_colors: None,
        uvs: None,
        double_sided: true,
        texture_path: None,
        flat_shade: false,
    }
}

#[allow(clippy::too_many_arguments)]
fn screws(
    face: &SteleFace,
    cu: f32,
    cv: f32,
    w_m: f32,
    h_m: f32,
    face_w: f32,
    face_h: f32,
    front_inset: f32,
) -> Vec<TriangleMesh> {
    let r = 0.0025; // 2.5 mm screw head
    let head_h = 0.0012;
    let inset_edge = 0.009; // 9 mm from the plate edge
    let dx = (w_m * 0.5 - inset_edge).max(0.004);
    let dy = (h_m * 0.5 - inset_edge).max(0.004);
    let (_, _, _, normal) = face_basis(face);
    let mut meshes = Vec::new();
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let center_uv = [cu + sx * dx / face_w, cv + sy * dy / face_h];
        let n = 16;
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        // Head cap (slightly proud of plate front)
        let top_inset = front_inset - head_h;
        let center_top = uv_to_world(face, center_uv[0], center_uv[1], top_inset);
        positions.push(center_top);
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let u = center_uv[0] + r * a.cos() / face_w;
            let v = center_uv[1] + r * a.sin() / face_h;
            positions.push(uv_to_world(face, u, v, top_inset));
        }
        for i in 0..n {
            let a = 1 + i as u32;
            let b = 1 + ((i + 1) % n) as u32;
            indices.extend_from_slice(&[0, a, b]);
        }
        // Side wall of the head
        let base = positions.len() as u32;
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let u = center_uv[0] + r * a.cos() / face_w;
            let v = center_uv[1] + r * a.sin() / face_h;
            positions.push(uv_to_world(face, u, v, top_inset));
            positions.push(uv_to_world(face, u, v, front_inset));
        }
        for i in 0..n {
            let i0 = base + (i * 2) as u32;
            let i1 = base + (i * 2 + 1) as u32;
            let j0 = base + (((i + 1) % n) * 2) as u32;
            let j1 = base + (((i + 1) % n) * 2 + 1) as u32;
            indices.extend_from_slice(&[i0, j0, j1, i0, j1, i1]);
        }
        let mut m = TriangleMesh {
            normals: vec![normal; positions.len()],
            positions,
            indices,
            color: [0.42, 0.43, 0.45],
            vertex_colors: None,
            uvs: None,
            double_sided: true,
            texture_path: None,
            flat_shade: false,
        };
        m.recompute_normals();
        meshes.push(m);
    }
    meshes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_have_area_and_are_closed_loops() {
        for shape in MedallionShape::all() {
            let pts = shape_outline(*shape, 0.13, 0.18);
            assert!(pts.len() >= 4, "{shape:?} too few points");
            // Shoelace area
            let mut a = 0.0;
            for i in 0..pts.len() {
                let p = pts[i];
                let q = pts[(i + 1) % pts.len()];
                a += p[0] * q[1] - q[0] * p[1];
            }
            let area = (a * 0.5).abs();
            assert!(area > 0.13 * 0.18 * 0.5, "{shape:?} area too small: {area}");
            assert!(
                area <= 0.13 * 0.18 * 1.25,
                "{shape:?} area too large: {area}"
            );
        }
    }

    #[test]
    fn catalog_matches_photoceramic_spec() {
        use crate::model::MedallionMaterial::*;
        assert_eq!(
            MedallionShape::Oval.sizes_cm(CeramicGranite),
            &[(13.0, 18.0), (18.0, 24.0), (20.0, 30.0), (30.0, 40.0)]
        );
        assert_eq!(
            MedallionShape::Rect.sizes_cm(Porcelain),
            &[(9.0, 12.0), (13.0, 18.0), (18.0, 24.0)]
        );
        assert_eq!(
            MedallionShape::Rect.sizes_cm(MetalCeramic),
            &[(13.0, 18.0), (18.0, 24.0), (17.0, 22.0)]
        );
        assert_eq!(
            MedallionShape::Arch.sizes_cm(CeramicGranite),
            &[(13.0, 18.0), (18.0, 24.0)]
        );
        assert!(MedallionShape::Arch.sizes_cm(Porcelain).is_empty());
        assert!(MedallionShape::Arch.sizes_cm(MetalCeramic).is_empty());
        assert_eq!(
            MedallionShape::Barrel.sizes_cm(MetalCeramic),
            &[(13.0, 18.0), (17.0, 22.0)]
        );
        assert!(MedallionShape::Barrel.sizes_cm(CeramicGranite).is_empty());
        assert!(!MountType::Screws.available_for(CeramicGranite));
        assert!(MountType::Screws.available_for(MetalCeramic));
        assert!(!MountType::Niche.available_for(MetalCeramic));
        assert_eq!(MountType::default_for(MetalCeramic), MountType::Screws);
        assert_eq!(MountType::default_for(Porcelain), MountType::Niche);
    }
}
