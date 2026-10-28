mod app;
mod config;
mod list;
mod theme;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rusty Todo.txt Manager")
            .with_inner_size([900.0, 650.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rtmgui",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
