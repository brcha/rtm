# AGENTS.md — rtmgui (egui desktop app)

## What This Component Is

An egui/eframe desktop app for the Todo.txt manager, with the same look as the Tauri `rtmapp`. `rtmgui` is
a **working name**: it replaces `rtmapp` and is renamed to `rtmapp` when it reaches feature parity, at
which point the Tauri crate is removed (epic RTM-32; the cut-over is RTM-39). Targets GNU/Linux, macOS and
Windows.

Done so far (RTM-34): window shell in the Inter-based theme, config loading, "Load File" through a
native dialog, and a read-only item list. Still to come: filters, sorting, add, complete and edit
(RTM-35, RTM-36), a custom calendar date picker (RTM-40), native dependencies in CI and Nix (RTM-37) and
installers (RTM-38).

---

## Architecture / Structure

```
rtmgui/
├── Cargo.toml              ← Standalone workspace, own release profile
├── Cargo.lock              ← Committed; separate from the root lock
├── assets/fonts/           ← Inter Regular and SemiBold (OTF) and the licence
└── src/
    ├── main.rs             ← Window options, eframe::run_native
    ├── app.rs              ← App state, panels, Load File, load_library
    ├── config.rs           ← AppConfig and rtm/config.toml
    ├── list.rs             ← Item rows
    └── theme.rs            ← Palettes, fonts, egui styling
```

---

## Standalone workspace

`rtmgui` is not a member of the root Cargo workspace. `rfd`'s `build.rs` panics when its `gtk3` and
`xdg-portal` features are enabled together ("You can't enable both `gtk3` and `xdg-portal` features at
once"). `tauri-plugin-dialog` in the root workspace enables `gtk3` and `rtmgui` uses `xdg-portal`, and a
whole-workspace build (which is what CI runs) unifies the features of the one `rfd` it compiles, so it
would fail. A separate workspace keeps `rtmgui`'s `rfd` on its own.

Consequences:
- Build and test it with `--manifest-path rtmgui/Cargo.toml`; the root `cargo build` and `cargo test` do
  not cover it. `rust.yml` has separate steps for this.
- `rtmgui/Cargo.toml` repeats the root `[profile.release]`, because profiles only apply at a workspace
  root. Keep the two in step.
- `rtmgui/Cargo.lock` is committed and is separate from the root lock. `scripts/set-version.ps1` and
  `scripts/set-version.sh` bump `rtmgui/Cargo.toml` but, like for the other crates, do not refresh any
  lock file.
- **Merging back (RTM-39):** when the Tauri app is removed, delete the `[workspace]` table, the copied
  `[profile.release]` and `rtmgui/Cargo.lock`, add the crate to the root `members`, drop the two `rtmgui`
  steps from `rust.yml`, and re-run `cargo build` so the root lock is regenerated.

---

## Key Dependencies

| Crate           | Purpose                                              |
|-----------------|------------------------------------------------------|
| eframe 0.37     | Window, event loop, egui integration                 |
| todotxt (path)  | Parsing, serialization, `TodoLibrary`                |
| rfd 0.16        | Native file dialog (`xdg-portal` backend on Linux)   |
| serde, toml 0.8 | Config file                                          |
| dirs 5          | OS config directory                                  |
| chrono          | Dates (same as the core library)                     |

`dirs` and `toml` are pinned to the versions `rtmapp` uses, because both apps write the same file.

---

## Conventions

- **Rust edition:** 2024. No async.
- **Config:** `rtm/config.toml` under `dirs::config_dir()`, shared with `rtmapp` until RTM-39, so field
  names and defaults must not diverge (`#[serde(default)]` on `AppConfig`). The file is written only
  after a successful file load, never at startup.
- **egui 0.37 API:** `eframe::App::ui` (not `update`) and `egui::Panel` (not `SidePanel` and
  `TopBottomPanel`). A label inside `ui.horizontal` does not wrap unless `wrap_mode(TextWrapMode::Wrap)`
  is set.
- **Look:** palettes and sizes come from `rtmapp/src/styles.css`, as worked out in
  `spikes/egui-theme/` (see its `FINDINGS.md`; `shots.sh` there takes comparison screenshots). Palette
  fields are added as widgets are ported. egui has no font weights: Inter Regular is the text face and
  SemiBold is the `bold` family (title, headings, priorities). The ☐ and ☑ glyphs are missing from most
  fonts, so checkboxes are painted.
- **Item rows** are read-only and in file order until RTM-35. A done item is struck through entirely, as
  the CSS does. Recurrence is shown through `TodoRecurrence`'s `Display` (`rec:+1m` appears as `rec:+m`),
  like `rtmapp`.
- Tests are inline `#[cfg(test)]` modules; they use temp files and never open a window.

---

## Fonts and licence

Inter 4.0 (`Inter-Regular.otf`, `Inter-SemiBold.otf`) is embedded with `include_bytes!`, from the
official `Inter-4.0.zip` release of github.com/rsms/inter, with `assets/fonts/Inter-LICENSE.txt` (SIL
Open Font License 1.1) next to it. The licence requires its text and the copyright notice to accompany
every redistributed copy, so **installers and packages must ship `Inter-LICENSE.txt`** (RTM-38).

---

## File dialog

`rfd` uses the `xdg-portal` backend on Linux, so a running `xdg-desktop-portal` with a FileChooser
backend (for example `-gtk` or `-kde`) is needed at runtime. If the portal fails, `rfd` tries `zenity`
and then returns `None`, which is also what a cancelled dialog returns; no message can be shown for it.
The fallback is the `gtk3` feature (`rfd = { version = "0.16", default-features = false, features =
["gtk3"] }`), which needs GTK 3 at runtime.

A load error (unreadable file, missing path) is shown above the list.

---

## Build & Run

```sh
# Linux: enter the dev shell first (from the repo root)
nix-shell

cargo run --manifest-path rtmgui/Cargo.toml
cargo test --manifest-path rtmgui/Cargo.toml
```

To try it without touching your own config, point `XDG_CONFIG_HOME` at a scratch directory that has an
`rtm/config.toml` with a `file_name` line.

---

## Known Issues

- **Dev shell on some machines:** `~/.cargo/config.toml` may set `sccache` as the rustc wrapper, which
  fails inside `nix-shell` (missing temp directory); build with `RUSTC_WRAPPER=`. A binary linked inside
  the shell can need a glibc fix-up to start; both problems are described in
  `spikes/egui-theme/FINDINGS.md`. `shell.nix` has no clippy: run it through
  `nix-shell -p clippy` inside the shell. These belong to RTM-37.
- Dialog failures look like a cancel (see File dialog above).
