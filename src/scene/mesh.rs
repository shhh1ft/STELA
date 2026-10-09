use glam::Vec3;

#[derive(Clone, Debug)]
pub struct TriangleMesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub indices: Vec<u32>,
    /// Flat material color (used when vertex_colors is None).
    pub color: [f32; 3],
    /// Optional per-vertex RGB (same length as positions).
    pub vertex_colors: Option<Vec<[f32; 3]>>,
    pub uvs: Option<Vec<[f32; 2]>>,
    /// Draw both sides (fences, thin panels). Safe with Z-buffer.
    pub double_sided: bool,
    /// Optional albedo texture path (from texture library).
    pub texture_path: Option<String>,
    /// Nearly unlit solid color (engraving fill + walls look the same).
    pub flat_shade: bool,
}

impl TriangleMesh {
    pub fn recompute_normals(&mut self) {
        self.normals = vec![Vec3::ZERO; self.positions.len()];
        for tri in self.indices.chunks_exact(3) {
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;
            if i0 >= self.positions.len()
                || i1 >= self.positions.len()
                || i2 >= self.positions.len()
            {
                continue;
            }
            let a = self.positions[i0];
            let b = self.positions[i1];
            let c = self.positions[i2];
            let n = (b - a).cross(c - a);
            self.normals[i0] += n;
            self.normals[i1] += n;
            self.normals[i2] += n;
        }
        for n in &mut self.normals {
            *n = n.normalize_or_zero();
        }
    }

    pub fn aabb(&self) -> Option<(Vec3, Vec3)> {
        if self.positions.is_empty() {
            return None;
        }
        let mut min = self.positions[0];
        let mut max = self.positions[0];
        for p in &self.positions {
            min = min.min(*p);
            max = max.max(*p);
        }
        Some((min, max))
    }

    pub fn centroid(&self) -> Vec3 {
        if self.positions.is_empty() {
            return Vec3::ZERO;
        }
        let sum: Vec3 = self.positions.iter().copied().sum();
        sum / self.positions.len() as f32
    }

    /// Flip triangle winding so face normals point away from the mesh centroid.
    pub fn ensure_outward_winding(&mut self) {
        let center = self.centroid();
        let mut flipped = 0usize;
        let mut total = 0usize;
        for tri in self.indices.chunks_exact_mut(3) {
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;
            if i0 >= self.positions.len()
                || i1 >= self.positions.len()
                || i2 >= self.positions.len()
            {
                continue;
            }
            let a = self.positions[i0];
            let b = self.positions[i1];
            let c = self.positions[i2];
            let n = (b - a).cross(c - a);
            let face_center = (a + b + c) / 3.0;
            total += 1;
            if n.dot(face_center - center) < 0.0 {
                tri.swap(1, 2);
                flipped += 1;
            }
        }
        // If most faces pointed inward, winding was globally reversed — already fixed per-face.
        let _ = (flipped, total);
        self.recompute_normals();
    }

    /// Unit cube 1×1×1 centered at origin, outward normals, for exact size fitting.
    pub fn unit_box(color: [f32; 3]) -> Self {
        // 24 unique vertices (4 per face) so normals stay sharp
        let faces: [(Vec3, [Vec3; 4]); 6] = [
            // +Z
            (
                Vec3::Z,
                [
                    Vec3::new(-0.5, -0.5, 0.5),
                    Vec3::new(0.5, -0.5, 0.5),
                    Vec3::new(0.5, 0.5, 0.5),
                    Vec3::new(-0.5, 0.5, 0.5),
                ],
            ),
            // -Z
            (
                -Vec3::Z,
                [
                    Vec3::new(0.5, -0.5, -0.5),
                    Vec3::new(-0.5, -0.5, -0.5),
                    Vec3::new(-0.5, 0.5, -0.5),
                    Vec3::new(0.5, 0.5, -0.5),
                ],
            ),
            // +X
            (
                Vec3::X,
                [
                    Vec3::new(0.5, -0.5, 0.5),
                    Vec3::new(0.5, -0.5, -0.5),
                    Vec3::new(0.5, 0.5, -0.5),
                    Vec3::new(0.5, 0.5, 0.5),
                ],
            ),
            // -X
            (
                -Vec3::X,
                [
                    Vec3::new(-0.5, -0.5, -0.5),
                    Vec3::new(-0.5, -0.5, 0.5),
                    Vec3::new(-0.5, 0.5, 0.5),
                    Vec3::new(-0.5, 0.5, -0.5),
                ],
            ),
            // +Y
            (
                Vec3::Y,
                [
                    Vec3::new(-0.5, 0.5, 0.5),
                    Vec3::new(0.5, 0.5, 0.5),
                    Vec3::new(0.5, 0.5, -0.5),
                    Vec3::new(-0.5, 0.5, -0.5),
                ],
            ),
            // -Y
            (
                -Vec3::Y,
                [
                    Vec3::new(-0.5, -0.5, -0.5),
                    Vec3::new(0.5, -0.5, -0.5),
                    Vec3::new(0.5, -0.5, 0.5),
                    Vec3::new(-0.5, -0.5, 0.5),
                ],
            ),
        ];

        let mut positions = Vec::with_capacity(24);
        let mut normals = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (n, corners) in faces {
            let base = positions.len() as u32;
            for c in corners {
                positions.push(c);
                normals.push(n);
            }
            // CCW when viewed from outside along normal
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self {
            positions,
            normals,
            indices,
            color,
            vertex_colors: None,
            uvs: None,
            double_sided: false,
            texture_path: None,
            flat_shade: false,
        }
    }
}

/// Fit mesh into an axis-aligned box of exact size (meters), bottom on y=base_y, centered at (cx, cz).
pub fn fit_to_size(
    meshes: &mut [TriangleMesh],
    target: Vec3,
    cx: f32,
    base_y: f32,
    cz: f32,
    yaw_rad: f32,
) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;
    for m in meshes.iter() {
        if let Some((lo, hi)) = m.aabb() {
            min = min.min(lo);
            max = max.max(hi);
            any = true;
        }
    }
    if !any || !min.is_finite() || !max.is_finite() {
        return;
    }
    let size = (max - min).max(Vec3::splat(1e-4));
    let scale = Vec3::new(target.x / size.x, target.y / size.y, target.z / size.z);
    let (sy, cy) = yaw_rad.sin_cos();
    for m in meshes.iter_mut() {
        for p in &mut m.positions {
            let local = Vec3::new(
                (p.x - (min.x + max.x) * 0.5) * scale.x,
                (p.y - min.y) * scale.y,
                (p.z - (min.z + max.z) * 0.5) * scale.z,
            );
            let rx = local.x * cy - local.z * sy;
            let rz = local.x * sy + local.z * cy;
            *p = Vec3::new(cx + rx, base_y + local.y, cz + rz);
        }
        // Non-uniform scale invalidates old normals — recompute from geometry
        m.ensure_outward_winding();
    }
}
