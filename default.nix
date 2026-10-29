# Legacy nix-build entry point. For flakes, use flake.nix.
# Usage:
#   nix-build -A rtmcli
#   nix-build -A rtmapp   # desktop app (Linux only)

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

  # Run-time libraries for rtmapp (egui/winit/wgpu). It loads all of them with dlopen, so they are
  # not build inputs but go on the binary's rpath.
  eguiDeps = with pkgs; [
    wayland
    libxkbcommon
    libGL
    vulkan-loader
    libxcb
    libx11
    libxcursor
    libxrandr
    libxi
    fontconfig
  ];

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

  # ── rtmapp ──────────────────────────────────────────────────────────────────
  # The egui desktop app. No native build inputs: every library is loaded at run time.
  rtmapp = pkgs.rustPlatform.buildRustPackage {
    pname   = "rtmapp";
    version = (pkgs.lib.importTOML ./rtmapp/Cargo.toml).package.version;
    inherit src;
    cargoLock.lockFile = ./Cargo.lock;
    cargoBuildFlags    = [ "-p" "rtmapp" ];
    cargoTestFlags     = [ "-p" "rtmapp" ];
    postInstall = ''
      # The embedded Inter fonts' licence (SIL OFL) must ship with every copy.
      install -Dm644 $src/rtmapp/assets/fonts/Inter-LICENSE.txt \
        $out/share/licenses/rtmapp/Inter-LICENSE.txt

      # XDG desktop entry and hicolor icons
      install -Dm644 $src/rtmapp/assets/rtmapp.desktop $out/share/applications/rtmapp.desktop
      icons=$src/rtmapp/assets/icons
      hicolor=$out/share/icons/hicolor
      install -Dm644 $icons/32x32.png      $hicolor/32x32/apps/rtmapp.png
      install -Dm644 $icons/128x128.png    $hicolor/128x128/apps/rtmapp.png
      install -Dm644 $icons/128x128@2x.png $hicolor/256x256/apps/rtmapp.png
      install -Dm644 $icons/icon.png       $hicolor/512x512/apps/rtmapp.png
    '';
    # After the rpath shrinking in fixupPhase, which would drop these unused-at-link entries.
    postFixup = ''
      patchelf --add-rpath ${pkgs.lib.makeLibraryPath eguiDeps} $out/bin/rtmapp
    '';
    meta = with pkgs.lib; {
      description = "Rusty Todo.txt Manager — desktop app";
      license     = licenses.mit;
      platforms   = platforms.linux;
      mainProgram = "rtmapp";
    };
  };
}
