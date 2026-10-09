use crate::app::StelaApp;
use crate::ui::{self, Mark, Screen};
use eframe::egui::{self, Color32, RichText, Vec2};

pub fn show(app: &mut StelaApp, ctx: &egui::Context) {
    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !ctx.wants_keyboard_input() {
        app.create_project_from_dialog();
        return;
    }

    egui::TopBottomPanel::top("home_top").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Stela")
                    .size(20.0)
                    .color(Color32::from_rgb(245, 240, 230))
                    .strong(),
            );
            ui.label(
                RichText::new("конфигуратор памятников")
                    .size(13.0)
                    .color(Color32::from_rgb(170, 160, 145)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::nav_button(ui, Mark::Library, "Библиотека", false).clicked() {
                    if app.active_project().is_none() {
                        if let Some(id) = app.data.projects.first().map(|p| p.id.clone()) {
                            app.open_project(&id);
                        }
                    }
                    app.screen = Screen::Library;
                }
            });
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let wide = ui.available_width() > 920.0;
                if wide {
                    ui.horizontal_top(|ui| {
                        let left_w = (ui.available_width() * 0.46).clamp(380.0, 520.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(left_w, ui.available_height()),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| hero(app, ui),
                        );
                        ui.add_space(18.0);
                        ui.vertical(|ui| projects(app, ui));
                    });
                } else {
                    hero(app, ui);
                    ui.add_space(18.0);
                    projects(app, ui);
                }
            });
    });
}

fn hero(app: &mut StelaApp, ui: &mut egui::Ui) {
    ui.add_space(16.0);
    ui.label(
        RichText::new("Участок, стела, надпись")
            .size(34.0)
            .color(Color32::from_rgb(250, 246, 238))
            .strong(),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Соберите памятник из импортированных моделей: стела, тумба, ограда, цветник.\n\
             Размеры в сантиметрах. Текст врезается в камень. Фото — объёмный медальон\n\
             на лицевой или обратной стороне.",
        )
        .size(15.0)
        .color(Color32::from_rgb(180, 185, 195)),
    );
    ui.add_space(16.0);

    egui::Frame::none()
        .fill(Color32::from_rgb(32, 30, 26))
        .rounding(8.0)
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.label(RichText::new("Новый участок").strong().size(16.0));
            ui.add_space(6.0);
            ui.label("Название проекта");
            ui.add(
                egui::TextEdit::singleline(&mut app.new_project_name)
                    .desired_width(ui.available_width().min(360.0))
                    .hint_text("Участок 1"),
            );
            ui.add_space(8.0);
            if ui::nav_button(ui, Mark::Plus, "Новый проект…", true).clicked() {
                app.create_project_from_dialog();
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new("Откроется диалог: папка и файл .stela.json.")
                    .size(11.0)
                    .color(Color32::GRAY),
            );
        });

    ui.add_space(16.0);
    feature(
        ui,
        "Модели",
        "OBJ, glTF и GLB из Blender. Каталог по роли детали.",
    );
    feature(
        ui,
        "Гравировка",
        "Надпись вырезается в стеле на 3–4 мм, на лице или с оборота.",
    );
    feature(
        ui,
        "Фотокерамика",
        "Овал, прямоугольник, бочка, арка. Керамогранит, фарфор, металлокерамика.",
    );
}

fn feature(ui: &mut egui::Ui, title: &str, text: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
        ui.painter().circle_filled(
            rect.center(),
            4.0,
            Color32::from_rgb(196, 140, 72),
        );
        ui.vertical(|ui| {
            ui.label(RichText::new(title).strong());
            ui.label(RichText::new(text).color(Color32::from_rgb(160, 165, 175)));
        });
    });
}

fn projects(app: &mut StelaApp, ui: &mut egui::Ui) {
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Проекты")
                .size(22.0)
                .color(Color32::from_rgb(245, 240, 230))
                .strong(),
        );
        ui.label(
            RichText::new(format!("{}", app.data.projects.len()))
                .size(14.0)
                .color(Color32::from_rgb(180, 150, 110)),
        );
    });
    ui.add_space(8.0);

    if app.data.projects.is_empty() {
        egui::Frame::none()
            .fill(Color32::from_rgb(28, 30, 34))
            .rounding(8.0)
            .inner_margin(18.0)
            .show(ui, |ui| {
                ui.label(RichText::new("Список пуст").strong().size(16.0));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Создайте первый участок слева. Файл останется там, куда вы его сохраните.",
                    )
                    .color(Color32::from_rgb(150, 155, 165)),
                );
            });
        return;
    }

    let projects: Vec<_> = app.data.projects.iter().cloned().collect();
    for p in projects {
        let id = p.id.clone();
        egui::Frame::none()
            .fill(Color32::from_rgb(30, 32, 38))
            .rounding(8.0)
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.label(RichText::new(&p.name).size(18.0).strong());
                ui.label(
                    RichText::new(format!(
                        "{} памятников · {} деталей · {}",
                        p.monuments.len(),
                        p.parts.len(),
                        p.updated_at.format("%d.%m.%Y %H:%M")
                    ))
                    .size(12.0)
                    .color(Color32::from_rgb(150, 155, 165)),
                );
                if let Some(path) = &p.file_path {
                    ui.label(
                        RichText::new(path)
                            .size(11.0)
                            .color(Color32::from_rgb(120, 125, 135)),
                    );
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui::nav_button(ui, Mark::Projects, "Открыть", true).clicked() {
                        app.open_project(&id);
                    }
                    if ui.button("Удалить").clicked() {
                        app.request_delete_project(&id);
                    }
                });
            });
        ui.add_space(8.0);
    }
}
