# NEEDED: MacOS (Darwin) support (Please read. I know it's a bit long.)
# I (Dusk) do not use MacOS for reasons I will not get into here.
# But due to this I cannot make this flake with MacOS in mind.
#
# Before anyone suggests it, no I will not use AI. I have my personal
# issues with AI, but aside from that I cannot validate that the flake
# works on MacOS.
#
# If anyone who uses MacOS and Nix could help contribute, that would
# be a massive help.
#
# Thank you.
# -Dusk (May 10th, 2026)
{
  description = "Usagi Engine Flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
  flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages.${system};
      rustPlatform = pkgs.rustPlatform;
    in {
      packages.default = rustPlatform.buildRustPackage {
        pname = "usagi";
        version = "0.8.0-dev";
        src = ./.;
        cargoHash = "sha256-ldq9exVQYlYwD43HLww5h/S/n5ofpfuIWbXGprLywVA=";

        nativeBuildInputs = [
          rustPlatform.bindgenHook
          pkgs.pkg-config
          pkgs.cmake
          pkgs.makeWrapper
        ];

        buildInputs = with pkgs; [
          raylib
          libx11
          libxrandr
          libxi
          libxcursor
          libxinerama
          alsa-lib

          zip
          unzip
        ];

        postFixup = ''
          wrapProgram $out/bin/usagi --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath[
              pkgs.libGL
              pkgs.mesa
            ]}
        '';
      };

      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          cargo
          rustc
          rustfmt
          clippy
          rust-analyzer

          cmake
          pkg-config
          raylib

          libx11
          libxrandr
          libxi
          libxcursor
          libxinerama
          alsa-lib

          zip
          unzip

          libGL
        ];

        nativeBuildInputs = [
          rustPlatform.bindgenHook
          pkgs.libGL
        ];

        shellHook = ''
          export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath[
            pkgs.libGL
            pkgs.mesa
          ]}:$LD_LIBRARY_PATH
        '';
      };
      env.RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    }
  );
}
