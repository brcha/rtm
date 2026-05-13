# Plan: Remove rtmgui (egui frontend)

**Date:** 2026-05-13  
**Branch:** main  
**Topic:** remove-rtmgui

---

## Goal

Completely remove the `rtmgui` egui/eframe desktop GUI crate from the workspace, leaving no
dangling references in build files, Nix expressions, documentation, or AGENTS.md files.

---

## Context & Constraints

- `rtmgui/` is a self-contained Cargo crate with a single source file (`src/main.rs`).
- It is a workspace member in the root `Cargo.toml` and has its own `Cargo.lock` contribution.
- It is referenced in: `Cargo.toml`, `flake.nix`, `default.nix`, `AGENTS.md`, `README.md`,
  and several historical plan files under `.opencode/plans/`.
- `screenshot_gui.png` at the repo root depicts the egui GUI and is referenced only in `README.md`.
- The `guiDeps` block in `flake.nix` and `default.nix` was originally labelled "Shared GUI deps
  (rtmgui)". After removal, `rtmapp` still needs those same GTK3/X11/Wayland libraries, so the
  deps block must be kept — only the label and the `rtmgui` derivation are removed.
- The `wrapGAppsHook` (GTK3 variant) in `flake.nix` `devShells.default` was there for `rtmgui`.
  After removal it is no longer needed in the dev shell (only `wrapGAppsHook4` is needed for
  `rtmapp`). Remove it from the dev shell's `nativeBuildInputs`.
- Historical plan files (`.opencode/plans/`) are **read-only artefacts** — do not edit them.
- `Cargo.lock` will be regenerated automatically by Cargo after the workspace member is removed;
  it must remain committed.
- No other crate depends on `rtmgui` (it is a binary crate, not a library).

---

## Steps

### 1. Delete the `rtmgui/` directory tree

Remove the entire directory:

```
rtmgui/
├── AGENTS.md
├── Cargo.toml
└── src/
    └── main.rs
```

Also delete `rtmgui/target/` if present (it is gitignored but may exist locally).

**Acceptance:** `rtmgui/` no longer exists on disk.

---

### 2. Remove `rtmgui` from the Cargo workspace

**File:** `Cargo.toml` (root)

Remove the `"rtmgui"` entry from the `members` array:

```toml
# Before
members = [
    "todotxt",
    "rtmcli",
    "rtmgui",          ← remove this line
    "rtmapp/src-tauri",
]

# After
members = [
    "todotxt",
    "rtmcli",
    "rtmapp/src-tauri",
]
```

**Acceptance:** `cargo metadata --no-deps` lists only `todotxt`, `rtmcli`, `rtmapp`.

---

### 3. Update `flake.nix`

Three changes:

**3a. Remove the `rtmgui` derivation** (lines 68–84):

Delete the entire `# ── rtmgui ───` block including the closing `});`.

**3b. Update the `guiDeps` comment** (line 24):

```nix
# Before
# Shared GUI deps (rtmgui — GTK3 + X11/Wayland/OpenGL).

# After
# Shared GUI deps for rtmapp (GTK3 + X11/Wayland/OpenGL).
```

**3c. Remove `rtmgui` from `packages` and `apps`** (lines 134, 141):

```nix
# Before
packages = {
  inherit rtmcli rtmgui rtmapp;
  default = rtmcli;
};

apps = {
  rtmcli = flake-utils.lib.mkApp { drv = rtmcli; };
  rtmgui = flake-utils.lib.mkApp { drv = rtmgui; };   ← remove
  rtmapp = flake-utils.lib.mkApp { drv = rtmapp; };
  default = flake-utils.lib.mkApp { drv = rtmcli; };
};

# After
packages = {
  inherit rtmcli rtmapp;
  default = rtmcli;
};

apps = {
  rtmcli = flake-utils.lib.mkApp { drv = rtmcli; };
  rtmapp = flake-utils.lib.mkApp { drv = rtmapp; };
  default = flake-utils.lib.mkApp { drv = rtmcli; };
};
```

**3d. Remove `wrapGAppsHook` from `devShells.default` `nativeBuildInputs`:**

`wrapGAppsHook` (GTK3 variant) was only needed to build `rtmgui`. The dev shell already has
`wrapGAppsHook4` (for `rtmapp`). Remove the GTK3 variant:

```nix
# Before
nativeBuildInputs = with pkgs; [
  rustc cargo rustfmt clippy rust-analyzer
  pkg-config
  gobject-introspection
  wrapGAppsHook    ← remove
  nodejs
];

# After
nativeBuildInputs = with pkgs; [
  rustc cargo rustfmt clippy rust-analyzer
  pkg-config
  gobject-introspection
  nodejs
];
```

**Acceptance:** `nix flake check` passes (or at minimum `nix eval .#packages.x86_64-linux`
lists only `rtmcli`, `rtmapp`, `default`).

---

### 4. Update `default.nix`

Two changes:

**4a. Remove the `rtmgui` derivation** (lines 60–79):

Delete the entire `# ── rtmgui ───` block.

**4b. Update the `guiDeps` comment** (line 19):

```nix
# Before
# Shared GUI deps (rtmgui — GTK3 + X11/Wayland/OpenGL).

# After
# Shared GUI deps for rtmapp (GTK3 + X11/Wayland/OpenGL).
```

**4c. Update the header comment** (line 4):

```nix
# Before
#   nix-build -A rtmgui

# After
(remove that line entirely)
```

**Acceptance:** `nix-build -A rtmcli` and `nix-build -A rtmapp` still work; `nix-build -A rtmgui`
errors with "attribute 'rtmgui' missing" (expected).

---

### 5. Update root `AGENTS.md`

Remove all `rtmgui`-specific content:

- **Subpart list:** Remove `- [\`rtmgui/AGENTS.md\`](rtmgui/AGENTS.md) — egui desktop GUI`
- **Architecture diagram:** Remove `├── rtmgui/  ← Native desktop GUI (eframe/egui + rfd)` line
- **Conventions:** Update the Rust edition note — remove `rtmgui` from the list:
  `2024 (todotxt, rtmcli)` instead of `2024 (todotxt, rtmcli, rtmgui)`
- **Important Notes:** Remove the entire `rtmgui Wayland` bullet and the `rtmgui uses rfd` bullet
- **Nix Packaging:** Remove `nix build .#rtmgui` from the `flake.nix` line and
  `nix-build -A rtmgui` from the `default.nix` line
- **What This Project Is:** Update "three frontends" → "two frontends"

**Acceptance:** No `rtmgui` references remain in `AGENTS.md`.

---

### 6. Update `README.md`

- Remove the screenshot section referencing `screenshot_gui.png`:
  ```markdown
  Here's a screenshot of the GUI:

  ![screenshot_gui](screenshot_gui.png "RTM GUI")

  and of the CLI:
  ```
  Replace with simply:
  ```markdown
  Here's a screenshot of the CLI:
  ```
- Delete `screenshot_gui.png` from the repo root.

**Acceptance:** `README.md` contains no reference to `screenshot_gui.png`; the file is gone.

---

### 7. Regenerate `Cargo.lock`

After removing the workspace member, run:

```sh
cargo generate-lockfile
```

or simply:

```sh
cargo check
```

This removes the `rtmgui` package entry and all its exclusive dependencies (eframe, egui, rfd,
etc.) from `Cargo.lock`. Commit the updated lockfile.

**Acceptance:** `Cargo.lock` no longer contains `name = "rtmgui"`. `cargo build` succeeds for
the remaining workspace members.

---

### 8. Verify the workspace builds cleanly

```sh
cargo build
cargo test
```

Both must succeed with zero errors and zero warnings related to `rtmgui`.

**Acceptance:** Clean build and test run.

---

## Risks & Mitigations

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| `guiDeps` block accidentally removed, breaking `rtmapp` build | Low | Steps 3 & 4 explicitly retain the `guiDeps` block; only the label comment and `rtmgui` derivation are removed |
| `wrapGAppsHook` removal breaks `rtmapp` build | Low | `rtmapp` uses `wrapGAppsHook4`, not `wrapGAppsHook`; they are distinct hooks |
| Cargo.lock left stale with `rtmgui` entries | Low | Step 7 explicitly regenerates it |
| `screenshot_gui.png` still tracked by git after deletion | Low | Use `git rm screenshot_gui.png` to stage the deletion |

---

## Open Questions

1. **`screenshot_gui.png`** — Is this the egui screenshot or the Tauri app screenshot? If it
   depicts `rtmapp`, it should be kept and the `README.md` reference updated rather than removed.
   *(Assumption: it depicts the egui GUI and should be deleted. Verify before executing Step 6.)*

2. **Historical plan files** — Several `.opencode/plans/` files reference `rtmgui` extensively.
   These are read-only historical artefacts and are intentionally left untouched.
