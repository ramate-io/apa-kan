{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/8c50a710ddca43d7a530fb805ad55bde8d0141c5";
    rust-overlay.url = "github:oxalica/rust-overlay/e7a078c7feb51f37955a832b22a96de5fccb1f7a";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { nixpkgs, rust-overlay, flake-utils, crane, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        pre_host = pkgs.stdenv.hostPlatform.config;
        host = pkgs.lib.replaceStrings [ "arm64-" ] [ "aarch64-" ] pre_host;

        toolchain = p: (p.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          extensions = [ "rustfmt" "clippy" ];
          targets = [ host "wasm32-unknown-unknown" ];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain(toolchain);

        dependencies = with pkgs; [
          gh
          protobuf
          grpcurl
          grpcui
          perl
          llvmPackages.bintools
          openssl
          openssl.dev
          libiconv 
          pkg-config
          libclang.lib
          libz
          clang
          pkg-config
          rustPlatform.bindgenHook
          lld
          coreutils
          gcc
          rust
        ] ++ lib.optionals stdenv.isDarwin [
          libelf
        ] ++ lib.optionals stdenv.isLinux [
          udev
          systemd
          bzip2
          elfutils
          jemalloc
          alsa-lib
          wayland
        ];

        rust = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          targets = [ host "wasm32-unknown-unknown" ];
        };

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rust;
          rustc = rust;
        };
    
      in {
        devShells = rec {
          default = docker-build;
          docker-build = pkgs.mkShell {
            ROCKSDB = pkgs.rocksdb;
            OPENSSL_DEV = pkgs.openssl.dev;

            hardeningDisable = ["fortify"];

            buildInputs = with pkgs; [
              (toolchain pkgs)
            ] ++ dependencies;

            LD_LIBRARY_PATH = "${pkgs.stdenv.cc.cc.lib}/lib/";

            RUSTFLAGS = pkgs.lib.optionalString pkgs.stdenv.isDarwin
              "-L native=${pkgs.libiconv}/lib";
            LIBRARY_PATH = pkgs.lib.optionalString pkgs.stdenv.isDarwin
              "${pkgs.libiconv}/lib";

            shellHook = ''
              #!/usr/bin/env ${pkgs.bash}

              set -e

              if [[ "${pkgs.stdenv.hostPlatform.system}" =~ "darwin" ]]; then
                export MACOSX_DEPLOYMENT_TARGET=$(sw_vers -productVersion)
                export LDFLAGS="-L${pkgs.libiconv}/lib -L/opt/homebrew/opt/zlib/lib''${LDFLAGS:+ $LDFLAGS}"
                export CPPFLAGS="-I/opt/homebrew/opt/zlib/include''${CPPFLAGS:+ $CPPFLAGS}"

                if [ -d /Applications/Xcode.app/Contents/Developer ]; then
                  export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
                  if sdkroot="$(xcrun --sdk macosx --show-sdk-path 2>/dev/null)" && [ -d "$sdkroot" ]; then
                    export SDKROOT="$sdkroot"
                    export CFLAGS="-isysroot $sdkroot''${CFLAGS:+ $CFLAGS}"
                    export CXXFLAGS="-isysroot $sdkroot -stdlib=libc++''${CXXFLAGS:+ $CXXFLAGS}"
                  fi
                fi
              fi

              export PATH="$PATH:$(pwd)/target/debug"
              export PATH="$PATH:$(pwd)/target/release"

              cp $(pwd)/.githooks/pre-commit $(pwd)/.git/hooks/pre-commit
              chmod +x $(pwd)/.git/hooks/pre-commit

              git config --local include.path ../.gitconfig

              echo ""
              echo "Apa Kan"
              echo "A small turn-based game of colors and adjacency."
            '';
          };
        };
      }
    );
}
