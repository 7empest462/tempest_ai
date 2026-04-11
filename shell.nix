{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # Core Build Tools
    gnumake
    gcc
    pkg-config

    # Required by tempest-monitor & reqwest
    openssl
    fontconfig
    freetype

    # Required by pnet (Raw Network Sockets)
    libpcap

    # Required by arboard (Clipboard access on Linux)
    xorg.libxcb

    # Environment Fixes
    glibcLocales
  ];

  shellHook = ''
    # Fix the locale warning and garbled text
    export LOCALE_ARCHIVE="${pkgs.glibcLocales}/lib/locale/locale-archive"
    export LANG="en_US.UTF-8"
    export LC_ALL="en_US.UTF-8"

    # Map all C-headers for the Rust compiler
    export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:${pkgs.fontconfig.dev}/lib/pkgconfig:${pkgs.freetype.dev}/lib/pkgconfig:${pkgs.xorg.libxcb.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
    
    # Explicit OpenSSL bindings
    export OPENSSL_DIR="${pkgs.openssl.dev}"
    export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
    export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include"
    
    echo "Tempest AI Fleet Build Environment Loaded 🦀"
  '';
}

