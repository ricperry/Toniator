# Toniator Desktop — native vertical slice

This subtree is the first functional slice of Toniator's Linux desktop rebuild. It is a native Rust application built with GTK 4 and libadwaita; it does not embed a browser or manipulate an SVG DOM for preview rendering. The existing web application remains unchanged beside it as the behavior reference.

User-defined marks use closed cubic Bézier paths. Legacy polygon nodes migrate
without changing their outline: independent outgoing and incoming controls are
placed at one-third and two-thirds of each straight edge. Moving an anchor
carries both controls; either control remains independently editable afterward.

## Build and run on Fedora

Install a Rust toolchain plus the native development libraries:

```sh
sudo dnf install gtk4-devel libadwaita-devel gcc pkgconf-pkg-config
cd desktop
cargo build
cargo run
```

The SVG importer uses the pure-Rust `usvg`, `resvg`, and `tiny-skia` crates, so `librsvg2-devel` is not required. SVG text is rasterized with installed system fonts. When an SVG names a font that is unavailable, Toniator substitutes an installed generic family; exact text metrics can therefore differ from the originating system unless the same fonts are installed.

The bottom-right preview state is a fixed-size, SVG-backed vector `T`: solid for
the source, a calm eased solid-to-halftone animation while the newest rendered
preview is pending, and fully halftoned when that preview is installed. GTK
drives the 1.8-second ping-pong phase because SVG SMIL playback is not reliable
in GTK; both composed layers are parsed and rendered from the named groups in
`assets/preview-indicator.svg`, rather than duplicated as drawing code. The
indicator is tinted from the current theme and honors the desktop reduced-motion
setting. Its tooltip and accessible label track the exact current state. For deterministic review,
`--indicator-state source|active|rendered` freezes the vector state and
`--indicator-report PATH` records its generation, view, phase, layer opacity,
accessibility label, and allocation.

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

```

Any artifact run requesting a preset, screenshot, export, document save, or treatment save deliberately disables real XDG recovery reads, writes, and cleanup. It cannot overwrite a user's recovery snapshot. Plain `--demo` and `--demo-adjusted` launches remain normal recovery-enabled editing sessions when no output flag is present.

Normal launch opens a responsive start view using the embedded Toniator hero artwork. `--demo` opens the built-in artwork with the default Shapes halftone. Keyboard shortcuts are `Ctrl+O`, `Ctrl+S`, `Ctrl+Z`, `Ctrl+Shift+Z`/`Ctrl+Y`, and `Ctrl+E` for the export chooser.

## What works

- native PNG, JPEG, WebP, and SVG import plus drag-and-drop;
- immediate CMYK Shapes or Curves, with a useful result before adjustment; legacy Lines is not offered for new work;
- safe `.tntr` v1 built-in-shape treatment import from the Open menu, inspector, or drag-and-drop; validation runs off the UI thread and applies as one undoable treatment edit without replacing artwork or document identity;
- a Shapes inspector with Circle, 3–6 sided Regular Polygon, and User Defined marks; the cubic Bézier editor supports independent incoming/outgoing handles, anchor dragging that carries its handles, curve-preserving double-click insertion, guarded deletion, keyboard nudging, and Done/Cancel;
- shared or independent Shapes geometry per ink, including simultaneous circles, per-ink 3–6 sided polygons, and cubic user-defined paths; All/C/M/Y/K targeting also covers grid angle, mark angle, width/height scale, color, coverage, threshold, opacity, and detail, with explicit Mixed state;
- source artwork proportions are authoritative for documents, legacy and native presets, preview, Fit, SVG, and PNG; legacy canvas dimensions retain their long-edge scale but can no longer distort the artwork, and custom PNG dimensions remain linked;
- a Source Mapping control with live input-to-output diagrams and plain-language Color-to-CMYK, darkness, crosshatch, and lightness modes; crosshatch is a genuine Curves / Across Page / Straight treatment using one configurable monochrome color and independently editable K/C/M/Y layers at 45°, -45°, 0°, and 90° by default;
- a mode-aware Curves inspector with Straight, Soft Wave, Deep Wave, and Custom profiles; an inline anchor-and-handle editor; shared or per-ink paths; direct point insertion/deletion; and progressive controls for weight, spacing, coverage, angle, position, opacity, threshold, detail, and joined ends;
- repeated curve motifs within the Curves treatment, with automatic canvas coverage or bounded custom rows/columns, motif size, row spacing/stagger, alternate-copy transforms, and optional on-canvas position/angle/spacing handles;
- one format-first Export flow for editable SVG or flattened PNG; PNG supports document size, 2×, linked custom pixel dimensions, and white or genuinely transparent backgrounds;
- one Load Preset menu shared by the inspector and Open menu, with three embedded curated presets, refreshed user presets from `$XDG_DATA_HOME/toniator/presets` (or `~/.local/share/toniator/presets`), and Browse for arbitrary `.tntr` files;
- Save Preset defaults to the native user preset folder, enforces `.tntr`, and derives the internal preset name from the final filename; presets never include artwork or document identity and remain separate from document Open/Save;
- bounded, background raster preview generation with generation-tagged success and errors;
- instantaneous cached Rendered/Source switching and one coherent Fit/minus/slider/editable-percentage/plus zoom control from 5% to 800%; toggling a warmed view schedules no renderer work, settings retain the source cache while invalidating rendered output, and zoom growth refines only the selected missing resolution;
- an inspector whose wheel scrolling works over sliders and numeric entries without changing their values;
- explicit All Inks numeric bases with additive per-ink overrides for both Shapes and Curves; individual ink editing changes only the selected separation;
- gesture-transaction document undo/redo, so one slider drag is one edit;
- versioned `.toniator` JSON with cheaply shared embedded source bytes, atomic save, reopen, dirty state, and debounced XDG-state recovery autosave; document v3 explicitly migrates v1 and v2 files while preserving their renderer and treatment state;
- New/Open/Save header actions and centralized Save / Discard / Cancel protection before New, replace, recover, drag/drop, or close, with synchronous recovery flush on shutdown;
- background, duplicate-guarded SVG and PNG export with filename feedback;
- close inhibition while an export is still running;
- deterministic, editable SVG using multiply-composited, Inkscape-compatible Cyan, Magenta, Yellow, and Black layers, full source artboard dimensions, and no embedded source image.

Artwork and document candidates are decoded off the GTK thread before they may replace the current document. Invalid input therefore leaves the current document and recovery snapshot untouched. Autosave failures are surfaced in the Document section and as an error toast.

The renderer, document model, persistence, preset parser, and SVG serializer are independent from GTK. Raster preview, SVG, and PNG share canonical resolved geometry. `web-shape-v1` stores custom polygon nodes once and shares one resolved node array across every generated mark, including rotated anisotropic marks. The isolated `native-basic-v1` path remains only for inexpensive legacy import/reference coverage. `web-curve-v1` generates editable filled cubic outlines from shared or per-ink paths in full-width or repeated-motif layouts without a browser SVG DOM.

Legacy `.tntr` import accepts useful v1 built-in shapes, full-width curve presets, and repeated curve motifs. Shared rectangle, triangle, pentagon, and hexagon presets normalize to the current editable Regular Polygon model. Applying a preset is one undoable halftone-only edit. Active custom mark paths, unknown geometry, malformed input, and unsupported versions are rejected before document mutation with a “Nothing was changed” explanation. Imported independent geometry remains independently editable, including per-channel polygon sides and cubic paths.

Run verification with:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Intentionally deferred

This is a vertical slice, not feature parity. CPU preview generation can still become expensive at extreme detail on large inputs, and a large synchronous working-document save may briefly pause the UI. No fixed node-count warning is used: total complexity is driven mainly by mark count multiplied by polygon segments and visible inks, so warning policy is deferred until profiling provides a defensible threshold. Arbitrary imported SVG/non-curve motifs, freeform per-copy editing, preset libraries/thumbnails/tags, separate-ink PNG packages, DPI/physical-size workflows, and remaining legacy behaviors are later increments. `.tntr` is preset-only; `.toniator` is the working-document format.
