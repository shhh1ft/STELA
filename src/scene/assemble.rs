use crate::model::*;
use crate::scene::clip::{clip_meshes_above_ground, ensure_box_uvs, DEFAULT_TEXTURE_TILE_M};
use crate::scene::engrave::{self, resolve_font_path};
use crate::scene::import::load_model;
use crate::scene::medallion;
use crate::scene::mesh::{fit_to_size, TriangleMesh};
use glam::Vec3;
use std::collections::HashMap;
use std::path::Path;

/// Resolved stele face (front or back) for decal placement.
#[derive(Clone, Debug)]
pub struct SteleFace {
    pub part_id: String,
    pub side: FaceSide,
    /// Bottom-left, bottom-right, top-right, top-left as seen from outside (world).
    pub corners: [Vec3; 4],
    pub size_m: Vec3,
}

#[derive(Clone, Debug)]
pub struct SceneBuild {
    pub meshes: Vec<TriangleMesh>,
    pub stele_faces: Vec<SteleFace>,
    pub warnings: Vec<String>,
}

/// Assemble the plot: scale every part to exact cm sizes and stack stele on pedestal.
pub fn assemble_scene(project: &Project) -> SceneBuild {
    let mut warnings = Vec::new();
    let mut meshes = Vec::new();
    let mut stele_faces = Vec::new();

    // Pedestal top = offset_y + height (world), so sinking pedestals still stack correctly
    let mut pedestal_top: HashMap<&str, f32> = HashMap::new();
    for m in &project.monuments {
        let top = project
            .parts
            .iter()
            .filter(|p| p.visible && p.monument_id == m.id && p.role == PartRole::Pedestal)
            .map(|p| p.offset_y + p.effective_size_m().y)
            .fold(0.0_f32, f32::max);
        pedestal_top.insert(m.id.as_str(), top);
    }

    for part in &project.parts {
        if !part.visible {
            continue;
        }
        let Some(monument) = project.monuments.iter().find(|m| m.id == part.monument_id) else {
            warnings.push(format!(
                "Деталь «{}» ссылается на неизвестный памятник.",
                part.name
            ));
            continue;
        };

        let size = part.effective_size_m();
        let yaw = part.yaw_deg.to_radians();
        let cx = monument.x + part.offset_x;
        let cz = monument.z + part.offset_z;

        let stack_base =
            if part.auto_stack && matches!(part.role, PartRole::Stele | PartRole::Cross) {
                *pedestal_top.get(part.monument_id.as_str()).unwrap_or(&0.0)
            } else {
                0.0
            };
        let base_y = stack_base + part.offset_y;

        let placeholder_color = part.role.color();
        let mut part_meshes = if let Some(path) = part.model_path.as_ref() {
            match load_model(Path::new(path)) {
                Ok(loaded) => loaded,
                Err(e) => {
                    warnings.push(format!("{}: {e} — использована заглушка.", part.name));
                    vec![TriangleMesh::unit_box(placeholder_color)]
                }
            }
        } else {
            vec![TriangleMesh::unit_box(placeholder_color)]
        };

        fit_to_size(&mut part_meshes, size, cx, base_y, cz, yaw);

        for m in &mut part_meshes {
            m.texture_path = part.texture_path.clone();
            if part.texture_path.is_some() {
                // Always box-map so sides/tops aren't stretched (planar UV was wrong).
                ensure_box_uvs(m, DEFAULT_TEXTURE_TILE_M);
            }
        }

        // Clip anything that went under the ground plane
        clip_meshes_above_ground(&mut part_meshes, 0.0);

        if part.role == PartRole::Stele {
            let faces = [
                stele_face(part, cx, base_y, cz, size, yaw, FaceSide::Front),
                stele_face(part, cx, base_y, cz, size, yaw, FaceSide::Back),
            ];
            let mut extra: Vec<TriangleMesh> = Vec::new();
            for face in &faces {
                // Лицо привязано к своей стеле: при нескольких памятниках чужие id не гравируем.
                if face.part_id != part.id {
                    continue;
                }
                let decals_on_side: Vec<Decal> = part
                    .decals
                    .iter()
                    .filter(|d| d.side == face.side)
                    .cloned()
                    .collect();
                if decals_on_side.is_empty() {
                    continue;
                }
                let eng_warn = engrave::engrave_stele_texts(
                    &mut part_meshes,
                    face,
                    &decals_on_side,
                    &|style| resolve_font_path(style),
                );
                warnings.extend(eng_warn);

                let (med_meshes, med_warn) =
                    medallion::build_medallions(&mut part_meshes, face, &decals_on_side);
                extra.extend(med_meshes);
                warnings.extend(med_warn);
            }
            // Carving rebuilds front triangles without UVs — restore box mapping
            // so the stone texture stays on the stele (not on the cavity).
            if part.texture_path.is_some() {
                for m in &mut part_meshes {
                    if m.texture_path.is_some() {
                        ensure_box_uvs(m, DEFAULT_TEXTURE_TILE_M);
                    }
                }
            }
            part_meshes.extend(extra);
            stele_faces.extend(faces);
        }

        meshes.extend(part_meshes);
    }

    if meshes.is_empty() {
        warnings.push("На участке нет деталей. Добавьте стелу, тумбу или другую модель.".into());
    }

    SceneBuild {
        meshes,
        stele_faces,
        warnings,
    }
}

fn stele_face(
    part: &PlacedPart,
    cx: f32,
    base_y: f32,
    cz: f32,
    size: Vec3,
    yaw: f32,
    side: FaceSide,
) -> SteleFace {
    let (sy, cy) = yaw.sin_cos();
    let half_w = size.x * 0.5;
    let half_d = size.z * 0.5;
    // Face plane sits exactly on the stone surface; carving measures from here.
    let local = match side {
        FaceSide::Front => [
            Vec3::new(-half_w, 0.0, half_d),
            Vec3::new(half_w, 0.0, half_d),
            Vec3::new(half_w, size.y, half_d),
            Vec3::new(-half_w, size.y, half_d),
        ],
        // Mirrored so right×up points outward (−Z) when viewed from behind.
        FaceSide::Back => [
            Vec3::new(half_w, 0.0, -half_d),
            Vec3::new(-half_w, 0.0, -half_d),
            Vec3::new(-half_w, size.y, -half_d),
            Vec3::new(half_w, size.y, -half_d),
        ],
    };
    let corners = local.map(|p| {
        let rx = p.x * cy - p.z * sy;
        let rz = p.x * sy + p.z * cy;
        Vec3::new(cx + rx, base_y + p.y, cz + rz)
    });
    SteleFace {
        part_id: part.id.clone(),
        side,
        corners,
        size_m: size,
    }
}

/// Размер фотопластины в метрах (ширина, высота). Для текста — `None`.
pub fn photo_plate_size_m(decal: &Decal) -> Option<(f32, f32)> {
    let spec = decal.photo_spec()?;
    Some((
        (spec.width_cm / 100.0).max(0.02),
        (spec.height_cm / 100.0).max(0.02),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stele_sits_on_pedestal() {
        let p = create_project("test");
        let built = assemble_scene(&p);
        assert!(!built.meshes.is_empty());
        assert_eq!(built.stele_faces.len(), 2);

        let face = built
            .stele_faces
            .iter()
            .find(|f| f.side == FaceSide::Front)
            .expect("front face");
        let pedestal = p
            .parts
            .iter()
            .find(|x| x.role == PartRole::Pedestal)
            .unwrap();
        let ped_h = pedestal.effective_size_m().y;
        assert!(
            (face.corners[0].y - ped_h).abs() < 0.01,
            "stele base y={} expected on pedestal top {}",
            face.corners[0].y,
            ped_h
        );

        let stele = p.parts.iter().find(|x| x.role == PartRole::Stele).unwrap();
        assert_eq!(face.part_id, stele.id);

        for m in &built.meshes {
            for v in &m.positions {
                assert!(v.y >= -0.001, "vertex under ground: {}", v.y);
            }
        }
    }

    #[test]
    fn sizes_match_cm_input() {
        let mut p = create_project("sizes");
        for part in &mut p.parts {
            if part.role == PartRole::Stele {
                part.size = SizeCm::new(50.0, 120.0, 10.0);
            }
            if part.role == PartRole::Pedestal {
                part.size = SizeCm::new(80.0, 25.0, 50.0);
            }
        }
        let built = assemble_scene(&p);
        let face = &built.stele_faces[0];
        let w = (face.corners[1] - face.corners[0]).length();
        let h = (face.corners[3] - face.corners[0]).length();
        assert!((w - 0.50).abs() < 0.02, "width {w}");
        assert!((h - 1.20).abs() < 0.02, "height {h}");
        assert!((face.corners[0].y - 0.25).abs() < 0.02);
    }

    #[test]
    fn negative_y_is_clipped() {
        let mut p = create_project("clip");
        for part in &mut p.parts {
            if part.role == PartRole::Pedestal {
                part.offset_y = -0.1;
                part.auto_stack = false;
            }
        }
        // Clear stele so we only test pedestal clip
        p.parts.retain(|x| x.role == PartRole::Pedestal);
        let built = assemble_scene(&p);
        assert!(!built.meshes.is_empty());
        for m in &built.meshes {
            for v in &m.positions {
                assert!(v.y >= -0.001, "vertex under ground after clip: {}", v.y);
            }
        }
    }

    #[test]
    fn textured_parts_get_box_uvs() {
        let mut p = create_project("tex");
        for part in &mut p.parts {
            if part.role == PartRole::Pedestal {
                part.texture_path = Some("/tmp/fake_tex.png".into());
            }
        }
        p.parts.retain(|x| x.role == PartRole::Pedestal);
        let built = assemble_scene(&p);
        assert!(!built.meshes.is_empty());
        let m = &built.meshes[0];
        let uvs = m.uvs.as_ref().expect("box UVs assigned");
        assert_eq!(uvs.len(), m.positions.len());
        // Top and side faces must not share collapsed UVs (planar bug).
        assert!(uvs
            .iter()
            .any(|uv| uv[0].abs() > 0.01 || uv[1].abs() > 0.01));
    }

    #[test]
    fn photo_extent_uses_centimetres() {
        let mut decal = Decal::new_photo("portrait.jpg");
        if let DecalKind::Photo { spec, .. } = &mut decal.kind {
            spec.width_cm = 13.0;
            spec.height_cm = 18.0;
        }
        let (w, h) = photo_plate_size_m(&decal).expect("photo size");
        assert!((w - 0.13).abs() < 1e-4, "width {w}");
        assert!((h - 0.18).abs() < 1e-4, "height {h}");
    }
}
