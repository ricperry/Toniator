# Corresponding sources and build provenance

Obtain `Toniator-0.3.2-windows-x64-dev-20260927-sources.zip` alongside the binary ZIP
on the same GitHub prerelease page. It contains:

- `repository-source.tar`: the exact checkpoint named in PACKAGE-MANIFEST.json,
  including Cargo.lock, resources, Windows build helpers and gvsbuild patches.
- `native/`: original retained GTK stack source archives, including library and
  icon sources. Archive hashes are recorded in SOURCE-MANIFEST.json.
- `accesskit-c/`: the exact source tree for AccessKit C commit
  `0c52a8ce2357bbeb927f90dc9a1c19c8ec1bd2c3` used for its static MSVC build.
- `cargo/`: original public registry `.crate` source archives observed in retained
  native Cargo dependency files for Toniator, AccessKit and librsvg. These also
  cover host build dependencies. Each archive checksum is checked against its
  retained Cargo.lock entry before copying. Extra retained test/build
  dependencies may be present; inclusion does not mean every crate is linked.
- `gvsbuild/`: installed gvsbuild 2026.8.0 recipe and patch sources, including the
  project-specific recipe changes, original notices and package metadata.
- `build-records/`: retained build options for the GTK stack and AccessKit.

Extract the repository source tar and follow `docs/windows-build.md` to rebuild
Toniator and replace its executables or individual DLLs. Restore each `.crate`
archive into the matching Cargo registry cache or use Cargo's normal crates.io
retrieval, preserving Cargo.lock. Native source archives plus gvsbuild recipes
and build records allow rebuilding the individual LGPL libraries. The package
uses dynamic GTK DLLs; there is no restriction on substituting modified DLLs.

The recorded toolchain is Rust 1.94.1 x86_64-pc-windows-msvc, MSVC 14.44.35207
and Windows SDK 10.0.22621.0. The Rust release source is available at
https://github.com/rust-lang/rust/tree/1.94.1 and its original copyright inventory
is in the binary notices. Microsoft compiler/runtime components are obtained
from Microsoft under their own terms; Windows system DLLs are not included.

This bundle records inputs and recipes, not a claim of byte-identical builds of
all bootstrap tools. Development executables are copied without rebuilding.
FFmpeg is downloaded explicitly by Setup-Media.ps1 and is not redistributed here.
