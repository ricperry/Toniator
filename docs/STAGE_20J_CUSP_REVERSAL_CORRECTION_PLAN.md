# Stage 20J — Cusp/Reversal Correction

Status: **Complete at correction checkpoint
`f848ff995c9e30f89a85fbc01b5b8d97cc8de3d5`** (2026-08-22). Direct inspection of the
unclipped Inkscape output disproved the prior review claim: after the tangential
endpoint extensions crossed, cleanup removed the extensions but still published
their isolated authored cusp fragments, producing a floating chevron/diamond.
The extended offset envelope now collapses at that crossing without weakening
standalone cusp-fragment isolation. The regenerated clipped and clip-released
outputs were inspected directly and contain one clean terminal cusp with no
floating or re-entering descendants. Stage 20J's original accepted checkpoint
remains preserved; this correction is checkpointed with the accepted Stage 20K
implementation. The user explicitly accepted the correction and Stage 20K
separately on 2026-08-22; publication remains separate.

## Objective and authority

Correct the reusable geometry-owned path-offset service so cubic offsets
isolate cusps and omit reversal geometry instead of publishing folded middle
intervals. Valid source-consistent fragments survive as ordered components while
their requested endpoint envelope remains non-exhausted.
The correction belongs in geometry and its existing patterns/engine consumers:
renderers do not clip or repair it, and a cusp does not discard a whole
repetition.

Document schema v4, preset format v2, document commands, Stage 20G effective
authority, Stage 20I outline construction, and the SVG/PNG consumer boundary
remain unchanged. Protected specifications, `ToniatorLegacy/`, GTK, later
stages, compatibility adapters, and publication are excluded.

## Geometry contract

For each cubic source interval, isolate the signed offset-orientation numerator

```text
g(t) = |v(t)|^3 - distance * cross(v(t), a(t))
```

where `v` and `a` are the source curve velocity and acceleration. `g > 0`
retains authored traversal; `g < 0` is reversal geometry and is discarded.
Intervals containing zero are isolated dyadically using conservative
velocity/acceleration bounds. Subdivision stops only when the uncertain
offset-locus band is within the existing `1/64` geometry tolerance; that tiny
band is omitted so every published endpoint remains nonstationary. Exhausted
depth, cusp-isolation work, cancellation, unresolved classification, or
non-finite arithmetic fails the request atomically.

`PathOffsetLimits` gains a request-wide cusp-isolation work limit whose fixed
default is `262_144`. Exhaustion has a distinct stable diagnostic. Geometry
exports a versioned `PATH_OFFSET_ALGORITHM_CONTRACT_ID`; the algorithm contract
and every fixed default limit participate in normal-offset family/cache
identity.

Private construction produces ordered offset runs. Only genuinely adjacent
retained source intervals join; discarded cusp/reversal intervals are never
bridged. Each run passes independently through the existing crossing cleanup,
retains ordered source-interval provenance, and receives component ordinals in
source order. Closed paths without a break preserve existing closure and
winding. When cleanup creates breaks, first and last runs merge across the
authored seam only if they remain adjacent there, and all resulting pieces are
published as open components.

Tangential extension defines one finite extended offset envelope, not separate
permission to publish an endpoint fragment after its construction ray is no
longer valid. When the offset start and end extensions cross each other, the
extended envelope has exhausted and the request collapses. Cleanup must publish
neither the exterior extension remnants nor the isolated authored fragments
between those crossed terminal extensions. `Preserve` mode remains independent:
its source-consistent cusp fragments survive without an extension envelope.

Normal-offset coverage accepts authoritative monotonically ordered source
interval gaps produced by cusp cleanup. It continues to reject missing authored
endpoints, overlaps, reversed ordering, insufficient projection, and missing
requested-side coverage.

## Focused verification

Geometry witnesses cover the diagnostic cubic below its `162.5625` minimum
curvature-radius threshold, through the standalone cusp range at distances
`168` and `216`, and beyond its approximately `224.078` endpoint curvature
radius at distances `228`, `240`, and `252`. In `Preserve` mode, reversed middle
intervals disappear while two ordered authored fragments survive with regular
tangents. With tangential extension, distance `168` still publishes two regular
branches, while distance `180` and farther collapse when both terminal
extensions cross; neither exterior extension remnants nor floating authored
fragments publish. The mirrored curve with mirrored signed distance covers the
other side. Existing line offsets, zero identity, ordinary crossings, closed
winding, component ordering, cancellation, and failure-atomic depth/work limits
remain green.

Patterns/engine witnesses cover the 320×320 spacing-12 diagnostic: repetition
14 at distance `168` publishes the last two extended, source-consistent
components; repetition 15 at distance `180` and farther publish nothing after
terminal-extension collapse and never form a floating chevron/diamond or
re-enter the canvas. Coverage accepts that authoritative terminal collapse as
well as legitimate cusp gaps while rejecting overlap, disorder, missing source
identity, insufficient bracketing, and requested-side gaps. Re-run Stage 20J
cache/stale-publication witnesses and the directly relevant Stage 20K
round/square `NormalOffset`, path-neutral identity, equal-arc continuity, and
render tests.

The intent-only diagnostic `.toniator` must retain byte-identical
save/reopen/save output. Regenerate `normal-offset-{raster,vector,cubic}` and
`holiday-divergent` PNG/SVG evidence under
`target/validation/stage-20j/`. Inspect native PNG alpha/RGB and SVG structure.
The unclipped Inkscape render is mandatory review evidence: cusp branches may
terminate separately, but no path may reconnect them through a folded/reversed
middle, re-enter from outside, or remain as a floating chevron/diamond after
the terminal extensions cross. Recheck XML and Inkscape opening, disposable
clip release, one final canvas clip, compact direct paths, immutable asset
hashes, and unchanged Stage 20K artifacts.

Run only focused Stage 20J geometry, patterns, engine, IO, and render tests plus
directly relevant Stage 20K parametric tests. Finish with affected production
check and strict Clippy, format check, architecture validation,
`git diff --check`, protected-path audit, read-only semantic-map worktree
review, checkout-aware evidence update, and an independent geometry-focused
review.

## Gate

This correction and Stage 20K are accepted and checkpointed at
`f848ff995c9e30f89a85fbc01b5b8d97cc8de3d5`. Do not push, publish, or begin
Stage 20L without separate authorization.
