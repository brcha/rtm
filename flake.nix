{
  description = "Rusty Todo.txt Manager";

  inputs = {
    nixpkgs.url     = "github:NixOS/nixpkgs/nixos-unstable";
    # nixos-unstable ships Rust 1.85+ which supports edition 2024
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

        # Shared GUI deps for rtmapp (GTK3 + X11/Wayland/OpenGL).
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

        # ── rtmapp ────────────────────────────────────────────────────────────
        # Best-effort: plain `cargo build -p rtmapp` (bypasses Tauri bundler).
        # Frontend assets are copied into $out/share/rtmapp/ via postInstall.
        rtmapp = pkgs.rustPlatform.buildRustPackage (commonArgs // {
          pname = "rtmapp";
          cargoBuildFlags = [ "-p" "rtmapp" ];
          nativeBuildInputs = with pkgs; [
            pkg-config
            gobject-introspection  # build-time GObject type system tool
            wrapGAppsHook4         # GTK4/WebKitGTK wrapper for Tauri v2
          ];
          buildInputs = tauriDeps;
          # Required on some compositors to prevent WebKit rendering issues.
          WEBKIT_DISABLE_COMPOSITING_MODE = "1";
          postInstall = ''
            # Frontend assets (served by the embedded WebKit view)
            mkdir -p $out/share/rtmapp
            cp -r $src/rtmapp/src $out/share/rtmapp/

            # XDG desktop entry
            mkdir -p $out/share/applications
            cp $src/rtmapp/rtmapp.desktop $out/share/applications/rtmapp.desktop

            # Icons — hicolor theme, standard sizes
            mkdir -p $out/share/icons/hicolor/32x32/apps
            mkdir -p $out/share/icons/hicolor/128x128/apps
            mkdir -p $out/share/icons/hicolor/256x256/apps
            cp $src/rtmapp/src-tauri/icons/32x32.png \
               $out/share/icons/hicolor/32x32/apps/rtmapp.png
            cp $src/rtmapp/src-tauri/icons/128x128.png \
               $out/share/icons/hicolor/128x128/apps/rtmapp.png
            cp $src/rtmapp/src-tauri/icons/128x128@2x.png \
               $out/share/icons/hicolor/256x256/apps/rtmapp.png
            mkdir -p $out/share/icons/hicolor/512x512/apps
            cp $src/rtmapp/src-tauri/icons/icon.png \
               $out/share/icons/hicolor/512x512/apps/rtmapp.png
          '';
          meta = with pkgs.lib; {
            description = "Rusty Todo.txt Manager — Tauri desktop app (best-effort)";
            license     = licenses.mit;
            platforms   = platforms.linux;
            mainProgram = "rtmapp";
          };
        });

      in {
        # ── Packages ──────────────────────────────────────────────────────────
        packages = {
          inherit rtmcli rtmapp;
          default = rtmcli;
        };

        # ── Apps ──────────────────────────────────────────────────────────────
        apps = {
          rtmcli = flake-utils.lib.mkApp { drv = rtmcli; };
          rtmapp = flake-utils.lib.mkApp { drv = rtmapp; };
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
            gobject-introspection
            nodejs
          ];
          buildInputs = tauriDeps;
          shellHook = ''
            export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${pkgs.lib.makeLibraryPath tauriDeps}
          '';
        };
      }
    );
}
