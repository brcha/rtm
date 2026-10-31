// No console window behind the GUI in Windows release builds (as the Tauri app had); log output
// on stderr is then not visible there.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod calendar;
mod config;
mod edit;
mod list;
mod sync;
mod theme;

use eframe::egui;

/// The title bar and taskbar icon (Windows, X11). Wayland compositors take the icon from the desktop
/// entry matched by the app id instead.
const ICON_PNG: &[u8] = include_bytes!("../assets/icons/128x128@2x.png");

/// The Windows taskbar groups a window with a shortcut or pin only when both have the same ID. Must
/// match `System.AppUserModel.ID` on the shortcuts in `wix/main.wxs`; the Tauri app's (up to 0.1.3)
/// Start Menu shortcut had it too, so pins made with it keep working.
#[cfg(windows)]
const APP_USER_MODEL_ID: &str = "com.brcha.rtm";

#[cfg(windows)]
fn set_app_user_model_id() -> windows_sys::core::HRESULT {
    let id: Vec<u16> = APP_USER_MODEL_ID.encode_utf16().chain([0]).collect();
    // SAFETY: `id` is NUL-terminated and outlives the call; Windows copies the string.
    unsafe { windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(id.as_ptr()) }
}

fn main() -> eframe::Result {
    // rfd reports file dialog failures (no portal, no zenity) only through `log`.
    env_logger::init();
    // Before the first window: the taskbar reads the ID when the window appears.
    #[cfg(windows)]
    {
        let hr = set_app_user_model_id();
        if hr < 0 {
            eprintln!("SetCurrentProcessExplicitAppUserModelID failed: HRESULT {hr:#010x}");
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rusty Todo.txt Manager")
            .with_inner_size([900.0, 650.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(ICON_PNG).expect("embedded icon is a valid PNG"),
            ),
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

#[cfg(test)]
#[path = "tests/main_window_tests.rs"]
mod window_tests;
