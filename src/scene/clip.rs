use crate::scene::mesh::TriangleMesh;
use glam::Vec3;

/// Clip mesh geometry so nothing remains below `ground_y` (default 0).
/// Triangles straddling the plane are split; underground parts discarded.
pub fn clip_meshes_above_ground(meshes: &mut Vec<TriangleMesh>, ground_y: f32) {
    for mesh in meshes.iter_mut() {
        clip_mesh(mesh, ground_y);
    }
    meshes.retain(|m| !m.indices.is_empty() && !m.positions.is_empty());
}

fn clip_mesh(mesh: &mut TriangleMesh, ground_y: f32) {
    let old_pos = std::mem::take(&mut mesh.positions);
    let old_idx = std::mem::take(&mut mesh.indices);
    let old_uvs = mesh.uvs.take();
    let old_vcol = mesh.vertex_colors.take();

    let mut new_pos: Vec<Vec3> = Vec::new();
    let mut new_uvs: Vec<[f32; 2]> = Vec::new();
    let mut new_vcol: Vec<[f32; 3]> = Vec::new();
    let mut new_idx: Vec<u32> = Vec::new();

    let has_uv = old_uvs
        .as_ref()
        .map(|u| u.len() == old_pos.len())
        .unwrap_or(false);
    let has_vc = old_vcol
        .as_ref()
        .map(|c| c.len() == old_pos.len())
        .unwrap_or(false);

    let push_v = |pos: &mut Vec<Vec3>,
                  uvs: &mut Vec<[f32; 2]>,
                  vcol: &mut Vec<[f32; 3]>,
                  p: Vec3,
                  uv: [f32; 2],
                  vc: [f32; 3]|
     -> u32 {
        let i = pos.len() as u32;
        pos.push(p);
        if has_uv {
            uvs.push(uv);
        }
        if has_vc {
            vcol.push(vc);
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
        let uvs = [
            old_uvs.as_ref().and_then(|u| u.get(ia).copied()).unwrap_or([0.0, 0.0]),
            old_uvs.as_ref().and_then(|u| u.get(ib).copied()).unwrap_or([0.0, 0.0]),
            old_uvs.as_ref().and_then(|u| u.get(ic).copied()).unwrap_or([0.0, 0.0]),
        ];
        let vcs = [
            old_vcol
                .as_ref()
                .and_then(|c| c.get(ia).copied())
                .unwrap_or(mesh.color),
            old_vcol
                .as_ref()
                .and_then(|c| c.get(ib).copied())
                .unwrap_or(mesh.color),
            old_vcol
                .as_ref()
                .and_then(|c| c.get(ic).copied())
                .unwrap_or(mesh.color),
        ];

        let above = [
            pts[0].y >= ground_y - 1e-5,
            pts[1].y >= ground_y - 1e-5,
            pts[2].y >= ground_y - 1e-5,
        ];
        let n_above = above.iter().filter(|&&a| a).count();
        if n_above == 0 {
            continue;
        }
        if n_above == 3 {
            let i0 = push_v(&mut new_pos, &mut new_uvs, &mut new_vcol, pts[0], uvs[0], vcs[0]);
            let i1 = push_v(&mut new_pos, &mut new_uvs, &mut new_vcol, pts[1], uvs[1], vcs[1]);
            let i2 = push_v(&mut new_pos, &mut new_uvs, &mut new_vcol, pts[2], uvs[2], vcs[2]);
            new_idx.extend_from_slice(&[i0, i1, i2]);
            continue;
        }

        // Clip polygon against y = ground_y (keep above)
        let mut poly: Vec<(Vec3, [f32; 2], [f32; 3])> = Vec::new();
        for i in 0..3 {
            let j = (i + 1) % 3;
            let a = pts[i];
            let b = pts[j];
            let ua = uvs[i];
            let ub = uvs[j];
            let ca = vcs[i];
            let cb = vcs[j];
            let a_in = above[i];
            let b_in = above[j];
            if a_in {
                poly.push((a, ua, ca));
            }
            if a_in != b_in {
                let dy = b.y - a.y;
                if dy.abs() > 1e-8 {
                    let t = (ground_y - a.y) / dy;
                    let p = a + (b - a) * t;
                    let uv = [
                        ua[0] + (ub[0] - ua[0]) * t,
                        ua[1] + (ub[1] - ua[1]) * t,
                    ];
                    let c = [
                        ca[0] + (cb[0] - ca[0]) * t,
                        ca[1] + (cb[1] - ca[1]) * t,
                        ca[2] + (cb[2] - ca[2]) * t,
                    ];
                    poly.push((Vec3::new(p.x, ground_y, p.z), uv, c));
                }
            }
        }
        if poly.len() < 3 {
            continue;
        }
        // Fan triangulate
        for k in 1..poly.len().saturating_sub(1) {
            let i0 = push_v(
                &mut new_pos,
                &mut new_uvs,
                &mut new_vcol,
                poly[0].0,
                poly[0].1,
                poly[0].2,
            );
            let i1 = push_v(
                &mut new_pos,
                &mut new_uvs,
                &mut new_vcol,
                poly[k].0,
                poly[k].1,
                poly[k].2,
            );
            let i2 = push_v(
                &mut new_pos,
                &mut new_uvs,
                &mut new_vcol,
                poly[k + 1].0,
                poly[k + 1].1,
                poly[k + 1].2,
            );
            new_idx.extend_from_slice(&[i0, i1, i2]);
        }
    }

    mesh.positions = new_pos;
    mesh.indices = new_idx;
    mesh.uvs = if has_uv { Some(new_uvs) } else { None };
    mesh.vertex_colors = if has_vc { Some(new_vcol) } else { None };
    mesh.recompute_normals();
}

/// Box / cube UV mapping in world metres — each face uses its own plane so
/// sides and tops are not stretched like a single planar projection.
/// Rebuilds indices with unique verts per corner (shared verts can't hold two UVs).
pub fn ensure_box_uvs(mesh: &mut TriangleMesh, meters_per_tile: f32) {
    let tile = meters_per_tile.max(0.05);
    let old_pos = std::mem::take(&mut mesh.positions);
    let old_idx = std::mem::take(&mut mesh.indices);
    let old_n = std::mem::take(&mut mesh.normals);
    let old_vc = mesh.vertex_colors.take();
    let _ = mesh.uvs.take();

    let has_n = old_n.len() == old_pos.len();
    let has_vc = old_vc
        .as_ref()
        .map(|c| c.len() == old_pos.len())
        .unwrap_or(false);

    let mut new_pos: Vec<Vec3> = Vec::with_capacity(old_idx.len());
    let mut new_n: Vec<Vec3> = Vec::with_capacity(old_idx.len());
    let mut new_uv: Vec<[f32; 2]> = Vec::with_capacity(old_idx.len());
    let mut new_vc: Vec<[f32; 3]> = Vec::new();
    let mut new_idx: Vec<u32> = Vec::with_capacity(old_idx.len());

    for tri in old_idx.chunks_exact(3) {
        let ia = tri[0] as usize;
        let ib = tri[1] as usize;
        let ic = tri[2] as usize;
        if ia >= old_pos.len() || ib >= old_pos.len() || ic >= old_pos.len() {
            continue;
        }
        let pts = [old_pos[ia], old_pos[ib], old_pos[ic]];
        let face_n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalize_or_zero();
        let ax = face_n.x.abs();
        let ay = face_n.y.abs();
        let az = face_n.z.abs();

        let ids = [ia, ib, ic];
        for (corner, &src) in ids.iter().enumerate() {
            let p = pts[corner];
            // Project onto the plane most aligned with the face.
            let uv = if ay >= ax && ay >= az {
                // Top / bottom → XZ
                [p.x / tile, p.z / tile]
            } else if ax >= az {
                // ±X sides → ZY
                [p.z / tile, p.y / tile]
            } else {
                // ±Z front/back → XY
                [p.x / tile, p.y / tile]
            };
            let ni = new_pos.len() as u32;
            new_pos.push(p);
            new_n.push(if has_n { old_n[src] } else { face_n });
            new_uv.push(uv);
            if has_vc {
                new_vc.push(old_vc.as_ref().unwrap()[src]);
            }
            new_idx.push(ni);
        }
    }

    mesh.positions = new_pos;
    mesh.normals = new_n;
    mesh.indices = new_idx;
    mesh.uvs = Some(new_uv);
    mesh.vertex_colors = if has_vc { Some(new_vc) } else { None };
}

/// Default tile size for stone / paint textures (~50 cm per UV repeat).
pub const DEFAULT_TEXTURE_TILE_M: f32 = 0.5;
