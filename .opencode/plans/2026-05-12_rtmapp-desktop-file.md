# Plan: rtmapp .desktop File + Nix Install

## Goal

Add a proper XDG `.desktop` file for `rtmapp` so it appears in desktop environment application
launchers (GNOME, KDE, etc.), and update both `flake.nix` and `default.nix` to install it into
the correct XDG location (`$out/share/applications/`).

---

## Context & Constraints

- `rtmapp` is a Tauri v2 desktop app; its Nix packaging bypasses the Tauri bundler and uses plain
  `cargo build -p rtmapp` + a `postInstall` hook.
- Current `postInstall` only copies `rtmapp/src/` into `$out/share/rtmapp/`.
- The app binary lands at `$out/bin/rtmapp`.
- Icons already exist in `rtmapp/src-tauri/icons/` — specifically `32x32.png`, `128x128.png`,
  `128x128@2x.png`, and `icon.png`. These need to be installed into `$out/share/icons/hicolor/`.
- The `tauri.conf.json` product name is `"rtm"`, window title is `"Rusty Todo.txt Manager"`,
  identifier is `"rtm.brcha.com"`.
- Both `flake.nix` and `default.nix` must be updated identically (they share the same `rtmapp`
  derivation logic).
- The `.desktop` file must live in the source tree (committed) so it is available inside the Nix
  sandbox at build time. Canonical location: `rtmapp/rtmapp.desktop`.
- `wrapGAppsHook4` (already present) will automatically pick up `$out/share/applications/` and
  `$out/share/icons/` — no extra wrapper flags needed.
- Branch is `main` → no branch segment in plan filename.

---

## Steps

### 1. Create `rtmapp/rtmapp.desktop`

Create the file at `rtmapp/rtmapp.desktop` with the following content:

```ini
[Desktop Entry]
Version=1.0
Type=Application
Name=Rusty Todo.txt Manager
GenericName=Todo Manager
Comment=Manage your todo.txt files
Exec=rtmapp
Icon=rtmapp
Categories=Office;ProjectManagement;
Keywords=todo;task;productivity;
StartupNotify=true
StartupWMClass=rtmapp
```

**Notes:**
- `Exec=rtmapp` — relies on the binary being on `$PATH` (standard for Nix-installed apps).
- `Icon=rtmapp` — references the icon by theme name; we install it as `rtmapp.png` in the hicolor
  theme (step 2).
- `StartupWMClass=rtmapp` — matches the binary name; adjust if the WM class differs at runtime.
- No `Terminal=false` line needed (default is false), but add it explicitly for clarity if desired.

**Acceptance criteria:** File exists, passes `desktop-file-validate rtmapp/rtmapp.desktop`.

---

### 2. Update `postInstall` in both `flake.nix` and `default.nix`

Replace the current `postInstall` block in the `rtmapp` derivation in **both files**:

**Current:**
```nix
postInstall = ''
  mkdir -p $out/share/rtmapp
  cp -r $src/rtmapp/src $out/share/rtmapp/
'';
```

**New:**
```nix
postInstall = ''
  # Frontend assets (served by the embedded WebKit view)
  mkdir -p $out/share/rtmapp
  cp -r $src/rtmapp/src $out/share/rtmapp/

  # XDG desktop entry
  mkdir -p $out/share/applications
  cp $src/rtmapp/rtmapp.desktop $out/share/applications/rtmapp.desktop

  # Icons — hicolor theme, standard sizes
  mkdir -p $out/share/icons/hicolor/32x32/apps
  mkdir -p $out/share/icons/hicolor/128x128/apps
  mkdir -p $out/share/icons/hicolor/256x256/apps
  cp $src/rtmapp/src-tauri/icons/32x32.png \
     $out/share/icons/hicolor/32x32/apps/rtmapp.png
  cp $src/rtmapp/src-tauri/icons/128x128.png \
     $out/share/icons/hicolor/128x128/apps/rtmapp.png
  cp $src/rtmapp/src-tauri/icons/128x128@2x.png \
     $out/share/icons/hicolor/256x256/apps/rtmapp.png
'';
```

**Notes:**
- `128x128@2x.png` is a 256×256 image (2× HiDPI scale of 128×128) — install it into `256x256/`.
- `icon.png` (the full-resolution source, likely 512×512 or larger) could also be installed into
  `$out/share/icons/hicolor/512x512/apps/rtmapp.png` if desired. Check actual dimensions first;
  add as an optional sub-step.
- Both `flake.nix` and `default.nix` must receive identical changes to stay in sync.

**Acceptance criteria:**
- `nix build .#rtmapp` (flake) and `nix-build -A rtmapp` (legacy) succeed.
- `$out/share/applications/rtmapp.desktop` exists in the build result.
- `$out/share/icons/hicolor/*/apps/rtmapp.png` exist at the correct sizes.

---

### 3. (Optional) Verify icon dimensions and add 512×512

Before committing, check the actual pixel dimensions of `icon.png`:

```sh
file rtmapp/src-tauri/icons/icon.png
# or
identify rtmapp/src-tauri/icons/icon.png   # ImageMagick
```

If it is 512×512 (common Tauri default), add to `postInstall`:

```nix
mkdir -p $out/share/icons/hicolor/512x512/apps
cp $src/rtmapp/src-tauri/icons/icon.png \
   $out/share/icons/hicolor/512x512/apps/rtmapp.png
```

**Acceptance criteria:** Icon size matches the hicolor directory name.

---

### 4. (Optional) Add `copyDesktopItems` / `makeDesktopItem` alternative

If the team prefers a fully Nix-native approach instead of a committed `.desktop` file, the
derivation can use `pkgs.makeDesktopItem` and `copyDesktopItems`. This avoids a committed file but
couples the metadata to the Nix expression. **Recommendation: keep the committed file** — it is
simpler, version-controlled alongside the app, and works with both flake and legacy builds.

---

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `StartupWMClass` mismatch — app may not be associated with the launcher icon | Check with `xprop WM_CLASS` at runtime; update the `.desktop` file if needed |
| `icon.png` is not 512×512 — wrong hicolor directory | Verify with `file` / `identify` before adding the 512×512 entry (step 3) |
| `desktop-file-validate` not available in Nix sandbox | Validate locally before committing; add `desktop-file-utils` to `nativeBuildInputs` if CI validation is desired |
| `flake.nix` and `default.nix` drift | Both files are edited in the same commit; note in commit message |

---

## Open Questions

1. **`icon.png` dimensions** — Is it 512×512? Determines whether step 3 applies.
2. **`StartupWMClass`** — What does the running app report? (`xprop WM_CLASS` on the window.)
   If it differs from `rtmapp`, the `.desktop` file needs adjustment.
3. **`Terminal=false`** — Include explicitly for clarity, or rely on the default?
4. **`Categories`** — `Office;ProjectManagement;` is reasonable; adjust to taste.
5. **`rtmgui` desktop file** — Should a similar `.desktop` file be created for `rtmgui` in the
   same session? (Out of scope for this plan but a natural follow-up.)
