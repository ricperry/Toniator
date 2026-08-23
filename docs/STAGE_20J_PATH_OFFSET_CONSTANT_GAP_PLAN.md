# Stage 20J — Path Offset and Constant Gap

Status: **Complete at implementation checkpoint
`2edbb8659a82106ce8de904ef1ce9155e3b4d777`**. The user accepted Stage 20J on
2026-08-22. Publication and later Stage 20L+ remain separate.

Stage 20J adds persisted `NormalOffset` guide repetition and one reusable
geometry-owned line/cubic offset service. An authored positive spacing is the
absolute document-space gap between adjacent centerlines. Every side mode
includes the source at index zero; two-sided repetition emits negative offsets,
the source, then positive offsets. Open paths are tangentially extended to the
padded local generation bounds before offsetting. Crossing cleanup is the
single accepted `DissolveCrossings` branch.

The domain stores only structural repetition intent. Patterns plan coverage and
invoke geometry; geometry constructs finite offset centerlines; Stage 20I
remains the only filled-outline authority; renderers consume canonical strokes
without inferring offsets. Stage 20G remains the sole effective-pattern
resolver and Stage 20H remains read-only.

## Domain and geometry contract

`GuideRepetition` gains `NormalOffset { spacing, sides, cleanup }`, with
`OffsetSides::{Left, Right, Both}` and the single
`OffsetCleanup::DissolveCrossings`. Spacing is positive, finite, and measured in
document units. Left offsets use positive signed distances, right offsets use
negative distances, and zero returns the exact source path. A normal-offset
dimension requires zero phase so the source remains index zero. Closed paths
retain authored traversal and winding; sign is never normalized to visual
inside or outside.

Geometry exposes a cancellable `PathOffsetRequest`/`PathOffsetResult` boundary.
Results are either ordered finite `OffsetPathComponent`s or an explicit
collapsed outcome. Each component retains its source interval and deterministic
component ordinal. Lines offset analytically. Cubics use bounded adaptive De
Casteljau subdivision and deterministic cubic fitting under a fixed `1/64`
document-unit error tolerance. Construction preserves source segment order and
emits compact line/cubic geometry rather than point, circle, or polygon swarms.

The accepted join behavior is compact round outer joins with finite inner
tangent intersections and bevel fallback. Offset centerlines add no caps;
Stage 20I supplies the final round stroke caps. Geometry supports preserved
endpoints and tangential extension to generic bounds; patterns always select
the latter with the inverse-transformed padded generation bounds. Wrap-around
endpoints remain later connection-program work.

Stationary points, cusps, and self-crossings are split at deterministic source
locations. Crossing cleanup follows source-parameter traversal, retains edges
consistent with the requested signed side, dissolves reversal loops, and orders
surviving components by earliest source location. Fully removed geometry is a
collapsed result, not a partial success or a clamped distance. Isolated
zero-length segments may be omitted when adjacent geometry supplies continuity;
an entirely stationary path collapses. Non-finite arithmetic, unresolved
normals, invalid continuity, cancellation, or exhausted limits fail atomically.

Derived guide identity retains dimension and signed repetition index and adds a
component ordinal; ordinary and unsplit guides use component zero. This full
identity participates in path sets, nominal-basis lookup, provenance,
fingerprints, diagnostics, canonical strokes, and scene identity.

## Pipeline, persistence, and consumers

Patterns extend the open prototype to the padded local domain, then offset the
original path independently by `index * spacing`; offsets are never generated
iteratively. Each emitted component uses spacing as its Stage 20I nominal
thickness basis. Coverage derives a conservative signed index interval from the
padded domain, source and extension bounds, stroke support, antialias margin,
and guard steps. Collapsed indices never terminate generation early; evaluation
must continue through the planned range and prove that surviving outer
components bracket every requested side or return a stable coverage error.

Normal-offset edits are structural and invalidate `Family`; connected
thickness remains `Realization`. Family/cache identity includes the repetition
discriminant, exact spacing bits, side, cleanup and algorithm contracts,
geometry limits, canvas, and component identity. Cancellation and stale
publication remain failure-atomic. Stage 20I profile sampling and
`build_variable_width_outline_cancellable` realize every surviving centerline;
preview, native RGBA PNG, and live-text SVG consume the same scene and apply the
single existing final canvas clip.

Container version 1, document schema v4, and preset format v2 remain current.
The v4 guide-repetition DTO gains the additive `normal_offset` branch and saves
only spacing, sides, and cleanup. Derived extensions, offset paths, components,
outlines, and caches are never serialized. Existing v4 bytes remain unchanged
when documents are unchanged; v1-v3 remain rejected; mark-only preset-v2 bytes
and behavior remain unchanged.

Request-wide defaults are subdivision depth 48, 262,144 derived offset
segments, 1,048,576 crossing-work items, and 65,536 components. Cancellation
polls during subdivision, intersection discovery, cleanup traversal, component
assembly, and family emission.

No channel offset delta, renderer offset code, preset-name dispatch, adjacency,
connection program, Voronoi/region topology, composite output, parametric
curve, gallery recipe, Pattern Wizard page, GTK reorganization, temporal work,
Legacy work, protected-specification revision, or compatibility decoder belongs
to Stage 20J. GTK work is limited to mechanical exhaustive matching required by
the existing generic editor; it must not create a dedicated workflow.

## Verification and gate

Focused geometry witnesses cover signed line/cubic offsets, zero identity,
error tolerance, joins, endpoint extension, closed winding, zero-length and
stationary input, cusps, crossings, split/collapse ordering, cancellation, and
every limit. Domain/IO witnesses cover validation, typed edits, stale/no-op and
history behavior, descriptor applicability, read-only capabilities,
invalidation, deterministic v4 save/reopen, omission of derived state, unchanged
existing v4 fixtures, v1-v3 rejection, and unchanged preset-v2 bytes.

Patterns/engine witnesses cover absolute gaps, signed/component ordering,
source inclusion, one- and two-sided coverage, independent offsets,
transform-stack regressions, cache misses/reuse, realization-only thickness,
cancellation, stale publication, and Stage 20I outline reuse. Render/CLI
evidence exercises both immutable source artworks at intrinsic dimensions with
a 96-unit gap, a low-complexity centerline diagnostic with roughly 10–16
visible guides, and the divergent-channel Holiday regression. SVG evidence
requires direct filled paths rather than circle/polygon thickness construction,
bounded size/nodes, one final clip, successful XML/Inkscape parsing and
disposable clip-release/export, plus native alpha and hidden-RGB inspection.

Run only Stage 20J and directly relevant Stage 20D/20G–20I tests, affected
package formatting/check/strict Clippy, architecture validation,
`git diff --check`, protected-path and immutable-asset hashes, semantic-map
read-only worktree review, and independent review. If `toniator-app` changes,
run the private Wayland/AT-SPI harness and distinguish automated wlroots
evidence from GNOME/Mutter acceptance.

## Verified implementation and acceptance record

The accepted implementation is recorded at checkpoint
`2edbb8659a82106ce8de904ef1ce9155e3b4d777`, whose parent is the accepted Stage
20I documentation closeout. The user accepted Stage 20J on 2026-08-22 after
the final independent re-review reported no remaining finding.

Focused domain, geometry, patterns, engine, IO, and render witnesses passed,
along with affected-package check and strict production-target Clippy,
formatting, architecture validation, and diff/protected-path checks. The
intrinsic raster and vector witnesses, the compact cubic diagnostic, and the
divergent-channel Holiday regression are under
`target/validation/stage-20j/`. Their SVGs use direct filled paths under one
final clip, parse as XML, and pass headless Inkscape export; disposable cubic
clip release also completed without a crash. Native RGBA and hidden-RGB rules
remain preserved, and the two immutable source artworks remain byte-stable.

The existing generic resource editor exposes the persisted gap, sides, and
cleanup fields without adding a dedicated workflow. A fresh private Sway run
covered effective display, preview update, editor-local undo, keyboard focus,
and cancel with empty Toniator logs. That is automated wlroots evidence, not
manual GNOME Shell/Mutter review. Container version 1, document schema v4, and
preset format v2 remain current; derived offsets, components, outlines, scenes,
and caches remain omitted from persistence.

This acceptance does not authorize publication, Stage 20K implementation,
adjacency, connection programs, regions, composites, Pattern Wizard work,
temporal work, Legacy changes, or protected-specification revision.
