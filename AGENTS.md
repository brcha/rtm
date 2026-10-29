# AGENTS.md — Rusty Todo.txt Manager (rtm)

## What This Project Is

A [Todo.txt](https://github.com/todotxt/todo.txt) manager written in Rust. Targets GNU/Linux,
macOS, and Windows. Early development — core functionality works across two frontends.

Subpart AGENTS.md files:
- [`todotxt/AGENTS.md`](todotxt/AGENTS.md) — core library
- [`rtmcli/AGENTS.md`](rtmcli/AGENTS.md) — CLI frontend
- [`rtmapp/AGENTS.md`](rtmapp/AGENTS.md) — egui desktop app

---

## Architecture / Structure

```
rtm/                        ← Cargo workspace root
├── todotxt/                ← Core library (no UI, no binary)
├── rtmcli/                 ← CLI (clap-based)
├── rtmapp/                 ← Desktop app (egui/eframe)
├── flake.nix, default.nix  ← Nix packages
└── shell.nix               ← Nix dev shell
```

`todotxt`, `rtmcli` and `rtmapp` share the root Cargo workspace (`Cargo.lock` at root). The `todotxt`
library is the single source of truth for parsing, serialization, and business logic. Frontends depend
on it via `path` dependency.

`rtmapp` replaced a Tauri app of the same name in RTM-39 (epic RTM-32): same features, same look and
the same config file. The Tauri sources are gone; references to them name commit `5076649`, the last
one that had them.

`rtmapp` stores its config in the OS config directory (`dirs::config_dir()`), under
`rtm/config.toml`. `rtmcli` has no config file (file from `-f` or `$TODOTXT`).

---

## Conventions

- **Rust edition:** 2024 everywhere.
- **No async** in the core library, the CLI or the desktop app.
- **Shared config key:** `file_name` (TOML string) — path to the active todo.txt file.
- **Date format:** `%Y-%m-%d` everywhere (chrono `NaiveDate`).
- **Priority encoding:** stored as `u8` (0 = A, 1 = B, …).
- All `save()` calls happen immediately after mutations (no deferred/batched writes).
- **Branches:** issue work goes on `issues/RTM-NN/<slug>`, branched from `main`.

---

## Important Notes

- **Nix shell:** `shell.nix` (or `nix develop`) provides the toolchain and the libraries `rtmapp` loads
  at run time. Use it before running `cargo build` on NixOS.
- **Native dependency sync:** When adding a new system/native library dependency to any crate in
  the workspace, register it in **all three** of the following locations to keep CI, Nix flake
  builds, and legacy nix-build in sync:
  1. `.github/workflows/rust.yml` — an `apt-get install` step (there is none today: no crate links a
     native library; reuse the hardened pattern from `release.yml` when adding one, see RTM-28 below)
  2. `flake.nix` — the matching list (`eguiDeps` today), and `devShells.default` if needed
  3. `default.nix` — the matching list

  **Run-time-only libraries are the exception to item 1.** `rtmapp` links no native libraries beyond
  glibc: egui, winit, wgpu and the font provider load Wayland, xkbcommon, X11, GL, Vulkan and
  fontconfig with `dlopen`. Those go only into the Nix `eguiDeps` lists (put on the binary's rpath,
  and on the flake dev shell's `LD_LIBRARY_PATH`) and into the `shellHook` library list in
  `shell.nix`, which has no `eguiDeps` list of its own; not into an apt step, because CI builds and
  tests without ever starting the GUI. A new run-time library also goes into the `rtmapp` deb's
  `depends` (or `suggests`, if the app runs fully without it) in `[package.metadata.deb]` of
  `rtmapp/Cargo.toml`; see the run-time notes in `rtmapp/AGENTS.md` for how the current list was found.

  This rule governs **additions**. It does not invert: apt's `-dev` packages pull each other in
  transitively, so an apt list is intentionally a minimal subset of what the Nix lists declare. Do
  not trim the Nix lists to match — Nix `buildInputs` do not propagate `pkg-config` search paths the
  way apt's package interdependencies do.
- **Linux CI apt hardening (RTM-28):** GitHub-hosted Ubuntu runners resolve apt through a
  mirrorlist that tries `azure.archive.ubuntu.com` before falling back to `archive.ubuntu.com`.
  When the Azure-local mirror degrades — dropping connections rather than refusing them — apt
  retries it per index file across roughly twenty files, which can stall a job for hours with no
  bound. Any job that installs apt packages (today only `release.yml`'s Linux job; `rust.yml` needs
  none since RTM-39) overwrites `/etc/apt/apt-mirrors.txt` with the canonical archive before
  updating, and passes explicit `Acquire::Retries`/timeout options to both `apt-get update` and
  `apt-get install` as defence in depth. **Do not revert this for a marginally faster "good day"
  run** — it trades a small, predictable slowdown for an unbounded hang becoming a fast, loud
  failure instead.

---

## Nix Packaging

- **`flake.nix`** — modern entry point. `nix build .#rtmcli`, `nix build .#rtmapp`.
- **`default.nix`** — legacy entry point. `nix-build -A rtmcli`, `nix-build -A rtmapp`.
- **`Cargo.lock` must stay committed** — `importCargoLock` in `buildRustPackage` requires it.
- **`rtmapp` package (Linux only):** no native build inputs; `eguiDeps` are added to the binary's
  rpath with `patchelf --add-rpath` in `postFixup` (after the rpath shrinking, which would drop
  them). `postInstall` installs `rtmapp/assets/rtmapp.desktop`, the hicolor icons (PNGs from 16 to
  512 px and the scalable SVG, see `rtmapp/AGENTS.md`) and
  `Inter-LICENSE.txt` (under `share/licenses/rtmapp`). Its version is read from
  `rtmapp/Cargo.toml`; `rtmcli` still has a hard-coded `version` in `commonArgs`.
- **nixpkgs pin:** the flake follows the stable `nixos-26.05` (rustc 1.95, the minimum egui 0.37
  needs); consumers can override it with `inputs.rtm.inputs.nixpkgs.follows`. Move to
  `nixos-26.11` when it is released (RTM-45). `default.nix` and `shell.nix` use the caller's
  `<nixpkgs>`, which must also have rustc ≥ 1.95.
- **Dev shell:** `nix develop` (flake) or `nix-shell` (legacy). `.envrc` uses `use_nix`; change
  to `use flake` if using nix-direnv ≥ 2.30. Both shells have `rustfmt` and `clippy`, and both
  start `NIX_LDFLAGS`, `NIX_CFLAGS_LINK`, `NIX_CFLAGS_COMPILE` and `PKG_CONFIG_PATH` empty and set
  `RUSTC_WRAPPER` to empty: a calling session that exports `-L/usr/lib` otherwise links against the
  system glibc instead of the shell's (the binary then fails with `GLIBC_2.xx not found`), and a
  user-configured sccache fails inside the shell. Keep these in both shells.

---

## Release Process

- **Workflow:** `.github/workflows/release.yml` (named `Release`) — triggers on a `v*` tag push
  (publishes a draft GitHub Release), manual `workflow_dispatch` (dry run: builds and uploads to the
  Actions run, publishes nothing, no tag required), and pull requests against `main` that touch the
  crates, `Cargo.toml`/`Cargo.lock`, `LICENSE`, `scripts/build-appimage.sh` or the workflow (builds and
  tests the packages, publishes nothing). x64 only — no ARM64, no 32-bit, on either platform.
- **Assets:** `rtmapp_<v>_x64_en-US.msi`, `rtmcli-<v>-x64.exe`, `rtmapp_<v>_amd64.deb`,
  `rtmapp_<v>_amd64.AppImage`, `rtmcli_<v>_amd64.deb`. Since RTM-38 they are built without Tauri:
  cargo-wix, cargo-deb and appimagetool.
- **Job structure:** `version` (resolves and verifies the release version once, on `ubuntu-latest`)
  → `windows` and `linux` (build and test in parallel, each `needs: version`, each uploading its own
  `dist/` as a plain build artifact — `dist-windows` / `dist-linux`) → `publish` (`needs: [version,
  windows, linux]`, skipped on pull requests, downloads both artifact sets and performs the dry-run
  upload or draft-release publish exactly once). Both platform jobs deliberately do **not** call
  `softprops/action-gh-release` themselves — two independent jobs calling it concurrently would race
  to create/update the same tagged release.
- **Versioning:** `major.minor.patch`, all numeric (MSI `ProductVersion` allows nothing else). Releases
  stay on 0.x while the project is in alpha; the switch to CalVer (`YY.minor.patch`) comes later. The
  `[package]` version of `rtmapp/Cargo.toml` is the single source of truth — the release workflow reads
  it (only that table: other tables have their own `version` keys) and never writes to it. On a tag
  push, the tag is required to match it exactly; a mismatch fails the build loudly instead of shipping
  packages whose internal version disagrees with their file names.
- **`scripts/set-version.ps1`** (PowerShell) and **`scripts/set-version.sh`** (bash) are two ports of
  one manual pre-release tool, not something CI invokes; keep them in step. Run either locally to bump
  the version across `rtmapp/Cargo.toml`, `rtmcli/Cargo.toml` and `todotxt/Cargo.toml` in lockstep,
  review the diff, commit it, and only then tag. It scopes its match to the `[package]` table
  specifically — `todotxt/Cargo.toml` also has a `[dependencies.uuid]` table with its own `version`
  key. A new release must have a higher version than the last one, or neither the MSI nor apt
  replaces the installed copy.
- **Identity:** the binary is `rtmapp` everywhere (`flake.nix`/`default.nix` `mainProgram`, the desktop
  entry's `Exec=rtmapp`, `Icon=rtmapp` and `StartupWMClass=rtmapp`, the deb package name); the display
  name is "Rusty Todo.txt Manager"; the reverse-DNS identifier (Tauri's `identifier`, kept for a macOS
  bundle and the Windows AppUserModelID, RTM-11 and RTM-48) is `com.brcha.rtm`.
- **Merges are squashes.** One PR becomes exactly one commit on `main`, so PR titles are the
  changelog — the release workflow's `generate_release_notes: true` consumes them directly, and
  `.github/workflows/pr-title.yml` already forces them to be conventional and RTM-tagged.
- **No raw platform binaries are published redundantly with their installer.** `rtmapp.exe` is not
  staged on Windows — the MSI installs the identical binary. `rtmcli.exe` **is** staged, because
  neither the MSI nor (on Linux) the `rtmapp` deb packages the CLI; it is the only channel for
  `rtmcli` on Windows. This asymmetry is deliberate, not an oversight — do not "fix" it for symmetry.
- **Unsigned** for now (RTM-10).

### Windows

- **MSI source:** `rtmapp/wix/main.wxs` (WiX v3), built by `cargo wix -p rtmapp` (cargo-wix 0.3.9,
  `cargo install --locked`; WiX 3.14.1 is on the runner image, installed with choco if a future image
  drops it). Its header lists what it keeps from the Tauri MSI; `rtmapp/AGENTS.md` has the details.
- **`UpgradeCode` `E0E423D1-1101-50D4-BAEE-4A27FA127509` must never change.** It is the Tauri MSI's
  (Tauri derived it from the product name), and with it `MajorUpgrade` replaces an installed version
  instead of installing beside it. Per-machine install, downgrades allowed (the build's only ICE
  warning, ICE61, comes from that).
- **Exe resources:** `rtmapp/build.rs` embeds `icon.ico` and the product name in `rtmapp.exe` with
  `winresource`, checking `CARGO_CFG_TARGET_OS` (not `#[cfg]`, which tests the host).
- **Tests in the job:** a silent fresh install (files, both shortcuts, the exe's version information,
  uninstall), then an upgrade from the v0.1.3 Tauri MSI installed into a custom folder (one product
  afterwards, at the new version, still in that folder). Both read the product from the uninstall
  entries in the registry: the Windows Installer COM API failed with `DISP_E_TYPEMISMATCH` from
  PowerShell. **The upgrade test downloads the v0.1.3 release asset, so that release must stay.**
  More tests: RTM-49.

### Linux (RTM-8)

- **Runner floor: `ubuntu-22.04`, not `ubuntu-latest`.** Packages built against `noble`'s (24.04)
  glibc would not install on Debian bookworm or older Ubuntu LTS releases. Building on the older jammy
  floor maximizes where the artifacts actually work.
- **debs:** both come from cargo-deb, described in `[package.metadata.deb]` of `rtmapp/Cargo.toml`
  and `rtmcli/Cargo.toml` (package name, maintainer, assets, dependencies). `rtmapp`'s `depends` lists
  the libraries it loads with `dlopen` by hand (`$auto` sees only glibc); see `rtmapp/AGENTS.md`.
- **AppImage:** `scripts/build-appimage.sh` lays out an AppDir by hand (binary, desktop entry, hicolor
  icons, licences; no bundled libraries) and packs it with appimagetool 1.9.1 and the type2 runtime
  20251108, downloaded from fixed release URLs and checked against their SHA-256 (upstream also
  publishes moving "continuous" builds). appimagetool runs with `--appimage-extract-and-run`, so no
  FUSE is needed. The downloads are a network dependency in the release path, so the step has its own
  `timeout-minutes`.
- **Tests in the job:** `apt-get install` of both debs (every `Depends` must resolve on jammy), checks
  that `$auto` added `libc6`, that the desktop entry and all icon sizes are installed and that a portal
  backend with a file chooser came with `rtmapp`; then the AppImage is extracted and checked.
- **Apt hardening is reused, not reinvented.** The job pins the `archive.ubuntu.com` mirror and passes
  `Acquire::Retries`/timeout options (see "Linux CI apt hardening (RTM-28)" under Important Notes). It
  installs no build dependencies, but the install test resolves the debs' dependencies from the index.
- **No raw Linux binaries are published.** Only the `.deb` packages and the `.AppImage`. RPM (RTM-9)
  and non-Debian distros more generally have no release artifact for `rtmcli` as a result — accepted
  for now, revisit if RTM-9 is scheduled.
- **Prebuilt-binary install actions are a trap on this runner (RTM-29).** `cargo-deb` was originally
  installed via `taiki-e/install-action`, which fetched upstream's only published asset — built by
  upstream CI on `ubuntu-latest`, which has meant **noble** (24.04, glibc 2.39) since GitHub's runner
  migration. This runner is jammy (22.04, glibc 2.35) by the deliberate choice above, so the binary
  failed immediately: `` GLIBC_2.39' not found ``. Fixed by compiling with `cargo install cargo-deb
  --locked --version 3.7.0` instead, which links against the runner's own glibc by construction.
  **This generalizes:** prefer `cargo install --locked` for Rust tooling on this job unless a tool's
  release process is confirmed to target the jammy floor specifically.

## Future Plans

- Cloud sync via private git repo for todo.txt files
- Mobile GUI (Android-centric, cross-platform)
- Subtasks (uuid + sub tags already in the data model, needs UI)
- Comments per item (uuid-keyed, stored in a subdir of the todo.txt directory)
