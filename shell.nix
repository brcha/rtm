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
    export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${makeLibraryPath (with pkgs.xorg; with pkgs; [ libX11 libXcursor libXrandr libXi libGL libxkbcommon libsoup_3 atk gdk-pixbuf pango cairo gtk3 glib wayland ])}
  '';
}
