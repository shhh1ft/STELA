use crate::app::{LibraryPicker, StelaApp, TextureTarget};
use crate::model::*;
use crate::persist;
use crate::scene::draw_viewport;
use crate::ui::{self, Screen};
use eframe::egui::{self, Color32, RichText, Vec2};

pub fn show(app: &mut StelaApp, ctx: &egui::Context) {
    let project_name = app
        .active_project()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "Проект".into());

    egui::TopBottomPanel::top("studio_top").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui::nav_button(ui, ui::Mark::Projects, "Проекты", false).clicked() {
                app.screen = Screen::Home;
                app.persist();
            }
            ui.separator();
            ui.label(RichText::new("Stela").strong());
            ui.separator();
            ui.label(&project_name);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::nav_button(ui, ui::Mark::Library, "Библиотека", false).clicked() {
                    app.screen = Screen::Library;
                }
                if ui::nav_button(ui, ui::Mark::Save, "Сохранить", false).clicked() {
                    app.persist();
                    app.status = "Сохранено".into();
                }
                if ui.button("Экспорт").clicked() {
                    if let Some(p) = app.active_project().cloned() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name(format!("{}.stela.json", p.name))
                            .save_file()
                        {
                            match persist::export_project(&p, &path) {
                                Ok(()) => app.status = "Экспортировано".into(),
                                Err(e) => app.status = format!("Ошибка: {e}"),
                            }
                        }
                    }
                }
                if ui.button("Импорт проекта").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Stela", &["json"])
                        .pick_file()
                    {
                        match persist::import_project(&path) {
                            Ok(p) => {
                                let id = p.id.clone();
                                app.data.projects.insert(0, p);
                                app.open_project(&id);
                                app.status = "Проект импортирован".into();
                            }
                            Err(e) => app.status = format!("Ошибка: {e}"),
                        }
                    }
                }
                ui.label(
                    RichText::new(&app.status)
                        .size(11.0)
                        .color(Color32::from_rgb(160, 165, 175)),
                );
            });
        });
    });

    egui::SidePanel::left("studio_left")
        .default_width(280.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                left_panel(app, ui);
            });
        });

    egui::SidePanel::right("studio_right")
        .default_width(320.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                right_panel(app, ui);
            });
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.label(
            RichText::new("СБОРКА УЧАСТКА")
                .size(11.0)
                .color(Color32::from_rgb(150, 155, 165)),
        );
        let ground = app.active_project().map(|p| p.ground).unwrap_or(true);
        let (plot_w, plot_l) = app
            .active_project()
            .map(|p| p.plot_size.to_meters())
            .unwrap_or((1.8, 2.0));
        let plot_tex = app
            .active_project()
            .and_then(|p| p.plot_texture_path.clone());
        let warning = app.build_warnings.first().map(|s| s.as_str());
        draw_viewport(
            ui,
            &app.meshes,
            &mut app.camera,
            ground,
            plot_w,
            plot_l,
            plot_tex.as_deref(),
            warning,
        );
    });
}

fn left_panel(app: &mut StelaApp, ui: &mut egui::Ui) {
    ui.heading("Участок");
    plot_panel(app, ui);
    ui.add_space(12.0);
    ui.separator();
    ui.heading("Памятники");
    ui.horizontal(|ui| {
        if ui::bronze_button(ui, "+ Памятник").clicked() {
            app.add_monument();
        }
        if ui.button("Удалить").on_hover_text("Delete").clicked() {
            app.request_delete_active_monument();
        }
    });
    ui.add_space(6.0);

    let monuments = app
        .active_project()
        .map(|p| p.monuments.clone())
        .unwrap_or_default();
    let active_m = app
        .active_project()
        .and_then(|p| p.active_monument_id.clone());

    for m in &monuments {
        let selected = active_m.as_deref() == Some(m.id.as_str());
        if ui.selectable_label(selected, &m.name).clicked() {
            if let Some(p) = app.active_project_mut() {
                p.active_monument_id = Some(m.id.clone());
                if let Some(part) = p.parts.iter().find(|x| x.monument_id == m.id) {
                    p.active_part_id = Some(part.id.clone());
                }
            }
            app.persist();
        }
    }

    // Monument position
    if let Some(mid) = active_m.clone() {
        ui.add_space(4.0);
        if let Some(p) = app.active_project_mut() {
            if let Some(m) = p.monuments.iter_mut().find(|x| x.id == mid) {
                let mut changed = false;
                changed |= ui
                    .add(egui::DragValue::new(&mut m.x).speed(0.05).prefix("X м "))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut m.z).speed(0.05).prefix("Z м "))
                    .changed();
                if changed {
                    p.updated_at = chrono::Utc::now();
                    app.rebuild_needed = true;
                }
            }
        }
    }

    ui.add_space(12.0);
    ui.separator();
    ui.heading("Добавить деталь");
    ui.label(
        RichText::new("Габаритная заглушка до импорта модели")
            .size(11.0)
            .color(Color32::GRAY),
    );
    for role in PartRole::all() {
        if ui
            .add(egui::Button::new(role.label()).min_size(egui::vec2(ui.available_width(), 26.0)))
            .clicked()
        {
            app.add_part(*role);
        }
    }

    ui.add_space(12.0);
    ui.separator();
    ui.heading("На участке");
    let parts = app
        .active_project()
        .map(|p| p.parts.clone())
        .unwrap_or_default();
    let active_part = app
        .active_project()
        .and_then(|p| p.active_part_id.clone());

    for part in &parts {
        let mon_name = app
            .active_project()
            .and_then(|p| {
                p.monuments
                    .iter()
                    .find(|m| m.id == part.monument_id)
                    .map(|m| m.name.clone())
            })
            .unwrap_or_default();
        let label = format!("{} · {} ({})", part.name, part.role.label(), mon_name);
        let selected = active_part.as_deref() == Some(part.id.as_str());
        let btn = egui::Button::new(if selected {
            RichText::new(&label)
                .strong()
                .color(Color32::from_rgb(255, 220, 170))
        } else {
            RichText::new(&label)
        })
        .fill(if selected {
            Color32::from_rgb(70, 55, 35)
        } else {
            Color32::from_rgb(36, 40, 48)
        })
        .min_size(egui::vec2(ui.available_width(), 28.0));
        if ui.add(btn).clicked() {
            let part_id = part.id.clone();
            let mid = part.monument_id.clone();
            let top_decal = part.decals.last().map(|d| d.id.clone());
            if let Some(p) = app.active_project_mut() {
                p.active_part_id = Some(part_id);
                p.active_monument_id = Some(mid);
            }
            app.active_decal_id = top_decal;
            app.persist();
        }
    }
}

fn right_panel(app: &mut StelaApp, ui: &mut egui::Ui) {
    ui.heading("Свойства детали");

    let part_id = app
        .active_project()
        .and_then(|p| p.active_part_id.clone());
    let Some(part_id) = part_id else {
        ui.label("Выберите деталь слева.");
        return;
    };

    let mut changed = false;
    let mut want_import = false;
    let mut want_import_lib = false;
    let mut want_clear_model = false;
    let mut want_texture_lib = false;
    let mut want_clear_texture = false;
    let mut want_remove_part = false;
    let mut want_add_text = false;
    let mut want_add_photo = false;
    let mut want_delete_decal = false;
    let mut move_decal: Option<i32> = None;
    let mut select_decal: Option<String> = None;

    if let Some(p) = app.active_project_mut() {
        let Some(part) = p.parts.iter_mut().find(|x| x.id == part_id) else {
            ui.label("Деталь не найдена.");
            return;
        };

        ui.label(RichText::new(part.role.label()).color(Color32::from_rgb(200, 180, 140)));
        changed |= ui
            .add(egui::TextEdit::singleline(&mut part.name).desired_width(240.0))
            .changed();

        ui.add_space(8.0);
        ui.label(RichText::new("Размеры, см").strong());
        ui.label(
            RichText::new("Модель масштабируется точно под эти габариты")
                .size(11.0)
                .color(Color32::GRAY),
        );
        changed |= ui
            .add(
                egui::Slider::new(&mut part.size.width, 5.0..=400.0)
                    .text("Ширина")
                    .suffix(" см"),
            )
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut part.size.height, 1.0..=400.0)
                    .text("Высота")
                    .suffix(" см"),
            )
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut part.size.depth, 1.0..=400.0)
                    .text("Глубина")
                    .suffix(" см"),
            )
            .changed();
        changed |= ui
            .add(
                egui::Slider::new(&mut part.scale, 0.1..=3.0)
                    .text("Масштаб")
                    .suffix("×"),
            )
            .changed();
        if (part.scale - 1.0).abs() > 0.01 {
            let eff = part.effective_size_m();
            ui.label(
                RichText::new(format!(
                    "Итого: {:.0}×{:.0}×{:.0} см",
                    eff.x * 100.0,
                    eff.y * 100.0,
                    eff.z * 100.0
                ))
                .size(11.0)
                .color(Color32::from_rgb(180, 175, 160)),
            );
        }

        ui.add_space(8.0);
        ui.label(RichText::new("Положение").strong());
        changed |= ui
            .add(egui::DragValue::new(&mut part.offset_x).speed(0.02).prefix("ΔX м "))
            .changed();
        changed |= ui
            .add(
                egui::DragValue::new(&mut part.offset_y)
                    .speed(0.01)
                    .prefix("ΔY м ")
                    .range(-2.0..=3.0),
            )
            .on_hover_text("Отрицательное — уходит под землю (обрезается)")
            .changed();
        changed |= ui
            .add(egui::DragValue::new(&mut part.offset_z).speed(0.02).prefix("ΔZ м "))
            .changed();
        if part.offset_y < -0.001 {
            ui.label(
                RichText::new("Участок обрезает часть под землёй")
                    .size(11.0)
                    .color(Color32::from_rgb(180, 150, 100)),
            );
        }
        changed |= ui
            .add(
                egui::Slider::new(&mut part.yaw_deg, -180.0..=180.0)
                    .text("Поворот")
                    .suffix("°"),
            )
            .changed();
        if matches!(part.role, PartRole::Stele | PartRole::Cross) {
            changed |= ui
                .checkbox(&mut part.auto_stack, "Ставить на тумбу")
                .changed();
        }
        changed |= ui.checkbox(&mut part.visible, "Видима").changed();

        ui.add_space(10.0);
        ui.separator();
        ui.label(RichText::new("3D-модель").strong());
        if let Some(path) = &part.model_path {
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(path);
            ui.label(
                RichText::new(format!("Файл: {name}"))
                    .size(11.0)
                    .color(Color32::from_rgb(200, 190, 160)),
            );
        } else {
            ui.label(
                RichText::new("Заглушка по размерам (импортируйте модель)")
                    .size(11.0)
                    .color(Color32::GRAY),
            );
        }
        ui.horizontal(|ui| {
            if ui::bronze_button(ui, "Импорт модели…").clicked() {
                want_import = true;
            }
            if ui.button("Из библиотеки…").clicked() {
                want_import_lib = true;
            }
        });
        if part.model_path.is_some() && ui.button("Убрать модель").clicked() {
            want_clear_model = true;
        }

        ui.add_space(10.0);
        ui.separator();
        ui.label(RichText::new("Текстура").strong());
        if let Some(path) = &part.texture_path {
            let name = std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(path);
            ui.label(
                RichText::new(format!("Назначена: {name}"))
                    .size(11.0)
                    .color(Color32::from_rgb(200, 190, 160)),
            );
        } else {
            ui.label(
                RichText::new("Цвет роли (выберите текстуру из библиотеки)")
                    .size(11.0)
                    .color(Color32::GRAY),
            );
        }
        ui.horizontal(|ui| {
            if ui::bronze_button(ui, "Из библиотеки…").clicked() {
                want_texture_lib = true;
            }
            if part.texture_path.is_some() && ui.button("Снять").clicked() {
                want_clear_texture = true;
            }
        });

        ui.add_space(10.0);
        if ui
            .button("Удалить деталь")
            .on_hover_text("Delete / Backspace")
            .clicked()
        {
            want_remove_part = true;
        }

        if changed {
            p.updated_at = chrono::Utc::now();
        }
    }

    // —— Слои эскиза на стеле (как в Photoshop) ——
    let is_stele = app
        .active_part()
        .map(|p| p.role == PartRole::Stele)
        .unwrap_or(false);
    if is_stele {
        ui.add_space(12.0);
        ui.separator();
        ui.heading("Слои эскиза");
        ui.label(
            RichText::new("Выберите слой слева в списке — как объекты участка.")
                .size(11.0)
                .color(Color32::GRAY),
        );

        let decals: Vec<(String, String, bool)> = app
            .active_part()
            .map(|p| {
                p.decals
                    .iter()
                    .rev() // top layer first, like Photoshop
                    .map(|d| {
                        (
                            d.id.clone(),
                            format!("{} · {}", d.layer_label(), d.side.label()),
                            d.visible,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        if decals.is_empty() {
            ui.label(
                RichText::new("Пока нет слоёв — добавьте текст или фотокерамику.")
                    .color(Color32::GRAY),
            );
        }

        // Ensure a selection exists
        if app.active_decal_id.is_none() {
            if let Some((id, _, _)) = decals.first() {
                app.active_decal_id = Some(id.clone());
            }
        }

        for (id, label, visible) in &decals {
            let selected = app.active_decal_id.as_deref() == Some(id.as_str());
            ui.horizontal(|ui| {
                let mut vis = *visible;
                if ui.checkbox(&mut vis, "").on_hover_text("Видимость").changed() {
                    if let Some(p) = app.active_project_mut() {
                        if let Some(part) = p.parts.iter_mut().find(|x| x.id == part_id) {
                            if let Some(d) = part.decals.iter_mut().find(|d| d.id == *id) {
                                d.visible = vis;
                                changed = true;
                                p.updated_at = chrono::Utc::now();
                            }
                        }
                    }
                }
                let btn = egui::Button::new(if selected {
                    RichText::new(label)
                        .strong()
                        .color(Color32::from_rgb(255, 220, 170))
                } else {
                    RichText::new(label)
                })
                .fill(if selected {
                    Color32::from_rgb(70, 55, 35)
                } else {
                    Color32::from_rgb(36, 40, 48)
                })
                .min_size(egui::vec2(ui.available_width() - 8.0, 26.0));
                if ui.add(btn).clicked() {
                    select_decal = Some(id.clone());
                }
            });
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Выше").clicked() {
                move_decal = Some(1);
            }
            if ui.button("Ниже").clicked() {
                move_decal = Some(-1);
            }
            if ui.button("Удалить слой").clicked() {
                want_delete_decal = true;
            }
        });

        ui.add_space(8.0);
        ui.label("Добавить слой:");
        ui.horizontal(|ui| {
            if ui::nav_button(ui, ui::Mark::Text, "Эскиз текста…", true).clicked() {
                want_add_text = true;
            }
            if ui::nav_button(ui, ui::Mark::Photo, "Фотокерамика…", true).clicked() {
                want_add_photo = true;
            }
        });

        // Properties of selected layer
        let active_id = app.active_decal_id.clone();
        let mut want_edit_text = false;
        let mut want_edit_photo = false;
        if let Some(active_id) = active_id {
            if let Some(p) = app.active_project_mut() {
                if let Some(part) = p.parts.iter_mut().find(|x| x.id == part_id) {
                    if let Some(decal) = part.decals.iter_mut().find(|d| d.id == active_id) {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.label(RichText::new("Свойства слоя").strong());
                        changed |= ui
                            .add(
                                egui::TextEdit::singleline(&mut decal.name)
                                    .desired_width(ui.available_width())
                                    .hint_text("Имя слоя"),
                            )
                            .changed();
                        changed |= ui
                            .add(egui::Slider::new(&mut decal.u, 0.0..=1.0).text("Влево–вправо"))
                            .changed();
                        changed |= ui
                            .add(egui::Slider::new(&mut decal.v, 0.0..=1.0).text("Низ–верх"))
                            .changed();
                        let is_photo = matches!(decal.kind, DecalKind::Photo { .. });
                        if !is_photo {
                            changed |= ui
                                .add(
                                    egui::Slider::new(&mut decal.width, 0.1..=1.0)
                                        .text("Область шир."),
                                )
                                .changed();
                            changed |= ui
                                .add(
                                    egui::Slider::new(&mut decal.height, 0.1..=1.0)
                                        .text("Область выс."),
                                )
                                .changed();
                        }
                        ui.horizontal(|ui| {
                            ui.label("Сторона:");
                            for side in FaceSide::all() {
                                if ui.selectable_label(decal.side == *side, side.label()).clicked()
                                {
                                    decal.side = *side;
                                    changed = true;
                                }
                            }
                        });
                        if let DecalKind::Text { content, style, .. } = &decal.kind {
                            ui.label(
                                RichText::new(format!(
                                    "«{}» · {:.1} см · {:.1} мм",
                                    content.lines().next().unwrap_or(""),
                                    style.letter_cm,
                                    style.depth_mm
                                ))
                                .size(11.0)
                                .color(Color32::from_rgb(200, 190, 160)),
                            );
                            if ui::bronze_button(ui, "Редактировать текст…").clicked() {
                                want_edit_text = true;
                            }
                        }
                        if let DecalKind::Photo { path, spec } = &decal.kind {
                            ui.label(
                                RichText::new(format!(
                                    "{} · {} · {:.0}×{:.0} см · {}",
                                    spec.material.label(),
                                    spec.shape.label(),
                                    spec.width_cm,
                                    spec.height_cm,
                                    spec.mount.label()
                                ))
                                .size(11.0)
                                .color(Color32::from_rgb(200, 190, 160)),
                            );
                            ui.label(
                                RichText::new(format!("Файл: {path}"))
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                            if ui::bronze_button(ui, "Редактировать фотокерамику…").clicked() {
                                want_edit_photo = true;
                            }
                        }
                        if changed {
                            p.updated_at = chrono::Utc::now();
                        }
                    }
                }
            }
            if want_edit_text {
                app.open_text_editor_edit(&active_id);
            }
            if want_edit_photo {
                app.open_photo_editor_edit(&active_id);
            }
        }
    }

    if let Some(id) = select_decal {
        app.active_decal_id = Some(id);
    }
    if let Some(delta) = move_decal {
        app.move_active_decal(delta);
    }
    if want_delete_decal {
        app.delete_active_decal();
    }
    if want_clear_model {
        if let Some(p) = app.active_project_mut() {
            if let Some(part) = p.parts.iter_mut().find(|x| x.id == part_id) {
                part.model_path = None;
            }
        }
        changed = true;
    }
    if want_import {
        app.import_model_for_active_part();
    }
    if want_import_lib {
        app.open_model_library_picker();
    }
    if want_texture_lib {
        app.open_texture_library_picker();
    }
    if want_clear_texture {
        app.clear_part_texture();
    }
    if want_remove_part {
        app.request_delete_active_part();
    }
    if want_add_text {
        app.add_text_decal();
    }
    if want_add_photo {
        app.add_photo_decal();
    }

    if changed {
        app.rebuild_needed = true;
        app.persist();
    }

    ui.add_space(12.0);
    ui.separator();
    if let Some(path) = app.active_project().and_then(|p| p.file_path.clone()) {
        ui.label(
            RichText::new(format!("Файл:\n{path}"))
                .size(10.0)
                .color(Color32::GRAY),
        );
    }
}

/// Модальный каталог моделей / текстур для активной детали.
pub fn show_library_picker(app: &mut StelaApp, ctx: &egui::Context) {
    let Some(picker) = app.library_picker.clone() else {
        return;
    };

    let title = match &picker {
        LibraryPicker::Models { role } => format!("Библиотека · {}", role.label()),
        LibraryPicker::Textures {
            target: TextureTarget::ActivePart,
            ..
        } => "Текстура детали".into(),
        LibraryPicker::Textures {
            target: TextureTarget::Plot,
            ..
        } => "Текстура участка".into(),
    };

    let mut open = true;
    let mut want_close = false;
    let mut pick_model: Option<String> = None;
    let mut pick_texture: Option<String> = None;
    let tex_target = match &picker {
        LibraryPicker::Textures { target, .. } => *target,
        _ => TextureTarget::ActivePart,
    };
    let mut filter_cat = match &picker {
        LibraryPicker::Textures { category_id, .. } => category_id.clone(),
        _ => None,
    };

    egui::Window::new(title)
        .collapsible(false)
        .resizable(true)
        .default_size([520.0, 420.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(&mut open)
        .show(ctx, |ui| {
            match &picker {
                LibraryPicker::Models { role } => {
                    ui.label(
                        RichText::new(format!(
                            "Модели роли «{}». Выберите, чтобы назначить на деталь.",
                            role.label()
                        ))
                        .size(12.0)
                        .color(Color32::from_rgb(170, 175, 185)),
                    );
                    ui.add_space(8.0);
                    let items: Vec<_> = app
                        .library
                        .items
                        .iter()
                        .filter(|i| i.role == *role)
                        .cloned()
                        .collect();
                    if items.is_empty() {
                        ui.label(
                            RichText::new("В библиотеке нет моделей этой роли. Добавьте на экране «Библиотека».")
                                .color(Color32::GRAY),
                        );
                    }
                    egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                        for item in &items {
                            ui.horizontal(|ui| {
                                let preview = app.preview_cache.get(ui.ctx(), item);
                                let size = Vec2::splat(56.0);
                                if let Some(tex) = preview {
                                    ui.image((tex.id(), size));
                                } else {
                                    let (resp, painter) =
                                        ui.allocate_painter(size, egui::Sense::hover());
                                    painter.rect_filled(
                                        resp.rect,
                                        4.0,
                                        Color32::from_rgb(40, 44, 52),
                                    );
                                }
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&item.name).strong());
                                    ui.label(
                                        RichText::new(item.role.label())
                                            .size(11.0)
                                            .color(Color32::from_rgb(200, 180, 140)),
                                    );
                                    if ui::bronze_button(ui, "Выбрать").clicked() {
                                        pick_model = Some(item.id.clone());
                                    }
                                });
                            });
                            ui.add_space(6.0);
                            ui.separator();
                        }
                    });
                }
                LibraryPicker::Textures { .. } => {
                    ui.horizontal(|ui| {
                        ui.label("Библиотека:");
                        let selected_label = filter_cat
                            .as_ref()
                            .and_then(|id| {
                                app.library
                                    .texture_categories
                                    .iter()
                                    .find(|c| &c.id == id)
                                    .map(|c| c.name.clone())
                            })
                            .unwrap_or_else(|| "Все".into());
                        egui::ComboBox::from_id_salt("picker_tex_cat")
                            .selected_text(selected_label)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(filter_cat.is_none(), "Все")
                                    .clicked()
                                {
                                    filter_cat = None;
                                }
                                for cat in app.library.texture_categories.clone() {
                                    let sel = filter_cat.as_deref() == Some(cat.id.as_str());
                                    if ui.selectable_label(sel, &cat.name).clicked() {
                                        filter_cat = Some(cat.id);
                                    }
                                }
                            });
                    });
                    ui.add_space(8.0);
                    let items: Vec<_> = app
                        .library
                        .textures
                        .iter()
                        .filter(|t| {
                            filter_cat
                                .as_ref()
                                .map(|cid| t.category_id.as_ref() == Some(cid))
                                .unwrap_or(true)
                        })
                        .cloned()
                        .collect();
                    if items.is_empty() {
                        ui.label(
                            RichText::new("Нет текстур. Добавьте на экране «Библиотека» → Текстуры.")
                                .color(Color32::GRAY),
                        );
                    }
                    egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                        let width = ui.available_width();
                        let cols = ((width / 140.0).floor() as usize).clamp(1, 4);
                        let mut col_i = 0;
                        ui.columns(cols, |columns| {
                            for item in &items {
                                let ui = &mut columns[col_i % cols];
                                col_i += 1;
                                ui.group(|ui| {
                                    let preview = app.preview_cache.get_texture(ui.ctx(), item);
                                    let size = Vec2::splat(96.0);
                                    if let Some(tex) = preview {
                                        ui.image((tex.id(), size));
                                    } else {
                                        let (resp, painter) =
                                            ui.allocate_painter(size, egui::Sense::hover());
                                        painter.rect_filled(
                                            resp.rect,
                                            4.0,
                                            Color32::from_rgb(40, 44, 52),
                                        );
                                    }
                                    ui.label(RichText::new(&item.name).strong().size(12.0));
                                    if ui::bronze_button(ui, "Выбрать").clicked() {
                                        pick_texture = Some(item.id.clone());
                                    }
                                });
                                ui.add_space(4.0);
                            }
                        });
                    });
                }
            }

            ui.add_space(8.0);
            if ui.button("Закрыть").clicked() {
                want_close = true;
            }
        });

    if let LibraryPicker::Textures { .. } = picker {
        if app.library_picker.is_some() {
            app.library_picker = Some(LibraryPicker::Textures {
                category_id: filter_cat,
                target: tex_target,
            });
        }
    }

    if let Some(id) = pick_model {
        app.apply_model_from_library(&id);
    }
    if let Some(id) = pick_texture {
        app.apply_texture_from_library(&id, tex_target);
    }
    if !open || want_close {
        app.library_picker = None;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && app.confirm.is_none() {
        app.library_picker = None;
    }
}

fn plot_panel(app: &mut StelaApp, ui: &mut egui::Ui) {
    let Some(plot) = app.active_project().map(|p| p.plot_size) else {
        return;
    };
    let ground = app.active_project().map(|p| p.ground).unwrap_or(true);
    let plot_tex = app
        .active_project()
        .and_then(|p| p.plot_texture_path.clone());

    ui.label(
        RichText::new(format!("Текущий: {}", plot.label()))
            .size(12.0)
            .color(Color32::from_rgb(200, 190, 160)),
    );
    ui.label(
        RichText::new("Стандартные размеры, см (ширина × длина)")
            .size(11.0)
            .color(Color32::GRAY),
    );

    let mut changed = false;
    let mut new_size = plot;
    let mut want_plot_tex = false;
    let mut want_clear_plot_tex = false;
    for preset in PlotSizeCm::presets() {
        let selected = plot == *preset;
        let label = format!("{:.0}×{:.0}", preset.width, preset.length);
        if ui.selectable_label(selected, label).clicked() {
            new_size = *preset;
            changed = true;
        }
    }

    ui.add_space(6.0);
    ui.label(RichText::new("Свой размер, см").strong());
    ui.horizontal(|ui| {
        changed |= ui
            .add(
                egui::DragValue::new(&mut new_size.width)
                    .speed(1.0)
                    .range(50.0..=800.0)
                    .prefix("Ш "),
            )
            .changed();
        changed |= ui
            .add(
                egui::DragValue::new(&mut new_size.length)
                    .speed(1.0)
                    .range(50.0..=800.0)
                    .prefix("Д "),
            )
            .changed();
    });

    let mut show_ground = ground;
    if ui.checkbox(&mut show_ground, "Показывать участок").changed() {
        if let Some(p) = app.active_project_mut() {
            p.ground = show_ground;
            p.updated_at = chrono::Utc::now();
        }
        app.persist();
    }

    ui.add_space(8.0);
    ui.label(RichText::new("Покрытие участка").strong());
    if let Some(path) = &plot_tex {
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(path);
        ui.label(
            RichText::new(format!("Текстура: {name}"))
                .size(11.0)
                .color(Color32::from_rgb(200, 190, 160)),
        );
    } else {
        ui.label(
            RichText::new("Без текстуры (трава по умолчанию)")
                .size(11.0)
                .color(Color32::GRAY),
        );
    }
    ui.horizontal(|ui| {
        if ui::bronze_button(ui, "Из библиотеки…").clicked() {
            want_plot_tex = true;
        }
        if plot_tex.is_some() && ui.button("Снять").clicked() {
            want_clear_plot_tex = true;
        }
    });

    if want_plot_tex {
        app.open_plot_texture_picker();
    }
    if want_clear_plot_tex {
        app.clear_plot_texture();
    }

    if changed {
        if let Some(p) = app.active_project_mut() {
            p.plot_size = new_size;
            p.updated_at = chrono::Utc::now();
        }
        app.frame_camera = true;
        app.rebuild_needed = true;
        app.persist();
        app.status = format!("Участок: {}", new_size.label());
    }
}
