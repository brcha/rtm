{
  description = "Rusty Todo.txt Manager";

  inputs = {
    # Stable channel: rustc 1.95, the minimum egui 0.37 needs. Consumers can override it with
    # `inputs.rtm.inputs.nixpkgs.follows`. Move to nixos-26.11 when it is released (RTM-45).
    nixpkgs.url     = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";
    # Builds the Cargo dependencies in a derivation of their own (default.nix).
    crane.url       = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # The packages are defined once, in default.nix.
        inherit (import ./default.nix { inherit pkgs; craneLib = crane.mkLib pkgs; }) rtmcli rtmapp ci;
      in {
        packages = {
          inherit rtmcli rtmapp;
          default = rtmcli;
        };

        apps = {
          rtmcli = flake-utils.lib.mkApp { drv = rtmcli; };
          rtmapp = flake-utils.lib.mkApp { drv = rtmapp; };
          default = flake-utils.lib.mkApp { drv = rtmcli; };
        };

        # What nix.yml builds: both packages without LTO, tests included.
        checks = { inherit (ci) rtmcli rtmapp; };

        devShells.default = import ./shell.nix { inherit pkgs; };
      }
    );
}
