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
    # GTK and related libraries for Dioxus desktop
    gtk3
    glib
    gobject-introspection
    libsoup_3
    atk
    gdk-pixbuf
    pango
    cairo
    # WebKit/JavaScriptCore for Tauri webview
    webkitgtk_4_1

    # X11 and graphics
    xorg.libxcb
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    libGL
    libxkbcommon
    wayland

    # Build tools
    pkg-config
    rustc
    cargo
  ];
  # buildInputs is for 32bit versions
  buildInputs = with pkgs.pkgsi686Linux; [
  ];
  shellHook = with pkgs.xorg; ''
    export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${makeLibraryPath (with pkgs.xorg; with pkgs; [ libX11 libXcursor libXrandr libXi libGL libxkbcommon libsoup_3 atk gdk-pixbuf pango cairo gtk3 glib wayland webkitgtk_4_1 ])}
  '';
}
