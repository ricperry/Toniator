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

Tests using the SVG must prove that live text is accepted and handled by the
declared text/font policy. Do not use exact text raster pixels as a portable
golden until the test supplies a deterministic font; system font fallback can
vary while the fixture itself remains stable.
