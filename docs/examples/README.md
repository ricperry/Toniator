# Example output

These files are unmodified exports from Toniator 0.2.0's headless renderer.
They use the default circular-mark Pattern at density 30, density aspect 1,
zero rotation/offsets, and two guard steps. No canvas override is supplied.

| Output | Source | Color mode | Dimensions | Background |
| --- | --- | --- | --- | --- |
| `rgb-dots.png` | `assets/raster-sample.png` | RGB | 1024×1024 | Default black |
| `cmyk-dots.png` | `assets/raster-sample.png` | CMYK | 1024×1024 | Default white |
| `vector-dots.svg` | `assets/vector-sample.svg` | CMYK | 900×620 | Transparent |

The original inputs remain unchanged. These documentation copies come from
`target/validation/release-v0.2.0/`; no post-processing, flattening, or recoloring
is applied. PNG antialiasing uses the default `on`. The SVG source contains live
text, whose appearance can vary with installed fonts; see the
[source artwork notes](../../assets/README.md).

From the repository root after building, reproduce the PNGs with:

```bash
mkdir -p target/validation/examples
for model in rgb cmyk; do
  ./target/release/toniator render \
    -i assets/raster-sample.png -o "target/validation/examples/${model}-dots.png" \
    --channel-model "$model" --density 30 --density-aspect 1 \
    --rotation 0 --offset-x 0 --offset-y 0 --guard-steps 2
done
./target/release/toniator render \
  -i assets/vector-sample.svg -o target/validation/examples/vector-dots.svg \
  --channel-model cmyk --density 30 --density-aspect 1 \
  --rotation 0 --offset-x 0 --offset-y 0 --guard-steps 2
```

For these release examples the command was supplied by the AppImage's `--cli`
entry point, built with the same shared renderer as the native executable.
