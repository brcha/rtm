# The development shell, used by `nix-shell` (and direnv's `use_nix`) and by `nix develop`, which
# imports it from flake.nix with the flake's pinned nixpkgs.
#
# If you use nix-direnv ≥ 2.30, change .envrc from `use_nix` to `use flake` to get a
# faster, cached dev shell from the flake instead.

{ pkgs ? import <nixpkgs> {} }:

let
  # The libraries rtmapp loads with dlopen, from its package in default.nix.
  inherit (import ./default.nix { inherit pkgs; }) rtmapp;
in
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer
    pkg-config
    # CHANGELOG.md on release branches (scripts/changelog.sh)
    git-cliff
  ];
  # Start from empty values instead of whatever the calling session exports: a leaked
  # `-L/usr/lib` links against the system glibc instead of the shell's, and the binary
  # then fails to start. The stdenv setup hooks append their own flags to these.
  NIX_LDFLAGS = "";
  NIX_CFLAGS_LINK = "";
  NIX_CFLAGS_COMPILE = "";
  PKG_CONFIG_PATH = "";
  # A rustc wrapper such as sccache from ~/.cargo/config.toml fails inside the shell.
  RUSTC_WRAPPER = "";
  # Linux only: the libraries do not exist on macOS, where the system frameworks are used.
  shellHook = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
    export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${pkgs.lib.makeLibraryPath rtmapp.eguiDeps}
  '';
}
