{
  description = "Tempest AI - Hardware-Aware Autonomous Engineer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        
        # Explicitly pin Rust 1.95.0
        rustToolchain = pkgs.rust-bin.stable."1.95.0".default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
        };

        # Shared dependencies for build and runtime
        runtimeDeps = with pkgs; [
          openssl
          libpcap
          fontconfig
          freetype
          dbus
          zlib
          expat
          # X11 libs for arboard (clipboard)
          libx11
          libxcursor
          libxinerama
          libxi
          libxrandr
          libxfixes
          # Intel GPU / OpenGL libs
          libGL
          libglvnd
        ];

        nativeBuildDeps = with pkgs; [
          rustToolchain
          pkg-config
          cmake
          git
          # Needed for bindgen (used by some Rust crates)
          llvmPackages.libclang
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = nativeBuildDeps;
          buildInputs = runtimeDeps;

          # Environment variables for Rust tools
          shellHook = ''
            export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath runtimeDeps}:$LD_LIBRARY_PATH"
            echo "🌪️ Tempest AI Development Shell Powered by Nix"
            echo "🦀 Rust Version: $(rustc --version)"
            echo "🛡️ Hardware-Aware dependencies loaded (Intel/Linux/macOS)"
          '';
        };
      });
}
