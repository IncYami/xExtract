# Project notes for coding agents

## Project

- xExtract is a Rust 2024 desktop application for inspecting Matroska files and extracting tracks, chapters, and attachments with MKVToolNix.
- The GUI uses egui/wgpu and winit with Wayland and X11 backends. In sessions exposing both `WAYLAND_DISPLAY` and `DISPLAY`, xExtract prefers X11/XWayland because winit 0.30 does not deliver file-drop events on its Wayland backend. `XEXTRACT_BACKEND=wayland` overrides that preference.
- `mkvmerge` and `mkvextract` are external runtime commands; the Nix package adds MKVToolNix to `PATH`.

## NixOS build and launch

- Use `nix-build` to build the wrapped app package. Run it with `./result/bin/xextract`; this launcher sets the runtime library path, `XKB_CONFIG_ROOT`, and MKVToolNix `PATH` without entering `nix-shell` or installing the package into a Nix profile.
- `target/release/xextract` is the raw Cargo ELF. It can fail outside the development shell because NixOS runtime libraries such as `libxkbcommon` are dynamically loaded and are not all discoverable by the ELF on its own.
- Keep runtime library and XKB data configuration in `default.nix` in sync with `shell.nix`.

## Rust changes

- Keep dependency and build changes compatible with the locked dependency set; use `cargo ... --locked` for reproducible commands.
- Preserve file-drop support through X11/XWayland and retain Wayland startup for Wayland-only sessions.

## Technical writing

- Use the current [ASD-STE100 Simplified Technical English standard](https://www.asd-ste100.org/) for new or edited English documentation and code comments. Apply it to README files, configuration comments, user-facing help and error messages, commit messages, and release or pull request descriptions when useful.
- Use the standard's approved words and meanings. Use consistent project terms for names such as Rust, Matroska, MKVToolNix, commands, API identifiers, and file paths. Explain a project term when readers might not know it.
- Write one topic per sentence. Use active voice. Keep procedural sentences at 20 words or fewer and descriptive sentences at 25 words or fewer. Write one action per instruction and start instructions with a command verb.
- Review the text against the official rules and dictionary before you claim ASD-STE100 compliance. Keep required code syntax, exact command names, quoted output, and translated text intact.

## Rust maintenance and security

- For every Rust application or dependency change, check the latest stable Rust release and the latest stable releases of direct dependencies. Update the Rust toolchain, direct dependencies, and `Cargo.lock` to those versions, adapting application code to changed APIs and keeping the project's build and packaging configuration compatible. If an update is blocked, document the concrete incompatibility and revisit it in the next Rust change.
- Keep the architecture clean and simple. Prefer the standard library or dependencies already in use, remove unused crates, and add only the smallest set of dependencies needed. Prefer well-maintained crates backed by an official vendor, an established organization, or active professional contributors; review maintenance and provenance before adding one.
- Run `cargo audit` against `Cargo.lock` after dependency changes and before completing Rust application changes. Resolve actionable security advisories before committing, and document any advisory that cannot yet be resolved.

## Git and versioning

- After completing any project change, commit it and push the updated `main` branch to `origin`. Do not leave completed changes only in the local working tree.
- Every commit that changes Rust application code must also increase the package version in `Cargo.toml` and update `Cargo.lock` in the same commit. Use semantic versioning: increase the patch number for fixes or internal changes, the minor number for new functionality while the project is in `0.x`, and the major number for incompatible changes after `1.0.0`.
- Documentation-only, instruction-only, and Nix packaging-only changes do not require an application version increase.
