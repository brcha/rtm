# egui theme fidelity spike (RTM-33)

Question: can egui reproduce the look of the Tauri `rtmapp` closely enough to justify the port (RTM-32)?

**Decision: go.** The main view, both themes, the focus ring, wrapped item text and the edit dialog
were reproduced with egui 0.37 / eframe 0.37 in about 500 lines. The largest remaining gap is the
date picker popup, which works but looks like stock egui, not flatpickr (see below).

## What was built

`src/main.rs` is a throwaway prototype with hardcoded sample items and no `todotxt` dependency. It
reproduces the header, the 220 px sidebar (button, file box, filters, stats), the add-item row, the
item list (wrapped, multi-coloured item text, completed rows, row borders, hover), and the edit
dialog (modal, backdrop, shadow, form controls, date picker). The light and dark palettes come from
`rtmapp/src/styles.css`. The theme follows the system preference by default.

The crate is deliberately outside the root workspace (own `[workspace]` table) so it does not touch
the root `Cargo.lock` or CI. Its own `Cargo.lock` records the egui version the code was written against.

## How it was compared

- egui: `SPIKE_SHOT=<file>.png` makes the app save a screenshot and exit; `shots.sh` takes the full
  set (light and dark, dialog, focus ring, date picker). Other knobs are listed in `src/main.rs`.
- Reference: the unmodified `rtmapp/src/` served over HTTP in the built-in browser pane, with a mocked
  `window.__TAURI__` returning the same five sample items. This is Chromium, not the WebKitGTK webview
  the real app uses, so small text-rendering differences are expected. The real Tauri app was not run.
- Metrics: the reference sizes were read from the DOM (`getBoundingClientRect`): header 50 px, sidebar
  220 px, text input 37 px, list rows 46 px, dialog 500 px wide. The egui sizes were read off the
  screenshots, so they are approximate, but they agree to within a few pixels.
- Both light and dark were captured for the main view and the dialog. The reference was compared in
  the light dialog and the dark main view only.

## Deviations and gaps

| Area | Status | Notes |
|---|---|---|
| Palette, spacing, radii, borders, shadows | Matches | Both themes. |
| Input focus ring | Matches | 3 px translucent stroke painted outside the border. |
| Wrapped item text with coloured chips | Matches | `LayoutJob` with per-span font, colour and strikethrough. A label inside `ui.horizontal` does not wrap unless `wrap_mode(TextWrapMode::Wrap)` is set, which the first version missed. |
| Font | **Decision needed** | See "Font" below. |
| Font weights | Differs, acceptable | egui has no weights, so each weight is a separate font file. The CSS uses 500 (labels), 600 (title, dialog heading) and 700 (priority). With `font-synthesis: none` the labels render regular, so the spike draws them regular. The title, dialog heading and priority use one bold slot (SemiBold with Inter). |
| Glyph coverage | Differs | `☐` and `☑` render as missing-glyph boxes, so the checkboxes are painted. The date picker button's calendar icon is also a missing glyph (shown as a box) with the fonts used here. |
| Date picker popup | **Largest gap** | See "Date picker" below. |
| Native `<select>` and checkboxes | Differs | Replaced by `ComboBox` and egui's checkbox. They can be restyled but will not match the webview's native widgets exactly. |
| Transitions | Not matched | The CSS 0.2 s hover transition and egui's modal fade are different. Button hover colours work but were not captured. |

### Font

`styles.css` asks for `Inter, Avenir, Helvetica, Arial, sans-serif`, but declares no `@font-face` and
`rtmapp/src/assets/` has no font file. Today the app therefore shows Inter only where it is installed
system-wide, and otherwise falls back to the platform sans (Avenir on macOS, Arial on Windows, the
fontconfig match on Linux). Inter is not installed on the author's machine, so the reference here is
a fallback face.

Consequence: embedding Inter would make `rtmgui` look different from today's app on most machines, not
the same. Inter is available as OpenType files (Regular, SemiBold and Bold were checked, each about 0.6 MB).
The spike ran with Regular and SemiBold: it renders correctly, is slightly wider than Arial (so more items wrap), and the
SemiBold title looks right. The choices are:

1. Embed Inter (consistent look on every platform, about 0.6 MB per weight as OTF, OFL licence).
2. Keep the current "whatever the platform has" behaviour. egui does not read system fonts on its own,
   so this needs extra code or a font-discovery crate. Not tried.
3. Keep egui's built-in font. Not compared.

### Date picker

`egui_extras` 0.37 uses `jiff`, not `chrono`, so dates need converting at the boundary (todotxt uses
`chrono`). The popup's open state is private to `egui_extras`, so the spike opens it by injecting a
click through `raw_input_hook` (`SPIKE_DATEPICKER=1`). The popup opens and is usable. It looks clearly
different from flatpickr:

- weekends are highlighted in pink, there is a week-number column, grey navigation buttons, and the
  popup is anchored under the button so it spills over the dialog's right edge;
- `DatePickerButton` has options (`highlight_weekends`, `calendar_week`, `combo_boxes`, `arrows`,
  `show_icon`) that were not tried and may close part of the gap;
- if they do not, a small custom calendar (about 100 lines) is the fallback.

The dark-theme popup was captured but not inspected.

## Not verified

- Windows and macOS rendering. Only Linux/Wayland was run.
- HiDPI scaling, IME input, CJK font fallback, accessibility.
- Keyboard behaviour (Enter to add, Escape to close the dialog).
- The compositor ignored the requested 1000x700 window size and gave 1261x1386, so screenshots were
  compared at that size.

## Build notes for RTM-37 (native dependencies)

- egui 0.37 changed its API: `eframe::App::ui` replaces `update`, and `Panel` replaces `SidePanel` and
  `TopBottomPanel`. Copy from this spike rather than from older tutorials.
- Inside `nix-shell` (the repo's `shell.nix`) on the author's machine, two environment problems appeared.
  Neither was fixed in the repo.
  - `~/.cargo/config.toml` sets `sccache` as the rustc wrapper, which fails because the nix-shell temp
    directory is missing. Building with `RUSTC_WRAPPER=` works (`shots.sh` does this).
  - The binary linked against glibc 2.44 symbols (`libm`) while its interpreter and rpath pointed to
    glibc 2.42, so it did not start. Patching a copy with `patchelf --set-interpreter` and `--set-rpath`
    to the 2.44 glibc made it run (`shots.sh` does this when `GLIBC` is set). The cause is probably the
    mixed toolchain in the shell and should be checked when the Nix files are updated for egui.
  - Update (RTM-37): the cause was the calling session exporting `NIX_LDFLAGS=-L/usr/lib -L/usr/lib32`
    (and similar), so the linker found the system's glibc 2.44 in `/usr/lib`. Both dev shells now start
    those variables empty and disable `RUSTC_WRAPPER`, so neither workaround is needed any more.
- `shell.nix` already provides the X11, Wayland, GL and xkbcommon libraries egui needs at runtime.

## Reproduce

Inside the repo's `nix-shell`:

```sh
spikes/egui-theme/shots.sh /tmp/shots [regular-font.ttf bold-font.ttf]
# on a machine with the glibc mismatch described above:
GLIBC=/nix/store/<glibc-2.44-...> spikes/egui-theme/shots.sh /tmp/shots
```

One run, for example (this fails with the glibc mismatch above; use `shots.sh` there):

```sh
cd spikes/egui-theme
RUSTC_WRAPPER= SPIKE_THEME=dark SPIKE_DIALOG=1 SPIKE_SHOT=/tmp/out.png cargo run
```
