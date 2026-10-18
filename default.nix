{ pkgs ? import <nixpkgs> {} }:

let
  cargoPackage = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
in

pkgs.rustPlatform.buildRustPackage {
  pname = cargoPackage.name;
  version = cargoPackage.version;

  src = pkgs.lib.cleanSource ./.;
  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [
    pkgs.makeWrapper
    pkgs.pkg-config
  ];

  buildInputs = [
    pkgs.libGL
    pkgs.libxkbcommon
    pkgs.vulkan-loader
    pkgs.wayland
    pkgs.xkeyboard_config
    pkgs.libx11
    pkgs.libxcursor
    pkgs.libxi
    pkgs.libxrandr
    pkgs.libxfixes
    pkgs.libxrender
    pkgs.libxext
    pkgs.libxcb
  ];

  postInstall = ''
    wrapProgram $out/bin/xextract \
      --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.mkvtoolnix-cli ]} \
      --set XKB_CONFIG_ROOT ${pkgs.xkeyboard_config}/share/X11/xkb \
      --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [
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
      ]}

    mkdir -p $out/share/applications
    cat > $out/share/applications/xextract.desktop <<EOF
    [Desktop Entry]
    Type=Application
    Name=xExtract
    Comment=Extract tracks from Matroska files
    Exec=$out/bin/xextract %F
    Icon=video-x-generic
    MimeType=video/x-matroska;
    Terminal=false
    Categories=AudioVideo;Utility;
    EOF
    chmod 755 $out/share/applications/xextract.desktop
  '';
}
