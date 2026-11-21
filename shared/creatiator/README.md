# creatiator

Shared sRGB transfer functions and GTK4 presentation code for Toniator,
Chromiator, and Bezziator. The default build exposes only double-precision color
transfers and has no GTK dependency. Enable the `gtk` feature for the common
native theme bridge and embedded suite stylesheet. Each app keeps its document
model, commands, history, file workflows, UI templates, and specific stylesheet.

```rust
let linear = creatiator::color::srgb_to_linear(0.5);
let encoded = creatiator::color::linear_to_srgb(linear);
```

With `features = ["gtk"]`, call `inherit_system_color_scheme()` on GTK's main
thread and retain its `SystemThemeBridge` for the application lifetime. After
registering application resources, call
`install_styles(&display, Some("/example/app/application.css"))` to install the
shared baseline below the app's own stylesheet. Pass `None` for an application
that needs only the shared stylesheet.

Use a local path dependency to a checked-in copy for standalone source builds;
this crate requires no registry publication and has `publish = false`. GTK4
0.11.4 with the GTK 4.12 API baseline is shared by the consumers. GTK and its
native libraries are required only when the `gtk` feature is enabled. Color
transfers do not clip, narrow through `f32`, or alter alpha; consumers retain
their existing validation and clipping policy.

See [PROVENANCE.md](PROVENANCE.md) for the extracted source and license. Existing
theme behavior is preserved: GNOME preferences are read without changing desktop
settings, and Flatpak/AppImage builds also follow the Settings portal. An initial
portal reply never replaces a newer portal signal.
