# Baseline test artwork

These files are Toniator's project-wide source-artwork fixtures. Use
them in relevant source loading, sampling, rendering, preview, and export tests
alongside smaller synthetic fixtures for isolated edge cases.

| File | Required characteristics | SHA-256 |
| --- | --- | --- |
| `raster-sample.png` | 1024×1024, 8-bit sRGB RGBA; alpha spans fully transparent through fully opaque pixels. | `324ac232e319002a13fbcfac46538ca5d7e8ba8a127eea2eaf20e8ddb3ed2ef2` |
| `vector-sample.svg` | 900×620 SVG with gradients, transparency, a stroked path, and a live `<text>` element containing `T`. | `42eb5e23111a5dbad66f2b1802a7cc06391c7ede829b99eb28aeb1ac91596e2e` |
| `video-sample0001-0010.mp4` | 10-frame 1080×1920 H.264 High/yuv420p video at 6 fps (1.666667 seconds), reserved for future multiframe and animation work. | `c84d4a42cf62803d41ac35152fd3fea1719a664c633900cb946b9b5a6d6bef81` |

Treat the source files as immutable baselines. Write derived artifacts under
`target/validation/`, not `assets/`. Replacing or editing a baseline requires
explicit approval plus synchronized hash, documentation, and test updates.

The video fixture is not part of the current still-image Stage 6 evaluation
gate. Exercise it only in a later explicitly approved multiframe or animation
stage.

## Obsolete-schema rejection witness

`HolidayMugs_2024_2025.toniator` is a tracked real-world container-v1,
document-schema-v5 test case. The current-only schema-v7 boundary uses
it solely to prove that obsolete documents are rejected rather than migrated.
Preserve its bytes and hash; do not regenerate it or treat it as a current-
format round-trip fixture.

| File | Role | SHA-256 |
| --- | --- | --- |
| `HolidayMugs_2024_2025.toniator` | Obsolete schema-v5 rejection witness. | `253e3977ed0a9447ad9d4bf4df789ac51db7bec07721b51495206b261ba70d4b` |

## Current schema-v7 persistence fixtures

`raster-sample.toniator` and `vector-sample.toniator` are current schema-v7
containers derived from the immutable still-image baselines at their intrinsic
canvases. They retain normalized-fill intent, use the Stage 21A
Density/Density-aspect authority, and carry the post-21A Curve Motif
document-v7 boundary. They are current-format validation inputs, not migration
fixtures.

| File | SHA-256 |
| --- | --- |
| `raster-sample.toniator` | `637bb411ac52cd7a0c8b4f343cccac0f50d54d0202113da7268d1b103a49efdf` |
| `vector-sample.toniator` | `ffe7d23a168eae795760db333ecddfaa1aee9fe84765ac00ba13bc753f1aff81` |

## Stage 10 small-preview regressions

`Reddit.png` and `Reddit.svg` are user-provided small-preview regression inputs,
not replacements for the immutable project-wide baselines above. Keep their
bytes unchanged. `Reddit.png` is 128×128 RGBA with SHA-256
`83842723c8cfdf3bda1a4f76bfcde13175a623123380ce155de932dd319cd185`.
`Reddit.svg` declares 13.509999×13.509999 with viewBox 123.51999×123.51999,
and has SHA-256
`f37963d793f17ca381e7d356ca1a0af1c85c548ccf5522a1c0a425e3b97acb45`.
The accepted decoder resolves that SVG to a 14×14 source identity; tests must
not reinterpret its declared sizing or use font-dependent pixel goldens.

Tests using the SVG must prove that live text is accepted and handled by the
declared text/font policy. Do not use exact text raster pixels as a portable
golden until the test supplies a deterministic font; system font fallback can
vary while the fixture itself remains stable.
