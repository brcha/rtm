# Nix Packaging

- **`default.nix`** — the packages, defined once: `nix-build -A rtmcli`, `nix-build -A rtmapp`.
- **`flake.nix`** — the flake entry point: `nix build .#rtmcli`, `nix build .#rtmapp`. It imports
  the packages from `default.nix` and the dev shell from `shell.nix`, both with its pinned nixpkgs,
  and defines only the apps itself.
- **`shell.nix`** — the dev shell, defined once (see Dev shell); it takes `eguiDeps` from
  `default.nix` (`rtmapp.eguiDeps`, a `passthru`).
- **`Cargo.lock` must stay committed** — `importCargoLock` in `buildRustPackage` requires it.
- **`rtmapp` package (Linux and macOS):** no native build inputs; on Linux `eguiDeps` are added to
  the binary's rpath with `patchelf --add-rpath` in `postFixup` (after the rpath shrinking, which
  would drop them); on macOS the system frameworks come from stdenv's Apple SDK, and the result is
  a plain binary under `bin/`, not an app bundle (the dmg comes from `scripts/build-macos.sh`).
  `postInstall` installs `rtmapp/assets/rtmapp.desktop`, the hicolor icons (PNGs from 16 to 512 px and the scalable SVG, see `rtmapp/AGENTS.md`) and
  `Inter-LICENSE.txt` (under `share/licenses/rtmapp`).
- **Metadata from `Cargo.toml` (RTM-75):** nothing about the packages is hard-coded in the Nix
  files. `fromCargo` in `default.nix` takes `pname`, `version`, `meta.description`,
  `meta.license` (an SPDX ID; an unknown one fails evaluation) and `meta.homepage` (from
  `repository`) from each crate's `[package]`; only what Cargo has no field for (platforms,
  `mainProgram`, build flags) is written in Nix. The Cargo `description` is also the deb summary.
- **Tests in the build (RTM-71):** the check phase runs the sync tests, so `gitMinimal`, `pijul` and
  `openssh` (the Pijul sync test signs through a scratch `ssh-agent`) are `nativeCheckInputs`;
  `rtmcli` also runs `rtmsync`'s tests, with `RTM_REQUIRE_PIJUL` set so the Pijul tests cannot skip
  themselves. At
  run time both packages use `git`/`pijul` from the user's `PATH`; they are not wrapped.
- **CI:** `.github/workflows/nix.yml` builds `.#rtmcli` and `.#rtmapp`, tests included, on Linux and
  macOS for pushes to `main` that change more than Markdown and pull requests against `main` or
  `releases/**` (RTM-78) that change one of its inputs (RTM-76, RTM-82; see "Which changes start
  which build" in [CI](ci.md)).
  `nix-build` is not run in CI (it takes the caller's `<nixpkgs>`), but the flake builds the same
  definitions from `default.nix` with its pin.
- **nixpkgs pin:** the flake follows the stable `nixos-26.05` (rustc 1.95, the minimum egui 0.37
  needs); consumers can override it with `inputs.rtm.inputs.nixpkgs.follows`. Move to
  `nixos-26.11` when it is released (RTM-45). Called directly (`nix-build`), `default.nix` uses
  the caller's `<nixpkgs>`, which must also have rustc ≥ 1.95. `shell.nix` called without `pkgs`
  uses the revision locked in `flake.lock` instead (RTM-88).
- **Dev shell:** `nix develop`, `nix-shell` and direnv's `use_nix` all use `shell.nix` with the
  nixpkgs pinned in `flake.lock`: the flake passes its `pkgs`, and called without one `shell.nix`
  reads the lock file itself (finding the node through `nodes.root.inputs.nixpkgs`), so every entry
  point gets the same nixpkgs as `nix develop` and the Nix builds in CI (`rust.yml` uses the
  runner's rustup toolchain instead). `.envrc` uses `use_nix`; change to `use flake` if
  using nix-direnv ≥ 2.30 for a cached shell. It has `rustfmt`, `clippy`, `rust-analyzer`,
  `git-cliff`, and `zola` and `lychee` (the project website, `site/`, and its link check), sets
  `LD_LIBRARY_PATH` to `eguiDeps` on Linux only (the libraries do not exist on macOS), and sets `NIX_LDFLAGS`, `NIX_CFLAGS_LINK`, `NIX_CFLAGS_COMPILE` and `PKG_CONFIG_PATH` empty and
  `RUSTC_WRAPPER` to empty: a calling session that exports `-L/usr/lib` otherwise links against the
  system glibc instead of the shell's (the binary then fails with `GLIBC_2.xx not found`), and a
  user-configured sccache fails inside the shell.
