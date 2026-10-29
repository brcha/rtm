# NOTE: The canonical development environment is now defined in flake.nix.
#
#   Preferred:  nix develop          (uses flake.nix devShells.default)
#   Legacy:     nix-shell            (uses this file — kept for `use_nix` / direnv compatibility)
#
# If you use nix-direnv ≥ 2.30, change .envrc from `use_nix` to `use flake` to get a
# faster, cached dev shell from the flake instead.
#
# This file is intentionally kept in sync with the buildInputs in flake.nix.

let pkgs = import <nixpkgs> {};
in
  with pkgs.stdenv;
  with pkgs.lib;
pkgs.mkShell {
  # nativeBuildInputs is usually what you need -- tools you need to run
  nativeBuildInputs = with pkgs; [
    # X11, Wayland and graphics (loaded by rtmapp with dlopen)
    libxcb
    libx11
    libxcursor
    libxrandr
    libxi
    libGL
    libxkbcommon
    wayland

    # Build tools
    pkg-config
    rustc
    cargo
    rustfmt
    clippy
  ];
  # buildInputs is for 32bit versions
  buildInputs = with pkgs.pkgsi686Linux; [
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

  # rtmapp loads fontconfig and the Vulkan loader with dlopen as well.
  shellHook = ''
    export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${makeLibraryPath (with pkgs; [ libxcb libx11 libxcursor libxrandr libxi libGL libxkbcommon wayland fontconfig vulkan-loader ])}
  '';
}
