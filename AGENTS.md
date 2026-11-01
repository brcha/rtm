# AGENTS.md — Rusty Todo.txt Manager (rtm)

## What This Project Is

A [Todo.txt](https://github.com/todotxt/todo.txt) manager written in Rust. Targets GNU/Linux,
macOS, and Windows. Early development — core functionality works across two frontends.

Subpart AGENTS.md files:
- [`todotxt/AGENTS.md`](todotxt/AGENTS.md) — core library
- [`rtmcli/AGENTS.md`](rtmcli/AGENTS.md) — CLI frontend
- [`rtmapp/AGENTS.md`](rtmapp/AGENTS.md) — egui desktop app
- [`rtmsync/AGENTS.md`](rtmsync/AGENTS.md) — sync through a VCS (epic RTM-1)
- [`site/AGENTS.md`](site/AGENTS.md) — the project website (Zola, epic RTM-79)

Documents:
- [`doc/release-process.md`](doc/release-process.md) — making a release, the Release workflow, and
  the Windows, Linux and macOS packages and their tests
- [`doc/nix-packaging.md`](doc/nix-packaging.md) — the Nix packages, the dev shell and the nixpkgs pin
- [`doc/ci.md`](doc/ci.md) — which changes start which build, and the Linux CI apt hardening

---

## Architecture / Structure

```
rtm/                        ← Cargo workspace root
├── todotxt/                ← Core library (no UI, no binary)
├── rtmcli/                 ← CLI (clap-based)
├── rtmapp/                 ← Desktop app (egui/eframe)
├── rtmsync/                ← Sync of todo.txt through Git or Pijul (library)
├── site/                   ← Project website (Zola); its scripts are in scripts/site/
├── vercel.json             ← How Vercel builds and deploys the website (site/AGENTS.md)
├── doc/                    ← Detailed reference: release process, Nix packaging, CI
├── .claude/skills/         ← Repository skills, each pointing to its document in doc/
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
  `releases/vYY.M` for a PR against a release branch (see [Release Process](doc/release-process.md)).
- **Commit subjects** (RTM-110): `type(scope): summary -- RTM-NN`, a conventional-commit type and
  exactly one scope, for commits on issue branches and PR titles alike (merges are squashes, so a PR
  title becomes the commit). `.github/workflows/pr-title.yml` rejects a PR title without one of
  these scopes; a new part of the repository adds its scope there and here in the same PR.
  - `todotxt`, `rtmcli`, `rtmapp`, `rtmsync`: the crates (for `rtmapp`, its packaging metadata in
    `rtmapp/Cargo.toml` too).
  - `site`: the website, `scripts/site/`, `vercel.json`, its workflow and the release posts.
  - `release`: `release.yml`, the packages, `scripts/set-version.*`, `scripts/changelog.sh`,
    `cliff.toml`, `doc/release-process.md` and the release PRs.
  - `nix`: `default.nix`, `flake.nix`, `shell.nix`, `doc/nix-packaging.md`.
  - `github`: the other GitHub Actions workflows, the rulesets and `doc/ci.md`.
  - `workspace`: repository-wide files (root `AGENTS.md`, `README.md`, the workspace `Cargo.toml`
    and `Cargo.lock`, `.claude/`) and changes spread over several parts.

  A change that touches several parts takes the scope of the part it is about (a crate change with
  its documentation and tests is that crate's); `workspace` only when no part dominates. The type
  says what kind of change it is, the scope where: `ci(github): ...`, not `ci(ci): ...`.
- **Tests** (RTM-83):
  - Unit tests, which reach a module's private items, live in `<crate>/src/tests/`, one file per
    source file and topic, named `<source file stem>_<topic>_tests.rs`
    (`todo_item_parsing_tests.rs`, `config_file_tests.rs`); the topic is always present. The source
    file declares each at its end:
    ```rust
    #[cfg(test)]
    #[path = "tests/todo_item_parsing_tests.rs"]
    mod parsing_tests;
    ```
    `#[path]` resolves relative to the declaring file's directory, and the module stays a child of
    the source module, so `use super::*;` reaches private items. No inline `mod tests` blocks, no
    `mod.rs` and no plain `tests.rs`.
  - Helpers shared by several topic files of one source file go into
    `<source file stem>_support_tests.rs`, declared as `mod support_tests;`, with the helpers
    `pub(super)`; the topic files import them with `use super::support_tests::*;`.
  - A source file in a subdirectory (none yet) mirrors it under `src/tests/`: `src/foo/bar.rs`
    declares `#[path = "../tests/foo/bar_<topic>_tests.rs"]`.
  - Integration tests, which use only the public interface, live in each crate's top-level
    `tests/`, one test binary per file, named `<area>_<topic>_tests.rs` (`library_file_tests.rs`,
    `cli_sync_tests.rs`). New `rtmcli` tests run the binary through `CARGO_BIN_EXE_rtmcli`
    (`cli_commands_tests.rs` still goes through `cargo run`). `rtmapp` is a binary crate and has
    none; its UI tests run in-process with `egui_kittest`, as unit tests (`app_ui_tests.rs`, see
    `rtmapp/AGENTS.md`).
  - Every test works in scratch directories under the system temp directory and never reads or
    writes the user's real `rtm/config.toml` or todo.txt. A new test that runs a program removes
    the environment variables that would change its result (`TODOTXT`, `RTM_NO_SYNC`, `RTM_VCS`).

---

## Important Notes

- **Nix shell:** `shell.nix` (or `nix develop`) provides the toolchain and the libraries `rtmapp` loads
  at run time. Use it before running `cargo build` on NixOS.
- **Native dependency sync:** When adding a new system/native library dependency to any crate in
  the workspace, register it in **both** of the following locations to keep CI and the Nix builds
  in sync:
  1. `.github/workflows/rust.yml` — an `apt-get install` step (no crate links a native library
     today; the existing apt step installs pijul's libraries for the `rtmsync` tests, see below;
     use the hardened pattern from `release.yml`, see RTM-28 in [CI](doc/ci.md))
  2. `default.nix` — the matching list (`eguiDeps` today; `flake.nix` and `shell.nix` import it),
     plus `shell.nix`'s inputs if `cargo build` in the dev shell needs the library

  **Run-time-only libraries are the exception to item 1.** `rtmapp` links no native libraries beyond
  glibc: egui, winit, wgpu and the font provider load Wayland, xkbcommon, X11, GL, Vulkan and
  fontconfig with `dlopen`. Those go only into the Nix `eguiDeps` list (put on the binary's rpath,
  and, on Linux, on the dev shell's `LD_LIBRARY_PATH`); not into an apt step: `rust.yml` builds and
  tests without ever starting the GUI, and the Release workflow's smoke launches get them from the
  installed deb's `depends`, installing only its `suggests` and the Mesa drivers themselves. A new run-time library also goes into the `rtmapp` deb's `depends` (or `suggests`, if the
  app runs fully without it) in `[package.metadata.deb]` of `rtmapp/Cargo.toml`, and by soname into
  the `rtmapp` RPM's `requires` (or `suggests`) in `[package.metadata.generate-rpm]`; see the run-time
  notes in `rtmapp/AGENTS.md` for how the current list was found.

  This rule governs **additions**. It does not invert: apt's `-dev` packages pull each other in
  transitively, so an apt list is intentionally a minimal subset of what the Nix lists declare. Do
  not trim the Nix lists to match — Nix `buildInputs` do not propagate `pkg-config` search paths the
  way apt's package interdependencies do.

  **Test tools are not native dependencies.** `rust.yml` builds `pijul` (RTM-18), which the
  `rtmsync` tests drive, on Linux and macOS, installing the libraries it needs (OpenSSL and
  libsodium, from apt and Homebrew), and downloads a prebuilt `pijul.exe` on Windows. No crate
  links them, so they do not go into the Nix lists or the debs; in Nix they go into
  `nativeCheckInputs` only, never `eguiDeps` (see [Nix Packaging](doc/nix-packaging.md)). See `rtmsync/AGENTS.md`.

---

## Future Plans

- Mobile GUI (Android-centric, cross-platform)
- Subtasks (uuid + sub tags already in the data model, needs UI)
- Comments per item (uuid-keyed, stored in a subdir of the todo.txt directory)
