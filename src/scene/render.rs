use crate::scene::mesh::TriangleMesh;
use crate::scene::raster::rasterize_scene;
use egui::{Color32, FontId, Mesh, Rect, Sense, TextureOptions, Ui, Vec2};
use glam::{Mat4, Vec3};

fn finite_available(ui: &Ui) -> Vec2 {
    let mut size = ui.available_size();
    if !size.x.is_finite() || size.x < 1.0 {
        size.x = 480.0;
    }
    if !size.y.is_finite() || size.y < 1.0 {
        size.y = 480.0;
    }
    Vec2::new(size.x.clamp(64.0, 8192.0), size.y.clamp(64.0, 8192.0))
}

#[derive(Clone, Debug)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            yaw: 0.55,
            pitch: 0.42,
            distance: 4.5,
            target: Vec3::new(0.0, 0.7, 0.0),
        }
    }
}

impl OrbitCamera {
    pub fn eye(&self) -> Vec3 {
        let cp = self.pitch.cos();
        self.target
            + Vec3::new(
                self.distance * cp * self.yaw.sin(),
                self.distance * self.pitch.sin(),
                self.distance * cp * self.yaw.cos(),
            )
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye(), self.target, Vec3::Y);
        let proj = Mat4::perspective_rh(0.65, aspect.max(0.1), 0.05, 80.0);
        proj * view
    }

    pub fn frame_height(&mut self, max_y: f32) {
        self.target.y = (max_y * 0.45).clamp(0.3, 2.5);
        self.distance = (max_y * 2.4 + 1.8).clamp(2.5, 16.0);
    }
}

pub fn draw_viewport(
    ui: &mut Ui,
    meshes: &[TriangleMesh],
    camera: &mut OrbitCamera,
    ground: bool,
    plot_w_m: f32,
    plot_l_m: f32,
    plot_texture: Option<&str>,
    warning: Option<&str>,
) {
    let available = finite_available(ui);
    let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
    let rect = response.rect;

    if response.dragged() {
        let delta = response.drag_delta();
        camera.yaw -= delta.x * 0.01;
        camera.pitch = (camera.pitch + delta.y * 0.01).clamp(-1.15, 1.35);
    }
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if response.hovered() && scroll != 0.0 {
        camera.distance = (camera.distance * (1.0 - scroll * 0.0015)).clamp(1.2, 24.0);
    }

    let aspect = rect.width() / rect.height().max(1.0);
    let vp = camera.view_proj(aspect);
    let eye = camera.eye();

    // 1:1 with display pixels (respect HiDPI). Cap to keep CPU raster usable.
    let ppp = ui.ctx().pixels_per_point();
    let mut rw = (rect.width() * ppp).round().clamp(320.0, 1920.0) as usize;
    let mut rh = (rect.height() * ppp).round().clamp(240.0, 1200.0) as usize;
    // Keep aspect of the widget
    let widget_aspect = rect.width() / rect.height().max(1.0);
    let buf_aspect = rw as f32 / rh as f32;
    if (buf_aspect - widget_aspect).abs() > 0.02 {
        if buf_aspect > widget_aspect {
            rw = (rh as f32 * widget_aspect).round() as usize;
        } else {
            rh = (rw as f32 / widget_aspect).round() as usize;
        }
    }
    rw = rw.max(160);
    rh = rh.max(120);

    // 2× SSAA when not orbiting — smooth edges without potato upscale
    let dragging = response.dragged();
    let pixel_budget = rw * rh;
    let ssaa2 = !dragging && pixel_budget <= 900_000;

    let (color_image, stats) = rasterize_scene(
        meshes,
        vp,
        eye,
        rw,
        rh,
        ground,
        ssaa2,
        plot_w_m,
        plot_l_m,
        plot_texture,
    );
    let tex = ui.ctx().load_texture(
        "stela-viewport-zbuffer",
        color_image,
        TextureOptions::LINEAR,
    );

    // Draw rasterized scene stretched to the painter rect
    let mut img_mesh = Mesh::with_texture(tex.id());
    let i = 0u32;
    let white = Color32::WHITE;
    let corners = [
        (rect.left_top(), egui::pos2(0.0, 0.0)),
        (rect.right_top(), egui::pos2(1.0, 0.0)),
        (rect.right_bottom(), egui::pos2(1.0, 1.0)),
        (rect.left_bottom(), egui::pos2(0.0, 1.0)),
    ];
    for (p, uv) in corners {
        img_mesh.colored_vertex(p, white);
        img_mesh.vertices.last_mut().unwrap().uv = uv;
    }
    img_mesh
        .indices
        .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    painter.add(egui::Shape::mesh(img_mesh));

    let badge_bg = Color32::from_rgba_unmultiplied(20, 22, 26, 210);
    let badge = format!(
        "{} / {} граней · {}×{}{}",
        stats.drawn,
        stats.triangles,
        rw,
        rh,
        if ssaa2 { " · SSAA" } else { "" }
    );
    let galley = painter.layout_no_wrap(
        badge,
        FontId::proportional(12.0),
        Color32::from_rgb(240, 235, 225),
    );
    let r1 = Rect::from_min_size(
        rect.left_top() + Vec2::new(10.0, 10.0),
        galley.size() + Vec2::splat(12.0),
    );
    painter.rect_filled(r1, 4.0, badge_bg);
    painter.galley(r1.min + Vec2::splat(6.0), galley, Color32::WHITE);

    if let Some(w) = warning {
        let g = painter.layout_no_wrap(
            w.to_string(),
            FontId::proportional(11.0),
            Color32::from_rgb(255, 214, 140),
        );
        let wr = Rect::from_min_size(
            rect.left_bottom() + Vec2::new(10.0, -28.0),
            g.size() + Vec2::splat(10.0),
        );
        painter.rect_filled(wr, 4.0, badge_bg);
        painter.galley(wr.min + Vec2::splat(5.0), g, Color32::WHITE);
    }

    let hint = painter.layout_no_wrap(
        "ЛКМ — орбита · колесо — зум · Delete — удалить".into(),
        FontId::proportional(11.0),
        Color32::from_rgb(160, 165, 175),
    );
    painter.galley(
        rect.right_bottom() - hint.size() - Vec2::new(12.0, 14.0),
        hint,
        Color32::WHITE,
    );
}

