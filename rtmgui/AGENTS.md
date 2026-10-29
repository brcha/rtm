# AGENTS.md — rtmgui (egui desktop app)

## What This Component Is

An egui/eframe desktop app for the Todo.txt manager, with the same look as the Tauri `rtmapp`. `rtmgui` is
a **working name**: it replaces `rtmapp` and is renamed to `rtmapp` when it reaches feature parity, at
which point the Tauri crate is removed (epic RTM-32; the cut-over is RTM-39). Targets GNU/Linux, macOS and
Windows.

Done so far: window shell in the Inter-based theme, config loading, "Load File" through a native dialog
and the item list (RTM-34); logging of file dialog failures (RTM-42); filters, sorting, quick-add and
complete/uncomplete (RTM-35); the edit dialog (RTM-36) and its calendar date picker (RTM-40); the Nix
package and dev shells (RTM-37). Still to come: installers (RTM-38) and the cut-over (RTM-39).

---

## Architecture / Structure

```
rtmgui/
├── Cargo.toml              ← Standalone workspace, own release profile
├── Cargo.lock              ← Committed; separate from the root lock
├── assets/fonts/           ← Inter Regular and SemiBold (OTF) and the licence
└── src/
    ├── main.rs             ← Window options, eframe::run_native
    ├── app.rs              ← App state, panels, Load File, filters, quick-add, complete/uncomplete,
    │                         saving an edit
    ├── calendar.rs         ← Date field with the calendar popup (`date_field`), week start
    ├── config.rs           ← AppConfig and rtm/config.toml
    ├── edit.rs             ← Edit dialog: the form (`EditForm`, validation in `apply`) and the modal
    ├── list.rs             ← Filtering and sorting (`visible`), item rows (`RowAction`)
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
  steps from `rust.yml`, and re-run `cargo build` so the root lock is regenerated. In `flake.nix` and
  `default.nix`, the `rtmgui` package then drops `cargoRoot`, `buildAndTestSubdir` and its own lock
  and uses the root `Cargo.lock` like the others.

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
| env_logger 0.11 | Logs to stderr (`rfd`'s file dialog errors)          |
| sys-locale 0.3  | The OS locale, for the calendar's week start         |
| icu_calendar 2.4 | CLDR first day of the week for a locale          |
| icu_locale_core 2.4 | Parses the locale for `icu_calendar`          |

`dirs` and `toml` are pinned to the versions `rtmapp` uses, because both apps write the same file.

---

## Conventions

- **Rust edition:** 2024. No async.
- **Config:** `rtm/config.toml` under `dirs::config_dir()`, shared with `rtmapp` until RTM-39, so field
  names and defaults must not diverge (`#[serde(default)]` on `AppConfig`). The file is written after a
  successful file load or a filter change (as `rtmapp` does), never at startup.
- **egui 0.37 API:** `eframe::App::ui` (not `update`) and `egui::Panel` (not `SidePanel` and
  `TopBottomPanel`). A label inside `ui.horizontal` does not wrap unless `wrap_mode(TextWrapMode::Wrap)`
  is set. A `Popup` shown from inside a `Modal` must be made a sublayer of the modal's layer
  (`ctx.set_sublayer`), or a click in the dialog draws the dialog over it. `Sense::click()` includes
  `FOCUSABLE`: hit areas inside a popup use `Sense::CLICK`, so Tab does not walk into them.
- **Look:** palettes and sizes come from `rtmapp/src/styles.css`, as worked out in
  `spikes/egui-theme/` (see its `FINDINGS.md`; `shots.sh` there takes comparison screenshots). Palette
  fields are added as widgets are ported. egui has no font weights: Inter Regular is the text face and
  SemiBold is the `bold` family (title, headings, priorities). The ☐ and ☑ glyphs are missing from most
  fonts, so checkboxes are painted.
- **Filtering and sorting** (`list::visible`) follow `rtmapp`'s `get_items`, and a test checks the
  filters against its predicate, copied verbatim, for every flag combination. Sorting puts items
  without a due date first, then dated items, with priority as the tie-break (none last). **One
  deliberate difference:** dates run earliest first and `reverse_sort` gives latest first; `rtmapp` has
  it the other way round, so until RTM-39 the shared `reverse_sort` setting gives opposite orders in the
  two apps.
- **Item rows** show the visible items in that order. A done item is struck through entirely, as the CSS
  does. Recurrence is shown through `TodoRecurrence`'s `Display` (`rec:+1m` appears as `rec:+m`), like
  `rtmapp`. The painted checkbox has a hit area larger than its 14 px box, so the row layout stays as
  the CSS has it. Rows report the clicked item's index in the file, never its position in the filtered
  list.
- **Mutations** call the same `TodoLibrary` methods in the same order as `rtmapp` and save at once:
  add is `add_item` (which sets a creation date when the item has none) then `save`; uncomplete is
  `uncomplete_item` then `save`; complete is `complete_item` (which appends the next occurrence of a
  recurring item that has a due date) then `save`, then `load` once the save succeeded. Errors go to
  the error line above the list, not to a dialog; the quick-add input is cleared only after a
  successful save. Every save rewrites the whole file, and recurrence goes through `Display`, so a
  count of 1 is written in its short form (`rec:+1m` becomes `rec:+m`); this is intended.
- **Edit dialog** (`edit.rs`): a click on an item's text opens it, filled as `rtmapp`'s
  `openEditDialog` does; Save (or Enter in a field) writes the fields as `rtmapp`'s `update_item` does,
  then `save`. Fields the dialog does not show (done, dates of completion and creation, `uuid`, `sub`)
  are kept. **Deliberate differences:** the priority dropdown offers A to Z, not just A to C; an invalid
  date or recurrence keeps the dialog open with a message and writes nothing, where `rtmapp` drops the
  field silently; one leading `+` or `@` on a project or context name is stripped, and a name with
  whitespace is refused, where `rtmapp` writes it as typed and it splits into several words on reload;
  Escape and a click outside the dialog cancel, where `rtmapp` closes only on its buttons.
- **Calendar** (`calendar.rs`): Due Date and Threshold are text fields (typing still works, as
  flatpickr's `allowInput`) with a popup drawn at flatpickr's sizes (`rtmapp/src/assets/flatpickr.min.css`).
  It opens on a click or focus and closes on a pick, Clear, Escape, Tab or a click outside. Escape and a
  click elsewhere in the dialog close only the calendar; a click on the backdrop still cancels the
  dialog (egui's `Modal` does not hold that back for open popups, as with the priority dropdown).
  **Deliberate differences from flatpickr:** dark theme from the app's dark palette (flatpickr is
  light only); a Clear button; the week starts on the locale's first day, not always Sunday; month
  arrows instead of the month dropdown; the mouse wheel over the year steps it (touchpad scrolling is
  gathered into whole notches); names stay English; no keyboard navigation of the day grid.
- **Week start:** the locale is `LC_ALL`, then `LC_TIME`, then `LANG` when set (`#[cfg(unix)]`, so this
  includes macOS started from a terminal), otherwise `sys-locale` (Windows, and macOS started from
  Finder); `icu_calendar`'s CLDR week data gives the first day for it. `C`, `POSIX`, an empty or
  unparsable locale start on Sunday. The explicit "first day of week" setting of macOS and Windows is
  not read yet (RTM-44).
- Tests are inline `#[cfg(test)]` modules; they use temp files and never open a window. `App` tests
  build `App { .. }` directly with a temp `config_path` (see `app_with`), never
  `config::config_path()`: a load or filter save would overwrite the user's real `rtm/config.toml`.

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
and then returns `None`, which is also what a cancelled dialog returns, so the UI cannot tell the two
apart. The fallback is the `gtk3` feature (`rfd = { version = "0.16", default-features = false, features =
["gtk3"] }`), which needs GTK 3 at runtime.

`rfd` logs both failures at `error` level, and `env_logger` prints them to stderr: run the app from a
terminal to see why "Load File" did nothing. A cancel logs nothing. `RUST_LOG=warn` (or `debug`) shows
more.

A load error (unreadable file, missing path) is shown above the list.

---

## Build & Run

```sh
# Linux: enter the dev shell first (from the repo root)
nix-shell

cargo run --manifest-path rtmgui/Cargo.toml
cargo test --manifest-path rtmgui/Cargo.toml

# Or as a Nix package (Linux; tests run during the build)
nix build .#rtmgui      # or: nix-build -A rtmgui
```

The binary links no native libraries; it loads Wayland, xkbcommon, X11, GL, Vulkan and fontconfig
with `dlopen`. The dev shells put them on `LD_LIBRARY_PATH`, and the Nix package on the binary's
rpath (`eguiDeps` in `flake.nix` and `default.nix`, see the root `AGENTS.md`).

To try it without touching your own config, point `XDG_CONFIG_HOME` at a scratch directory that has an
`rtm/config.toml` with a `file_name` line.

`RTMGUI_THEME=light` or `RTMGUI_THEME=dark` forces a theme instead of following the system, for
screenshots of both themes; any other value is ignored.

---

## Known Issues

- Dialog failures look like a cancel in the UI; the cause is on stderr (see File dialog above).
- The edit dialog does not scroll: a `ScrollArea` inside egui's `Modal` stays too short. The form fits
  the default window; revisit if it grows (see the `shortcut:` comment in `edit::show`).
