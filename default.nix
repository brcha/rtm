# The packages, built with nix-build or through flake.nix, which imports them from here.
# Usage:
#   nix-build -A rtmcli
#   nix-build -A rtmapp   # desktop app

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

  # pname, version and meta from a crate's [package], so nothing here can drift from it.
  fromCargo = crate:
    let p = (pkgs.lib.importTOML (./. + "/${crate}/Cargo.toml")).package; in {
      pname = p.name;
      inherit (p) version;
      meta = {
        inherit (p) description;
        homepage = p.repository;
        # getLicenseFromSpdxId only warns on an unknown ID.
        license  = pkgs.lib.getLicenseFromSpdxIdOr p.license
          (throw "${crate}: unknown SPDX licence ${p.license}");
      };
    };

in {
  # ── rtmcli ──────────────────────────────────────────────────────────────────
  rtmcli = let crate = fromCargo "rtmcli"; in pkgs.rustPlatform.buildRustPackage (crate // {
    inherit src;
    cargoLock.lockFile = ./Cargo.lock;
    cargoBuildFlags    = [ "-p" "rtmcli" ];
    cargoTestFlags     = [ "-p" "rtmcli" "-p" "todotxt" "-p" "rtmsync" ];
    # The sync tests drive git and pijul; pijul signs through a scratch ssh-agent.
    nativeCheckInputs  = with pkgs; [ gitMinimal pijul openssh ];
    # Without it the Pijul test skips itself when pijul is missing.
    env.RTM_REQUIRE_PIJUL = "1";
    meta = crate.meta // {
      platforms   = pkgs.lib.platforms.all;
      mainProgram = "rtmcli";
    };
  });

  # ── rtmapp ──────────────────────────────────────────────────────────────────
  # The egui desktop app. No native build inputs: on Linux every library is loaded at run time, on
  # macOS the system frameworks come from stdenv's Apple SDK.
  rtmapp = let crate = fromCargo "rtmapp"; in pkgs.rustPlatform.buildRustPackage (crate // {
    inherit src;
    cargoLock.lockFile = ./Cargo.lock;
    cargoBuildFlags    = [ "-p" "rtmapp" ];
    cargoTestFlags     = [ "-p" "rtmapp" ];
    # The sync tests drive git.
    nativeCheckInputs  = [ pkgs.gitMinimal ];
    postInstall = ''
      # The embedded Inter fonts' licence (SIL OFL) must ship with every copy.
      install -Dm644 $src/rtmapp/assets/fonts/Inter-LICENSE.txt \
        $out/share/licenses/rtmapp/Inter-LICENSE.txt

      # XDG desktop entry and hicolor icons
      install -Dm644 $src/rtmapp/assets/rtmapp.desktop $out/share/applications/rtmapp.desktop
      icons=$src/rtmapp/assets/icons
      hicolor=$out/share/icons/hicolor
      install -Dm644 $icons/16x16.png      $hicolor/16x16/apps/rtmapp.png
      install -Dm644 $icons/24x24.png      $hicolor/24x24/apps/rtmapp.png
      install -Dm644 $icons/32x32.png      $hicolor/32x32/apps/rtmapp.png
      install -Dm644 $icons/128x128.png    $hicolor/128x128/apps/rtmapp.png
      install -Dm644 $icons/128x128@2x.png $hicolor/256x256/apps/rtmapp.png
      install -Dm644 $icons/icon.png       $hicolor/512x512/apps/rtmapp.png
      install -Dm644 $icons/rtmapp.svg     $hicolor/scalable/apps/rtmapp.svg
    '';
    # After the rpath shrinking in fixupPhase, which would drop these unused-at-link entries.
    postFixup = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
      patchelf --add-rpath ${pkgs.lib.makeLibraryPath eguiDeps} $out/bin/rtmapp
    '';
    # For the dev shell (shell.nix), which puts them on LD_LIBRARY_PATH.
    passthru = { inherit eguiDeps; };
    meta = crate.meta // {
      platforms   = with pkgs.lib.platforms; linux ++ darwin;
      mainProgram = "rtmapp";
    };
  });
}
