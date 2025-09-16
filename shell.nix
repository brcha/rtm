let pkgs = import <nixpkgs> {};
in
  with pkgs.stdenv;
  with pkgs.lib;
pkgs.mkShell {
  # nativeBuildInputs is usually what you want -- tools you need to run
  nativeBuildInputs = with pkgs; [
    xorg.libxcb libGL libxkbcommon libsoup_3 atk gdk-pixbuf pango pkg-config cairo webkitgtk_4_1
  ];
  # buildInputs is for 32bit versions
  buildInputs = with pkgs.pkgsi686Linux; [
  ];
  shellHook = with pkgs.xorg; ''
    export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${makeLibraryPath (with pkgs.xorg; with pkgs; [ libX11 libXcursor libXrandr libXi libGL libxkbcommon libsoup_3 atk gdk-pixbuf pango cairo webkitgtk_4_1 ])}
  '';
}
