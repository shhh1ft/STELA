mod app;
mod library;
mod model;
mod persist;
mod scene;
mod ui;

use app::StelaApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 680.0])
            .with_title("Stela — конфигуратор памятников"),
        ..Default::default()
    };

    eframe::run_native(
        "Stela",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(StelaApp::default()))
        }),
    )
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Bundled so Cyrillic and punctuation render on Windows without a system font.
    // Default egui fonts stay in the family as fallback for anything DejaVu lacks.
    let bytes = include_bytes!("../assets/DejaVuSans.ttf").to_vec();
    fonts
        .font_data
        .insert("cyrillic".into(), egui::FontData::from_owned(bytes));
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "cyrillic".into());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "cyrillic".into());
    ctx.set_fonts(fonts);
}
