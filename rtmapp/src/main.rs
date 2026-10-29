// No console window behind the GUI in Windows release builds (as the Tauri app had); log output
// on stderr is then not visible there.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod calendar;
mod config;
mod edit;
mod list;
mod theme;

use eframe::egui;

fn main() -> eframe::Result {
    // rfd reports file dialog failures (no portal, no zenity) only through `log`.
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rusty Todo.txt Manager")
            .with_inner_size([900.0, 650.0]),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        // Also the Wayland app id and the X11 WM_CLASS: matches `StartupWMClass` in rtmapp.desktop.
        "rtmapp",
        options,
        Box::new(|cc| {
            theme_override(&cc.egui_ctx);
            Ok(Box::new(app::App::new(cc)))
        }),
    )
}

/// `RTMAPP_THEME=light` or `dark` forces that theme, so both can be screenshotted without changing
/// the system setting. Anything else, or unset, follows the system.
fn theme_override(ctx: &egui::Context) {
    let theme = match std::env::var("RTMAPP_THEME").as_deref() {
        Ok("light") => egui::ThemePreference::Light,
        Ok("dark") => egui::ThemePreference::Dark,
        _ => return,
    };
    ctx.set_theme(theme);
}
