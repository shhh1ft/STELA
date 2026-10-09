use crate::app::StelaApp;
use crate::model::{FaceSide, TextAlign, TextStyle};
use crate::ui;
use eframe::egui::{self, Color32, RichText};

/// Modal text-engraving editor.
pub fn show(app: &mut StelaApp, ctx: &egui::Context) {
    let Some(mut editor) = app.text_editor.clone() else {
        return;
    };

    let mut open = true;
    let mut apply = false;
    let mut cancel = false;

    egui::Window::new("Эскиз текста — гравировка")
        .collapsible(false)
        .resizable(true)
        .default_size([720.0, 560.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "Текст вырезается в стеле на глубину гравировки (3–4 мм). \
                     Выберите шрифт из библиотеки и оформите надпись.",
                )
                .size(12.0)
                .color(Color32::from_rgb(170, 175, 185)),
            );
            ui.add_space(8.0);

            // —— Preview ——
            ui.group(|ui| {
                ui.set_min_height(140.0);
                ui.set_width(ui.available_width());
                let (rect, _resp) =
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), 130.0), egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 4.0, Color32::from_rgb(48, 50, 54));
                // Stone-like band
                let band = egui::Rect::from_center_size(
                    rect.center(),
                    egui::vec2(rect.width() * 0.88, rect.height() * 0.72),
                );
                painter.rect_filled(band, 2.0, Color32::from_rgb(70, 72, 76));

                let style = &editor.style;
                let color = Color32::from_rgb(
                    (style.color[0] * 255.0) as u8,
                    (style.color[1] * 255.0) as u8,
                    (style.color[2] * 255.0) as u8,
                );
                let mut font = egui::FontId::proportional((18.0 + style.letter_cm * 2.5).clamp(14.0, 42.0));
                if style.bold {
                    // egui has no real bold for custom fonts — slightly larger
                    font.size *= 1.05;
                }
                let lines: Vec<&str> = editor.content.lines().collect();
                let line_h = font.size * style.line_gap.max(1.0);
                let total_h = lines.len().max(1) as f32 * line_h;
                let mut y = band.center().y - total_h * 0.5 + line_h * 0.5;
                for line in lines {
                    let anchor = match style.align {
                        TextAlign::Left => egui::Align2::LEFT_CENTER,
                        TextAlign::Center => egui::Align2::CENTER_CENTER,
                        TextAlign::Right => egui::Align2::RIGHT_CENTER,
                    };
                    let x = match style.align {
                        TextAlign::Left => band.left() + 16.0,
                        TextAlign::Center => band.center().x,
                        TextAlign::Right => band.right() - 16.0,
                    };
                    // Engraved look: dark recess + slight offset shadow
                    painter.text(
                        egui::pos2(x + 1.0, y + 1.0),
                        anchor,
                        line,
                        font.clone(),
                        Color32::from_rgba_unmultiplied(0, 0, 0, 180),
                    );
                    painter.text(egui::pos2(x, y), anchor, line, font.clone(), color);
                    y += line_h;
                }
                if editor.content.trim().is_empty() {
                    painter.text(
                        band.center(),
                        egui::Align2::CENTER_CENTER,
                        "Предпросмотр надписи",
                        egui::FontId::proportional(16.0),
                        Color32::GRAY,
                    );
                }
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("Сторона стелы:");
                for side in FaceSide::all() {
                    if ui.selectable_label(editor.side == *side, side.label()).clicked() {
                        editor.side = *side;
                    }
                }
            });
            ui.add_space(6.0);

            // —— Text ——
            ui.label(RichText::new("Текст").strong());
            ui.add(
                egui::TextEdit::multiline(&mut editor.content)
                    .desired_width(ui.available_width())
                    .desired_rows(5)
                    .hint_text("Фамилия Имя Отчество\n1950 — 2020"),
            );

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                // Bold / Italic
                if ui
                    .selectable_label(editor.style.bold, RichText::new("Ж").strong())
                    .on_hover_text("Жирный")
                    .clicked()
                {
                    editor.style.bold = !editor.style.bold;
                }
                if ui
                    .selectable_label(
                        editor.style.italic,
                        RichText::new("К").italics(),
                    )
                    .on_hover_text("Курсив")
                    .clicked()
                {
                    editor.style.italic = !editor.style.italic;
                }
                ui.separator();
                ui.label("Выравнивание:");
                for a in TextAlign::all() {
                    if ui
                        .selectable_label(editor.style.align == *a, a.label())
                        .clicked()
                    {
                        editor.style.align = *a;
                    }
                }
            });

            ui.add_space(6.0);
            // Font from library
            ui.horizontal(|ui| {
                ui.label("Шрифт:");
                let current_name = editor
                    .style
                    .font_id
                    .as_ref()
                    .and_then(|id| {
                        app.library
                            .fonts
                            .iter()
                            .find(|f| &f.id == id)
                            .map(|f| f.name.clone())
                    })
                    .or_else(|| {
                        editor
                            .style
                            .font_path
                            .as_ref()
                            .and_then(|p| {
                                std::path::Path::new(p)
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .map(|s| s.to_string())
                            })
                    })
                    .unwrap_or_else(|| "Системный".into());
                egui::ComboBox::from_id_salt("text_editor_font")
                    .selected_text(current_name)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for font in app.library.fonts.clone() {
                            let sel = editor.style.font_id.as_deref() == Some(font.id.as_str());
                            if ui.selectable_label(sel, &font.name).clicked() {
                                editor.style.font_id = Some(font.id);
                                editor.style.font_path = Some(font.file_path);
                            }
                        }
                    });
                if ui.button("Библиотека шрифтов…").clicked() {
                    app.library_tab = crate::app::LibraryTab::Fonts;
                    app.screen = crate::ui::Screen::Library;
                    // keep editor open when returning — user can come back
                }
            });

            ui.add_space(4.0);
            ui.add(
                egui::Slider::new(&mut editor.style.letter_cm, 1.0..=20.0)
                    .text("Высота буквы")
                    .suffix(" см"),
            );
            ui.add(
                egui::Slider::new(&mut editor.style.depth_mm, 1.5..=8.0)
                    .text("Глубина гравировки")
                    .suffix(" мм"),
            );
            ui.add(
                egui::Slider::new(&mut editor.style.line_gap, 1.0..=2.2)
                    .text("Межстрочный интервал"),
            );

            ui.label(
                RichText::new("Цвет гравировки (дно и стенки одинаковые)")
                    .size(11.0)
                    .color(Color32::GRAY),
            );
            ui.horizontal(|ui| {
                let mut rgb = [
                    editor.style.color[0],
                    editor.style.color[1],
                    editor.style.color[2],
                ];
                if ui.color_edit_button_rgb(&mut rgb).changed() {
                    editor.style.color = rgb;
                }
                if ui.small_button("Тёмный камень").clicked() {
                    editor.style.color = [0.10, 0.10, 0.11];
                }
                if ui.small_button("Золото").clicked() {
                    editor.style.color = [0.72, 0.58, 0.28];
                }
                if ui.small_button("Серебро").clicked() {
                    editor.style.color = [0.72, 0.74, 0.76];
                }
            });

            ui.add_space(14.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui::bronze_button(ui, "Применить").clicked() {
                    apply = true;
                }
                if ui.button("Отмена").clicked() {
                    cancel = true;
                }
                ui.label(
                    RichText::new("Esc — отмена")
                        .size(11.0)
                        .color(Color32::GRAY),
                );
            });
        });

    if apply {
        app.text_editor = Some(editor);
        app.apply_text_editor();
    } else if cancel || !open {
        app.text_editor = None;
    } else if ctx.input(|i| i.key_pressed(egui::Key::Escape))
        && app.confirm.is_none()
        && app.library_picker.is_none()
    {
        app.text_editor = None;
    } else {
        app.text_editor = Some(editor);
    }
}

/// Draft state for the text engraving editor window.
#[derive(Clone, Debug)]
pub struct TextEditorState {
    pub part_id: String,
    /// None = create new decal; Some = edit existing.
    pub decal_id: Option<String>,
    pub content: String,
    pub style: TextStyle,
    pub name: String,
    pub side: FaceSide,
}

impl TextEditorState {
    pub fn new_for_part(part_id: impl Into<String>, draft: &str) -> Self {
        Self {
            part_id: part_id.into(),
            decal_id: None,
            content: draft.to_string(),
            style: TextStyle::default(),
            name: String::new(),
            side: FaceSide::Front,
        }
    }
}
