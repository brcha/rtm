# Plan: Create AGENTS.md Files

## Goal

Create `AGENTS.md` files at the repository root and in each workspace subpart so that AI agents
have full project context on session start.

---

## Files to Create

### 1. `/AGENTS.md` (repository root)

```markdown
# AGENTS.md — Rusty Todo.txt Manager (rtm)

## What This Project Is

A [Todo.txt](https://github.com/todotxt/todo.txt) manager written in Rust. Targets GNU/Linux,
macOS, and Windows. Early development — core functionality works across three frontends.

Subpart AGENTS.md files:
- [`todotxt/AGENTS.md`](todotxt/AGENTS.md) — core library
- [`rtmcli/AGENTS.md`](rtmcli/AGENTS.md) — CLI frontend
- [`rtmgui/AGENTS.md`](rtmgui/AGENTS.md) — egui desktop GUI
- [`rtmapp/AGENTS.md`](rtmapp/AGENTS.md) — Tauri web-based desktop app

---

## Architecture / Structure

```
rtm/                        ← Cargo workspace root
├── todotxt/                ← Core library (no UI, no binary)
├── rtmcli/                 ← CLI (clap-based)
├── rtmgui/                 ← Native desktop GUI (eframe/egui + rfd)
├── rtmapp/                 ← Tauri v2 desktop app
│   ├── src/                ← Frontend: vanilla JS + HTML + CSS
│   └── src-tauri/          ← Tauri Rust backend
└── shell.nix               ← Nix dev shell (GTK, WebKit, X11/Wayland)
```

All Rust crates share a single Cargo workspace (`Cargo.lock` at root). The `todotxt` library is the
single source of truth for parsing, serialization, and business logic. Frontends depend on it via
`path` dependency.

Config is stored per-frontend in the OS config directory (`dirs::config_dir()`), under `rtm/config.toml`.

---

## Conventions

- **Rust edition:** 2024 (todotxt, rtmcli, rtmgui); 2021 (rtmapp/src-tauri)
- **No async** in the core library or CLI. Tauri backend is also synchronous (Mutex-guarded state).
- **Shared config key:** `file_name` (TOML string) — path to the active todo.txt file.
- **Date format:** `%Y-%m-%d` everywhere (chrono `NaiveDate`).
- **Priority encoding:** stored as `u8` (0 = A, 1 = B, …); serialized as `i32` in DTOs for JS compatibility.
- **No framework** in the Tauri frontend — plain HTML/CSS/JS with flatpickr for date pickers.
- All `save()` calls happen immediately after mutations (no deferred/batched writes).

---

## Important Notes

- **`load_file` on Linux (rtmapp):** Known bug — file loading fails on GNU/Linux. Root cause is
  likely `std::path::Path::canonicalize()` receiving a `file://` URI from the GTK file dialog, or
  a platform difference in the `tauri-plugin-dialog` return value. See
  `.opencode/plans/2026-05-12_fix-load-file-linux.md` for the full diagnosis and fix plan.
- **rtmgui Wayland:** `rtmgui/src/main.rs` has a commented-out `WINIT_UNIX_BACKEND=x11` override —
  uncomment if Wayland causes rendering issues.
- **Nix shell:** `shell.nix` provides all GTK/WebKit/X11/Wayland libraries needed to build and run
  all frontends on NixOS or with `nix-shell`. Use `nix-shell` before running `cargo build`.
- **`rtmgui` uses `rfd`** for file dialogs (not Tauri). `rtmapp` uses `tauri-plugin-dialog`.

---

## Future Plans

- Cloud sync via private git repo for todo.txt files
- Mobile GUI (Android-centric, cross-platform)
- Subtasks (uuid + sub tags already in the data model, needs UI)
- Comments per item (uuid-keyed, stored in a subdir of the todo.txt directory)
```

---

### 2. `todotxt/AGENTS.md`

```markdown
# AGENTS.md — todotxt (core library)

## What This Component Is

The core Rust library for parsing, representing, and serializing Todo.txt files. No binary, no UI.
All three frontends depend on this crate via `path` dependency.

---

## Architecture / Structure

```
todotxt/src/
├── lib.rs              ← Public API re-exports
├── todoitem.rs         ← TodoItem struct + FromStr + Display
├── todolibrary.rs      ← TodoLibrary (load/save/CRUD)
├── todocontext.rs      ← @context tag
├── todoproject.rs      ← +project tag
├── todopriority.rs     ← (A)-(Z) priority
└── todorecurrence.rs   ← rec: tag (daily/weekly/monthly/yearly/business-day)
```

---

## Conventions

- **Edition:** Rust 2024
- **No external dependencies** except `chrono` (dates) and `uuid` (item identity).
- `TodoItem` implements `FromStr` (parse from a todo.txt line) and `Display` (serialize back).
- `TodoLibrary` owns a `Vec<TodoItem>` and a file path string. `load()` reads from disk,
  `save()` writes back. No async.
- `complete_item(index)` marks done, handles recurrence (appends new item), returns
  `Option<bool>` (None = out of bounds, Some(true) = had recurrence, Some(false) = no recurrence).
- `TodoItem` fields: `done`, `priority`, `completion_date`, `creation_date`, `description`,
  `projects`, `contexts`, `due`, `recurrence`, `threshold`, `uuid`, `sub`.
- Business-day recurrence is currently approximated as daily (known limitation).
- Monthly recurrence is approximated as 30 days; yearly as 365 days.

---

## Important Notes

- `uuid` and `sub` fields exist in the data model for future subtask support but are not yet
  surfaced in any UI.
- Tests live in `todolibrary.rs` (inline `#[cfg(test)]` module). Run with `cargo test -p todotxt`.
```

---

### 3. `rtmcli/AGENTS.md`

```markdown
# AGENTS.md — rtmcli (CLI frontend)

## What This Component Is

A command-line interface for the Todo.txt manager. Uses `clap` for argument parsing and the
`todotxt` library for all data operations.

---

## Architecture / Structure

```
rtmcli/src/
└── main.rs     ← Single file: CLI definition + all command handlers
rtmcli/tests/   ← Integration tests
```

---

## Commands

| Command    | Description                                              |
|------------|----------------------------------------------------------|
| `list`     | List items, optionally filtered by completion and date   |
| `add`      | Add a new item in Todo.txt format                        |
| `complete` | Complete items by filter, index, or UUID                 |

---

## Conventions

- **Edition:** Rust 2024
- File path resolved from: `-f <file>` flag → `$TODOTXT` env var → `todo.txt` (cwd fallback).
- Filters: `today`, `week`, `all`, `overdue`, `future` (date-range based).
- `complete` accepts: a filter name (optionally + index/UUID), an absolute index, or a UUID.
- No config file — stateless, all options via CLI args/env.

---

## Build & Run

```sh
cargo build -p rtmcli
cargo run -p rtmcli -- -f ~/todo.txt list today
cargo test -p rtmcli
```
```

---

### 4. `rtmgui/AGENTS.md`

```markdown
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
```

---

### 5. `rtmapp/AGENTS.md`

```markdown
# AGENTS.md — rtmapp (Tauri desktop app)

## What This Component Is

A Tauri v2 desktop application for the Todo.txt manager. Rust backend exposes commands via IPC;
frontend is vanilla HTML/CSS/JS (no framework). Targets GNU/Linux, macOS, and Windows.

---

## Architecture / Structure

```
rtmapp/
├── src/                    ← Frontend (served as static files by Tauri)
│   ├── index.html
│   ├── main.js             ← All UI logic; calls Tauri commands via invoke()
│   └── styles.css
└── src-tauri/              ← Rust backend
    ├── src/
    │   ├── lib.rs          ← All Tauri commands + AppState
    │   └── main.rs         ← Entry point (calls lib::run())
    ├── capabilities/
    │   └── default.json    ← Tauri permission grants
    └── tauri.conf.json     ← Tauri configuration
```

---

## Tauri Commands (Rust → JS IPC)

| Command           | Description                                      |
|-------------------|--------------------------------------------------|
| `load_file`       | Load a todo.txt file by path; saves path to config |
| `get_file_name`   | Return currently loaded file path                |
| `has_file_loaded` | Return bool                                      |
| `save_file`       | Persist current items to disk                    |
| `get_items`       | Return filtered+sorted `Vec<TodoItemDto>`        |
| `get_item_count`  | Return total item count (unfiltered)             |
| `add_item`        | Parse + append item, auto-save                   |
| `complete_item`   | Mark done, handle recurrence, auto-save+reload   |
| `uncomplete_item` | Mark undone, auto-save                           |
| `update_item`     | Replace item fields, auto-save                   |
| `get_config`      | Return `AppConfig`                               |
| `save_config`     | Update display settings, persist                 |

---

## Key Dependencies

| Crate / Package          | Purpose                          |
|--------------------------|----------------------------------|
| tauri v2                 | App framework + IPC              |
| tauri-plugin-dialog v2   | Native file open dialog          |
| tauri-plugin-opener v2   | Open files/URLs                  |
| todotxt (path dep)       | Core data library                |
| chrono                   | Date handling                    |
| dirs                     | OS config directory              |
| toml                     | Config serialization             |
| flatpickr (CDN)          | Date picker in frontend          |

---

## Conventions

- **Rust edition:** 2021 (Tauri scaffold default)
- `AppState` holds `Mutex<Option<TodoLibrary>>` and `Mutex<AppConfig>`. All commands lock these.
- `TodoItemDto` is the serialization boundary — `TodoItem` never crosses the IPC boundary directly.
- Priority is `u8` in the library, `i32` in the DTO (JS `Number` compatibility).
- Config path: `dirs::config_dir()/rtm/config.toml`.
- Frontend uses `window.__TAURI__.core.invoke` and `window.__TAURI__.dialog.open` (global Tauri
  injected via `withGlobalTauri: true` in `tauri.conf.json`).
- No bundler/build step for the frontend — files are served directly from `src/`.

---

## Build & Run

```sh
cd rtmapp

# Development (hot-reload frontend, rebuild Rust on change):
npm run tauri dev

# Production build:
npm run tauri build
```

On Linux, run inside `nix-shell` from the repo root first to ensure GTK/WebKit libraries are
available.

---

## Known Issues

- **Load File fails on GNU/Linux.** The GTK file dialog (via `tauri-plugin-dialog`) may return a
  `file://` URI or an unexpected format, which causes `std::path::Path::canonicalize()` in
  `load_file` to fail. Works correctly on Windows and macOS. Fix plan:
  `.opencode/plans/2026-05-12_fix-load-file-linux.md`.
```

---

## Steps

1. Create `AGENTS.md` at repository root (content in section 1 above)
2. Create `todotxt/AGENTS.md` (content in section 2 above)
3. Create `rtmcli/AGENTS.md` (content in section 3 above)
4. Create `rtmgui/AGENTS.md` (content in section 4 above)
5. Create `rtmapp/AGENTS.md` (content in section 5 above)

No source files are modified. No build step required.

---

## Risks & Mitigations

- None significant — these are documentation-only files.
- If project structure changes significantly, AGENTS.md files will need updating.
