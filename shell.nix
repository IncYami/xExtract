{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  packages = with pkgs; [
    cargo
    clippy
    pkg-config
    rustc
    rustfmt
    mkvtoolnix-cli

    # Native Wayland windowing and GPU development files used by egui/wgpu.
    libGL
    libxkbcommon
    vulkan-loader
    vulkan-headers
    wayland
    xkeyboard_config
    libx11
    libxcursor
    libxi
    libxrandr
    libxfixes
    libxrender
    libxext
    libxcb
  ];

  # Lets build scripts and local runs find the native libraries in the shell.
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
    pkgs.libGL
    pkgs.libxkbcommon
    pkgs.vulkan-loader
    pkgs.wayland
    pkgs.libx11
    pkgs.libxcursor
    pkgs.libxi
    pkgs.libxrandr
    pkgs.libxfixes
    pkgs.libxrender
    pkgs.libxext
    pkgs.libxcb
  ];

  # libxkbcommon needs the keyboard layout data at runtime on NixOS.
  XKB_CONFIG_ROOT = "${pkgs.xkeyboard_config}/share/X11/xkb";

  RUST_BACKTRACE = "1";
  RUST_LOG = "warn";

  shellHook = ''
    export CARGO_TERM_COLOR=always
    echo "xExtract Rust + egui/wgpu development shell"
  '';
}
