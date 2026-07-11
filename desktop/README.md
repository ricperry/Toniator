# Toniator Desktop — native vertical slice

This subtree is the first functional slice of Toniator's Linux desktop rebuild. It is a native Rust application built with GTK 4 and libadwaita; it does not embed a browser or manipulate an SVG DOM for preview rendering. The existing web application remains unchanged beside it as the behavior reference.

## Build and run on Fedora

Install a Rust toolchain plus the native development libraries:

```sh
sudo dnf install gtk4-devel libadwaita-devel gcc pkgconf-pkg-config
cd desktop
cargo build
cargo run
```

The SVG importer uses the pure-Rust `usvg`, `resvg`, and `tiny-skia` crates, so `librsvg2-devel` is not required. SVG text is rasterized with installed system fonts. When an SVG names a font that is unavailable, Toniator substitutes an installed generic family; exact text metrics can therefore differ from the originating system unless the same fonts are installed.

Useful deterministic review hooks:

```sh
# Quiet start window screenshot
cargo run -- --screenshot test-artifacts/start-window.png

# Default editor screenshot
cargo run -- --demo --screenshot test-artifacts/default-window.png

# Adjusted workflow, actual GTK screenshot, working document, and editable SVG
cargo run -- --demo-adjusted \
  --screenshot test-artifacts/adjusted-window.png \
  --export-svg test-artifacts/demo-export.svg \
  --save-document test-artifacts/demo-document.toniator

# Apply a legacy shape preset through the production import path
cargo run -- --preset ../presets/ComicBook.tntr \
  --screenshot test-artifacts/comic-book-window.png \
  --export-svg test-artifacts/comic-book-export.svg \
  --save-document test-artifacts/comic-book-document.toniator

# Paired source evidence through the same imported document state
cargo run -- --preset ../presets/ComicBook.tntr --compare-source \
  --screenshot test-artifacts/comic-book-source-window.png

# Start with the useful native Curves treatment
cargo run -- --demo-curves \
  --screenshot test-artifacts/default-curves-window.png \
  --export-svg test-artifacts/default-curves-export.svg \
  --save-document test-artifacts/default-curves-document.toniator

# Import a legacy full-width curve preset through the production path
cargo run -- --preset "../presets/Skinny Curve.tntr" \
  --screenshot test-artifacts/skinny-curve-window.png \
  --export-svg test-artifacts/skinny-curve-export.svg \
  --save-document test-artifacts/skinny-curve-document.toniator

# Import the legacy repeated-motif stress preset
cargo run -- --preset "../presets/Tiled Stacked Motif Stress Test.tntr" \
  --screenshot test-artifacts/tiled-motif-window.png \
  --export-svg test-artifacts/tiled-motif-export.svg \
  --save-document test-artifacts/tiled-motif-document.toniator

# Capture the single-ink direct arrangement handles
cargo run -- --preset "../presets/Tiled Stacked Motif Stress Test.tntr" \
  --arrange-motif --screenshot test-artifacts/tiled-motif-arrange-window.png

# Export flattened PNG and save the reusable treatment without artwork
cargo run -- --preset "../presets/Tiled Stacked Motif Stress Test.tntr" \
  --screenshot test-artifacts/output-workflow-window.png \
  --export-png test-artifacts/tiled-motif-export.png \
  --save-treatment test-artifacts/tiled-motif-treatment.tntr

# Exact native Lines treatment save/reapply evidence
cargo run -- --demo-adjusted --save-treatment test-artifacts/lines-treatment.tntr
```

Any artifact run requesting a preset, screenshot, export, document save, or treatment save deliberately disables real XDG recovery reads, writes, and cleanup. It cannot overwrite a user's recovery snapshot. Plain `--demo` and `--demo-adjusted` launches remain normal recovery-enabled editing sessions when no output flag is present.

Normal launch opens the quiet start view. `--demo` opens the built-in artwork with the default Classic Dots treatment. Keyboard shortcuts are `Ctrl+O`, `Ctrl+S`, `Ctrl+Z`, `Ctrl+Shift+Z`/`Ctrl+Y`, and `Ctrl+E` for the export chooser.

## What works

- native PNG, JPEG, WebP, and SVG import plus drag-and-drop;
- immediate CMYK Classic Dots, Squares, Lines, or Curves, with a useful result before adjustment;
- safe `.tntr` v1 built-in-shape treatment import from the Open menu, inspector, or drag-and-drop; validation runs off the UI thread and applies as one undoable treatment edit without replacing artwork or document identity;
- a mode-aware compact Web Shape inspector with outcome-oriented color interpretations, shared shape choice, independent ink visibility and edit target, color, coverage, screen angle, faint-mark removal, opacity, and per-ink detail;
- a mode-aware Curves inspector with Straight, Soft Wave, Deep Wave, and Custom profiles; an inline anchor-and-handle editor; shared or per-ink paths; direct point insertion/deletion; and progressive controls for weight, spacing, coverage, angle, position, opacity, threshold, detail, and joined ends;
- repeated curve motifs within the Curves treatment, with automatic canvas coverage or bounded custom rows/columns, motif size, row spacing/stagger, alternate-copy transforms, and optional on-canvas position/angle/spacing handles;
- one format-first Export flow for editable SVG or flattened PNG; PNG supports document size, 2×, linked custom pixel dimensions, and white or genuinely transparent backgrounds;
- reusable treatment save/apply beside the Treatment selector: web-compatible v1 presets carry effective settings plus an exact native extension, while Classic Dots/Squares/Lines use exact native v2 settings; presets never include artwork or document identity;
- bounded, background raster preview generation with generation-tagged success and errors;
- compare source, Fit, Actual Size, and persistent percentage zoom controls;
- gesture-transaction document undo/redo, so one slider drag is one edit;
- versioned `.toniator` JSON with cheaply shared embedded source bytes, atomic save, reopen, dirty state, and debounced XDG-state recovery autosave; document v3 explicitly migrates v1 and v2 files while preserving their renderer and treatment state;
- centralized Save / Discard / Cancel protection before replace, recover, drag/drop, or close, with synchronous recovery flush on shutdown;
- background, duplicate-guarded SVG and PNG export with filename feedback;
- close inhibition while an export is still running;
- deterministic, editable SVG using multiply-composited, Inkscape-compatible Cyan, Magenta, Yellow, and Black layers, full source artboard dimensions, and no embedded source image.

Artwork and document candidates are decoded off the GTK thread before they may replace the current document. Invalid input therefore leaves the current document and recovery snapshot untouched. Autosave failures are surfaced in the Document section and as an error toast.

The renderer, document model, persistence, preset parser, and SVG serializer are independent from GTK. Raster preview and SVG export share canonical resolved geometry. Alongside the isolated original `native-basic-v1` treatment, the tagged `web-shape-v1` treatment supports the five legacy color interpretations, explicit output artboard and long-edge grid, per-ink resolution/visibility/color/opacity/threshold/scale/size/screen rotation/pivot/phase/mark rotation, and shared or independent circle, rectangle, triangle, pentagon, and hexagon geometry. The tagged `web-curve-v1` treatment generates editable filled cubic outlines from shared or per-ink paths in full-width or repeated-motif layouts without a browser SVG DOM. Motifs support exact endpoint chaining, auto/manual coverage, stacking, staggering, and alternating transforms. Source reduction is deterministic bilinear/triangle filtering; browser Canvas reduction and browser SVG arc-length sampling are implementation-dependent, so edge pixels and subpixel curve coordinates can vary slightly between browser engines.

Legacy `.tntr` import accepts v1 shape presets using built-in geometry, full-width curve presets, and repeated curve motifs, including shared and independent cubic paths. Native v2 `.tntr` stores Classic Dots/Squares/Lines settings exactly. Applying either format is one undoable treatment-only edit. Active custom mark paths, unknown geometry, malformed input, and unsupported versions are rejected before document mutation with a “Nothing was changed” explanation. Imported independent built-in shape geometry is rendered and identified, but the compact shape inspector intentionally does not edit per-ink shapes yet.

Run verification with:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Intentionally deferred

This is a vertical slice, not feature parity. CPU preview generation can still become expensive at extreme detail on large inputs, and a large synchronous working-document save may briefly pause the UI. Arbitrary imported SVG/non-curve motifs, freeform per-copy editing, per-ink built-in shape editing, treatment libraries/thumbnails/tags, separate-ink PNG packages, DPI/physical-size workflows, and remaining legacy behaviors are later increments. `.tntr` is treatment-only; `.toniator` is the working-document format.
