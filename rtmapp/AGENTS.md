# AGENTS.md — rtmapp (egui desktop app)

## What This Component Is

The egui/eframe desktop app for the Todo.txt manager. Targets GNU/Linux, macOS and Windows.

It replaced a Tauri app of the same name in RTM-39 (epic RTM-32), keeping its look, its features and
its config file. Until RTM-39 it was developed as `rtmgui`. The Tauri app's sources are gone; comments
that refer to them name commit `5076649`, the last commit that had them.

Done: window shell in the Inter-based theme, config loading, "Load File" through a native dialog and
the item list (RTM-34); logging of file dialog failures (RTM-42); filters, sorting, quick-add and
complete/uncomplete (RTM-35); the edit dialog (RTM-36) and its calendar date picker (RTM-40); the Nix
package and dev shells (RTM-37); the cut-over (RTM-39); the app icon (RTM-46); the MSI, deb and
AppImage (RTM-38); deleting from the edit dialog (RTM-24); the macOS bundle and dmg (RTM-11); sync
through Git or Pijul (RTM-64), in the background since RTM-20, with the check for a dirty or
untracked todo.txt on open (RTM-70); "New File" (RTM-55); file dialogs that do not block the UI
(RTM-65).

---

## Architecture / Structure

```
rtmapp/
├── Cargo.toml              ← Member of the root workspace; also the deb description (cargo-deb)
├── build.rs                ← Embeds the icon and version information in rtmapp.exe (Windows targets)
├── wix/main.wxs            ← The MSI (cargo-wix, WiX v3), see Packaging
├── assets/
│   ├── fonts/              ← Inter Regular and SemiBold (OTF) and the licence
│   ├── icons/              ← App icon: SVG masters and the files generated from them (see App icon)
│   ├── Info.plist          ← macOS bundle metadata (scripts/build-macos.sh)
│   └── rtmapp.desktop      ← XDG desktop entry (Nix package, deb and AppImage)
└── src/
    ├── main.rs             ← Window options, eframe::run_native
    ├── app.rs              ← App state, panels, New File and Load File, filters, quick-add,
    │                         complete/uncomplete, saving an edit, deleting
    ├── calendar.rs         ← Date field with the calendar popup (`date_field`), week start
    ├── config.rs           ← AppConfig and rtm/config.toml
    ├── edit.rs             ← Edit dialog: the form (`EditForm`, validation in `apply`) and the modal
    ├── list.rs             ← Filtering and sorting (`visible`), item rows (`RowAction`)
    ├── sync.rs             ← Sync through the file's repository: `find`, the worker thread
    ├── theme.rs            ← Palettes, fonts, egui styling
    └── tests/              ← Unit tests, one file per source file and topic
```

---

## Key Dependencies

| Crate               | Purpose                                            |
|---------------------|----------------------------------------------------|
| eframe 0.37         | Window, event loop, egui integration               |
| todotxt (path)      | Parsing, serialization, `TodoLibrary`              |
| rtmsync (path)      | Commit, pull and push through Git or Pijul         |
| rfd 0.16            | Native file dialog (`xdg-portal` backend on Linux) |
| serde, toml 0.8     | Config file                                        |
| dirs 5              | OS config directory                                |
| chrono              | Dates (same as the core library)                   |
| pollster 1          | Waits for `rfd`'s async file dialog on its thread  |
| env_logger 0.11     | Logs to stderr (`rfd`'s file dialog errors)        |
| sys-locale 0.3      | The OS locale, for the calendar's week start       |
| icu_calendar 2.4    | CLDR first day of the week for a locale            |
| icu_locale_core 2.4 | Parses the locale for `icu_calendar`               |
| windows-sys 0.52    | Windows only: the taskbar ID (winit's version)     |

`Cargo.lock` also has pollster 0.4: `rfd` up to 0.17.2 needs it on Linux for `xdg-portal`. rfd's main
branch is on pollster 1; upgrade `rfd` once that is released (RTM-84).

---

## Conventions

- **Rust edition:** 2024. No async, with one exception (RTM-65): the file dialogs are
  `rfd::AsyncFileDialog` futures, each awaited with `pollster::block_on` on a thread of its own (see
  File dialog). No runtime, no `async fn`; anything else stays synchronous.
- **Identity:** crate, binary, Nix package and eframe app name are `rtmapp`. The app name is also the
  Wayland app id and the X11 `WM_CLASS`, so it must match the desktop entry's file name and its
  `StartupWMClass`; `Exec=rtmapp` and `Icon=rtmapp` match the binary and the installed icons, and
  `CFBundleExecutable` in `assets/Info.plist` matches the binary (the macOS bundle does not start
  otherwise). Keep them all in agreement.
- **Config:** `rtm/config.toml` under `dirs::config_dir()`. The schema is the one the Tauri app wrote,
  so existing users' settings carry over: do not rename fields or change defaults (`#[serde(default)]`
  on `AppConfig`). The file is written after a successful file load (or creation) or a filter
  change, never at startup. Defaults live in a manual `impl Default` (since RTM-64, so `sync` can
  default to true): a new field gets its default there. Keys added for sync: `sync` (default true;
  false turns off commits as well as pulls and pushes) and `vcs` (`"git"` or `"pijul"`, any case,
  the answer when the file's directory has both; anything else means ask; cleared when another
  file is opened).
  `sync_interval_minutes` (RTM-20, default 10; 0 turns the timer off) sets the background sync.
- **egui 0.37 API:** `eframe::App::ui` (not `update`) and `egui::Panel` (not `SidePanel` and
  `TopBottomPanel`). A label inside `ui.horizontal` does not wrap unless `wrap_mode(TextWrapMode::Wrap)`
  is set. A `Popup` shown from inside a `Modal` must be made a sublayer of the modal's layer
  (`ctx.set_sublayer`), or a click in the dialog draws the dialog over it. `Sense::click()` includes
  `FOCUSABLE`: hit areas inside a popup use `Sense::CLICK`, so Tab does not walk into them.
- **Look:** reproduces the Tauri app's `styles.css`; see "Look" below.
- **Filtering and sorting** (`list::visible`) follow the Tauri app's `get_items`, and a test checks the
  filters against its predicate (rewritten for clippy, equivalent), for every flag combination. Sorting puts items
  without a due date first, then dated items, with priority as the tie-break (none last). **One
  deliberate difference:** dates run earliest first and `reverse_sort` gives latest first; the Tauri
  app had it the other way round, so the saved `reverse_sort` setting now gives the opposite order.
- **Item rows** show the visible items in that order. A done item is struck through entirely, as the
  CSS did. Recurrence is shown through `TodoRecurrence`'s `Display` (`rec:+1m` appears as `rec:+m`).
  The painted checkbox has a hit area larger than its 14 px box, so the row layout stays as the CSS had
  it. Rows report the clicked item's index in the file, never its position in the filtered list.
- **Mutations** call the same `TodoLibrary` methods in the same order as the Tauri app did and save at
  once: add is `add_item` (which sets a creation date when the item has none) then `save`; uncomplete
  is `uncomplete_item` then `save`; complete is `complete_item` (which appends the next occurrence of a
  recurring item that has a due date) then `save`, then `load` once the save succeeded; delete is
  `remove_item` (which also removes the item's sub-items) then `save`. Errors go to
  the error line above the list, not to a dialog; the quick-add input is cleared only after a
  successful save. Every save rewrites the whole file, and recurrence goes through `Display`, so a
  count of 1 is written in its short form (`rec:+1m` becomes `rec:+m`); this is intended.
- **Edit dialog** (`edit.rs`): a click on an item's text opens it, filled as the Tauri app's
  `openEditDialog` did; Save (or Enter in a field) writes the fields as its `update_item` did, then
  `save`. Fields the dialog does not show (done, dates of completion and creation, `uuid`, `sub`) are
  kept. **Deliberate differences from the Tauri app:** the priority dropdown offers A to Z, not just A
  to C; an invalid date or recurrence keeps the dialog open with a message and writes nothing, where
  the Tauri app dropped the field silently; one leading `+` or `@` on a project or context name is
  stripped, and a name with whitespace is refused, where the Tauri app wrote it as typed and it split
  into several words on reload; Escape and a click outside the dialog cancel, where the Tauri app
  closed only on its buttons. **Delete** (RTM-24, the Tauri app had none): a trash button at the
  left of the button row, opposite Cancel/Save, asks inline. The question gets its own line above
  the row ("Delete this item?", or "… and N sub-items?" with the count from `remove_item` on a copy
  of the library, since `uuid`/`sub` are not shown anywhere), and Delete (red) and Keep replace the
  trash button; Keep goes back, Escape or the backdrop cancel the whole dialog.
- **Calendar** (`calendar.rs`): Due Date and Threshold are text fields (typing still works, as
  flatpickr's `allowInput` did) with a popup drawn at the sizes of the flatpickr popup the Tauri app
  used (`rtmapp/src/assets/flatpickr.min.css` at `5076649`). It opens on a click or focus and closes on
  a pick, Clear, Escape, Tab or a click outside. Escape and a click elsewhere in the dialog close only
  the calendar; a click on the backdrop still cancels the dialog (egui's `Modal` does not hold that
  back for open popups, as with the priority dropdown). **Deliberate differences from flatpickr:** dark
  theme from the app's dark palette (flatpickr is light only); a Clear button; the week starts on the
  locale's first day, not always Sunday; month arrows instead of the month dropdown; the mouse wheel
  over the year steps it (touchpad scrolling is gathered into whole notches); names stay English; no
  keyboard navigation of the day grid.
- **Week start:** the locale is `LC_ALL`, then `LC_TIME`, then `LANG` when set (`#[cfg(unix)]`, so this
  includes macOS started from a terminal), otherwise `sys-locale` (Windows, and macOS started from
  Finder); `icu_calendar`'s CLDR week data gives the first day for it. `C`, `POSIX`, an empty or
  unparsable locale start on Sunday. The explicit "first day of week" setting of macOS and Windows is
  not read yet (RTM-44).
- **Windows:** release builds use the `windows` subsystem (`#![cfg_attr(...)]` in `main.rs`), as the
  Tauri app did, so no console window opens behind the GUI. The window opens centred at 900 × 650.
  Before it opens, `main.rs` sets the AppUserModelID `com.brcha.rtm` (`windows-sys`, the version winit
  uses), the one on the MSI shortcuts, so the taskbar groups the window with them and with old pins.
- **Sync (RTM-64, background since RTM-20)** follows rtmcli's rules (`sync::find`): `sync = false`,
  no repository or an ignored file → no sync and no commits; both Git and Pijul → the Git-or-Pijul
  dialog (or "Don't sync", which asks again next time); a conflicted repository (`is_conflicted`) →
  the file is not opened and the error line says to resolve it by hand; a failed check (e.g. `git`
  missing) → "Not syncing: …" in the header; no remote → commit only.
  - **The check on open (RTM-70)**, the RTM-57 table in `rtmsync/AGENTS.md`: `find` reads
    `Vcs::state` and returns `Found::Ask(repo, question)` for a modified or untracked file; no worker
    starts until `App::answer`. A modal (`sync_question`) asks. Modified: "Commit them" commits `rtm:
    commit outside changes` and syncs, "Don't open" closes the file with a "commit or revert by hand"
    error. Untracked: "Add it" commits `rtm: track <name>` (which adds it) and syncs, "Not now" opens
    it without sync, asked again next time. An untracked file in a conflicted repository opens
    without sync and without asking. A missing file keeps the old rule (tracked files sync), as in
    rtmcli: its state cannot be told reliably.
  - **One worker thread** (`sync::Worker`) runs every git or pijul call: `Commit` (local) and
    `Sync` (pull, then push) jobs, a result per job (`Done`), and `ctx.request_repaint()` after each,
    so results arrive without input. At most one `Sync` waits in the queue.
  - **When it syncs:** after the file is opened (after the answer, when the check on open asks),
    every `sync_interval_minutes` since the last sync was asked for, on the header's **Sync now**
    button (shown when there is a remote, disabled while a sync runs) and on exit. The timer and the results run in `App::logic`, which eframe also calls
    while the window is minimized or hidden; `request_repaint_after` wakes it when a sync is due.
  - **Editing never waits.** Each mutation is a `Change` (add, toggle, edit, delete) applied to the
    list at once and kept in `pending` until written. `flush` writes and queues one `Commit` only
    while the worker is idle (`Worker::busy`: jobs sent whose result was not handled yet, so a pull
    whose reload has not happened still counts); changes made during a job wait in memory and make
    one commit (`rtm: N changes`, one line each). Messages name the item's description only (`rtm:
    complete Pay bill`). New File and Load File are disabled during any job and while a file dialog is open, Sync
    now during a sync; nothing else.
  - **A pull that changed the file** (also one whose push then failed: `NotSynced { changed }`)
    reloads it and redoes `pending` on it, finding each item by equality (with two identical lines,
    the first); a change whose item the pull changed or removed is dropped and named in the error
    line. The edit dialog's item (kept in `edited`) is then found again; if it is gone, or what
    deleting it would remove changed, `stale` makes Save and Delete refuse. The header shows
    "Syncing…", or "Not synced: …" (git's first line; the rest on hover) after a failed pull or push.
  - **A conflict from a job closes the file**, as on open: a reload would read the markers as items.
    The error line counts the pending changes that could not be saved.
  - **On exit** (`on_exit`, eframe's wgpu build, `finish_jobs`): outstanding results are handled as
    in `logic`, pending changes are written and committed, and a last sync runs, within 5 s. When
    changes are waiting behind a running job, that job gets up to `SAVE_TIMEOUT` (125 s, a pull and
    a push timeout), so they are not lost.
- Tests are unit tests in `src/tests/`, one file per source file and topic (see Tests in the root
  `AGENTS.md`); rtmapp is a binary crate, so it has no integration tests (UI tests with
  `egui_kittest` are RTM-86). They use temp files and never open a window. `App` tests build
  `App { .. }` directly with a temp `config_path` (see `app_with` and `bare_app` in
  `app_support_tests.rs`), never `config::config_path()`: a load or filter save would overwrite the
  user's real `rtm/config.toml`. They do not start sync; the sync tests call `connect` on scratch
  git remotes and clones (local paths, local config keeping the user's identity, signing, hooks and
  rerere out) and `wait` for the worker, which a headless `egui::Context` serves.

---

## Look

`theme.rs` and the widgets reproduce the palettes, spacing, radii, borders and shadows of the Tauri
app's `rtmapp/src/styles.css` (at `5076649`), in both themes. The theme follows the system preference;
`RTMAPP_THEME=light` or `dark` forces one.

- **Reference sizes,** read from the Tauri app's DOM: header 50 px, sidebar 220 px, text input 37 px,
  list rows 46 px, dialog 500 px wide. The sidebar and dialog widths are set directly; the others
  follow from the CSS paddings and font sizes the code reproduces.
- **How it was compared (RTM-33):** the unmodified Tauri frontend in Chromium with a mocked
  `window.__TAURI__`, against screenshots of an egui prototype with the same sample items. Chromium is
  not the WebKitGTK webview the Tauri app used, so small text-rendering differences were expected.
- **Focus ring:** a 3 px translucent stroke painted outside the input's border.
- **Item text:** one `LayoutJob` per row (`list::row_job`), with per-span font, colour and
  strikethrough.
- **Known differences from the Tauri look:**
  - egui has no font weights: Inter Regular is the text face and SemiBold is the `bold` family (title,
    headings, priorities). The CSS weight 500 of the labels rendered regular under
    `font-synthesis: none`, so labels are regular.
  - The ☐ and ☑ glyphs are missing from most fonts, so the list's checkboxes are painted, and so is
    the edit dialog's trash button (`theme::trash_button`, with an accessible name). The sidebar
    filters use egui's checkbox with a grey outline, and the dialog's priority a `ComboBox` styled like
    the text inputs, in place of the native checkbox and `<select>`.
  - The CSS 0.2 s hover transition and egui's modal fade are not matched.
- **History:** the egui prototype, its full findings (`FINDINGS.md`) and its screenshot script
  (`shots.sh`) were in `spikes/egui-theme/`, last changed in `5076649`, until RTM-47 removed them.

---

## Fonts and licence

Inter 4.0 (`Inter-Regular.otf`, `Inter-SemiBold.otf`) is embedded with `include_bytes!`, from the
official `Inter-4.0.zip` release of github.com/rsms/inter, with `assets/fonts/Inter-LICENSE.txt` (SIL
Open Font License 1.1) next to it. The licence requires its text and the copyright notice to accompany
every redistributed copy, so **installers and packages must ship `Inter-LICENSE.txt`**. The Nix package
installs it under `share/licenses/rtmapp`, the deb and the AppImage under `usr/share/doc/rtmapp/`, the
MSI next to `rtmapp.exe`, and the macOS bundle in `Contents/Resources/`.

---

## App icon

A page whose first line is ticked, on a Rust-orange rounded square (`#ce422b`; the tick box is the
done green `#28a745`). `assets/icons/rtmapp.svg` is the master; `rtmapp-small.svg` is a simplified
variant (no text lines, a larger tick) for 16 and 24 px, where the master's lines blur. Edit the SVGs
by hand and keep them plain (no Inkscape metadata); every other file in `assets/icons/` is generated
from them and keeps its name, because the Nix packages and the installers refer to those names.

Regenerate after changing an SVG (from `rtmapp/assets/icons/`; `inkscape` is not in the dev shell):

```sh
t=$(mktemp -d)
for s in 16 24; do inkscape rtmapp-small.svg --export-type=png --export-width=$s --export-filename=$t/$s.png; done
for s in 32 48 64 128 256 512 1024; do inkscape rtmapp.svg --export-type=png --export-width=$s --export-filename=$t/$s.png; done
cp $t/16.png 16x16.png; cp $t/24.png 24x24.png; cp $t/32.png 32x32.png
cp $t/128.png 128x128.png; cp $t/256.png 128x128@2x.png; cp $t/512.png icon.png
nix-shell -p icoutils --run "icotool -c -o icon.ico $t/16.png $t/24.png $t/32.png $t/48.png $t/64.png $t/128.png -r $t/256.png"
nix-shell -p libicns --run "png2icns icon.icns $t/16.png $t/32.png $t/128.png $t/256.png $t/512.png $t/1024.png"
```

`icotool -r` stores the 256 px entry as PNG (ImageMagick writes it as an uncompressed bitmap, over three
times the size). The Nix packages install the PNGs as hicolor `rtmapp.png` from 16 to 512 px and the
master as `scalable/apps/rtmapp.svg`; the 16 and 24 px PNGs keep panels from scaling the master down.

The window icon (title bar and taskbar on Windows and X11) is `128x128@2x.png`, embedded in `main.rs`;
a test checks that it decodes. Wayland compositors take the icon from the desktop entry instead, and
macOS (Finder, Dock) from the bundle, which carries `icon.icns` as `Contents/Resources/rtmapp.icns`. Explorer and the taskbar take `rtmapp.exe`'s icon from its
resources: `build.rs` embeds `icon.ico` there (an installer can only set its shortcuts' icon). The deb
and the AppImage install the same icons and desktop entry as the Nix package; the MSI uses `icon.ico`
for its shortcuts and the Add/Remove Programs entry.

---

## File dialog

`rfd` uses the `xdg-portal` backend on Linux, so a running `xdg-desktop-portal` with a FileChooser
backend (for example `-gtk` or `-kde`) is needed at runtime. If the portal fails, `rfd` tries `zenity`
and then returns `None`, which is also what a cancelled dialog returns, so the UI cannot tell the two
apart. The fallback is the `gtk3` feature (`rfd = { version = "0.16", default-features = false, features =
["gtk3"] }`), which needs GTK 3 at runtime.

`rfd` logs both failures at `error` level, and `env_logger` prints them to stderr: run the app from a
terminal to see why "Load File" did nothing. A cancel logs nothing. `RUST_LOG=warn` (or `debug`) shows
more. Windows release builds have no console (see Conventions), so there the log is not visible; use a
debug build.

A load error (unreadable file, missing path) is shown above the list.

**The dialogs do not block the UI (RTM-65).** A synchronous `rfd::FileDialog` inside `ui` stopped the
window from repainting until it closed, so it looked hung. Now `pick_file` and `new_file` create an
`rfd::AsyncFileDialog` future on the UI thread, parented to the window (`set_parent` with
`eframe::Frame`), and `show_dialog` awaits it with `pollster::block_on` on a new thread, which sends
the path (or `None`) over a channel and calls `request_repaint`. The future must be created on the UI
thread: on macOS rfd runs the AppKit panel there, which the sync dialog on a worker thread cannot do.
`dialog_result`, called from `logic`, applies the result with `open_file` or `create_file`, but only
while no job runs and no item is being edited: on Linux the window still takes input behind the
portal dialog, so a sync may start or the edit dialog open meanwhile. Until then the result waits in
the channel; when the worker is idle its pending changes have been written to the old file. A
cancel, a failure (logged by rfd, as before) or a dialog thread that died clears the dialog and
changes nothing. New File and Load File are disabled while a dialog is open. Closing the app with a
dialog open leaves the thread to end with the process; it writes nothing.

"New File" (RTM-55) shows a save dialog (`.txt` filter, default name `todo.txt`), creates the file
empty (`File::create`) and opens it through the same path as "Load File" (`open_file`: config, check
on open). An existing file chosen there is **replaced** with an empty one: every backend `rfd` uses
asks before returning it (the portal's backends, gtk3, `zenity --confirm-overwrite`, the Windows and
macOS save panels). A new file inside a repository is untracked, so the check on open asks "Add it";
replacing a tracked file leaves it modified, so it asks "Commit them" (which commits and syncs the
empty file) or "Don't open" (the old items stay only in the file's history).

---

## Build & Run

```sh
# Linux: enter the dev shell first (from the repo root)
nix-shell

cargo run -p rtmapp
cargo test -p rtmapp

# Or as a Nix package (Linux; tests run during the build)
nix build .#rtmapp      # or: nix-build -A rtmapp
```

The binary links no native libraries beyond glibc; it loads Wayland, xkbcommon, X11, GL, Vulkan and
fontconfig with `dlopen`. The dev shell puts them on `LD_LIBRARY_PATH`, and the Nix package on the
binary's rpath (`eguiDeps` in `default.nix`, see [`doc/nix-packaging.md`](../doc/nix-packaging.md)).

What it needs at run time (RTM-38); the deb's `depends` and `suggests` and the RPM's `requires` and
`suggests` (by soname, RTM-9) in `Cargo.toml` follow this. The Wayland entries were found by starting
it on Wayland with reduced `LD_LIBRARY_PATH` sets and checking that it draws; the X11 list is what a
full run under XWayland loaded, not reduced further:

- **Wayland (required):** `libwayland-client.so.0` (without it winit fails with `NoWaylandLib`) and
  `libxkbcommon.so.0` (winit panics with `XKBNotFound`).
- **GPU (one of):** `libvulkan.so.1`, or `libEGL.so.1` plus `libwayland-egl.so.1` (without the latter
  the GL path panics). With neither, wgpu fails to create a surface. The driver itself comes with the
  system (Mesa).
- **Fonts (required by the deb):** `libfontconfig.so.1`. The UI starts and draws with the bundled Inter
  without it, but has no fallback for characters Inter lacks (CJK, emoji, many symbols).
- **Optional (Suggests):** the X11 set, used only without `WAYLAND_DISPLAY`: `libX11.so.6`,
  `libX11-xcb.so.1`, `libXcursor.so.1`, `libXi.so.6`, `libxkbcommon-x11.so.0`. GNOME is Wayland-only
  from Ubuntu 25.10 and Debian forky; X11 matters mainly for older or non-GNOME X sessions.
- **File dialog:** `rfd` talks to the XDG desktop portal over D-Bus (no library), so the deb depends on
  `xdg-desktop-portal` and a backend that implements `org.freedesktop.impl.portal.FileChooser`: gtk,
  gnome or kde on jammy. `xdg-desktop-portal-wlr` (and Hyprland's portal) has none, so wlroots desktops
  need gtk beside it. The dialog was not opened in those reduced runs.

To try it without touching your own config, point `XDG_CONFIG_HOME` at a scratch directory that has an
`rtm/config.toml` with a `file_name` line.

`RTMAPP_THEME=light` or `RTMAPP_THEME=dark` forces a theme instead of following the system, for
screenshots of both themes; any other value is ignored.

---

## Packaging

Built by the Release workflow ([`doc/release-process.md`](../doc/release-process.md)). The deb and the AppImage install
the binary, the desktop entry, the hicolor icons and both licences; the MSI installs `rtmapp.exe`
(icon embedded), both licences and the Start Menu and desktop shortcuts; the dmg holds the bundle with
the binary, `Info.plist`, the icon and both licences.

- **deb:** `[package.metadata.deb]` in `Cargo.toml` (cargo-deb). `depends` and `suggests` follow the
  run-time list under Build & Run.
- **RPM:** `[package.metadata.generate-rpm]` in `Cargo.toml` (cargo-generate-rpm); `requires` and
  `suggests` follow the same list, by soname (see "Linux (RTM-8)" in [`doc/release-process.md`](../doc/release-process.md)).
- **AppImage:** `scripts/build-appimage.sh <binary> <version> <output>` (also runs locally; set
  `APPIMAGE_TOOLS_DIR` to keep its downloads).
- **dmg:** `scripts/build-macos.sh <binary> <version> <output.dmg>`, on a Mac only. Bundle
  `Rusty Todo.txt Manager.app`, executable `rtmapp`, metadata from `assets/Info.plist` (identifier
  `com.brcha.rtm`, minimum macOS 11.0, `@VERSION@` replaced by the version), ad-hoc signature. The
  Release workflow passes a universal binary; a local `target/release/rtmapp` works too, for the host's
  architecture only. Gatekeeper blocks the first launch of a downloaded copy (see the README).
- **MSI:** `wix/main.wxs`, built with `cargo wix -p rtmapp --no-build` on Windows. It replaces the Tauri
  MSI of versions up to 0.1.3, so it keeps what that MSI had where it matters. The Tauri MSI, decompiled
  with WiX's `dark.exe` during RTM-38, had:
  - UpgradeCode `E0E423D1-1101-50D4-BAEE-4A27FA127509`, per-machine, `MajorUpgrade` allowing
    downgrades — **kept** (the UpgradeCode must never change);
  - Manufacturer `brcha` and the HKCU key `Software\brcha\Rusty Todo.txt Manager`, whose `InstallDir`
    value is read back by a `RegistrySearch`, so an upgrade keeps a custom install folder — **kept**
    (the Release workflow tests it);
  - install folder `Program Files\Rusty Todo.txt Manager` with the exe at its root, desktop shortcut,
    "Launch" checkbox on the finish screen — **kept**; the shortcuts are optional features here;
  - Start Menu shortcut inside a `Rusty Todo.txt Manager` folder — **now directly in Programs**;
  - AppUserModelID `com.brcha.rtm` on that shortcut — **kept**, on both shortcuts; the app sets the same
    ID at startup (RTM-48), and the Release workflow reads it back from the installed shortcuts;
  - an Uninstall shortcut in the install folder — **dropped** (Settings > Apps uninstalls);
  - a "PATH Environment Variable" feature that only referenced the exe and never set PATH, and the
    WebView2 bootstrapper — **dropped**.

---

## Known Issues

- Dialog failures look like a cancel in the UI; the cause is on stderr (see File dialog above).
- The edit dialog does not scroll: a `ScrollArea` inside egui's `Modal` stays too short. The form fits
  the default window; revisit if it grows (see the `shortcut:` comment in `edit::show`).
