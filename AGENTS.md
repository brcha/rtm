# AGENTS.md — Rusty Todo.txt Manager (rtm)

## What This Project Is

A [Todo.txt](https://github.com/todotxt/todo.txt) manager written in Rust. Targets GNU/Linux,
macOS, and Windows. Early development — core functionality works across two frontends.

Subpart AGENTS.md files:
- [`todotxt/AGENTS.md`](todotxt/AGENTS.md) — core library
- [`rtmcli/AGENTS.md`](rtmcli/AGENTS.md) — CLI frontend
- [`rtmapp/AGENTS.md`](rtmapp/AGENTS.md) — egui desktop app
- [`rtmsync/AGENTS.md`](rtmsync/AGENTS.md) — sync through a VCS (epic RTM-1)

---

## Architecture / Structure

```
rtm/                        ← Cargo workspace root
├── todotxt/                ← Core library (no UI, no binary)
├── rtmcli/                 ← CLI (clap-based)
├── rtmapp/                 ← Desktop app (egui/eframe)
├── rtmsync/                ← Sync of todo.txt through Git or Pijul (library)
├── flake.nix, default.nix  ← Nix packages
└── shell.nix               ← Nix dev shell
```

`todotxt`, `rtmcli`, `rtmapp` and `rtmsync` share the root Cargo workspace (`Cargo.lock` at root). The `todotxt`
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
- **No async** in the core library, the CLI or the desktop app; the one exception is rtmapp's file
  dialogs (RTM-65, see `rtmapp/AGENTS.md`).
- **Shared config key:** `file_name` (TOML string) — path to the active todo.txt file. `rtmapp`
  also has `sync` (default true), `vcs` (`"git"`/`"pijul"`) and `sync_interval_minutes` (default 10),
  see `rtmapp/AGENTS.md`.
- **Date format:** `%Y-%m-%d` everywhere (chrono `NaiveDate`).
- **Priority encoding:** stored as `u8` (0 = A, 1 = B, …).
- All `save()` calls happen immediately after mutations (no deferred/batched writes).
- **Branches:** issue work goes on `issues/RTM-NN/<slug>`, branched from `main`, or from
  `releases/vYY.M` for a PR against a release branch (see Release Process).

---

## Important Notes

- **Nix shell:** `shell.nix` (or `nix develop`) provides the toolchain and the libraries `rtmapp` loads
  at run time. Use it before running `cargo build` on NixOS.
- **Native dependency sync:** When adding a new system/native library dependency to any crate in
  the workspace, register it in **both** of the following locations to keep CI and the Nix builds
  in sync:
  1. `.github/workflows/rust.yml` — an `apt-get install` step (no crate links a native library
     today; the existing apt step installs pijul's libraries for the `rtmsync` tests, see below;
     use the hardened pattern from `release.yml`, see RTM-28 below)
  2. `default.nix` — the matching list (`eguiDeps` today; `flake.nix` and `shell.nix` import it),
     plus `shell.nix`'s inputs if `cargo build` in the dev shell needs the library

  **Run-time-only libraries are the exception to item 1.** `rtmapp` links no native libraries beyond
  glibc: egui, winit, wgpu and the font provider load Wayland, xkbcommon, X11, GL, Vulkan and
  fontconfig with `dlopen`. Those go only into the Nix `eguiDeps` list (put on the binary's rpath,
  and, on Linux, on the dev shell's `LD_LIBRARY_PATH`); not into an apt step: `rust.yml` builds and
  tests without ever starting the GUI, and the Release workflow's smoke launches get them from the
  installed deb's `depends`, installing only its `suggests` and the Mesa drivers themselves. A new run-time library also goes into the `rtmapp` deb's `depends` (or `suggests`, if the
  app runs fully without it) in `[package.metadata.deb]` of `rtmapp/Cargo.toml`; see the run-time
  notes in `rtmapp/AGENTS.md` for how the current list was found.

  This rule governs **additions**. It does not invert: apt's `-dev` packages pull each other in
  transitively, so an apt list is intentionally a minimal subset of what the Nix lists declare. Do
  not trim the Nix lists to match — Nix `buildInputs` do not propagate `pkg-config` search paths the
  way apt's package interdependencies do.

  **Test tools are not native dependencies.** `rust.yml` builds `pijul` (RTM-18), which the
  `rtmsync` tests drive, on Linux and macOS, installing the libraries it needs (OpenSSL and
  libsodium, from apt and Homebrew), and downloads a prebuilt `pijul.exe` on Windows. No crate
  links them, so they do not go into the Nix lists or the debs; in Nix they go into
  `nativeCheckInputs` only, never `eguiDeps` (see Nix Packaging). See `rtmsync/AGENTS.md`.
- **Linux CI apt hardening (RTM-28):** GitHub-hosted Ubuntu runners resolve apt through a
  mirrorlist that tries `azure.archive.ubuntu.com` before falling back to `archive.ubuntu.com`.
  When the Azure-local mirror degrades — dropping connections rather than refusing them — apt
  retries it per index file across roughly twenty files, which can stall a job for hours with no
  bound. Any job that installs apt packages (`release.yml`'s Linux job, and `rust.yml`'s Linux job
  for pijul's libraries since RTM-18) overwrites `/etc/apt/apt-mirrors.txt` with the canonical
  archive before updating, and passes explicit `Acquire::Retries`/timeout options to both
  `apt-get update` and `apt-get install` as defence in depth. **Do not revert this for a marginally
  faster "good day" run** — it trades a small, predictable slowdown for an unbounded hang becoming
  a fast, loud failure instead.
- **Which changes start which build (RTM-76):** Markdown alone (`**/*.md`) starts no build.
  - The ruleset "Run tests in the PR" covers `main` and `releases/**` (RTM-78). It requires the
    checks `validate` (`pr-title.yml`) and `rust`, `nix` and `release`: one gate job per
    workflow, which passes when its builds passed or were skipped and fails when any failed or was
    cancelled.
  - So `rust.yml`, `nix.yml` and `release.yml` run on every pull request, never filtered by
    `paths` on the trigger: a workflow skipped by a path filter never reports its checks, and the
    required check would block the merge. Their `changes` job reads the pull request's diff and
    skips the build jobs when nothing relevant changed: for Rust and Nix anything but Markdown,
    for Release its list of package inputs (the crates, `Cargo.toml`/`Cargo.lock`, `LICENSE`, the
    build scripts, the workflow), Markdown excluded. Keep that list in step with what goes into
    the packages.
  - Pushes to `main` start Rust and Nix unless every changed file is Markdown (`paths-ignore`; no
    checks are required there). Release also runs on `v*` tags and by manual dispatch, always
    building.
  - All four workflows run on every pull request against `main` or `releases/**`. The required
    checks are not enforced when a branch is created (`do_not_enforce_on_create`), so a release
    branch can be cut by pushing a `main` commit, which has no PR checks. Besides the checks, the
    ruleset forbids deleting and force-pushing those branches and allows only squash-merged PRs.

  A new required check needs the same shape: a gate job that always runs, not a path filter.

---

## Nix Packaging

- **`default.nix`** — the packages, defined once: `nix-build -A rtmcli`, `nix-build -A rtmapp`.
- **`flake.nix`** — the flake entry point: `nix build .#rtmcli`, `nix build .#rtmapp`. It imports
  the packages from `default.nix` and the dev shell from `shell.nix`, both with its pinned nixpkgs,
  and defines only the apps itself.
- **`shell.nix`** — the dev shell, defined once (see Dev shell); it takes `eguiDeps` from
  `default.nix` (`rtmapp.eguiDeps`, a `passthru`).
- **`Cargo.lock` must stay committed** — `importCargoLock` in `buildRustPackage` requires it.
- **`rtmapp` package (Linux and macOS):** no native build inputs; on Linux `eguiDeps` are added to
  the binary's rpath with `patchelf --add-rpath` in `postFixup` (after the rpath shrinking, which
  would drop them); on macOS the system frameworks come from stdenv's Apple SDK, and the result is
  a plain binary under `bin/`, not an app bundle (the dmg comes from `scripts/build-macos.sh`).
  `postInstall` installs `rtmapp/assets/rtmapp.desktop`, the hicolor icons (PNGs from 16 to 512 px and the scalable SVG, see `rtmapp/AGENTS.md`) and
  `Inter-LICENSE.txt` (under `share/licenses/rtmapp`).
- **Metadata from `Cargo.toml` (RTM-75):** nothing about the packages is hard-coded in the Nix
  files. `fromCargo` in `default.nix` takes `pname`, `version`, `meta.description`,
  `meta.license` (an SPDX ID; an unknown one fails evaluation) and `meta.homepage` (from
  `repository`) from each crate's `[package]`; only what Cargo has no field for (platforms,
  `mainProgram`, build flags) is written in Nix. The Cargo `description` is also the deb summary.
- **Tests in the build (RTM-71):** the check phase runs the sync tests, so `gitMinimal`, `pijul` and
  `openssh` (the Pijul test signs through a scratch `ssh-agent`) are `nativeCheckInputs`; `rtmcli`
  also runs `rtmsync`'s tests, with `RTM_REQUIRE_PIJUL` set so the Pijul test cannot skip itself. At
  run time both packages use `git`/`pijul` from the user's `PATH`; they are not wrapped.
- **CI:** `.github/workflows/nix.yml` builds `.#rtmcli` and `.#rtmapp`, tests included, on Linux and
  macOS for pushes to `main` and pull requests against `main` or `releases/**` (RTM-78) that change
  more than Markdown (RTM-76).
  `nix-build` is not run in CI (it takes the caller's `<nixpkgs>`), but the flake builds the same
  definitions from `default.nix` with its pin.
- **nixpkgs pin:** the flake follows the stable `nixos-26.05` (rustc 1.95, the minimum egui 0.37
  needs); consumers can override it with `inputs.rtm.inputs.nixpkgs.follows`. Move to
  `nixos-26.11` when it is released (RTM-45). Called directly (`nix-build`, `nix-shell`),
  `default.nix` and `shell.nix` use the caller's `<nixpkgs>`, which must also have rustc ≥ 1.95.
- **Dev shell:** `nix develop` (flake, pinned nixpkgs) or `nix-shell` (caller's `<nixpkgs>`), both
  from `shell.nix`. `.envrc` uses `use_nix`; change to `use flake` if using nix-direnv ≥ 2.30. It
  has `rustfmt`, `clippy`, `rust-analyzer` and `git-cliff`, sets `LD_LIBRARY_PATH` to `eguiDeps` on
  Linux only (the libraries do not exist on macOS), and sets `NIX_LDFLAGS`, `NIX_CFLAGS_LINK`, `NIX_CFLAGS_COMPILE` and `PKG_CONFIG_PATH` empty and
  `RUSTC_WRAPPER` to empty: a calling session that exports `-L/usr/lib` otherwise links against the
  system glibc instead of the shell's (the binary then fails with `GLIBC_2.xx not found`), and a
  user-configured sccache fails inside the shell.

---

## Release Process

- **Making a release (RTM-72):** releases are made on demand, when a user-visible change is worth
  shipping, from release branches `releases/vYY.M` cut from `main`. Each release has a YouTrack
  issue "Release X.Y.Z" (Component Packaging) whose key its PRs carry. After the cut, changes
  reach a release branch only through squash-merged PRs against it, which need the same checks as
  on `main` (the ruleset covers both, RTM-78). `main` always carries the next release's version.
  - **Minor release `YY.M.0`:**
    1. Cut `releases/vYY.M` from `main` (already at `YY.M.0`) and push it. The ruleset allows
       creating it without checks (`do_not_enforce_on_create`). If the push is still rejected
       (not tried yet; the ruleset has no bypass), remove `refs/heads/releases/**` from the
       ruleset's branches, push, and put the pattern back; record the outcome here.
    2. On `main`, a PR `build: set version to <next> -- RTM-NN` (`scripts/set-version.sh`, then
       `cargo update --workspace` to refresh `Cargo.lock`): the next minor, `YY.(M+1).0`, or the
       next year's first version (`27.1.0`) when no further minor release is wanted this year.
    3. On the branch, a PR `docs: changelog for YY.M.0 -- RTM-NN` with the `CHANGELOG.md`
       written by `scripts/changelog.sh YY.M.0`.
    4. Run the Release workflow on `releases/vYY.M`.
  - **Patch release `YY.M.P`:**
    1. Fixes land on `main` like all development and are cherry-picked to `releases/vYY.M` by PRs
       titled like the original PR (not the squash commit's subject, which ends in `(#NN)`).
       Several fixes can go in before a release.
    2. On the branch, a PR `build: set version to YY.M.P -- RTM-NN`: `set-version`,
       `cargo update --workspace` and `scripts/changelog.sh YY.M.P`, which needs the previous
       patch's tag, so its draft must be published first.
    3. Run the Release workflow on `releases/vYY.M`.
  - **The release run:** a `workflow_dispatch` of Release on a release branch builds and tests the
    packages and publishes a draft release tagged `vX.Y.Z` on that commit, with the version's
    `CHANGELOG.md` section as its body. It first checks that the branch matches the version
    (`releases/v26.1` releases only `26.1.*`) and that no release, draft included, or tag exists
    for it (RTM-73), and that `CHANGELOG.md` has entries for it (RTM-74; a hand-pushed `v*` tag
    needs them on its commit too). A patch on an older line (26.1.2 after 26.2.0) is created with
    `make_latest: false`; when publishing it, make sure "Set as the latest release" is unticked.
    Download and try the draft, then publish it by hand. Releases are not marked as prereleases.
    Keep the `build: set version` and `docs: changelog` subjects exact when squash-merging: the
    changelog leaves those commits out.
  - **`CHANGELOG.md`** lives on the release branches only, never on `main`, and is written by
    `scripts/changelog.sh X.Y.Z` (git-cliff with `cliff.toml`, in the Nix dev shell; bash only).
    A patch release's section covers the commits since the line's previous tag, prepended; a minor
    release's the commits since the previous release branch was cut (the first CalVer release:
    since `v0.1.3`), in a new file. If the release branch moves while the changelog or version PR
    is open, update the PR from the branch and run the script again. Release branches are never
    deleted: the next minor release's range starts at the previous one's cut.
  - **The first CalVer release:** `main` is at 0.1.4, which is never released. Its release issue
    first sets `main` to `YY.1.0` by PR, then follows the minor-release steps.
- **Workflow:** `.github/workflows/release.yml` (named `Release`) — triggers on a `v*` tag push
  (publishes a draft GitHub Release), manual `workflow_dispatch` (on a `releases/**` branch a
  release, see "Making a release"; on any other branch a dry run: builds and uploads to the Actions
  run, publishes nothing, no tag required), and pull requests against `main` or `releases/**`
  (`pr-title.yml` checks those too); on those it builds and tests the packages (publishes nothing)
  only when they touch the crates (`rtmsync` included since RTM-21), `Cargo.toml`/`Cargo.lock`,
  `LICENSE`, `scripts/build-appimage.sh`, `scripts/build-macos.sh` or the workflow, Markdown
  excluded (its `changes` job, see "Which changes start which build"). Windows and Linux are x64
  only — no ARM64, no 32-bit; macOS is one universal (arm64 + x86_64) build (RTM-11).
- **Assets:** `rtmapp_<v>_x64_en-US.msi`, `rtmcli-<v>-x64.exe`, `rtmapp_<v>_amd64.deb`,
  `rtmapp_<v>_amd64.AppImage`, `rtmcli_<v>_amd64.deb`, `rtmapp_<v>_universal.dmg`,
  `rtmcli-<v>-macos-universal.tar.gz`. Since RTM-38 they are built without Tauri: cargo-wix, cargo-deb,
  appimagetool, and (RTM-11) `scripts/build-macos.sh`.
- **Previous release (RTM-49):** the `version` job also outputs `previous`, the newest published
  (non-draft, prereleases included) release below the version being built, for the upgrade tests.
  A manual run can name another one with the `previous` input (`gh workflow run Release --ref <branch>
  -f previous=v0.1.3`), which also lifts the Windows v0.1.3 skip.
- **Job structure:** `version` (resolves and verifies the release version once, on `ubuntu-latest`;
  outputs the run's `mode`, `release`, `dry-run` or `check`, which `publish` acts on, `latest`, and
  `notes`, the draft's body)
  → `windows`, `linux` and `macos` (build and test in parallel, each `needs: version`, each uploading
  its own `dist/` as a plain build artifact — `dist-windows` / `dist-linux` / `dist-macos`) → `publish`
  (`needs: [version, windows, linux, macos]`, skipped on pull requests, downloads all artifact sets and
  performs the dry-run upload or draft-release publish exactly once). The platform jobs deliberately do
  **not** call `softprops/action-gh-release` themselves — independent jobs calling it concurrently
  would race to create/update the same tagged release.
- **Versioning:** `major.minor.patch`, all numeric (MSI `ProductVersion` allows nothing else). The
  0.x alpha ends with the next release, which is the first CalVer one (`YY.minor.patch`); 0.1.4,
  the version in the crates today, is never released. `minor` counts the releases within a year
  and starts at 1 (the first release of 2027 is `27.1.0`, the next `27.2.0`); `patch` fixes a
  release (`27.1.1`). Every `YY.*` is above `0.1.x`, so the MSI and apt upgrades keep working. The
  `[package]` version of `rtmapp/Cargo.toml` is the single source of truth — the release workflow reads
  it (only that table: other tables have their own `version` keys) and never writes to it. On a tag
  push, the tag is required to match it exactly, and on a release branch the branch name (see "Making a
  release"); a mismatch fails the build loudly instead of shipping packages whose internal version
  disagrees with their file names.
- **`scripts/set-version.ps1`** (PowerShell) and **`scripts/set-version.sh`** (bash) are two ports of
  one manual pre-release tool, not something CI invokes; keep them in step. Run either on `main`
  after a cut, or on a release branch for a patch release (see "Making a release"), to bump the
  version across `rtmapp/Cargo.toml`, `rtmcli/Cargo.toml`, `rtmsync/Cargo.toml` and
  `todotxt/Cargo.toml` in lockstep. The Nix packages read their versions from those files, so a
  bump needs no Nix change (RTM-75). It scopes its match to the
  `[package]` table specifically — `todotxt/Cargo.toml` also has a `[dependencies.uuid]` table
  with its own `version` key. A new release must have a higher version than the last one on its line
  (`releases/vYY.M`), or neither the MSI nor apt replaces the installed copy.
- **Identity:** the binary is `rtmapp` everywhere (`default.nix` `mainProgram`, the desktop
  entry's `Exec=rtmapp`, `Icon=rtmapp` and `StartupWMClass=rtmapp`, the deb package name); the display
  name is "Rusty Todo.txt Manager", also the macOS bundle's name (`Rusty Todo.txt Manager.app`, whose
  executable is `rtmapp`); the reverse-DNS identifier (Tauri's `identifier`) is `com.brcha.rtm`. It is
  the macOS `CFBundleIdentifier` (RTM-11) and the Windows AppUserModelID, set by the app at startup and
  on both MSI shortcuts (RTM-48).
- **Merges are squashes.** One PR becomes exactly one commit on its target branch, so PR titles are
  the changelog, and `.github/workflows/pr-title.yml` forces them to be conventional and RTM-tagged.
  git-cliff turns them into `CHANGELOG.md` on the release branches (RTM-74), the release body.
- **No raw platform binaries are published redundantly with their installer.** `rtmapp.exe` is not
  staged on Windows — the MSI installs the identical binary. `rtmcli.exe` **is** staged, because
  neither the MSI nor (on Linux) the `rtmapp` deb packages the CLI; it is the only channel for
  `rtmcli` on Windows. The same holds for the macOS `rtmcli` tarball: the dmg has only the app. This
  asymmetry is deliberate, not an oversight — do not "fix" it for symmetry.
- **Signing:** the Windows and Linux packages are unsigned; the macOS ones are signed ad hoc only (arm64
  code must carry a signature to run), so Gatekeeper asks on first launch. Developer ID signing and
  notarization: RTM-54.
- **Other channels:** Homebrew (RTM-52), Developer ID signing and notarization (RTM-54) and RPM
  (RTM-9) are not required to end the alpha; each is done when wanted. The Nix flake is not a
  release channel: users pin a tag (`github:brcha/rtm/vX.Y.Z`).

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
- **Tests in the job:** a silent fresh install (files, both shortcuts and their AppUserModelID, the
  exe's version information, a smoke launch, uninstall), then an upgrade from the v0.1.3 Tauri MSI
  installed into a custom folder (one product afterwards, at the new version, still in that folder),
  then an upgrade from the previous release's MSI in the default folder (skipped with a notice when that
  is v0.1.3, already covered, or when it has no MSI). All read the product from the uninstall entries in
  the registry: the Windows Installer COM API failed with `DISP_E_TYPEMISMATCH` from PowerShell. **The
  v0.1.3 upgrade test downloads that release's asset, so that release must stay.** The smoke launch
  starts the installed `rtmapp.exe` with a sample `todo.txt` (config in `%APPDATA%\rtm`) and expects it
  running after 5 s, then stops it by its PID; it only warns on failure, since the runner's desktop
  session and adapter (WARP) are not under our control. Its stderr (`RUST_LOG=info`) is in the
  `msi-logs` artifact.

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
  backend with a file chooser came with `rtmapp`; then two smoke launches of the installed `rtmapp`
  with a sample `todo.txt` (via `XDG_CONFIG_HOME`), each expected running after 5 s and stopped by its
  PID, logging with `RUST_LOG=info`: on X11 under `xvfb-run` with Mesa software rendering (lavapipe,
  llvmpipe; blocking, and its window must be mapped), and on Wayland under a headless Weston
  (non-blocking, a warning on failure). Then both debs are purged, the previous release's debs
  installed and the new ones over them (`dpkg-query` must report the new version; skipped with a
  notice when the release lacks a deb). v0.1.3 is not skipped here: it is the Tauri deb, also named
  `rtmapp`, and pulls webkit2gtk. Then the AppImage is extracted and checked.
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

### macOS (RTM-11)

- **Runner:** `macos-latest` (arm64). Both targets (`aarch64-apple-darwin`, `x86_64-apple-darwin`) are
  built there, with `MACOSX_DEPLOYMENT_TARGET=11.0` for the job (also `LSMinimumSystemVersion` in
  `rtmapp/assets/Info.plist`; keep the two equal, the job checks it), and joined with `lipo` into universal binaries. Each
  release build takes about 5 minutes per target.
- **dmg:** `scripts/build-macos.sh <binary> <version> <output.dmg>` lays out `Rusty Todo.txt
  Manager.app` by hand (binary, `Info.plist` with `@VERSION@` filled in, `icon.icns` as
  `Resources/rtmapp.icns`, both licences in `Resources/`), signs it ad hoc and packs it with an
  `/Applications` link using `hdiutil` (retried: it can fail with "Resource busy" on runners). Only
  tools that ship with macOS, no downloads. The rtmcli tarball (binary and `LICENSE`) is made in the
  workflow; its `rtmcli` is signed ad hoc there.
- **Tests in the job:** the dmg is mounted and the app copied to `/Applications`; then the Info.plist
  values, the resources, `codesign --verify --strict` on the app and `rtmcli`, both slices and their
  `minos` (`vtool`), `rtmcli --help` natively and under Rosetta (skipped with a notice without it).
  Then a smoke launch through Launch Services (`open -a`) with a sample `todo.txt` in the runner's own
  config path, expected running after 5 s. It proves only that the process stays up: there is no window
  check (`osascript` would need Accessibility permission), and `open --stderr` captured no log in the
  first run.
- **No upgrade test:** replacing a `.app` carries no installer state, unlike the MSI and the debs.
- `rust.yml` also runs the test suite on `macos-latest`.

## Future Plans

- Mobile GUI (Android-centric, cross-platform)
- Subtasks (uuid + sub tags already in the data model, needs UI)
- Comments per item (uuid-keyed, stored in a subdir of the todo.txt directory)
