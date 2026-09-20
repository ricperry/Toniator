# Artwork and application resources

## App icon and interface artwork

- `icon-final.svg`: approved editable application icon.
- `appicon.svg` and `appicon.png`: package exports; PNG is 512×512 RGBA.
- `Stage21D_Mockup/SplashMockup.png`: compiled welcome-screen artwork.
- `stage20s-preset-icons/`: compiled built-in Pattern gallery icons.
- `stage20s-preset-icon-source.svg`: synthetic source used by gallery thumbnails.

The historical directory names above remain because current resources consume
them. Unused mockups, icon experiments, and font-authoring material are preserved
locally under ignored `ToniatorLegacy/`, not required to build the application.

## Sample artwork and test fixtures

The raster, vector, and video samples can be used to explore Toniator and are
also immutable test inputs. Derived test output belongs under `target/validation/`.

| File | Description | SHA-256 |
| --- | --- | --- |
| `raster-sample.png` | 1024×1024 sRGB RGBA, including transparency. | `324ac232e319002a13fbcfac46538ca5d7e8ba8a127eea2eaf20e8ddb3ed2ef2` |
| `vector-sample.svg` | 900×620 SVG with gradients, paths, transparency, and live text. | `42eb5e23111a5dbad66f2b1802a7cc06391c7ede829b99eb28aeb1ac91596e2e` |
| `video-sample0001-0010.mp4` | Ten 1080×1920 H.264 frames at 6 fps. | `c84d4a42cf62803d41ac35152fd3fea1719a664c633900cb946b9b5a6d6bef81` |

The SVG's live text depends on available fonts. Exact text pixels are not a
portable golden unless the test supplies a deterministic font.

`Reddit.png` and `Reddit.svg` are small-image source-identity regression fixtures.
The `.toniator` files in this directory are retained test inputs, including
historical schema and rejection witnesses; they are not a guarantee that every
fixture opens in the current app. Import the PNG/SVG samples for a fresh project.
The tests reference these files directly, so preserve their bytes.
