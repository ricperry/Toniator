# TON-010 Stage 5 Framework Restart — Substage B parent review

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: `2026-08-01T08:15:20-04:00`
- Git HEAD: `54a8e37d2433781eb4b11f1aa2e4cc989de385be`
- Branch: `TON-010-Stage5-Framework-Restart`
- Producing agent: `desktop_implementer` (`019fbd35-0140-7b51-ae65-fc5bbbe13d0c`)
- Parent review: inspected the `src/render.rs` diff and worker evidence; no
  PNG, SVG, UI, persistence, site-distribution, Voronoi, or Shapes/Curves
  generation changes were included.

## Scope

Substage B corrected canonical semantic-region raster composition. SVG remains
the next bounded substage.

## Verified findings

- The former shared region Pixmap applied `DestinationOut` globally. A
  subtractive region could therefore erase already-rendered sibling channels;
  this is the confirmed raster channel-mixing root cause.
- Semantic region outputs are detected from complete, same-model
  `OutputChannelId` layer identity. Generic unchanneled canonical region
  algebra retains its existing renderer.
- Each semantic channel now receives an isolated antialiased coverage Pixmap;
  positive geometry is painted and genuine subtraction is applied only within
  that channel before color/model composition.
- RGB synthetic regions produce yellow, magenta, cyan, and white additive
  overlaps, independent of incoming layer order.
- CMYK cyan plus magenta retains both ink contributions; a cyan hole reveals
  underlying magenta; incoming order is irrelevant.
- Nonzero-gap coverage leaves the raw-bisector center transparent while visible
  edges retain coverage; zero-gap same-channel adjoining rectangles remain
  continuous.
- Synthetic canonical preview and PNG bytes match, and generic canonical
  subtraction remains covered.
- Transparent canonical output preserves transparent uncovered pixels;
  `white_background = true` composites the completed semantic result over white
  and is opaque. Preview Surface and Export Background remain downstream.

## Commands reported passing

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked render::tests` (40)
- `cargo test --locked png_export` (7)
- `cargo test --locked weighted_voronoi` (7)
- `cargo test --locked pattern::tests` (13)
- `git diff --check`

## Changed files in this substage

- `src/render.rs`
- Worker evidence:
  `.codex-work/agents/desktop-implementer/ton-010-stage5-framework-restart-substage-b-model-aware-raster.md`

## Inference and uncertainty

The canonical raster path now implements the required channel-local model
behavior without a Weighted-Voronoi-only renderer. SVG still needs a
compound-path-per-channel serializer and remains unverified against the
recorded Inkscape/Krita artifacts. No GTK/manual comparison was performed.

## Invalidation conditions

Invalidate this record if `src/render.rs`, canonical region/layer/polarity
definitions, PNG routing, the Weighted producer, SVG serializer, Git HEAD, or
the recorded dirty-worktree assumptions change.
