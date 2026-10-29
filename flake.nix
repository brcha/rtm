{
  description = "Rusty Todo.txt Manager";

  inputs = {
    # Stable channel: rustc 1.95, the minimum egui 0.37 needs. Consumers can override it with
    # `inputs.rtm.inputs.nixpkgs.follows`. Move to nixos-26.11 when it is released (RTM-45).
    nixpkgs.url     = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Filtered source — avoids copying build artefacts into the Nix store.
        src = builtins.path {
          path  = ./.;
          name  = "rtm-source";
          filter = path: type:
            !(builtins.elem (baseNameOf path)
              [ "target" ".git" "node_modules" ".direnv" ]);
        };

        # Run-time libraries for rtmgui (egui/winit/wgpu). It loads all of them with dlopen, so they
        # are not build inputs but go on the binary's rpath.
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

        # Shared build arguments for all crates.
        commonArgs = {
          inherit src;
          cargoLock.lockFile = ./Cargo.lock;
          version = "0.1.0";
        };

        # ── rtmcli ────────────────────────────────────────────────────────────
        rtmcli = pkgs.rustPlatform.buildRustPackage (commonArgs // {
          pname = "rtmcli";
          cargoBuildFlags = [ "-p" "rtmcli" ];
          cargoTestFlags  = [ "-p" "rtmcli" "-p" "todotxt" ];
          meta = with pkgs.lib; {
            description = "Rusty Todo.txt Manager — CLI";
            license     = licenses.mit;
            platforms   = platforms.all;
            mainProgram = "rtmcli";
          };
        });

        # ── rtmgui ────────────────────────────────────────────────────────────
        # A standalone Cargo workspace with its own lock (see rtmgui/AGENTS.md), so it does not
        # use commonArgs. No native build inputs: every library is loaded at run time.
        rtmgui = pkgs.rustPlatform.buildRustPackage {
          pname = "rtmgui";
          version = (pkgs.lib.importTOML ./rtmgui/Cargo.toml).package.version;
          inherit src;
          cargoRoot = "rtmgui";
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

      in {
        # ── Packages ──────────────────────────────────────────────────────────
        packages = {
          inherit rtmcli rtmgui;
          default = rtmcli;
        };

        # ── Apps ──────────────────────────────────────────────────────────────
        apps = {
          rtmcli = flake-utils.lib.mkApp { drv = rtmcli; };
          rtmgui = flake-utils.lib.mkApp { drv = rtmgui; };
          default = flake-utils.lib.mkApp { drv = rtmcli; };
        };

        # ── Dev shell ─────────────────────────────────────────────────────────
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
            rustfmt
            clippy
            rust-analyzer
            pkg-config
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
          shellHook = ''
            export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${pkgs.lib.makeLibraryPath eguiDeps}
          '';
        };
      }
    );
}
