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

## Release Process

- **Workflow:** `.github/workflows/release.yml` (named `Release`) — triggers on a `v*` tag push
  (publishes a draft GitHub Release) or manual `workflow_dispatch` (dry run: builds and uploads
  to the Actions run, publishes nothing, no tag required). x64 only — no ARM64, no 32-bit, on
  either platform.
- **Job structure:** `version` (resolves and verifies the release version once, on
  `ubuntu-latest`) → `windows` and `linux` (build in parallel, each `needs: version`, each
  uploading its own `dist/` as a plain build artifact — `dist-windows` / `dist-linux`) →
  `publish` (`needs: [version, windows, linux]`, downloads both artifact sets and performs the
  dry-run upload or draft-release publish exactly once). Both platform jobs deliberately do
  **not** call `softprops/action-gh-release` themselves — two independent jobs calling it
  concurrently would race to create/update the same tagged release.
- **Versioning is CalVer:** `vYY.minor.patch` (first release `v26.1.0`). The committed
  `rtmapp/src-tauri/tauri.conf.json` `version` field is the single source of truth — the release
  workflow reads it and never writes to it. On a tag push, the tag is required to match that field
  exactly; a mismatch fails the build loudly instead of shipping installers whose internal
  version metadata silently disagrees with its own release filename.
- **`scripts/set-version.ps1`** is a manual pre-release tool, not something CI invokes. Run it
  locally to bump the version across `rtmapp/src-tauri/tauri.conf.json`,
  `rtmapp/src-tauri/Cargo.toml`, `rtmcli/Cargo.toml`, `todotxt/Cargo.toml`, and
  `rtmapp/package.json` in lockstep, review the diff, commit it, and only then tag. For Cargo.toml
  files it scopes its match to the `[package]` table specifically, not just "the first line
  starting with `version`" — `todotxt/Cargo.toml` also has a `[dependencies.uuid]` table with its
  own `version` key, and a naive match would be correct only by accident of ordering. This
  already covers `rtmcli/Cargo.toml`, so the Debian package version tracks the CalVer bump with
  no separate step.
- **`mainBinaryName: "rtmapp"`** in `tauri.conf.json` is a load-bearing invariant, not cosmetic. It
  keeps the build's output filename aligned with `flake.nix`/`default.nix`
  (`mainProgram = "rtmapp"`) and `rtmapp/rtmapp.desktop` (`Exec=rtmapp`), which would otherwise
  drift from `productName` (the display string, `"Rusty Todo.txt Manager"`).
- **Merges are squashes.** One PR becomes exactly one commit on `main`, so PR titles are the
  changelog — the release workflow's `generate_release_notes: true` consumes them directly, and
  `.github/workflows/pr-title.yml` already forces them to be conventional and RTM-tagged.
- **No raw platform binaries are published redundantly with their installer.** `rtmapp.exe` is
  not staged on Windows — the MSI installs the identical binary, so shipping it loose adds
  nothing. `rtmcli.exe` **is** staged, because neither the MSI nor (on Linux) the `rtmapp` deb
  package the CLI; it is the only channel for `rtmcli` on Windows. This asymmetry is deliberate,
  not an oversight — do not "fix" it for symmetry.

### Windows

- **`bundle.windows.wix.upgradeCode`** is pinned explicitly in `tauri.conf.json` rather than left
  to Tauri's default derivation from `productName`. The MSI `UpgradeCode` GUID must never change
  once an installer has shipped publicly; pinning it removes `productName` drift as a second, less
  obvious way that could happen.
- Only `msi` is built (`bundle.targets: ["msi"]`). Linux (`deb`/`appimage`) and macOS (`dmg`/`app`)
  bundle types need their own runner OS — listing them in `bundle.targets` would not build them
  here regardless, since Tauri's bundler silently skips any target that doesn't match the host
  OS. The release workflow passes `--bundles` explicitly per platform instead.

### Linux (RTM-8)

- **Runner floor: `ubuntu-22.04`, not `ubuntu-latest`.** A `.deb`/AppImage built against
  `noble`'s (24.04) WebKitGTK and glibc would not install on Debian bookworm or older Ubuntu
  LTS releases. Building on the older jammy floor maximizes where the artifacts actually work.
- **Tool split:** Tauri's own bundler produces the `rtmapp` `.deb` and `.AppImage`
  (`--bundles deb,appimage`); `cargo-deb` packages `rtmcli` separately, since Tauri's bundler has
  no concept of packaging a plain, non-Tauri binary. `rtmcli`'s packaging metadata lives in
  `rtmcli/Cargo.toml` under `[package.metadata.deb]`.
- **The `rtmapp` deb's package name is pinned in CI, not in config — there is no config field
  for it.** `bundle.linux.deb` in `tauri.conf.json` (checked against the full schema and the
  `tauri-bundler` source) has no `name` property. Tauri derives the dpkg `Package:` field by
  kebab-casing `productName`, and the `.deb` filename from `productName` completely unmodified —
  for `"Rusty Todo.txt Manager"` that is `rusty-todo-txt-manager` internally and
  `Rusty Todo.txt Manager_<version>_amd64.deb` on disk. The Linux release job unpacks the built
  `.deb` with `dpkg-deb -R`, rewrites `DEBIAN/control`'s `Package:` line to `rtmapp`, and repacks
  with `dpkg-deb -b` directly to the correct filename. This is a load-bearing step, same
  category as `mainBinaryName` and the pinned `upgradeCode` — if it is ever removed, the package
  silently reverts to shipping as `rusty-todo-txt-manager` with no error, only a `dpkg-deb --field`
  assertion in the release job's own verification step catching it.
  `bundle.linux.deb.depends` is still declared explicitly in `tauri.conf.json`
  (`libwebkit2gtk-4.1-0`, `libgtk-3-0`, verified against `packages.ubuntu.com/jammy`), so the
  package fails loudly at install time on a system without WebKitGTK rather than at first launch.
- **Desktop entry: Tauri's generated `.desktop` file ships in the deb and the AppImage.**
  `rtmapp/rtmapp.desktop` is not used for either — it exists solely for the Nix packaging path
  (see `rtmapp/AGENTS.md`). The two must be kept in agreement on `Exec=rtmapp`, `Icon=rtmapp`,
  and `StartupWMClass=rtmapp` even though only one of them is Tauri's actual output.
- **AppImage build-time network dependency:** Tauri's AppImage bundler downloads `linuxdeploy`
  and `appimagetool` during the build. This is the same class of failure the apt hardening below
  exists to bound, so the build step carries its own `timeout-minutes`.
- **Apt hardening is reused, not reinvented.** The Linux release job pins the same
  `archive.ubuntu.com` mirror and `Acquire::Retries`/timeout options as `rust.yml` — see
  "Linux CI apt hardening (RTM-28)" under Important Notes above. Do not let the two drift apart
  without a reason.
- **No raw Linux binaries are published.** Only the `.deb` packages and the `.AppImage`. RPM
  (RTM-9) and non-Debian distros more generally have no release artifact for `rtmcli` as a
  result — accepted for now, revisit if RTM-9 is scheduled.
- **Prebuilt-binary install actions are a trap on this runner (RTM-29).** `cargo-deb` was
  originally installed via `taiki-e/install-action`, which fetched upstream's only published
  asset — built by upstream CI on `ubuntu-latest`, which has meant **noble** (24.04, glibc
  2.39) since GitHub's runner migration. This runner is jammy (22.04, glibc 2.35) by the
  deliberate choice above, so the binary failed immediately: `` GLIBC_2.39' not found ``. No
  older `cargo-deb` release fixes this — upstream has built on `ubuntu-latest` since well
  before the migration, so the mismatch isn't a version-specific regression. Fixed by
  compiling with `cargo install cargo-deb --locked --version 3.7.0` instead, which links
  against the runner's own glibc by construction. **This generalizes:** any future
  prebuilt-binary installer action added to the `linux` job can reintroduce the same failure.
  Prefer `cargo install --locked` for Rust tooling on this job unless a tool's release process
  is confirmed to target the jammy floor specifically.

---

## Future Plans

- Cloud sync via private git repo for todo.txt files
- Mobile GUI (Android-centric, cross-platform)
- Subtasks (uuid + sub tags already in the data model, needs UI)
- Comments per item (uuid-keyed, stored in a subdir of the todo.txt directory)
