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
            files = "\\.nix";
            entry = "${pkgs.deadnix}/bin/deadnix -e -f";
          };
          build = {
            enable = true;
            name = "Build";
            description = "Check that targets build";
            entry = "bazel build //...";
            pass_filenames = false;
          };
          test = {
            enable = true;
            name = "Test";
            description = "Run all tests";
            entry = "bazel test //...";
            pass_filenames = false;
          };
          buildifier = {
            enable = true;
            name = "buildifier";
            description = "Checks build files";
            files = "\\.bazel$";
            entry = "${pkgs.bazel-buildtools}/bin/buildifier";
          };
          breaking-test-targets-check = {
            enable = true;
            name = "Breaking test targets check";
            description = "Breaking test depends on all our proto_library";
            files = "\\.proto$";
            entry = let
              testScript = pkgs.writeScriptBin "breaking-test-targets-check" ''
                #!/usr/bin/env bash

                missing="$(bazel query 'kind("pl_proto_library", //protos/... - filter(//protos/, deps(//protos:breaking_test)))' | tr '\n' ' ')"
                if [ -n "$missing" ]; then
                    ${pkgs.bazel-buildtools}/bin/buildozer -f - <<< "add targets $missing|//protos:breaking_test"
                    exit 1
                fi
              '';
            in "${testScript}/bin/breaking-test-targets-check";
            pass_filenames = false;
          };
          format-protos = {
            enable = true;
            name = "Format proto files";
            entry = "bazel run @rules_buf_toolchains//:buf -- format -w --exit-code --path protos";
            files = "\\.proto";
            pass_filenames = false;
          };
          format-rust = {
            enable = true;
            name = "Format rust files";
            entry = "bazel run @rules_rust//:rustfmt --";
            files = "\\.rs";
            pass_filenames = false;
          };
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
            category = "generators";
          };

          repin-crates = {
            help = "Repin Rust crates";
            command = ''
              bazel run //third-party:crates_vendor -- --repin
              bazel run //third-party:crates_vendor
            '';
            category = "dependencies";
          };

          gen-proto-image-data = {
            help = "Generate Buf breaking test image.bin file";
            command = ''
              bazel run @rules_buf_toolchains//:buf \
                -- build \
                --exclude-imports \
                -o $PWD/protos/testdata/image.bin \
                $PWD/protos
            '';
            category = "generators";
          };

          rust-analyzer-check = {
            help = "Command to pass to rust-analyzer as a replacement to cargo check";
            command = ''
              bazel build --config=ra //rust/... 2>&1
            '';
            category = "helpers";
          };
        };

        packages = pkgs: with pkgs; [bazel_5 jdk11 zlib bazel-watcher];

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
