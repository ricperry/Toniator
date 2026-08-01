# TON-010 Stage 5 Framework Restart — Substage C parent review

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: `2026-08-01T08:24:19-04:00`
- Git HEAD: `54a8e37d2433781eb4b11f1aa2e4cc989de385be`
- Branch: `TON-010-Stage5-Framework-Restart`
- Producing agent: `desktop_implementer` (`019fbd35-0140-7b51-ae65-fc5bbbe13d0c`)
- Parent review: inspected the `src/svg_export.rs` diff and worker evidence;
  raster, PNG, UI, persistence, site-distribution, Voronoi, and Shapes/Curves
  generation paths were not changed in this substage.

## Scope

Substage C corrected canonical semantic SVG structure and export-background
routing.

## Verified findings

- A deterministic 48x32 Weighted RGB fixture with six cells per Red, Green,
  and Blue channel previously serialized 18 individual positive cell paths.
  It now serializes 3 named semantic/Inkscape groups and 3 compound positive
  paths containing the same 18 final-cell subpaths.
- Weighted direct-positive SVG has zero masks, no `-region-` cell objects, no
  even-odd cell-sizing paths, and no raw construction geometry. The file is
  2,412 bytes for that fixture; no pre-change byte measurement was materialized.
- CMYK emits four multiply-blended compound paths and zero masks.
- The artboard clip remains on semantic layers because canonical region
  coordinates may legally be out of bounds; it is a page/domain clip, not a
  cell-sizing mask.
- Genuine subtractive canonical geometry retains one layer-local SVG mask and
  can coexist with a compound positive path.
- SVG/raster automated mean channel drift is at most 2.0 on the deterministic
  Weighted RGB fixture; SVG parses with `usvg`.
- Canonical document SVG export now places configured `ExportBackground` in a
  named background layer. Public synthetic canonical helpers remain
  transparent by default, and Preview Surface remains excluded.

## Commands reported passing

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked svg_export` (14)
- `cargo test --locked weighted_voronoi` (7)
- `cargo test --locked render::tests` (40)
- `cargo test --locked png_export` (7)
- `cargo check --locked`
- `git diff --check`

## Changed files in this substage

- `src/svg_export.rs`
- Worker evidence:
  `.codex-work/agents/desktop-implementer/ton-010-stage5-framework-restart-substage-c-canonical-svg.md`

## Inference and uncertainty

The serializer now exposes direct final polygons as editable compound channel
paths and retains only genuine subtraction masks. Manual Inkscape Break Apart,
Krita comparison against supplied reference PNGs, and human GNOME/Wayland
acceptance remain unperformed.

## Invalidation conditions

Invalidate this record if SVG serialization, canonical algebra, raster
composition, Weighted producer, export-background behavior, Git HEAD, or the
recorded dirty-worktree assumptions change.
