use crate::app::StelaApp;
use crate::model::{
    FaceSide, MedallionMaterial, MedallionShape, MountType, PhotoSpec, TextAlign,
};
use crate::ui;
use eframe::egui::{self, Color32, RichText};

/// Modal photoceramic editor: shape, size, mount, cutout, background, caption.
pub fn show(app: &mut StelaApp, ctx: &egui::Context) {
    let Some(mut editor) = app.photo_editor.clone() else {
        return;
    };

    normalize_spec(&mut editor.spec);
    if let Some(id) = editor.spec.background_id.clone() {
        let ok = app
            .library
            .backgrounds
            .iter()
            .any(|b| b.id == id && b.shape == editor.spec.shape);
        if !ok {
            editor.spec.background_id = None;
            editor.spec.background_path = None;
        }
    }

    let mut open = true;
    let mut apply = false;
    let mut cancel = false;
    let mut pick_photo = false;
    let mut open_backgrounds = false;

    egui::Window::new("Фотокерамика")
        .collapsible(false)
        .resizable(true)
        .default_size([760.0, 640.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(&mut open)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(
                    RichText::new(
                        "Медальон — объёмная пластина на выбранной стороне стелы. \
                         Портрет вырезается с фона фото, кладётся на фон формы и подписывается. \
                         Портрет гравировкой по камню — отдельно, позже.",
                    )
                    .size(12.0)
                    .color(Color32::from_rgb(170, 175, 185)),
                );
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Сторона стелы:");
                    for side in FaceSide::all() {
                        if ui.selectable_label(editor.side == *side, side.label()).clicked() {
                            editor.side = *side;
                        }
                    }
                });

                ui.add_space(6.0);
                ui.label(RichText::new("Материал").strong());
                ui.horizontal(|ui| {
                    for m in MedallionMaterial::all() {
                        if ui
                            .selectable_label(editor.spec.material == *m, m.label())
                            .clicked()
                        {
                            editor.spec.material = *m;
                            normalize_spec(&mut editor.spec);
                        }
                    }
                });
                ui.label(
                    RichText::new(match editor.spec.material {
                        MedallionMaterial::MetalCeramic => {
                            "Металлокерамика тонкая, обычно небольшая и всегда на саморезах с герметизацией."
                        }
                        MedallionMaterial::Porcelain => {
                            "Фарфор тоньше керамогранита. Крепление — в нишу или на поверхность, на клей."
                        }
                        MedallionMaterial::CeramicGranite => {
                            "Керамогранит — самый стойкий. Ниша с клеем держит лучше, чем клей по поверхности."
                        }
                    })
                    .size(11.0)
                    .color(Color32::GRAY),
                );

                ui.add_space(6.0);
                ui.label(RichText::new("Форма").strong());
                ui.horizontal(|ui| {
                    for shape in MedallionShape::all() {
                        if !shape.available_for(editor.spec.material) {
                            continue;
                        }
                        if ui
                            .selectable_label(editor.spec.shape == *shape, shape.label())
                            .clicked()
                        {
                            if editor.spec.shape != *shape {
                                editor.spec.background_id = None;
                                editor.spec.background_path = None;
                            }
                            editor.spec.shape = *shape;
                            normalize_spec(&mut editor.spec);
                        }
                    }
                });

                ui.add_space(4.0);
                ui.label(RichText::new("Размер, см").strong());
                ui.horizontal_wrapped(|ui| {
                    let sizes = editor.spec.shape.sizes_cm(editor.spec.material);
                    for (w, h) in sizes {
                        let on = (editor.spec.width_cm - w).abs() < 0.05
                            && (editor.spec.height_cm - h).abs() < 0.05;
                        if ui
                            .selectable_label(on, format!("{w:.0}×{h:.0}"))
                            .clicked()
                        {
                            editor.spec.width_cm = *w;
                            editor.spec.height_cm = *h;
                        }
                    }
                });

                ui.add_space(4.0);
                ui.label(RichText::new("Крепление").strong());
                ui.horizontal(|ui| {
                    for mount in [MountType::Niche, MountType::SurfaceGlue, MountType::Screws] {
                        if !mount.available_for(editor.spec.material) {
                            continue;
                        }
                        if ui
                            .selectable_label(editor.spec.mount == mount, mount.label())
                            .clicked()
                        {
                            editor.spec.mount = mount;
                        }
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.label(RichText::new("Портрет").strong());
                ui.label(
                    RichText::new(&editor.path)
                        .size(11.0)
                        .color(Color32::GRAY),
                );
                if ui.button("Сменить фото…").clicked() {
                    pick_photo = true;
                }
                ui.checkbox(
                    &mut editor.spec.remove_background,
                    "Вырезать человека (убрать фон с краёв фото)",
                );
                ui.add(
                    egui::Slider::new(&mut editor.spec.bg_tolerance, 0.05..=0.8)
                        .text("Допуск фона"),
                );
                ui.add(
                    egui::Slider::new(&mut editor.spec.photo_scale, 0.35..=2.2).text("Масштаб портрета"),
                );
                ui.add(egui::Slider::new(&mut editor.spec.photo_dx, -1.0..=1.0).text("Сдвиг влево–вправо"));
                ui.add(egui::Slider::new(&mut editor.spec.photo_dy, -1.0..=1.0).text("Сдвиг вниз–вверх"));

                ui.add_space(8.0);
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Фон · библиотека «{}»",
                        editor.spec.shape.label()
                    ))
                    .strong(),
                );
                ui.label(
                    RichText::new("У каждой формы своя библиотека фонов.")
                        .size(11.0)
                        .color(Color32::GRAY),
                );
                let shape = editor.spec.shape;
                let backgrounds: Vec<_> = app
                    .library
                    .backgrounds
                    .iter()
                    .filter(|b| b.shape == shape)
                    .cloned()
                    .collect();
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(editor.spec.background_id.is_none(), "Без фона")
                        .clicked()
                    {
                        editor.spec.background_id = None;
                        editor.spec.background_path = None;
                    }
                    if ui.button("Библиотека фонов…").clicked() {
                        open_backgrounds = true;
                    }
                });
                if backgrounds.is_empty() {
                    ui.label(
                        RichText::new("Для этой формы фонов пока нет — будет студийный градиент.")
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    for bg in &backgrounds {
                        let on = editor.spec.background_id.as_deref() == Some(bg.id.as_str());
                        if ui.selectable_label(on, &bg.name).clicked() {
                            editor.spec.background_id = Some(bg.id.clone());
                            editor.spec.background_path = Some(bg.file_path.clone());
                        }
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.label(RichText::new("Подпись на медальоне").strong());
                ui.add(
                    egui::TextEdit::multiline(&mut editor.spec.text.content)
                        .desired_rows(3)
                        .desired_width(ui.available_width())
                        .hint_text("Иванов Иван Иванович\n01.01.1950 — 01.01.2020"),
                );
                ui.horizontal(|ui| {
                    ui.label("Шрифт:");
                    let current = editor
                        .spec
                        .text
                        .font_id
                        .as_ref()
                        .and_then(|id| {
                            app.library
                                .fonts
                                .iter()
                                .find(|f| &f.id == id)
                                .map(|f| f.name.clone())
                        })
                        .unwrap_or_else(|| "Системный".into());
                    egui::ComboBox::from_id_salt("photo_caption_font")
                        .selected_text(current)
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for font in app.library.fonts.clone() {
                                let sel =
                                    editor.spec.text.font_id.as_deref() == Some(font.id.as_str());
                                if ui.selectable_label(sel, &font.name).clicked() {
                                    editor.spec.text.font_id = Some(font.id);
                                    editor.spec.text.font_path = Some(font.file_path);
                                }
                            }
                        });
                    for a in TextAlign::all() {
                        if ui
                            .selectable_label(editor.spec.text.align == *a, a.label())
                            .clicked()
                        {
                            editor.spec.text.align = *a;
                        }
                    }
                });
                ui.add(
                    egui::Slider::new(&mut editor.spec.text.letter_cm, 0.4..=2.4)
                        .text("Высота буквы")
                        .suffix(" см"),
                );
                ui.horizontal(|ui| {
                    ui.label("Цвет подписи:");
                    ui.color_edit_button_rgb(&mut editor.spec.text.color);
                });

                ui.add_space(8.0);
                ui.label("Имя слоя:");
                ui.add(
                    egui::TextEdit::singleline(&mut editor.name)
                        .desired_width(ui.available_width()),
                );

                ui.add_space(12.0);
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
        });

    if pick_photo {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Портрет для медальона")
            .add_filter("Изображения", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        {
            editor.path = path.to_string_lossy().into_owned();
            if editor.name.trim().is_empty() {
                editor.name = std::path::Path::new(&editor.path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Фото")
                    .to_string();
            }
        }
    }

    if open_backgrounds {
        app.library_tab = crate::app::LibraryTab::Backgrounds;
        app.background_shape = editor.spec.shape;
        app.screen = crate::ui::Screen::Library;
    }

    if apply {
        app.photo_editor = Some(editor);
        app.apply_photo_editor();
    } else if cancel || !open {
        app.photo_editor = None;
    } else if ctx.input(|i| i.key_pressed(egui::Key::Escape))
        && app.confirm.is_none()
        && app.library_picker.is_none()
    {
        app.photo_editor = None;
    } else {
        app.photo_editor = Some(editor);
    }
}

fn normalize_spec(spec: &mut PhotoSpec) {
    if !spec.shape.available_for(spec.material) {
        spec.shape = MedallionShape::all()
            .iter()
            .copied()
            .find(|s| s.available_for(spec.material))
            .unwrap_or(MedallionShape::Oval);
        spec.background_id = None;
        spec.background_path = None;
    }
    if !spec.mount.available_for(spec.material) {
        spec.mount = MountType::default_for(spec.material);
    }
    let sizes = spec.shape.sizes_cm(spec.material);
    let matches = sizes.iter().any(|(w, h)| {
        (*w - spec.width_cm).abs() < 0.05 && (*h - spec.height_cm).abs() < 0.05
    });
    if !matches {
        if let Some((w, h)) = sizes.first() {
            spec.width_cm = *w;
            spec.height_cm = *h;
        }
    }
}

#[derive(Clone, Debug)]
pub struct PhotoEditorState {
    pub part_id: String,
    pub decal_id: Option<String>,
    pub path: String,
    pub name: String,
    pub side: FaceSide,
    pub spec: PhotoSpec,
}
