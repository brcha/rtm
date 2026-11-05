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
(RTM-65); UI tests with `egui_kittest` (RTM-86); the list of files in the config (RTM-124) and the
per-file state with its own error line (RTM-125) and sync with one worker per repository, the
check on open included (RTM-126) and the open files as tabs (RTM-127), for several open files
(RTM-66); the title bar's
menu and the Settings dialog (RTM-134).

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
    ├── app.rs              ← App state (`App`, and `OpenFile` per open todo.txt), panels, the
    │                         title bar's menu and shortcuts, the tab strip, New File and Load
    │                         File, closing and switching files, filters,
    │                         quick-add, complete/uncomplete, saving an edit, deleting
    ├── calendar.rs         ← Date field with the calendar popup (`date_field`), week start
    ├── config.rs           ← AppConfig and rtm/config.toml
    ├── edit.rs             ← Edit dialog: the form (`EditForm`, validation in `apply`) and the modal
    ├── list.rs             ← Filtering and sorting (`visible`), item rows (`RowAction`)
    ├── settings.rs         ← Settings dialog: the form (`SettingsForm`) and the modal
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
| egui_kittest 0.37   | Dev-dependency: the UI tests (see Conventions)     |

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
  on `AppConfig`). The file is written after a successful file load (or creation), a tab
  closed or clicked, a filter change or Save in the Settings dialog, never at startup. Defaults live in a manual `impl Default` (since RTM-64, so `sync` can
  default to true): a new field gets its default there. Keys added for sync: `sync` (default true;
  false turns off commits as well as pulls and pushes) and, until RTM-124, a top-level `vcs`.
  `sync_interval_minutes` (RTM-20, default 10; 0 turns the timer off) sets the background sync.
  - **The file list (RTM-124, epic RTM-66):** `[[files]]` tables, the saved session, written last
    (after the plain keys). Each has `path`, an optional `name` (the tab label, set by hand) and an
    optional `vcs` (`"git"` or `"pijul"`, any case, the answer when the file's directory has both;
    anything else means ask). `file_name` stays and names the active file, so an older rtmapp still
    opens it. Opening a file adds its entry (`add_file`), closing its tab removes it
    (`remove_file`), and clicking a tab makes it `file_name` (`set_active`).
  - **Migration**, in `AppConfig::load`, in memory only (written with the next save): entries
    without a `path` are dropped, `file_name` gets an entry when it has none, and a top-level `vcs`
    moves into that entry (an older version wrote it for `file_name`, so it replaces the entry's
    choice) and is never written again. An older rtmapp drops `[[files]]` when it saves; the
    migration rebuilds the active entry from `file_name`. At startup, when a loaded file's
    resolved path differs from its entry's (a symlink, `..`), the entry, and `file_name` when it
    names it, take the resolved path in memory (`rename`), since entries are looked up by the
    loaded file's path.
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
  the error lines above the list, not to a dialog; the quick-add input is cleared only after a
  successful save. Every save rewrites the whole file, and recurrence goes through `Display`, so a
  count of 1 is written in its short form (`rec:+1m` becomes `rec:+m`); this is intended.
- **Open files and error lines (RTM-125, epic RTM-66):** what belongs to one todo.txt lives in
  `OpenFile` (its path and tab name, its library, pending changes, the sync questions, the edit
  dialog's `edited`, `deleting` and `stale`, and its error line); `App` holds `files` and `active`,
  the path of the file shown (see Tabs). Each file also keeps the repository it syncs through (`repo`) and why
  it does not sync (`not_syncing`). Two error lines, the app's above the file's, each shown when set:
  - **the file line** (`OpenFile::error`): the configured file failing to load at startup, a
    conflict, "Don't open", a failed write, commit or reload, changes a pull dropped. Cleared by a
    successful change or by loading the file again.
  - **the app line** (`App::error`): a config save, a New File or Load File that failed, "Please
    load a file first", an unparsable quick-add. Cleared by a successful config save or change.
  So a file message stays when a filter changes, and a config or input error no longer hides it.
- **Tabs (RTM-127, epic RTM-66):** every `[[files]]` entry opens at startup (`open_session`), in
  order, and the one `file_name` names is shown, else the first. A file that fails to load (missing,
  unreadable) keeps its tab with the error in its file line; two entries resolving to one file open
  it once. Each `OpenFile` keeps its entry's `path` (canonical once loaded) and `name` (the entry's,
  else the file name), so an unloaded file keeps its tab; only loaded files are placed, checked or
  synced (`loaded_path`). `App::active` is that path (`Option<PathBuf>`, none when no file is open),
  never an index, so removing a tab cannot shift it onto another file; tab actions carry the path
  too (`TabAction`).
  - **The strip** (`App::tab_strip`, at the top of the content area, scrolling sideways when full):
    one tab per file, the shown one drawn like the list's panel, a name in the error colour while
    its file line is set, the path (and error) on hover, and a painted close cross
    (`theme::close_button`, named "Close <name>"). At its end a painted `+`
    (`theme::add_button`, "Open a file") opens New File… and Load File…, the menu's entries.
  - **Opening** (`open_file`): a file already open shows its tab; one whose tab is not loaded (it
    failed at startup, or a conflict or "Don't open" closed it) is loaded into that tab, which is
    the "load the file again" the file lines ask for; any other file gets a new tab and entry.
  - **Closing** (`close`): disabled while the edit dialog is open, while the file's check on open
    runs and while its repository has a job outstanding; pending changes are written and committed
    first (a failed write keeps the tab). The tab to its right is shown next, else the one to its
    left; closing the last tab leaves no file open and the empty state.
  - **Switching** (`switch`, a click on a tab) saves the file as `file_name`. A file in the
    background with a Git-or-Pijul or outside-changes question is brought to the front
    (`show_question`, in `logic`, one at a time in tab order, while no modal is open), without
    writing the config: checks finish a few frames after startup.
  - Settings' change of `sync` reconnects every open file (`connect_all`).
- **Menu and shortcuts (RTM-134):** a painted ☰ button (`theme::menu_button`, accessible name
  "Menu"; Inter has no such glyph) at the left of the title bar opens an `egui::Popup::menu` with
  New File…, Load File…, Settings… and Quit, each showing its shortcut: Ctrl+N, Ctrl+O, Ctrl+, and
  Ctrl+Q (Cmd on macOS, `Modifiers::COMMAND`). The shortcuts are inactive while a modal is open
  (`App::modal_open`). New File and Load File are disabled while a file dialog is open.
  Quit sends `ViewportCommand::Close`, so `on_exit` runs as when the window is closed. On macOS,
  winit's own app menu also has Quit (Cmd+Q) through `terminate:`; RTM-140 checks that it reaches
  `on_exit` too. A menu entry's accessible name ends in its shortcut text, which differs on macOS,
  so the UI tests match only its start (`entry` in `app_ui_tests.rs`).
- **Settings dialog** (`settings.rs`, RTM-134): the app-wide sync settings, `sync` and
  `sync_interval_minutes` (a `DragValue` from 0 to 1440 that keeps a larger value from
  `config.toml` until it is edited). Save writes the config at once. A new interval takes effect at
  once (`tick` reads it); a change of `sync` sets `reconnect`, and `reconnect_when_idle`, called in
  `logic`, runs `connect` once no job is outstanding, so a running job's result is not lost and the
  pending changes are written first. The Git-or-Pijul choice is per file (RTM-124) and is not in
  the dialog: it stays the question asked on open. Escape and the backdrop cancel, as in the edit
  dialog.
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
- **Sync (RTM-64, background since RTM-20)** follows rtmcli's rules (`sync::place`, then `sync::check`): `sync = false`,
  no repository or an ignored file → no sync and no commits; both Git and Pijul → the Git-or-Pijul
  dialog (or "Don't sync", which asks again next time); a conflicted repository (`is_conflicted`) →
  the file is not opened and the file's error line says to resolve it by hand; a failed check (e.g. `git`
  missing) → "Not syncing: …" in the header; no remote → commit only.
  - **The check on open (RTM-70)**, the RTM-57 table in `rtmsync/AGENTS.md`, in two parts since
    RTM-126. `place` runs no executables and stays on the UI thread: sync off or no repository →
    no sync, both VCSs and no choice for the file → the Git-or-Pijul question, otherwise the
    repository's directory and VCS. The file is then assigned to that repository (so its syncs list
    it) and `check` runs as a `Check` job on the repository's worker, so it never meets a pull of
    the same repository half-way and never blocks the window. Until its `Checked` result arrives the
    file is `checking`: the header shows "Checking…", and changes stay in `pending`, unwritten, as a
    write would look like an outside change. A sync queued before the file joined did not list it,
    so the file is read again when the result arrives (`OpenFile::reload`, redoing its pending
    changes). `check` reads `Vcs::state` and returns `Found::Ask(repo, question)` for a modified or
    untracked file; nothing is committed until `App::answer`, and a change made meanwhile is written
    but not committed. A modal (`sync_question`) asks. Modified: "Commit them" commits `rtm:
    commit outside changes` and syncs, "Don't open" closes the file with a "commit or revert by hand"
    error. Untracked: "Add it" commits `rtm: track <name>` (which adds it) and syncs, "Not now" opens
    it without sync, asked again next time. An untracked file in a conflicted repository opens
    without sync and without asking. A missing file keeps the old rule (tracked files sync), as in
    rtmcli: its state cannot be told reliably.
  - **One worker thread per repository (RTM-126)** (`sync::Worker`, held with its sync line and
    timer in `App::repos` as a `RepoSync`) runs every git or pijul call of that repository, for all
    its open files: `Commit { file, message }` (local) and `Sync(files)` (pull, then push, then a
    conflict check of each listed file, since Pijul writes markers into files) jobs, a result per
    job (`Done`), and `ctx.request_repaint()` after each, so results arrive without input. At most
    one `Sync` waits in the queue. A repository is keyed by its directory (`rtmsync::root`) and
    VCS: the Git-or-Pijul choice is per file (RTM-124), so two files in a directory holding both
    may need two workers. A file is assigned to its repository when it opens (a worker starts for
    the first file of a repository, its remote recorded by the first check that finds one) and
    leaves when its tab is closed, its repository has a conflict, or its check or answer means no
    sync; a repository no file uses any more stops. Closing the last tab of a repository with a
    remote queues one more sync; the worker moves to `App::closing` until it is done, and exit
    waits for it as for the open repositories.
  - **When it syncs:** after a file is opened (after the answer, when the check on open asks), every
    `sync_interval_minutes` since that repository's last sync was asked for, on the header's **Sync
    now** button (shown when a repository has a remote, disabled while all of those sync; it syncs
    every repository) and on exit. The timer and the results run in `App::logic`, which eframe also calls
    while the window is minimized or hidden; `request_repaint_after` wakes it when a sync is due.
  - **Editing never waits.** Each mutation is a `Change` (add, toggle, edit, delete) applied to the
    list at once and kept in its file's `pending` until written. `flush` writes a file and queues one
    `Commit` only while its repository's worker is idle (`Worker::busy`: jobs sent whose result was
    not handled yet, so a pull whose reload has not happened still counts); changes made during a
    job wait in memory and make one commit (`rtm: N changes`, one line each). Messages name the
    item's description only (`rtm: complete Pay bill`). Sync now is disabled while every
    repository with a remote syncs, a tab's close button while its file's repository has a job
    outstanding; nothing else. New File and Load File no longer wait for jobs (until RTM-127 they
    replaced the shown file); adding a tab leaves the other files alone.
  - **A pull that changed files** (also one whose push then failed: `NotSynced { changed }`)
    reloads each of them and redoes its `pending` on it, finding each item by equality (with two identical lines,
    the first); a change whose item the pull changed or removed is dropped and named in the file's
    error line. The edit dialog's item (kept in `edited`) is then found again; if it is gone, or what
    deleting it would remove changed, `stale` makes Save and Delete refuse. The header shows the
    shown file's repository: "Syncing…", or "Not synced: …" (git's first line; the rest on hover)
    after a failed pull or push; for a file outside any repository, why it does not sync.
  - **A conflict from a job closes every open file of that repository**, as on open: a reload would
    read the markers as items. Each keeps its place among the open files, and its error line counts
    the pending changes that could not be saved; the repository's worker stops.
  - **On exit** (`on_exit`, eframe's wgpu build, `finish_jobs`): outstanding results are handled as
    in `logic`, pending changes are written and committed, and a last sync of every repository runs,
    within 5 s. When changes are waiting behind a running job, the repositories get up to
    `SAVE_TIMEOUT` (125 s, a pull and a push timeout, counted once for all of them; their workers
    run at the same time), so they are not lost.
    On macOS, `scripts/check-macos-quit.sh` checks that quitting through `terminate:` (Cmd+Q, the
    quit Apple event) reaches `on_exit` and pushes (RTM-140; see "macOS" in
    [`doc/release-process.md`](../doc/release-process.md)).
- Tests are unit tests in `src/tests/`, one file per source file and topic (see Tests in the root
  `AGENTS.md`); rtmapp is a binary crate, so it has no integration tests, and its UI tests are
  unit tests too (see UI tests below). They use temp files and never open a window. `App` tests
  build `App { .. }` (with one `OpenFile`, or several for the sync and tab tests) directly with a temp `config_path` (see `app_with` and `bare_app` in
  `app_support_tests.rs`), never `config::config_path()`: a load or filter save would overwrite the
  user's real `rtm/config.toml`. They do not start sync; the sync tests call `connect` on scratch
  git remotes and clones (local paths, local config keeping the user's identity, signing, hooks and
  rerere out) and `wait` for the worker, which a headless `egui::Context` serves.
- **UI tests (RTM-86)** in `app_ui_tests.rs` drive the real app with `egui_kittest` (dev-dependency,
  `eframe` feature only): `build_eframe` runs `App::logic` and `App::ui` each frame, headless and
  in-process, with no display or GPU, so they run in `rust.yml` on all three OSes and in the Nix
  check phase like any other test. They click and type through the AccessKit tree, then check the
  todo.txt and config on disk.
  - The `harness` helper writes a temp todo.txt and `config.toml` (sync off) and starts the app
    with `App::with_config(cc, path)`, the constructor `App::new` calls with the real path. Never
    `App::new` in a test. Temp dirs use a `ui-` prefix: all unit tests share one process, and
    two tests that pass `temp_dir` the same name share, and wipe, one directory.
  - Widgets are found by role and accessible name. A row checkbox is named after its item's
    description (and is toggled when done), like the row's text label, so queries name the role
    (`get_by_role_and_label(Role::CheckBox, …)`). The edit dialog's fields and the priority
    dropdown are `labelled_by` their labels; kittest then returns the field, not the label, for
    `get_by_label("Description")`. The quick-add input is the only `Role::TextInput` outside the
    dialog.
  - kittest's accessibility check is on: after each `run()`, any visible input without an
    accessible name fails the test. A new input needs a name (a linked label, `accessible_name`, or
    a hint text).
  - The harness allows 60 steps per `run()` (the default 4 is too few for the modal fade and a
    focused text cursor).
  - File dialogs are not clicked (they would open a real dialog): a test hands the app the
    dialog's result with `dialog_returned`, as the file tests do.
  - With the calendar open (a date field has focus), a click elsewhere in the dialog only closes it,
    so the edit test presses Escape before Save.
  - Not covered: screenshot snapshots (kittest's `snapshot`/`wgpu` features would need a GPU or
    software renderer and stored images), and the sync dialogs.

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
while no item is being edited: on Linux the window still takes input behind the portal dialog, so
the edit dialog may open meanwhile, and opening a file shows its tab. Until then the result waits in
the channel. A
cancel, a failure (logged by rfd, as before) or a dialog thread that died clears the dialog and
changes nothing. The menu's New File and Load File are disabled while a dialog is open. Closing the app with a
dialog open leaves the thread to end with the process; it writes nothing.

"New File" (RTM-55) shows a save dialog (`.txt` filter, default name `todo.txt`), creates the file
empty (`File::create`) and opens it through the same path as "Load File" (`open_file`: config, check
on open). An existing file chosen there is **replaced** with an empty one: every backend `rfd` uses
asks before returning it (the portal's backends, gtk3, `zenity --confirm-overwrite`, the Windows and
macOS save panels). A file open in a tab is not replaced: its tab would keep the old items and
write them back, so the app line asks to close its tab first. A new file inside a repository is
untracked, so the check on open asks "Add it"; replacing a tracked file leaves it modified, so it
asks "Commit them" (which commits and syncs the empty file) or "Don't open" (the old items stay
only in the file's history).

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
