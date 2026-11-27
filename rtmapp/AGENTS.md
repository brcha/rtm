# AGENTS.md — rtmapp

The egui/eframe desktop app; its look, icons, run-time libraries and file dialog are in [doc/rtmapp.md](../doc/rtmapp.md).

- `rtmapp` is the binary, the Wayland app id and X11 `WM_CLASS`, the desktop entry's file name, `StartupWMClass`, `Exec`, `Icon` and `CFBundleExecutable`; keep them all equal.
- The AppUserModelID `com.brcha.rtm` is set at startup and on the MSI shortcuts; the MSI's UpgradeCode never changes.
- The config, `rtm/config.toml` under `dirs::config_dir()`, keeps the 0.1.x schema: never rename keys or change defaults; a new key gets its default in the manual `impl Default`.
- The config is written only after a user action or a successful load, never at startup, and never while it failed to load.
- Tests never touch the real config: build `App` with a temp `config_path` (`app_with`, `bare_app`) or through `App::with_config`, never `App::new` or `config::config_path()`.
- UI tests (`egui_kittest`, headless): every visible input needs an accessible name; temp directories need unique names (`ui-` prefix), as tests share a process.
- UI tests click row text with `click_row`, not `.click()` (the centre may hit a tag), and feed file dialogs with `dialog_returned`.
- egui 0.37: `eframe::App::ui` (not `update`) and `egui::Panel`; a `Popup` inside a `Modal` must be its sublayer; popup hit areas use `Sense::CLICK`.
- The UI thread never blocks: git and pijul run on one worker per repository, file dialogs on their own thread (the future is created on the UI thread, for macOS), and the write lock is only tried briefly.
- `flush` takes the write lock, then checks for outside changes, then saves, never in another order.
- Rows report the item's index in the file, never its place in the filtered list.
- Links open only `http`, `https` and `mailto` (never `file://`); the update notice opens only `https` release pages.
- `main` checks for `rtmapp mcp` before the logger and eframe: nothing else may write to stdout.
- Release builds use the Windows subsystem, so no console and no log; use a debug build to see `rfd`'s errors.
- The look reproduces the 0.1.x app's CSS at commit `5076649`.
- Inter's licence (`assets/fonts/Inter-LICENSE.txt`) ships in every package.
- Edit only the SVG masters in `assets/icons/`; the other files there are generated and keep their names.
- To run it without your own config, point `XDG_CONFIG_HOME` at a scratch directory; `RTMAPP_THEME=light` or `dark` forces a theme.
