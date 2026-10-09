use crate::app::{LibraryTab, StelaApp};
use crate::library;
use crate::model::{MedallionShape, PartRole};
use crate::ui::{self, Screen};
use eframe::egui::{self, Color32, RichText, Vec2};

pub fn show(app: &mut StelaApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("lib_top").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui::nav_button(ui, ui::Mark::Back, "Назад", false).clicked() {
                app.screen = if app.active_project().is_some() {
                    Screen::Studio
                } else {
                    Screen::Home
                };
            }
            ui.separator();
            ui.label(RichText::new("Библиотека").strong());
            ui.separator();
            if ui
                .selectable_label(app.library_tab == LibraryTab::Models, "Модели")
                .clicked()
            {
                app.library_tab = LibraryTab::Models;
            }
            if ui
                .selectable_label(app.library_tab == LibraryTab::Textures, "Текстуры")
                .clicked()
            {
                app.library_tab = LibraryTab::Textures;
            }
            if ui
                .selectable_label(app.library_tab == LibraryTab::Fonts, "Шрифты")
                .clicked()
            {
                app.library_tab = LibraryTab::Fonts;
            }
            if ui
                .selectable_label(app.library_tab == LibraryTab::Backgrounds, "Фоны")
                .clicked()
            {
                app.library_tab = LibraryTab::Backgrounds;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{}", library::library_dir().display()))
                        .size(11.0)
                        .color(Color32::GRAY),
                );
            });
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        match app.library_tab {
            LibraryTab::Models => show_models(app, ui),
            LibraryTab::Textures => show_textures(app, ui),
            LibraryTab::Fonts => show_fonts(app, ui),
            LibraryTab::Backgrounds => show_backgrounds(app, ui),
        }
        ui.add_space(8.0);
        ui.label(RichText::new(&app.status).size(11.0).color(Color32::GRAY));
    });
}

fn show_models(app: &mut StelaApp, ui: &mut egui::Ui) {
    ui.label(
        RichText::new(
            "Модели хранятся в папке библиотеки. Добавляйте OBJ / glTF / GLB из Blender,\n\
             задавайте название и роль. Превью строится автоматически.",
        )
        .color(Color32::from_rgb(170, 175, 185)),
    );
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label("Название:");
        ui.add(
            egui::TextEdit::singleline(&mut app.new_model_name)
                .desired_width(180.0)
                .hint_text("из имени файла, если пусто"),
        );
        ui.label("Роль:");
        egui::ComboBox::from_id_salt("catalog_role")
            .selected_text(app.catalog_role.label())
            .show_ui(ui, |ui| {
                for role in PartRole::all() {
                    ui.selectable_value(&mut app.catalog_role, *role, role.label());
                }
            });
        if ui::bronze_button(ui, "Добавить модель…").clicked() {
            app.import_to_library();
        }
    });

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(8.0);

    if app.library.items.is_empty() {
        ui.label(
            RichText::new("Библиотека пуста. Добавьте первую модель из Blender.")
                .color(Color32::GRAY),
        );
    }

    let items = app.library.items.clone();
    egui::ScrollArea::vertical().show(ui, |ui| {
        let width = ui.available_width();
        let cols = ((width / 200.0).floor() as usize).clamp(1, 4);
        let mut col_i = 0;
        ui.columns(cols, |columns| {
            for item in &items {
                let ui = &mut columns[col_i % cols];
                col_i += 1;

                ui.group(|ui| {
                    ui.set_min_width(170.0);
                    ui.set_min_height(200.0);

                    let preview = app.preview_cache.get(ui.ctx(), item);
                    let size = Vec2::splat(140.0);
                    if let Some(tex) = preview {
                        ui.image((tex.id(), size));
                    } else {
                        let (resp, painter) = ui.allocate_painter(size, egui::Sense::hover());
                        painter.rect_filled(resp.rect, 4.0, Color32::from_rgb(40, 44, 52));
                        painter.text(
                            resp.rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "нет превью",
                            egui::FontId::proportional(12.0),
                            Color32::GRAY,
                        );
                    }

                    ui.label(RichText::new(&item.name).strong().size(14.0));
                    ui.label(
                        RichText::new(item.role.label())
                            .size(11.0)
                            .color(Color32::from_rgb(200, 180, 140)),
                    );

                    ui.horizontal(|ui| {
                        if ui::bronze_button(ui, "На участок").clicked() {
                            app.add_part_from_catalog(&item.id);
                            if app.active_project().is_some() {
                                app.screen = Screen::Studio;
                            }
                        }
                        if ui.button("Удалить").clicked() {
                            app.request_delete_library_item(&item.id);
                        }
                    });
                });
                ui.add_space(8.0);
            }
        });
    });
}

fn show_textures(app: &mut StelaApp, ui: &mut egui::Ui) {
    ui.label(
        RichText::new(
            "Текстуры группируются в отдельные библиотеки (гранит, мрамор, металл…).\n\
             Назначьте текстуру детали в свойствах на участке.",
        )
        .color(Color32::from_rgb(170, 175, 185)),
    );
    ui.add_space(10.0);

    // Categories (texture libraries)
    ui.label(RichText::new("Библиотеки текстур").strong());
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.new_texture_category_name)
                .desired_width(160.0)
                .hint_text("Напр. Гранит"),
        );
        if ui::bronze_button(ui, "+ Библиотека").clicked() {
            app.add_texture_category();
        }
    });

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        let all_sel = app.texture_category_filter.is_none();
        if ui.selectable_label(all_sel, "Все").clicked() {
            app.texture_category_filter = None;
        }
        let cats = app.library.texture_categories.clone();
        for cat in &cats {
            let sel = app.texture_category_filter.as_deref() == Some(cat.id.as_str());
            if ui.selectable_label(sel, &cat.name).clicked() {
                app.texture_category_filter = Some(cat.id.clone());
            }
        }
    });

    if let Some(cid) = app.texture_category_filter.clone() {
        let name = app
            .library
            .texture_categories
            .iter()
            .find(|c| c.id == cid)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Текущая: {name}"))
                    .size(12.0)
                    .color(Color32::from_rgb(200, 190, 160)),
            );
            if ui.small_button("Удалить библиотеку").clicked() {
                app.request_delete_texture_category(&cid);
            }
        });
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label("Название:");
        ui.add(
            egui::TextEdit::singleline(&mut app.new_texture_name)
                .desired_width(180.0)
                .hint_text("из имени файла, если пусто"),
        );
        if ui::bronze_button(ui, "Добавить текстуру…").clicked() {
            app.import_texture_to_library();
        }
    });
    if app.texture_category_filter.is_none() {
        ui.label(
            RichText::new("Текстура попадёт без категории (или выберите библиотеку выше).")
                .size(11.0)
                .color(Color32::GRAY),
        );
    }

    ui.add_space(12.0);

    let filter = app.texture_category_filter.clone();
    let items: Vec<_> = app
        .library
        .textures
        .iter()
        .filter(|t| {
            filter
                .as_ref()
                .map(|cid| t.category_id.as_ref() == Some(cid))
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    if items.is_empty() {
        ui.label(
            RichText::new("Нет текстур в этой библиотеке. Добавьте PNG / JPG / WebP.")
                .color(Color32::GRAY),
        );
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        let width = ui.available_width();
        let cols = ((width / 180.0).floor() as usize).clamp(1, 5);
        let mut col_i = 0;
        ui.columns(cols, |columns| {
            for item in &items {
                let ui = &mut columns[col_i % cols];
                col_i += 1;

                ui.group(|ui| {
                    ui.set_min_width(150.0);
                    let preview = app.preview_cache.get_texture(ui.ctx(), item);
                    let size = Vec2::splat(120.0);
                    if let Some(tex) = preview {
                        ui.image((tex.id(), size));
                    } else {
                        let (resp, painter) = ui.allocate_painter(size, egui::Sense::hover());
                        painter.rect_filled(resp.rect, 4.0, Color32::from_rgb(40, 44, 52));
                    }
                    ui.label(RichText::new(&item.name).strong().size(13.0));
                    if let Some(cid) = &item.category_id {
                        let cat_name = app
                            .library
                            .texture_categories
                            .iter()
                            .find(|c| &c.id == cid)
                            .map(|c| c.name.as_str())
                            .unwrap_or("—");
                        ui.label(
                            RichText::new(cat_name)
                                .size(11.0)
                                .color(Color32::from_rgb(200, 180, 140)),
                        );
                    }
                    if ui.button("Удалить").clicked() {
                        app.request_delete_texture(&item.id);
                    }
                });
                ui.add_space(6.0);
            }
        });
    });
}

fn show_fonts(app: &mut StelaApp, ui: &mut egui::Ui) {
    ui.label(
        RichText::new(
            "Шрифты для гравировки на стеле (.ttf / .otf). \
             Системные шрифты подхватываются при первом запуске.",
        )
        .color(Color32::from_rgb(170, 175, 185)),
    );
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label("Название:");
        ui.add(
            egui::TextEdit::singleline(&mut app.new_font_name)
                .desired_width(180.0)
                .hint_text("из имени файла, если пусто"),
        );
        if ui::bronze_button(ui, "Добавить шрифт…").clicked() {
            app.import_font_to_library();
        }
    });

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(8.0);

    if app.library.fonts.is_empty() {
        ui.label(
            RichText::new("Библиотека шрифтов пуста. Добавьте .ttf / .otf.")
                .color(Color32::GRAY),
        );
    }

    let fonts = app.library.fonts.clone();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for font in &fonts {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&font.name).strong().size(15.0));
                        ui.label(
                            RichText::new(
                                std::path::Path::new(&font.file_path)
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or(&font.file_path),
                            )
                            .size(11.0)
                            .color(Color32::GRAY),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Удалить").clicked() {
                            app.request_delete_font(&font.id);
                        }
                    });
                });
            });
            ui.add_space(4.0);
        }
    });
}

fn show_backgrounds(app: &mut StelaApp, ui: &mut egui::Ui) {
    ui.label(
        RichText::new(
            "Фоны фотокерамики. У каждой формы — своя библиотека: овал, прямоугольник, бочка, арка.",
        )
        .color(Color32::from_rgb(170, 175, 185)),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Форма:");
        for shape in MedallionShape::all() {
            if ui
                .selectable_label(app.background_shape == *shape, shape.label())
                .clicked()
            {
                app.background_shape = *shape;
            }
        }
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Название:");
        ui.add(
            egui::TextEdit::singleline(&mut app.new_background_name)
                .desired_width(180.0)
                .hint_text("из имени файла, если пусто"),
        );
        if ui::bronze_button(ui, "Добавить фон…").clicked() {
            app.import_background_to_library();
        }
    });
    ui.label(
        RichText::new(format!(
            "Файл попадёт в библиотеку формы «{}».",
            app.background_shape.label()
        ))
        .size(11.0)
        .color(Color32::GRAY),
    );

    ui.add_space(12.0);
    let shape = app.background_shape;
    let items: Vec<_> = app
        .library
        .backgrounds
        .iter()
        .filter(|b| b.shape == shape)
        .cloned()
        .collect();
    if items.is_empty() {
        ui.label(
            RichText::new("В этой библиотеке фонов пока нет. Добавьте PNG / JPG / WebP.")
                .color(Color32::GRAY),
        );
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        let width = ui.available_width();
        let cols = ((width / 180.0).floor() as usize).clamp(1, 5);
        let mut col_i = 0;
        ui.columns(cols, |columns| {
            for item in &items {
                let ui = &mut columns[col_i % cols];
                col_i += 1;
                ui.group(|ui| {
                    ui.set_min_width(150.0);
                    let preview = app.preview_cache.get_background(ui.ctx(), item);
                    let size = Vec2::splat(120.0);
                    if let Some(tex) = preview {
                        ui.image((tex.id(), size));
                    } else {
                        let (resp, painter) = ui.allocate_painter(size, egui::Sense::hover());
                        painter.rect_filled(resp.rect, 4.0, Color32::from_rgb(40, 44, 52));
                    }
                    ui.label(RichText::new(&item.name).strong().size(13.0));
                    ui.label(
                        RichText::new(item.shape.label())
                            .size(11.0)
                            .color(Color32::from_rgb(200, 180, 140)),
                    );
                    if ui.button("Удалить").clicked() {
                        app.request_delete_background(&item.id);
                    }
                });
                ui.add_space(6.0);
            }
        });
    });
}
