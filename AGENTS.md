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
  a platform difference in the `tauri-plugin-dialog` return value.
- **Nix shell:** `shell.nix` provides all GTK/WebKit/X11/Wayland libraries needed to build and run
  all frontends on NixOS or with `nix-shell`. Use `nix-shell` before running `cargo build`.
- **Native dependency sync:** When adding a new system/native library dependency to any crate in
  the workspace, register it in **all three** of the following locations to keep CI, Nix flake
  builds, and legacy nix-build in sync:
  1. `.github/workflows/rust.yml` — the `apt-get install` step
  2. `flake.nix` — `guiDeps` or `tauriDeps` list (and `devShells.default` if needed)
  3. `default.nix` — matching `guiDeps` or `tauriDeps` list

  This rule governs **additions**. It does not invert: apt's `-dev` packages pull each other in
  transitively (e.g. `libgtk-3-dev` alone brings in a dozen others), so `rust.yml`'s explicit
  list is intentionally a minimal subset of what Nix's `guiDeps`/`tauriDeps` declare. Do not trim
  the Nix lists to match — Nix `buildInputs` do not propagate `pkg-config` search paths the way
  apt's package interdependencies do, and removing an "apparently redundant" entry there breaks
  the build.
- **Linux CI apt hardening (RTM-28):** GitHub-hosted Ubuntu runners resolve apt through a
  mirrorlist that tries `azure.archive.ubuntu.com` before falling back to `archive.ubuntu.com`.
  When the Azure-local mirror degrades — dropping connections rather than refusing them — apt
  retries it per index file across roughly twenty files, which can stall a job for hours with no
  bound. The Linux leg of `rust.yml` now overwrites `/etc/apt/apt-mirrors.txt` with the canonical
  archive before updating, and passes explicit `Acquire::Retries`/timeout options to both
  `apt-get update` and `apt-get install` as defence in depth. **Do not revert this for a
  marginally faster "good day" run** — it trades a small, predictable slowdown for an unbounded
  hang becoming a fast, loud failure instead.

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

## Windows Release Process

- **Workflow:** `.github/workflows/release.yml` — triggers on a `v*` tag push (publishes a draft
  GitHub Release) or manual `workflow_dispatch` (dry run: builds and uploads to the Actions run,
  publishes nothing, no tag required). Runs on `windows-latest` only. x64 only — no ARM64, no
  32-bit.
- **Versioning is CalVer:** `vYY.minor.patch` (first release `v26.1.0`). The committed
  `rtmapp/src-tauri/tauri.conf.json` `version` field is the single source of truth — the release
  workflow reads it and never writes to it. On a tag push, the tag is required to match that field
  exactly; a mismatch fails the build loudly instead of shipping an MSI whose internal
  `ProductVersion` silently disagrees with its own release filename.
- **`scripts/set-version.ps1`** is a manual pre-release tool, not something CI invokes. Run it
  locally to bump the version across `rtmapp/src-tauri/tauri.conf.json`,
  `rtmapp/src-tauri/Cargo.toml`, `rtmcli/Cargo.toml`, `todotxt/Cargo.toml`, and
  `rtmapp/package.json` in lockstep, review the diff, commit it, and only then tag. For Cargo.toml
  files it scopes its match to the `[package]` table specifically, not just "the first line
  starting with `version`" — `todotxt/Cargo.toml` also has a `[dependencies.uuid]` table with its
  own `version` key, and a naive match would be correct only by accident of ordering.
- **`mainBinaryName: "rtmapp"`** in `tauri.conf.json` is a load-bearing invariant, not cosmetic. It
  keeps the Windows build's output filename aligned with `flake.nix`/`default.nix`
  (`mainProgram = "rtmapp"`) and `rtmapp/rtmapp.desktop` (`Exec=rtmapp`), which would otherwise
  drift from `productName` (the display string, `"Rusty Todo.txt Manager"`).
- **`bundle.windows.wix.upgradeCode`** is pinned explicitly in `tauri.conf.json` rather than left
  to Tauri's default derivation from `productName`. The MSI `UpgradeCode` GUID must never change
  once an installer has shipped publicly; pinning it removes `productName` drift as a second, less
  obvious way that could happen.
- **Merges are squashes.** One PR becomes exactly one commit on `main`, so PR titles are the
  changelog — the release workflow's `generate_release_notes: true` consumes them directly, and
  `.github/workflows/pr-title.yml` already forces them to be conventional and RTM-tagged.
- Only `msi` is built (`bundle.targets: ["msi"]`). Linux (`deb`/`appimage`) and macOS (`dmg`/`app`)
  bundle types need their own runner OS and are separate, not-yet-scheduled work under the RTM-6
  epic — listing them in `bundle.targets` would not build them here regardless, since Tauri's
  bundler silently skips any target that doesn't match the host OS.

---

## Future Plans

- Cloud sync via private git repo for todo.txt files
- Mobile GUI (Android-centric, cross-platform)
- Subtasks (uuid + sub tags already in the data model, needs UI)
- Comments per item (uuid-keyed, stored in a subdir of the todo.txt directory)
