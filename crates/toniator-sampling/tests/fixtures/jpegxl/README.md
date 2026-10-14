# Synthetic JPEG XL fixtures

All artwork and generator source in this directory are authored for Toniator and
released under CC0-1.0. No external artwork is redistributed. Tests do not need
an encoder or external media tools to decode these fixtures.

`rgba-still.jxl` is a 3x2 lossless straight-RGBA image, generated with FFmpeg 8.1.3
and system libjxl 0.11.2 (`-c:v libjxl -distance 0`). Pixels in row order:
red/255, green/128, blue/0, (64,128,192)/255, white/64, black/255.
`rgba-animation.jxl` contains three solid 3x2 frames (red/255, green/128,
blue/255), at four frames/second, encoded with `-c:v libjxl_anim -distance 0
-f image2pipe`. It checks raw codestream support; the C-generated files use
JPEG XL containers. Input PNGs and executable are derived validation artifacts.

`generate.c` uses the libjxl 0.11.2 C encoder and Little CMS 2.16 only as offline
fixture tooling. Build command is in its header. It creates:

- `associated-alpha.jxl`: explicit primary associated alpha, rejected clearly.
- `vfr-loop-animation.jxl`: durations 1,2,3 ticks at 4 ticks/second, infinite loop
  tag; decoding remains one finite 1.5-second source pass.
- `gray.jxl`: grayscale samples and no alpha.
- `rgba16.jxl`: lossless 16-bit source equivalent to the still RGB/alpha values.
- `oriented.jxl`: the still pixels with 90-degree orientation (2x3 displayed).
- `reference-layers.jxl`: cropped alpha layers, saved references, and a hidden
  zero-duration intermediate; two visible frames retain the complete blended image.
- `linear-icc.jxl`: embedded linear-light RGB ICC profile with middle-gray RGB
  and straight alpha 128; sRGB output is approximately 188 with unchanged alpha.

Regeneration can vary container metadata/profile timestamps; decoded behavior
is authoritative. Preserve checked-in bytes unless intentionally updating tests.
