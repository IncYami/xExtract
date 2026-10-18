# xExtract

xExtract is a desktop application for inspecting Matroska files and extracting tracks, chapters, and attachments with MKVToolNix.

## Install on NixOS

Clone the `main` branch and install the package into your user profile:

```bash
git clone --branch main https://github.com/IncYami/xExtract.git
cd xExtract
nix-env -f . -i
xextract
```

To update later, run `git pull --ff-only origin main` followed by `nix-env -f . -i` inside the checkout. The Nix package includes a launcher that adds MKVToolNix to `PATH` and configures the native libraries and XKB data needed on NixOS. The package version comes from `Cargo.toml`, and `Cargo.lock` pins the Rust dependencies.

You can also build and launch without installing into a profile:

```bash
nix-build
./result/bin/xextract
```

The desktop entry is at `result/share/applications/xextract.desktop` after building. The package uses libraries from the Nix store; it does not bundle an X11 server.

## Development on NixOS

```bash
nix-shell
cargo run --locked
```

`shell.nix` provides Rust, Cargo, MKVToolNix, and the native Wayland, X11, and GPU libraries used by `egui` and `wgpu`. When both Wayland and XWayland are available, xExtract prefers X11 so file dropping works. In a Wayland-only session it uses Wayland. Set `XEXTRACT_BACKEND=wayland` to select Wayland explicitly.

`rust-toolchain.toml` pins the current upstream stable Rust release for rustup users. As of October 18, 2026, the Nixpkgs revision available here provides Rust 1.98.1 while upstream stable is 1.99.1. Nix builds use the packaged compiler until Nixpkgs provides 1.99.1; both toolchains are validated for this release.

The default window size is 960 × 840 logical points, with a minimum of 900 × 760.

To build an optimized development binary:

```bash
nix-shell --run 'cargo build --release --locked'
```

The raw ELF at `target/release/xextract` may fail outside `nix-shell` on NixOS because dynamically loaded libraries such as `libxkbcommon` and XKB data are not automatically discoverable. Use the Nix package launcher for normal use.

## Usage

Add Matroska files by dragging and dropping, using the file picker, or passing file paths as arguments:

```bash
xextract --profile "Subs Only" movie.mkv
```

`config.toml` can be placed in the current directory, next to the executable, or at `~/.config/xExtract/config.toml`. When migrating a YAML configuration, keep the equivalent fields and profiles in the TOML format shown in `config.toml`. Use `OutputDir = false` instead of `OutputDir: null`, or set `OutputDir = "/path/to/output"` to choose a directory.

xExtract uses `mkvmerge -J` to inspect files and `mkvextract` or `mkvmerge` to process them. External commands run on a worker thread so the interface stays responsive.
