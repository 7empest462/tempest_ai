# Legacy Nix Shell (Backward compatibility for non-Flake users)
let
  rustOverlay = builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz";
  pkgs = import <nixpkgs> {
    overlays = [ (import rustOverlay) ];
  };
  rustVersion = "1.95.0";
  rustToolchain = pkgs.rust-bin.stable.${rustVersion}.default.override {
    extensions = [ "rust-src" "rust-analyzer" ];
  };
in
pkgs.mkShell {
  nativeBuildInputs = [
    rustToolchain
    pkgs.pkg-config
    pkgs.cmake
    pkgs.gcc
    pkgs.gnumake
    pkgs.git
    pkgs.llvmPackages.libclang
  ];
  buildInputs = with pkgs; [
    openssl
    libpcap
    fontconfig
    freetype
    dbus
    zlib
    expat
    libx11
    libxcursor
    libxinerama
    libxi
    libxrandr
    libxfixes
    libGL
    libglvnd
  ];
  shellHook = ''
    export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"
    echo "🌪️ Legacy Nix Shell Initialized (Rust ${rustVersion})"
  '';
}
