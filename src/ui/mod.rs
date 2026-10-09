pub mod home;
pub mod library;
pub mod photo_editor;
pub mod studio;
pub mod text_editor;

pub use photo_editor::PhotoEditorState;
pub use text_editor::TextEditorState;

use eframe::egui::{self, Color32, RichText, Vec2, Visuals};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Studio,
    Library,
}

pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.window_fill = Color32::from_rgb(22, 24, 28);
    visuals.panel_fill = Color32::from_rgb(18, 20, 24);
    visuals.extreme_bg_color = Color32::from_rgb(14, 16, 20);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(36, 40, 48);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 48, 36);
    visuals.widgets.active.bg_fill = Color32::from_rgb(140, 90, 40);
    visuals.selection.bg_fill = Color32::from_rgb(140, 90, 40);
    visuals.override_text_color = Some(Color32::from_rgb(232, 228, 220));
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    ctx.set_style(style);
}

pub fn bronze_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::from_rgb(255, 244, 230)))
            .fill(Color32::from_rgb(160, 95, 35))
            .min_size(Vec2::new(120.0, 32.0)),
    )
}

/// Drawn marks — glyphs, not a font, so they stay visible on every system.
#[derive(Clone, Copy)]
pub enum Mark {
    Projects,
    Library,
    Save,
    Plus,
    Photo,
    Text,
    Back,
}

pub fn nav_button(ui: &mut egui::Ui, mark: Mark, label: &str, bronze: bool) -> egui::Response {
    let text_color = Color32::from_rgb(255, 244, 230);
    let galley = ui.ctx().fonts(|f| {
        f.layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(14.0),
            text_color,
        )
    });
    let h = 32.0_f32.max(galley.size().y + 12.0);
    let w = 18.0 + 16.0 + galley.size().x + 14.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::click());
    let fill = if bronze {
        if resp.hovered() {
            Color32::from_rgb(184, 112, 48)
        } else {
            Color32::from_rgb(160, 95, 35)
        }
    } else if resp.hovered() {
        Color32::from_rgb(55, 48, 36)
    } else {
        Color32::from_rgb(36, 40, 48)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, fill);
    let icon_c = rect.left_center() + Vec2::new(16.0, 0.0);
    paint_mark(painter, icon_c, mark, Color32::from_rgb(255, 214, 160));
    painter.galley(
        egui::pos2(
            rect.left() + 30.0,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        text_color,
    );
    resp
}

fn paint_mark(p: &egui::Painter, c: egui::Pos2, mark: Mark, color: Color32) {
    let stroke = egui::Stroke::new(1.6_f32, color);
    match mark {
        Mark::Projects => {
            let back = egui::Rect::from_center_size(c + Vec2::new(-2.0, 2.0), Vec2::new(12.0, 9.0));
            let front = egui::Rect::from_center_size(c + Vec2::new(2.5, -2.0), Vec2::new(12.0, 9.0));
            p.rect_stroke(back, 1.0, stroke);
            p.rect_filled(front, 1.0, Color32::from_rgb(28, 26, 22));
            p.rect_stroke(front, 1.0, stroke);
        }
        Mark::Library => {
            let r = egui::Rect::from_center_size(c, Vec2::new(13.0, 12.0));
            p.rect_stroke(r, 1.0, stroke);
            let y0 = r.top() + 4.0;
            p.line_segment([egui::pos2(r.left() + 2.0, y0), egui::pos2(r.right() - 2.0, y0)], stroke);
            p.line_segment(
                [egui::pos2(r.left() + 2.0, y0 + 3.5), egui::pos2(r.right() - 2.0, y0 + 3.5)],
                stroke,
            );
        }
        Mark::Save => {
            let r = egui::Rect::from_center_size(c, Vec2::new(12.0, 12.0));
            p.rect_stroke(r, 1.0, stroke);
            let notch = egui::Rect::from_min_size(
                egui::pos2(r.center().x - 3.0, r.top()),
                Vec2::new(6.0, 4.0),
            );
            p.rect_filled(notch, 0.0, color);
        }
        Mark::Plus => {
            p.line_segment([c + Vec2::new(-6.0, 0.0), c + Vec2::new(6.0, 0.0)], stroke);
            p.line_segment([c + Vec2::new(0.0, -6.0), c + Vec2::new(0.0, 6.0)], stroke);
        }
        Mark::Photo => {
            p.circle_stroke(c, 6.5, stroke);
            p.circle_filled(c, 2.2, color);
        }
        Mark::Text => {
            for i in -1..=1 {
                let y = c.y + i as f32 * 4.0;
                p.line_segment(
                    [egui::pos2(c.x - 6.0, y), egui::pos2(c.x + 6.0, y)],
                    stroke,
                );
            }
        }
        Mark::Back => {
            p.line_segment([c + Vec2::new(4.0, -6.0), c + Vec2::new(-4.0, 0.0)], stroke);
            p.line_segment([c + Vec2::new(-4.0, 0.0), c + Vec2::new(4.0, 6.0)], stroke);
        }
    }
}
