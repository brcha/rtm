# Legacy nix-build entry point. For flakes, use flake.nix.
# Usage:
#   nix-build -A rtmcli
#   nix-build -A rtmgui
#   nix-build -A rtmapp   # best-effort (Tauri, Linux only)

{ pkgs ? import <nixpkgs> {} }:

let
  # Filtered source — avoids copying build artefacts into the Nix store.
  src = builtins.path {
    path   = ./.;
    name   = "rtm-source";
    filter = path: type:
      !(builtins.elem (baseNameOf path)
        [ "target" ".git" "node_modules" ".direnv" ]);
  };

  # Shared GUI deps (rtmgui — GTK3 + X11/Wayland/OpenGL).
  guiDeps = with pkgs; [
    gtk3
    glib
    atk
    gdk-pixbuf
    pango
    cairo
    xorg.libxcb
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    libGL
    libxkbcommon
    wayland
  ];

  # Additional deps for rtmapp (Tauri v2 — WebKitGTK 4.1 + libsoup 3).
  tauriDeps = guiDeps ++ (with pkgs; [
    webkitgtk_4_1
    libsoup_3
  ]);

in {
  # ── rtmcli ──────────────────────────────────────────────────────────────────
  rtmcli = pkgs.rustPlatform.buildRustPackage {
    pname   = "rtmcli";
    version = "0.1.0";
    inherit src;
    cargoLock.lockFile = ./Cargo.lock;
    cargoBuildFlags    = [ "-p" "rtmcli" ];
    cargoTestFlags     = [ "-p" "rtmcli" "-p" "todotxt" ];
    meta = with pkgs.lib; {
      description = "Rusty Todo.txt Manager — CLI";
      license     = licenses.mit;
      platforms   = platforms.all;
      mainProgram = "rtmcli";
    };
  };

  # ── rtmgui ──────────────────────────────────────────────────────────────────
  rtmgui = pkgs.rustPlatform.buildRustPackage {
    pname   = "rtmgui";
    version = "0.1.0";
    inherit src;
    cargoLock.lockFile = ./Cargo.lock;
    cargoBuildFlags    = [ "-p" "rtmgui" ];
    nativeBuildInputs  = with pkgs; [
      pkg-config
      gobject-introspection
      wrapGAppsHook
    ];
    buildInputs = guiDeps;
    meta = with pkgs.lib; {
      description = "Rusty Todo.txt Manager — egui desktop GUI";
      license     = licenses.mit;
      platforms   = platforms.linux;
      mainProgram = "rtmgui";
    };
  };

  # ── rtmapp ──────────────────────────────────────────────────────────────────
  # Best-effort: plain `cargo build -p rtmapp` (bypasses Tauri bundler).
  # Frontend assets are copied into $out/share/rtmapp/ via postInstall.
  rtmapp = pkgs.rustPlatform.buildRustPackage {
    pname   = "rtmapp";
    version = "0.1.0";
    inherit src;
    cargoLock.lockFile = ./Cargo.lock;
    cargoBuildFlags    = [ "-p" "rtmapp" ];
    nativeBuildInputs  = with pkgs; [
      pkg-config
      gobject-introspection
      wrapGAppsHook4
    ];
    buildInputs = tauriDeps;
    WEBKIT_DISABLE_COMPOSITING_MODE = "1";
    postInstall = ''
      mkdir -p $out/share/rtmapp
      cp -r $src/rtmapp/src $out/share/rtmapp/
    '';
    meta = with pkgs.lib; {
      description = "Rusty Todo.txt Manager — Tauri desktop app (best-effort)";
      license     = licenses.mit;
      platforms   = platforms.linux;
      mainProgram = "rtmapp";
    };
  };
}
