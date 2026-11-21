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
The local `shared/creatiator` source is included with Toniator; no sibling
checkout or separate publication is required. Only `toniator-app` enables its
`gtk` feature. Domain, sampling and render crates share its default headless
sRGB transfers, retaining their existing input/output clipping policies.

The app enables two independent Cargo features by default.

- `geometry-compute` automatically assists eligible Tangent ribbons
  Spacing/Size/Blend construction with exact Vulkan ownership certificates and
  CPU correction/fallback. It also applies while the canvas uses Accurate preview.
- `fast-preview` builds the pinned Vello/wgpu Vulkan canvas renderer. Each window
  starts with Accurate CPU preview. The optional Fast mode needs a supported
  adapter and an eligible scene/target. Other cases use Accurate CPU fallback.

Both initialize GPU resources lazily. Build the GTK app without either GPU
feature using the following command.

```bash
cargo build --release --locked -p toniator-app --no-default-features
```

To retain exact geometry assistance without the approximate Fast renderer, add
`--features geometry-compute` to that command. The separate headless CLI build
above has no default GPU feature. Opt in explicitly with the following command.

```bash
cargo build --release --locked -p toniator-cli --features geometry-compute
```

Cargo unifies dependency features within one build, so build the CLI separately
when checking its CPU-only dependency boundary. A geometry-enabled executable can
be forced to CPU construction before Vulkan initialization with
`TONIATOR_GEOMETRY_COMPUTE=cpu`. The canvas preview mode has its own control.
See [geometry routing and measurements](PERFORMANCE.md#2026-11-18-development-exact-pattern-construction)
for automatic admission, fallback and diagnostic controls.

The production GPU dependencies are pinned by Cargo.lock, including the Vello git
revision. Run `cargo fetch --locked` while network access is available before an
offline build and retain both Cargo's registry and git caches. The GNOME SDK packaging
build uses that complete Cargo home. See also
[preview measurements and limits](PERFORMANCE.md#2026-11-02-development-fast--accurate-preview).

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

In an AppImage, prepend `./Toniator-0.4.7-x86_64.AppImage --cli` in place of
`toniator`. The Flatpak CLI is available through
`flatpak run --command=toniator io.github.ricperry.Toniator`; direct CLI file paths must be
accessible inside its sandbox.

## Build or publish the packages

The [packaging guide](../packaging/README.md) covers building AppImage and Flatpak
bundles with the GNOME SDK. [Release instructions](RELEASING.md) explain
GitHub tags, release notes, and uploading the packages.

The [README](../README.md) introduces the graphical workflow; the
[example projects](examples/README.md) reproduce the gallery.
