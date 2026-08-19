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
| `complete_item`   | Mark done, set completion date, handle recurrence, auto-save+reload |
| `uncomplete_item` | Mark undone, clear completion date, auto-save    |
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

## Windows Packaging

- **Install scope: per-machine.** The MSI installs to `Program Files` and requires administrator
  rights — the UAC prompt this triggers is expected, not a bug. There is no per-user/no-admin
  install mode.
- **Shortcuts: Start Menu, and an unconditional Desktop shortcut.** Both come from Tauri's stock
  WiX template, completely unmodified — `bundle.windows.wix.template` is not set, and there is no
  vendored `wix/` directory in this repo. Tauri's default MSI template creates the Desktop
  shortcut unconditionally with no config toggle to suppress it (checked against the full
  `bundle.windows.wix` schema: it exposes `template`, `fragmentPaths`,
  `componentRefs`/`componentGroupRefs`/`featureRefs`, `upgradeCode`, `version`, and banner/dialog
  image paths — nothing shortcut-related). Suppressing it would require vendoring and hand-editing
  `main.wxs` to delete the `DesktopFolder` component; that tradeoff was considered and declined in
  favor of simplicity. If revisited, fork the template from the exact `@tauri-apps/cli` version
  pinned in `rtmapp/package-lock.json` (not `dev` — the Handlebars variables drift between
  releases) and delete only the `DesktopFolder` block.
- **`bundle.windows.wix.upgradeCode`** is pinned explicitly (see root `AGENTS.md`) rather than
  left to derive from `productName`.
- **`bundle.targets: ["msi"]`** — no NSIS. Tauri's `"all"` target set also emits an NSIS `.exe`
  installer on Windows, which is deliberately out of scope here.
- The release workflow (`.github/workflows/release.yml`) builds `rtmapp.exe`, `rtmcli.exe`, and
  the MSI on `windows-latest`. It never modifies `tauri.conf.json` — see root `AGENTS.md` for the
  versioning convention and `scripts/set-version.ps1`.

---

## Known Issues

- **Load File fails on GNU/Linux.** The GTK file dialog (via `tauri-plugin-dialog`) may return a
  `file://` URI or an unexpected format, which causes `std::path::Path::canonicalize()` in
  `load_file` to fail. Works correctly on Windows and macOS.
