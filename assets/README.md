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

## Authorized current-format `.toniator` test case

`HolidayMugs_2024_2025.toniator` is a tracked real-world current-format
`.toniator` test case authorized for current-format validation and round-trip
checks. It is not an immutable migration/source baseline and must not be
regenerated or treated as a byte-frozen v1 fixture without a separate policy
decision.

| File | Role | SHA-256 |
| --- | --- | --- |
| `HolidayMugs_2024_2025.toniator` | Authorized current-format real-world test case. | `541b9c0a1e603e258a10df9be37a5d64b91e0f48736399e30c5d9a3768550c9e` |

Stage 20G ports this current case to container version 1 and document schema
version 4. Its first ordered channel supplies the shared pattern base; later
channels retain exact definition overrides and additive rotation deltas. It is
not a frozen source or migration baseline.

## Current normalized-fill persistence fixtures

`raster-sample.toniator` and `vector-sample.toniator` are current schema-v4
containers derived from the immutable still-image baselines at their intrinsic
canvases. Their per-channel 2.0/9.0 legacy diameters become
0.1414213562373095/0.6363961030678927 fill against their 10×10 representative
nominal cell, preserving average mark diameters. They are current-format
validation inputs, not migration fixtures.

| File | SHA-256 |
| --- | --- |
| `raster-sample.toniator` | `04013b6151ed7c3db8386fe31fadee95773de988731ac491b5cf8f2170527662` |
| `vector-sample.toniator` | `9579c42974d250f41475cffdc1ad0ea22ff364517ed77ed8caa73dc049c8085d` |

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
