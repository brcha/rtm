# The desktop app (rtmapp)

Reference for `rtmapp`'s look, fonts, icon, run-time libraries and file dialog. Its conventions are
in [`rtmapp/AGENTS.md`](../rtmapp/AGENTS.md).

## Look

`theme.rs` and the widgets reproduce the palettes, spacing, radii, borders and shadows of the
0.1.x app's `rtmapp/src/styles.css` at commit `5076649` (the last commit holding that app's
sources), in both themes. The theme follows the system preference; `RTMAPP_THEME=light` or `dark`
forces one, for screenshots of both themes; any other value is ignored.

- **Reference sizes,** read from the DOM of the app at `5076649`: header 50 px, sidebar 220 px,
  text input 37 px, list rows 46 px, dialog 500 px wide. The sidebar and dialog widths are set
  directly; the others follow from the CSS paddings and font sizes the code reproduces.
- **How it was compared:** the unmodified frontend at `5076649` in Chromium with its backend
  mocked, against screenshots of an egui prototype with the same sample items. Chromium is not the
  webview that frontend ran in, so small text-rendering differences were expected.
- **Focus ring:** a 3 px translucent stroke painted outside the input's border.
- **Dialogs** share `theme::dialog_frame` (the edit dialog's CSS frame). The questions (Git or
  Pijul, outside changes, untracked, closing a tab) go through `theme::question`: a bold title, a
  muted explanation, and the buttons on the right with the main answer rightmost.
- **Item text:** one `LayoutJob` per row (`list::row_job`), with per-span font, colour and
  strikethrough.
- **The calendar** popup is drawn at the sizes of the 0.1.x app's flatpickr popup
  (`rtmapp/src/assets/flatpickr.min.css` at `5076649`). Deliberate differences: a dark theme, a
  Clear button, the week starting on the locale's first day, month arrows instead of the month
  dropdown, the mouse wheel stepping the year, English names, no keyboard navigation of the grid.
- **Known differences from `styles.css`:**
  - egui has no font weights: Inter Regular is the text face and SemiBold is the `bold` family
    (title, headings, priorities). The CSS weight 500 of the labels rendered regular under
    `font-synthesis: none`, so labels are regular.
  - The ☐ and ☑ glyphs are missing from most fonts, so the list's checkboxes are painted, and so is
    the edit dialog's trash button (`theme::trash_button`, with an accessible name). The sidebar
    filters use egui's checkbox with a grey outline, and the dialog's priority a `ComboBox` styled
    like the text inputs, in place of the native checkbox and `<select>`.
  - The CSS 0.2 s hover transition and egui's modal fade are not matched.

## Fonts and licence

Inter 4.0 (`Inter-Regular.otf`, `Inter-SemiBold.otf`) is embedded with `include_bytes!`, from the
official `Inter-4.0.zip` release of github.com/rsms/inter, with `assets/fonts/Inter-LICENSE.txt`
(SIL Open Font License 1.1) next to it. The licence requires its text and the copyright notice to
accompany every redistributed copy, so installers and packages must ship `Inter-LICENSE.txt`. The
Nix package installs it under `share/licenses/rtmapp`, the deb and the AppImage under
`usr/share/doc/rtmapp/`, the MSI next to `rtmapp.exe`, and the macOS bundle in
`Contents/Resources/`.

## App icon

A page whose first line is ticked, on a Rust-orange rounded square (`#ce422b`; the tick box is the
done green `#28a745`). `assets/icons/rtmapp.svg` is the master; `rtmapp-small.svg` is a simplified
variant (no text lines, a larger tick) for 16 and 24 px, where the master's lines blur. Edit the
SVGs by hand and keep them plain (no Inkscape metadata); every other file in `assets/icons/` is
generated from them and keeps its name, because the Nix packages and the installers refer to those
names.

Regenerate after changing an SVG (from `rtmapp/assets/icons/`; `inkscape` is not in the dev shell):

```sh
t=$(mktemp -d)
for s in 16 24; do inkscape rtmapp-small.svg --export-type=png --export-width=$s --export-filename=$t/$s.png; done
for s in 32 48 64 128 256 512 1024; do inkscape rtmapp.svg --export-type=png --export-width=$s --export-filename=$t/$s.png; done
cp $t/16.png 16x16.png; cp $t/24.png 24x24.png; cp $t/32.png 32x32.png
cp $t/128.png 128x128.png; cp $t/256.png 128x128@2x.png; cp $t/512.png icon.png
nix shell nixpkgs#icoutils -c icotool -c -o icon.ico $t/16.png $t/24.png $t/32.png $t/48.png $t/64.png $t/128.png -r $t/256.png
nix shell nixpkgs#libicns -c png2icns icon.icns $t/16.png $t/32.png $t/128.png $t/256.png $t/512.png $t/1024.png
```

`icotool -r` stores the 256 px entry as PNG (ImageMagick writes it as an uncompressed bitmap, over
three times the size). The Nix packages install the PNGs as hicolor `rtmapp.png` from 16 to 512 px
and the master as `scalable/apps/rtmapp.svg`; the 16 and 24 px PNGs keep panels from scaling the
master down.

The window icon (title bar and taskbar on Windows and X11) is `128x128@2x.png`, embedded in
`main.rs`; a test checks that it decodes. Wayland compositors take the icon from the desktop entry
instead, and macOS (Finder, Dock) from the bundle, which carries `icon.icns` as
`Contents/Resources/rtmapp.icns`. Explorer and the taskbar take `rtmapp.exe`'s icon from its
resources: `build.rs` embeds `icon.ico` there (an installer can only set its shortcuts' icon). The
deb and the AppImage install the same icons and desktop entry as the Nix package; the MSI uses
`icon.ico` for its shortcuts and the Add/Remove Programs entry.

## Run-time libraries (Linux)

The binary links no native libraries beyond glibc; it loads Wayland, xkbcommon, X11, GL, Vulkan,
fontconfig and libdbus with `dlopen`. The dev shell puts them on `LD_LIBRARY_PATH`, and the Nix
package on the binary's rpath (`eguiDeps` in `default.nix`, see
[Nix packaging](nix-packaging.md)).

The deb's `depends` and `suggests` and the RPM's `requires` and `suggests` (by soname) in
`rtmapp/Cargo.toml` follow this list. The Wayland entries were found by starting the app on Wayland
with reduced `LD_LIBRARY_PATH` sets and checking that it draws; the X11 list is what a full run
under XWayland loaded, not reduced further.

- **Wayland (required):** `libwayland-client.so.0` (without it winit fails with `NoWaylandLib`) and
  `libxkbcommon.so.0` (winit panics with `XKBNotFound`).
- **GPU (one of):** `libvulkan.so.1`, or `libEGL.so.1` plus `libwayland-egl.so.1` (without the
  latter the GL path panics). With neither, wgpu fails to create a surface. The driver itself comes
  with the system (Mesa).
- **Fonts (required by the deb):** `libfontconfig.so.1`. The UI starts and draws with the bundled
  Inter without it, but has no fallback for characters Inter lacks (CJK, emoji, many symbols).
- **Optional (Suggests):** the X11 set, used only without `WAYLAND_DISPLAY`: `libX11.so.6`,
  `libX11-xcb.so.1`, `libXcursor.so.1`, `libXi.so.6`, `libxkbcommon-x11.so.0`. GNOME is
  Wayland-only from Ubuntu 25.10 and Debian forky; X11 matters mainly for older or non-GNOME X
  sessions.
- **File dialog:** `libdbus-1.so.3` (required by the deb and the RPM; without it rfd falls back to
  `zenity`), and `xdg-desktop-portal` with a backend that implements
  `org.freedesktop.impl.portal.FileChooser`: gtk, gnome or kde on jammy. `xdg-desktop-portal-wlr`
  (and Hyprland's portal) has none, so wlroots desktops need gtk beside it. The dialog was not
  opened in those reduced runs.

## File dialog

`rfd` uses the `xdg-portal` backend on Linux, so a running `xdg-desktop-portal` with a FileChooser
backend (for example `-gtk` or `-kde`) is needed at run time. rfd reaches the portal through
`libdbus-1.so.3`, which it loads with `dlopen`. If libdbus or the portal fails, `rfd` tries
`zenity` and then returns `None`, which is also what a cancelled dialog returns, so the UI cannot
tell the two apart. The fallback is the `gtk3` feature
(`rfd = { version = "0.18", default-features = false, features = ["gtk3"] }`), which needs GTK 3 at
run time.

`rfd` logs both failures at `error` level, and `env_logger` prints them to stderr: run the app from
a terminal to see why "Load File" did nothing. A cancel logs nothing. `RUST_LOG=warn` (or `debug`)
shows more. Windows release builds have no console, so there the log is not visible; use a debug
build.
