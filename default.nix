# Legacy nix-build entry point. For flakes, use flake.nix.
# Usage:
#   nix-build -A rtmcli
#   nix-build -A rtmgui   # egui app (Linux only)

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

  # Run-time libraries for rtmgui (egui/winit/wgpu). It loads all of them with dlopen, so they are
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

  # ── rtmgui ──────────────────────────────────────────────────────────────────
  # A standalone Cargo workspace with its own lock (see rtmgui/AGENTS.md). No native build
  # inputs: every library is loaded at run time.
  rtmgui = pkgs.rustPlatform.buildRustPackage {
    pname   = "rtmgui";
    version = (pkgs.lib.importTOML ./rtmgui/Cargo.toml).package.version;
    inherit src;
    cargoRoot          = "rtmgui";
    buildAndTestSubdir = "rtmgui";
    cargoLock.lockFile = ./rtmgui/Cargo.lock;
    postInstall = ''
      # The embedded Inter fonts' licence (SIL OFL) must ship with every copy.
      install -Dm644 $src/rtmgui/assets/fonts/Inter-LICENSE.txt \
        $out/share/licenses/rtmgui/Inter-LICENSE.txt
    '';
    # After the rpath shrinking in fixupPhase, which would drop these unused-at-link entries.
    postFixup = ''
      patchelf --add-rpath ${pkgs.lib.makeLibraryPath eguiDeps} $out/bin/rtmgui
    '';
    meta = with pkgs.lib; {
      description = "Rusty Todo.txt Manager — egui desktop app";
      license     = licenses.mit;
      platforms   = platforms.linux;
      mainProgram = "rtmgui";
    };
  };
}
