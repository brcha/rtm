## Goal

Add the missing WebKit/JavaScriptCore dependency to `shell.nix` so that `cargo build` succeeds for the Tauri-based `rtmapp` crate.

## Context & Constraints

- The workspace has three crates: `todotxt`, `rtmcli`, `rtmgui` (egui/eframe), and `rtmapp/src-tauri` (Tauri 2).
- Tauri 2 on Linux uses a WebKit2GTK webview, which requires `webkitgtk_4_1` from nixpkgs.
- The current `shell.nix` provides GTK3, libsoup, and X11/Wayland libraries but omits WebKit entirely.
- `webkitgtk_4_1` also pulls in `javascriptcore4_1` transitively, which is the missing symbol at build time.
- The fix must also expose the library path in `LD_LIBRARY_PATH` via `shellHook` so the linker can find it at runtime.

## Steps

1. **Add `webkitgtk_4_1` to `nativeBuildInputs`** in `shell.nix`

   In the `nativeBuildInputs` block, after the existing GTK/glib entries, add:
   ```nix
   # WebKit/JavaScriptCore for Tauri webview
   webkitgtk_4_1
   ```

2. **Extend `LD_LIBRARY_PATH` in `shellHook`** to include `webkitgtk_4_1`

   The current `makeLibraryPath` list in `shellHook` must include `webkitgtk_4_1`:
   ```nix
   shellHook = with pkgs.xorg; ''
     export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${makeLibraryPath (with pkgs.xorg; with pkgs; [
       libX11 libXcursor libXrandr libXi libGL libxkbcommon
       libsoup_3 atk gdk-pixbuf pango cairo gtk3 glib wayland
       webkitgtk_4_1
     ])}
   '';
   ```

3. **Re-enter the nix shell and verify**

   ```bash
   exit          # leave current shell
   nix-shell     # re-enter to pick up new packages
   cargo build   # should now find javascriptcore and succeed
   ```

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `webkitgtk_4_1` not available in the pinned nixpkgs channel | Check with `nix-env -qaP webkitgtk` — fall back to `webkitgtk` (4.0) and adjust Tauri feature flags if needed |
| Tauri 2 may require `libayatana-appindicator` or `xdotool` on some distros | Add those if secondary build errors appear after WebKit is resolved |
| 32-bit `buildInputs` block is currently empty but may need WebKit too if cross-compiling | Out of scope; address only if explicitly needed |

## Open Questions

- Is the nixpkgs channel pinned or floating (`<nixpkgs>`)? If pinned to an older snapshot, `webkitgtk_4_1` may not exist and the channel may need updating.
- Is the intent to build `rtmapp` (Tauri) as well as `rtmgui` (egui), or only one of them? If only `rtmgui` is needed, the workspace build can be scoped with `cargo build -p rtmgui` to avoid the Tauri dependency entirely.
