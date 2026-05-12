# AGENTS.md — rtmgui (egui desktop GUI)

## What This Component Is

A native desktop GUI for the Todo.txt manager built with `eframe`/`egui` and `rfd` for file
dialogs. Targets GNU/Linux, macOS, and Windows.

---

## Architecture / Structure

```
rtmgui/src/
└── main.rs     ← Single file: AppConfig, TodoApp (eframe::App impl), main()
```

---

## Key Dependencies

| Crate   | Purpose                        |
|---------|--------------------------------|
| eframe  | Native window + event loop     |
| egui    | Immediate-mode UI              |
| rfd     | Native file dialogs            |
| todotxt | Core data library              |
| dirs    | OS config directory            |
| toml    | Config serialization           |

---

## Conventions

- **Edition:** Rust 2024
- Config stored at `dirs::config_dir()/rtm/config.toml` (TOML). Fields: `file_name`,
  `show_completed_items`, `show_future_items`, `reverse_sort`.
- `save_config()` is called immediately on any setting change.
- File loading uses `rfd::FileDialog::new().pick_file()` — returns a plain `PathBuf` (no URI
  issues). `canonicalize()` is called with `unwrap_or(path)` fallback (safe).
- Wayland: a commented-out `WINIT_UNIX_BACKEND=x11` override exists in `main()` — uncomment if
  Wayland causes issues.
- Windows: `#![cfg_attr(windows, windows_subsystem = "windows")]` suppresses the console window.

---

## Build & Run

```sh
# In nix-shell (Linux):
nix-shell
cargo run -p rtmgui

# Other platforms:
cargo run -p rtmgui
cargo build --release -p rtmgui
```

---

## Important Notes

- `rtmgui` does NOT share config with `rtmapp` — both write to the same `rtm/config.toml` path,
  so running both simultaneously could cause config conflicts.
