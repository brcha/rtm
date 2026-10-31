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

        # The packages are defined once, in default.nix.
        inherit (import ./default.nix { inherit pkgs; }) rtmcli rtmapp;
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
        devShells.default = import ./shell.nix { inherit pkgs; };
      }
    );
}
# RTM-82 test, not to be merged.
