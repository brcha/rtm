# Plan: Fix GitHub Actions — Install System Dependencies for Tauri/GTK Build

**Date:** 2026-05-13  
**Branch:** main  
**Topic:** fix-github-workflow-deps

---

## Goal

Make `cargo build --verbose` and `cargo test --verbose` pass on `ubuntu-latest` in GitHub Actions
by installing all system libraries required by the workspace (GTK3, GLib, WebKitGTK 4.1, libsoup 3,
X11/Wayland, OpenGL, etc.).

Also document in `AGENTS.md` that any new system dependency must be added to three places in sync:
`.github/workflows/rust.yml`, `flake.nix`, and `default.nix`.

---

## Context & Constraints

- The Cargo workspace includes `rtmapp/src-tauri`, which links against GTK3, WebKitGTK 4.1,
  libsoup 3, GLib, and related X11/Wayland/OpenGL libraries.
- A bare `cargo build` on a stock Ubuntu runner has none of these installed → build fails with
  `pkg-config` errors (e.g., `glib-2.0 not found`).
- The canonical dep list lives in `flake.nix` (`tauriDeps` + `guiDeps`) and `shell.nix`.
- `ubuntu-latest` on GitHub Actions is currently Ubuntu 24.04; all required packages are available
  in the default `apt` repositories.
- No changes to Rust source code are needed.

### Nix → Ubuntu apt package mapping

| Nix package              | Ubuntu apt package              |
|--------------------------|---------------------------------|
| `pkg-config`             | `pkg-config`                    |
| `glib`                   | `libglib2.0-dev`                |
| `gtk3`                   | `libgtk-3-dev`                  |
| `atk`                    | `libatk1.0-dev`                 |
| `gdk-pixbuf`             | `libgdk-pixbuf2.0-dev`          |
| `pango`                  | `libpango1.0-dev`               |
| `cairo`                  | `libcairo2-dev`                 |
| `webkitgtk_4_1`          | `libwebkit2gtk-4.1-dev`         |
| `libsoup_3`              | `libsoup-3.0-dev`               |
| `gobject-introspection`  | `libgirepository1.0-dev`        |
| `xorg.libxcb`            | `libxcb1-dev`                   |
| `xorg.libX11`            | `libx11-dev`                    |
| `xorg.libXcursor`        | `libxcursor-dev`                |
| `xorg.libXrandr`         | `libxrandr-dev`                 |
| `xorg.libXi`             | `libxi-dev`                     |
| `libGL`                  | `libgl1-mesa-dev`               |
| `libxkbcommon`           | `libxkbcommon-dev`              |
| `wayland`                | `libwayland-dev`                |

---

## Steps

### 1. Update `.github/workflows/rust.yml`

Add an `apt-get` install step **before** the Build step. Install every package from the mapping
table above. Use `sudo apt-get update && sudo apt-get install -y ...` in a single shell run block.

**Acceptance criteria:** The workflow file has a named step `Install system dependencies` that
installs all 18 packages listed above, placed between `actions/checkout@v4` and the Build step.

**Target file:** `.github/workflows/rust.yml`

**Resulting workflow structure:**
```yaml
steps:
  - uses: actions/checkout@v4

  - name: Install system dependencies
    run: |
      sudo apt-get update
      sudo apt-get install -y \
        pkg-config \
        libglib2.0-dev \
        libgtk-3-dev \
        libatk1.0-dev \
        libgdk-pixbuf2.0-dev \
        libpango1.0-dev \
        libcairo2-dev \
        libwebkit2gtk-4.1-dev \
        libsoup-3.0-dev \
        libgirepository1.0-dev \
        libxcb1-dev \
        libx11-dev \
        libxcursor-dev \
        libxrandr-dev \
        libxi-dev \
        libgl1-mesa-dev \
        libxkbcommon-dev \
        libwayland-dev

  - name: Build
    run: cargo build --verbose

  - name: Run tests
    run: cargo test --verbose
```

---

### 2. Update `AGENTS.md` (root)

Add a note under **Important Notes** (or a new **Dependency Management** section) stating:

> When adding a new system/native dependency to any crate in the workspace, it must be registered
> in **all three** of the following locations to keep CI, Nix flake builds, and legacy nix-build
> in sync:
> 1. `.github/workflows/rust.yml` — `apt-get install` step
> 2. `flake.nix` — `guiDeps` or `tauriDeps` list (and `devShells.default` if needed)
> 3. `default.nix` — matching `guiDeps` or `tauriDeps` list

**Acceptance criteria:** The root `AGENTS.md` contains a clear, findable note about the three-way
sync requirement for native dependencies.

---

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `libwebkit2gtk-4.1-dev` not available on older Ubuntu runners | `ubuntu-latest` is 24.04 as of 2026; package is present. If runner changes, pin `ubuntu-24.04` explicitly. |
| Package name drift between Ubuntu versions | Names are stable across 22.04/24.04 for all listed packages. |
| `apt-get update` network flakiness | Standard GitHub-hosted runner risk; no mitigation needed beyond retry logic if it becomes chronic. |
| Future deps added to Nix but not to workflow | Mitigated by the AGENTS.md documentation added in Step 2. |

---

## Open Questions

- None. All package names are confirmed against Ubuntu 22.04/24.04 repositories.
