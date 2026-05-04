{
  description = "VoxelRuler Development Environment with rust-toolchain.toml support";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {inherit system overlays;};
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        linuxLibs = with pkgs; [
          libGL
          libx11
          libxcursor
          libxi
          libxrandr
          libxkbcommon
          wayland
        ];
        darwinLibs = with pkgs.darwin.apple_sdk.frameworks; [
          AppKit
          CoreGraphics
          CoreText
          Foundation
          OpenGL
        ];
      in {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
            cmake
          ];

          buildInputs =
            [
              rustToolchain
              pkgs.fontconfig
            ]
            ++ (
              if pkgs.stdenv.isDarwin
              then darwinLibs
              else linuxLibs
            );
          
          shellHook =
            if pkgs.stdenv.isDarwin
            then ''
              echo "🍎 macOS + rust-toolchain.toml 環境已就緒！"
            ''
            else ''
              export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath linuxLibs}:$LD_LIBRARY_PATH
              echo "❄️ Linux + rust-toolchain.toml 環境已就緒！"
            '';
        };
      }
    );
}
