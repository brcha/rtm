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
├── spikes/egui-theme/      ← Historical spike for the egui look (RTM-33), not built by CI
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
  tests without ever starting the GUI. The `.deb`'s run-time `Depends:` are packaging (RTM-38).

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

> **The Release workflow is broken until RTM-38.** RTM-39 removed the Tauri app, but `release.yml`
> still builds the Tauri MSI, deb and AppImage and reads the version from the deleted
> `tauri.conf.json`. Do not tag a release before RTM-38 rewrites it. The rest of this section
> describes the Tauri-based workflow as input for RTM-38.
>
> Identity values from the deleted `rtmapp/src-tauri/tauri.conf.json` that the new installers must
> keep: MSI `upgradeCode` **`e0e423d1-1101-50d4-baee-4a27fa127509`** (must never change once shipped),
> `productName` "Rusty Todo.txt Manager", `identifier` `com.brcha.rtm`, `mainBinaryName` `rtmapp`,
> deb package name `rtmapp`, deb `depends` `libwebkit2gtk-4.1-0` and `libgtk-3-0` (Tauri's; the egui
> app needs its own list). The release version's source of truth moves to the `rtmapp` crate's
> `Cargo.toml`.

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
- **`scripts/set-version.ps1`** (PowerShell) and **`scripts/set-version.sh`** (bash) are two
  ports of one manual pre-release tool, not something CI invokes; keep them in step. Run either
  locally to bump the version across `rtmapp/Cargo.toml`, `rtmcli/Cargo.toml` and
  `todotxt/Cargo.toml` in lockstep (since RTM-39; before, it also patched the Tauri JSON files),
  review the diff, commit it, and only then tag. It scopes its match to the `[package]` table
  specifically, not just "the first line
  starting with `version`" — `todotxt/Cargo.toml` also has a `[dependencies.uuid]` table with its
  own `version` key, and a naive match would be correct only by accident of ordering. This
  already covers `rtmcli/Cargo.toml`, so the Debian package version tracks the CalVer bump with
  no separate step.
- **`mainBinaryName: "rtmapp"`** in `tauri.conf.json` is a load-bearing invariant, not cosmetic. It
  keeps the build's output filename aligned with `flake.nix`/`default.nix`
  (`mainProgram = "rtmapp"`) and `rtmapp/assets/rtmapp.desktop` (`Exec=rtmapp`), which would otherwise
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
  `rtmapp/assets/rtmapp.desktop` is not used for either — it exists solely for the Nix packaging path
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
