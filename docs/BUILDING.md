# Building Toniator

Requirements: **Rust 1.94+**, a C compiler, `pkg-config`, **GTK 4.12+**, GLib
development tools, `blueprint-compiler`, and the native `dav1d` development
library for AVIF decoding. On Fedora, install the system dependencies with:

```bash
sudo dnf install git gcc pkgconf-pkg-config gtk4-devel glib2-devel \
  blueprint-compiler libdav1d-devel
```

Install a current Rust toolchain through [rustup](https://rustup.rs/), then:

```bash
git clone https://github.com/ricperry/Toniator.git
cd Toniator
cargo build --release --locked -p toniator-app -p toniator-cli
./target/release/toniator-app
```

To open the included sample directly:

```bash
./target/release/toniator-app assets/raster-sample.png
```

For development, `cargo run -p toniator-app` builds and launches the application.
For a headless build, use `cargo build --release --locked -p toniator-cli`;
GTK and Blueprint are unnecessary, but the native `dav1d` dependency still applies.

For the verified native Windows development build and its current limits, see
[Windows build notes](windows-build.md).

## Command-line rendering

Render a source image using the default circular-mark Pattern:

```bash
mkdir -p target/validation/examples
./target/release/toniator render \
  --input assets/raster-sample.png \
  --output target/validation/examples/halftone.png \
  --channel-model cmyk --density 30 --density-aspect 1 \
  --rotation 0 --offset-x 0 --offset-y 0 --guard-steps 2
```

Use an `.svg` output filename for vector export. Render a saved project using
its stored settings with `toniator render -i artwork.toniator -o artwork.svg`.
Run `toniator --help` or `toniator render --help` for the supported commands.

In an AppImage, prepend `./Toniator-0.4.5-x86_64.AppImage --cli` in place of
`toniator`. The Flatpak CLI is available through
`flatpak run --command=toniator io.github.ricperry.Toniator`; direct CLI file paths must be
accessible inside its sandbox.

## Build or publish the packages

The [packaging guide](../packaging/README.md) covers building AppImage and Flatpak
bundles with the GNOME SDK. [Release instructions](RELEASING.md) explain
GitHub tags, release notes, and uploading the packages.

The [README](../README.md) introduces the graphical workflow; the
[example projects](examples/README.md) reproduce the gallery.
