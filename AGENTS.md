# AGENTS.md — Rusty Todo.txt Manager (rtm)

## What This Project Is

A [Todo.txt](https://github.com/todotxt/todo.txt) manager written in Rust. Targets GNU/Linux,
macOS, and Windows. Early development — core functionality works across two frontends.

Subpart AGENTS.md files:
- [`todotxt/AGENTS.md`](todotxt/AGENTS.md) — core library
- [`rtmcli/AGENTS.md`](rtmcli/AGENTS.md) — CLI frontend
- [`rtmapp/AGENTS.md`](rtmapp/AGENTS.md) — Tauri web-based desktop app

---

## Architecture / Structure

```
rtm/                        ← Cargo workspace root
├── todotxt/                ← Core library (no UI, no binary)
├── rtmcli/                 ← CLI (clap-based)
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

- **Rust edition:** 2024 (todotxt, rtmcli); 2021 (rtmapp/src-tauri)
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
- **Nix shell:** `shell.nix` provides all GTK/WebKit/X11/Wayland libraries needed to build and run
  all frontends on NixOS or with `nix-shell`. Use `nix-shell` before running `cargo build`.
- **Native dependency sync:** When adding a new system/native library dependency to any crate in
  the workspace, register it in **all three** of the following locations to keep CI, Nix flake
  builds, and legacy nix-build in sync:
  1. `.github/workflows/rust.yml` — the `apt-get install` step
  2. `flake.nix` — `guiDeps` or `tauriDeps` list (and `devShells.default` if needed)
  3. `default.nix` — matching `guiDeps` or `tauriDeps` list

---

## Nix Packaging

- **`flake.nix`** — modern entry point. `nix build .#rtmcli`, `nix build .#rtmapp`.
- **`default.nix`** — legacy entry point. `nix-build -A rtmcli`.
- **`Cargo.lock` must stay committed** — `importCargoLock` in `buildRustPackage` requires it.
- **nixpkgs pin:** `nixos-unstable` (needs Rust ≥ 1.85 for edition 2024).
- **`rtmapp` packaging is best-effort** — Tauri's bundler is bypassed; frontend assets are copied
  into `$out/share/rtmapp/` via `postInstall`. Runtime behavior on NixOS may need further tuning.
- **Dev shell:** `nix develop` (flake) or `nix-shell` (legacy). `.envrc` uses `use_nix`; change
  to `use flake` if using nix-direnv ≥ 2.30.

---

## Future Plans

- Cloud sync via private git repo for todo.txt files
- Mobile GUI (Android-centric, cross-platform)
- Subtasks (uuid + sub tags already in the data model, needs UI)
- Comments per item (uuid-keyed, stored in a subdir of the todo.txt directory)
