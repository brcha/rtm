# The packages, built with nix-build or through flake.nix, which imports them from here.
# Usage:
#   nix-build -A rtmcli
#   nix-build -A rtmapp   # desktop app

{ pkgs ? import <nixpkgs> {}
  # flake.nix passes crane's library; without it, the revision locked in flake.lock is used.
, craneLib ? import (
    let
      lock = builtins.fromJSON (builtins.readFile ./flake.lock);
      crane = lock.nodes.${lock.nodes.root.inputs.crane}.locked;
    in
    # The URL below is GitHub's; another input type needs another fetcher.
    assert crane.type == "github";
    fetchTarball {
      url = "https://github.com/${crane.owner}/${crane.repo}/archive/${crane.rev}.tar.gz";
      sha256 = crane.narHash;
    }
  ) { inherit pkgs; }
}:

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
    dbus
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

  # Builds a crate in two stages: its dependencies, from a copy of the workspace whose sources are
  # empty programs, so that only Cargo.lock and the Cargo.toml files decide when they are rebuilt;
  # then the workspace crates on top of them. Both stages must run the same cargo commands, or cargo
  # compiles the dependencies again.
  # `profile` is a cargo profile for both stages, or null for crane's default (release).
  buildCrate = profile: crate: testPackages: args:
    let
      common = {
        inherit src;
        inherit (crate) version;
        pname = if profile == null then crate.pname else "${crate.pname}-${profile}";
        # No update check: Nix users update through the flake (doc/nix-packaging.md).
        cargoExtraArgs      = "--locked --no-default-features";
        cargoBuildExtraArgs = "-p ${crate.pname}";
        cargoTestExtraArgs  = pkgs.lib.concatMapStringsSep " " (p: "-p ${p}") testPackages;
      } // pkgs.lib.optionalAttrs (profile != null) { CARGO_PROFILE = profile; };
      cargoArtifacts = craneLib.buildDepsOnly (common // {
        # Without crane's default `cargo check --all-targets`, which the package stage never runs.
        buildPhaseCargoCommand =
          "cargoWithProfile build ${common.cargoExtraArgs} ${common.cargoBuildExtraArgs}";
        cargoTestExtraArgs = "${common.cargoTestExtraArgs} --no-run";
      });
    in
    craneLib.buildPackage (common // { inherit cargoArtifacts; } // args);

  rtmcli = profile: let crate = fromCargo "rtmcli"; in buildCrate profile crate
    [ "rtmcli" "todotxt" "rtmsync" "rtmmcp" ] {
    # The sync tests drive git and pijul; pijul signs through a scratch ssh-agent.
    nativeCheckInputs  = with pkgs; [ gitMinimal pijul openssh ];
    # Without it the Pijul test skips itself when pijul is missing.
    env.RTM_REQUIRE_PIJUL = "1";
    meta = crate.meta // {
      platforms   = pkgs.lib.platforms.all;
      mainProgram = "rtmcli";
    };
  };

  # The egui desktop app. No native build inputs: on Linux every library is loaded at run time, on
  # macOS the system frameworks come from stdenv's Apple SDK.
  rtmapp = profile: let crate = fromCargo "rtmapp"; in buildCrate profile crate [ "rtmapp" ] {
    # The sync tests drive git.
    nativeCheckInputs  = [ pkgs.gitMinimal ];
    postInstall = ''
      # The embedded Inter fonts' licence (SIL OFL) must ship with every copy.
      install -Dm644 $src/rtmapp/assets/fonts/Inter-LICENSE.txt \
        $out/share/licenses/rtmapp/Inter-LICENSE.txt

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
  };

in {
  rtmcli = rtmcli null;
  rtmapp = rtmapp null;
  # The flake's checks, which CI builds: [profile.ci] in Cargo.toml, no LTO.
  ci = {
    rtmcli = rtmcli "ci";
    rtmapp = rtmapp "ci";
  };
}
