use crate::scene::mesh::TriangleMesh;
use glam::Vec3;
use std::path::Path;

pub fn load_model(path: &Path) -> Result<Vec<TriangleMesh>, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut meshes = match ext.as_str() {
        "obj" => load_obj(path)?,
        "gltf" | "glb" => load_gltf(path)?,
        _ => {
            return Err(format!(
                "Неподдерживаемый формат «{ext}». Используйте .obj, .gltf или .glb"
            ))
        }
    };

    for m in &mut meshes {
        m.double_sided = true; // fences / thin panels visible from both sides
        m.ensure_outward_winding();
    }
    Ok(meshes)
}

fn load_obj(path: &Path) -> Result<Vec<TriangleMesh>, String> {
    let (models, materials) = tobj::load_obj(
        path,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ignore_points: true,
            ignore_lines: true,
            ..Default::default()
        },
    )
    .map_err(|e| format!("OBJ «{}»: {e}", path.display()))?;

    let materials = materials.unwrap_or_default();
    let mut meshes = Vec::new();

    for model in &models {
        let mesh = &model.mesh;
        if mesh.positions.len() < 9 {
            continue;
        }
        let mut positions = Vec::with_capacity(mesh.positions.len() / 3);
        for chunk in mesh.positions.chunks_exact(3) {
            positions.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
        }
        let indices = if mesh.indices.is_empty() {
            (0..positions.len() as u32).collect()
        } else {
            mesh.indices.clone()
        };
        if indices.len() < 3 {
            continue;
        }

        let color = mesh
            .material_id
            .and_then(|id| materials.get(id))
            .map(|m| {
                [
                    m.diffuse.unwrap_or([0.55, 0.55, 0.55])[0],
                    m.diffuse.unwrap_or([0.55, 0.55, 0.55])[1],
                    m.diffuse.unwrap_or([0.55, 0.55, 0.55])[2],
                ]
            })
            .unwrap_or([0.55, 0.52, 0.48]);

        let mut tri = TriangleMesh {
            positions,
            normals: vec![],
            indices,
            color,
            vertex_colors: None,
            uvs: None,
            double_sided: true,
            texture_path: None,
            flat_shade: false,
        };
        tri.recompute_normals();
        meshes.push(tri);
    }

    if meshes.is_empty() {
        return load_obj_manual(path);
    }
    Ok(meshes)
}

fn load_obj_manual(path: &Path) -> Result<Vec<TriangleMesh>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("OBJ: {e}"))?;
    let mut positions: Vec<Vec3> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(tag) = parts.next() else { continue };
        match tag {
            "v" => {
                let vals: Vec<f32> = parts.filter_map(|s| s.parse().ok()).collect();
                if vals.len() >= 3 {
                    positions.push(Vec3::new(vals[0], vals[1], vals[2]));
                }
            }
            "f" => {
                let mut face: Vec<u32> = Vec::new();
                for token in parts {
                    let idx_str = token.split('/').next().unwrap_or("");
                    if let Ok(mut i) = idx_str.parse::<i32>() {
                        if i < 0 {
                            i = positions.len() as i32 + i + 1;
                        }
                        if i >= 1 {
                            face.push((i - 1) as u32);
                        }
                    }
                }
                if face.len() >= 3 {
                    for i in 1..face.len().saturating_sub(1) {
                        indices.extend_from_slice(&[face[0], face[i], face[i + 1]]);
                    }
                }
            }
            _ => {}
        }
    }

    if positions.len() < 3 || indices.len() < 3 {
        return Err("Ручной разбор OBJ: мало вершин/граней".into());
    }

    let mut tri = TriangleMesh {
        positions,
        normals: vec![],
        indices,
        color: [0.55, 0.52, 0.48],
        vertex_colors: None,
        uvs: None,
        double_sided: true,
        texture_path: None,
        flat_shade: false,
    };
    tri.recompute_normals();
    Ok(vec![tri])
}

fn load_gltf(path: &Path) -> Result<Vec<TriangleMesh>, String> {
    let (document, buffers, _images) =
        gltf::import(path).map_err(|e| format!("glTF: {e}"))?;

    let mut meshes = Vec::new();
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let positions: Vec<[f32; 3]> = match reader.read_positions() {
                Some(iter) => iter.collect(),
                None => continue,
            };
            let indices: Vec<u32> = if let Some(idx) = reader.read_indices() {
                idx.into_u32().collect()
            } else {
                (0..positions.len() as u32).collect()
            };
            if positions.len() < 3 || indices.len() < 3 {
                continue;
            }

            let base_color = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_factor();
            let color = [base_color[0], base_color[1], base_color[2]];

            let vertex_colors = reader.read_colors(0).map(|cols| {
                cols.into_rgb_f32()
                    .map(|c| [c[0], c[1], c[2]])
                    .collect::<Vec<_>>()
            });

            let mut tri = TriangleMesh {
                positions: positions
                    .into_iter()
                    .map(|p| Vec3::new(p[0], p[1], p[2]))
                    .collect(),
                normals: vec![],
                indices,
                color,
                vertex_colors,
                uvs: None,
                double_sided: true,
                texture_path: None,
                flat_shade: false,
            };
            if let Some(niter) = reader.read_normals() {
                tri.normals = niter.map(|n| Vec3::new(n[0], n[1], n[2])).collect();
            } else {
                tri.recompute_normals();
            }
            meshes.push(tri);
        }
    }

    if meshes.is_empty() {
        return Err("В glTF не найдено треугольников".into());
    }
    Ok(meshes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_blender_style_obj_cube() {
        let dir = std::env::temp_dir().join("stela_obj_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("cube.obj");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"v -1 -1 1
v 1 -1 1
v 1 1 1
v -1 1 1
v -1 -1 -1
v 1 -1 -1
v 1 1 -1
v -1 1 -1
f 1 2 3 4
f 5 8 7 6
f 1 5 6 2
f 2 6 7 3
f 3 7 8 4
f 5 1 4 8
"#
        )
        .unwrap();
        let meshes = load_model(&path).expect("load obj");
        let tris: usize = meshes.iter().map(|m| m.indices.len() / 3).sum();
        assert!(tris >= 12, "expected >=12 tris, got {tris}");
        assert!(!meshes[0].positions.is_empty());
    }
}
