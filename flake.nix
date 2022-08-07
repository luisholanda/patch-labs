{
  description = "Patch Labs - The patch-based git host.";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    tm-nixpkgs.url = "github:terramagna/nixpkgs";
    tm-nixpkgs.inputs.flake-utils.follows = "flake-utils";
    tm-nixpkgs.inputs.nixpkgs.follows = "nixpkgs";

    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
    pre-commit-hooks.inputs.flake-utils.follows = "flake-utils";
    pre-commit-hooks.inputs.nixpkgs.follows = "nixpkgs";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.flake-utils.follows = "flake-utils";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";

    cargo2nix.url = "github:cargo2nix/cargo2nix/release-0.11.0";
    cargo2nix.inputs.flake-utils.follows = "flake-utils";
    cargo2nix.inputs.nixpkgs.follows = "nixpkgs";
    cargo2nix.inputs.rust-overlay.follows = "rust-overlay";
  };

  outputs =
    { self, nixpkgs, flake-utils, pre-commit-hooks, rust-overlay, tm-nixpkgs }:
    let
      rust-version = "2022-07-22";

      flake = flake-utils.lib.eachDefaultSystem (system:
        let
          inherit (pkgs.nix-gitignore) gitignoreSource;

          pkgs = import nixpkgs {
            inherit system;

            overlays = builtins.attrValues self.overlays;
          };

          pre-commit-check = pre-commit-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              nixfmt.enable = true;
              rustfmt.enable = true;
            };
          };
        in rec {
          checks = { inherit pre-commit-check; };

          devShells.default = tm-nixpkgs.lib.${system}.mkTmShell {
            name = "frontend";

            bubblewrap = true;

            packages = pkgs: with pkgs; [ nixfmt bazel_5 zlib ];

            startup.completion = ''
              source ${pkgs.bash-completion}/etc/profile.d/bash_completion.sh
            '';
            startup.pre-commit = pre-commit-check.shellHook;
            startup.tmpdir = ''
              export TEMPDIR=/tmp
              export TMPDIR=/tmp
              export TMP=/tmp
              export TEMP=/tmp
            '';
            startup.commands = ''
              alias gen-rs-proj="bazel run @rules_rust//tools/rust_analyzer:gen_rust_project"
              alias cargo-repin="CARGO_BAZEL_REPIN=1 bazel sync --only=crates"
            '';
          };

        });
    in flake;
}
