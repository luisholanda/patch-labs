{
  description = "Patch Labs - A Patch-focused Code Review system";

  inputs = {
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
    pre-commit-hooks.inputs.nixpkgs.follows = "tm-nixpkgs/nixpkgs";

    tm-nixpkgs.url = "github:terramagna/nixpkgs";
    tm-nixpkgs.inputs.pre-commit-hooks.follows = "pre-commit-hooks";
  };

  outputs = {
    nixpkgs,
    pre-commit-hooks,
    tm-nixpkgs,
    ...
  }: let
    inherit (tm-nixpkgs.lib) forEachSystem;

    systemAttrs = forEachSystem (system: let
      inherit (tm-nixpkgs.lib.${system}) mkTmShell;

      pkgs = import nixpkgs {
        inherit system;
      };

      pre-commit-check = pre-commit-hooks.lib.${system}.run {
        src = ./.;
        hooks = {
          alejandra.enable = true;
          statix.enable = true;
          deadnix = {
            enable = true;
            name = "deadnix";
            description = "A dead code analyzer for Nix expressions";
            types = ["file" "nix"];
            entry = "${pkgs.deadnix}/bin/deadnix -e -f";
          };
          build = {
            enable = true;
            name = "Build";
            description = "Check that targets build";
            entry = "TEMP=/tmp bazel build //...";
            pass_filenames = false;
          };
          test = {
            enable = true;
            name = "Test";
            description = "Run all tests";
            entry = "TEMP=/tmp bazel test //...";
            pass_filenames = false;
          };
          buildifier = {
            enable = true;
            name = "buildifier";
            description = "Checks build files";
            types = ["file" "bazel"];
            entry = "${pkgs.bazel-buildtools}/bin/buildifier";
          };
          rustfmt.enable = true;
        };
      };

      devShell = mkTmShell {
        name = "nbs";
        bubblewrap = true;

        commands = {
          c = {
            help = "Run repository checks";
            command = "pre-commit run -a";
            category = "checks";
          };

          gen-rust-project = {
            help = "Generate the rust-project.json file.";
            command = "bazel run //:gen-rust-project";
            category = "helpers";
          };

          repin-crates = {
            help = "Repin Rust crates";
            command = ''
              bazel run //third-party:crates_vendor -- --repin
              bazel run //third-party:crates_vendor
            '';
            category = "dependencies";
          };
        };

        packages = pkgs: with pkgs; [bazel_5 jdk11 zlib];

        startup.pre-commit = pre-commit-check.shellHook;
      };
    in {
      checks = {inherit pre-commit-check;};

      devShells.default = devShell;
    });
  in
    systemAttrs
    // {
      lib = import ./lib {nixpkgsLib = nixpkgs.lib;};
    };
}
