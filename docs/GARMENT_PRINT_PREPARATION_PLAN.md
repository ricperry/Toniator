# Draft — Garment Print Preparation

**Status: G0 headless evidence and measurement contract accepted as the
implementation baseline; G1a and G1b accepted and included in the
2026-10-04 work-in-progress checkpoint; G2a is included as implemented and
awaiting user review, not accepted. G2b visible UI, G2c GUI evidence, and G2R cleanup
remain separately gated.** The uncertainty band remains provisional. GTK verification stays
separately pending. G1a added settings, persistence, validation, Undo/Redo, and
compatibility; G1b adds the headless detector and CLI only. Cleanup remains
separately gated as G2R.

This plan is deliberately included in branch
`checkpoint/garment-preflight-g2a-20261004`, based on `a980005`. The checkpoint
preserves work for review; it does not approve later stages, GUI behavior, or
physical printing. References below to `target/validation/` are local evidence
archives, not distributed files. The broad local progress ledger and development
guidance remain outside the public tree.

## Checkpoint verification — 2026-10-04

Fresh bounded checks passed for this WIP checkpoint: affected domain, IO,
engine, CLI, and app compilation with `cargo check --locked`; strict production
Clippy (`--lib --bins -- -D warnings`); touched-file rustfmt; architecture
validation; and `git diff --check`.

The 36 passing focused tests comprise domain print intent (4), IO garment
persistence (3), engine preflight integration (5), render/cache parity (1), CLI
preflight (6), app print-preparation controller (9), engine detector units (5),
exact report/raster pair matrix (1), source-free Preset authority (1), and app
exact-content dirty tracking (1). Tests that regenerate the earlier stage's
native artifact directories were not rerun; their existing evidence and limits
remain recorded below. No product Rust was edited during checkpointing.

This verification does not accept G2a or establish GTK controls/chooser behavior,
GNOME/Mutter behavior, heavy-decode cancellation or stress performance, or
physical print quality. TON-013 is recorded only; its SVG fix remains deferred.

## Goal and limits

Toniator remains a general creative halftone tool; garment preparation is an
optional advisory use. It can report physical size/PPI, alpha coverage, isolated
marks, narrow positive features/gaps, and transparency/color context. Keep
editable art and ordinary render/export output authoritative.

This is not a print-safe certificate, supplier template, underbase simulator,
fabric proof, or prediction of adhesion, hand, wash life, or final color. A
clean report never guarantees a good print; confidence requires the exact
product, print area, process, and, where needed, a physical sample.

No supplier SKU or placement is selected. Start product-neutral, use user-entered
physical dimensions, and assume no universal DPI or minimum feature size.

## Evidence and confidence

### G0 baseline evidence — 2026-10-03

- The checkout is `a980005979d530e5caa89df83d5e3ed131ca2a9f` (`main` at
  `origin/main`); the unrelated user-owned Krita autosave remains untouched.
- At the G0 baseline, persistence was document schema 10/container 2, with
  unknown document fields rejected. G1a's current read/write behavior is
  recorded below; no general historical migration is added.
- `CanvasSpec` currently stores document-space width and height without a
  physical unit. The renderer converts its linear result to straight sRGBA;
  the current PNG call supplies RGBA8 pixels but no ICC profile or physical
  resolution metadata. Source raster decode reaches `to_rgba8()` without a
  Toniator-owned ICC conversion policy. Missing PNG metadata alone does not
  show that the pixel values are incorrectly encoded.
- The app and CLI target builds passed. The G0 report records 64 current
  CLI/output-target commands, 220 assertions, 16 separate contract-probe
  assertions, two existing focused renderer color tests, and the exact command
  records and artifacts. The tiny fixture covers RGB, CMYK, and Source color +
  alpha, both AA settings, and 1×/2× output targets. Direct outputs matched
  CLI renders from newly saved/reopened projects for the fade, both immutable
  assets, and all three tiny-fixture models. Both asset hashes match
  `assets/README.md`. This run used Fedora 44,
  rustc 1.94.1, and the current CLI 0.3.3; it is not an independent Debian
  13.6 reproduction.
- The detailed G0 report is
  [garment-g0-20261003/report.md](../target/validation/garment-g0-20261003/report.md).
  It separates current output from generated source fixtures, test masks, and
  review derivatives. It does not establish detector accuracy or print safety.
- These focused builds and tests do not establish broad memory, cancellation,
  performance, platform, packaging, or compositor behavior. Packaging and
  font-resolution reports remain separate from this feature.
- Initial source fixtures are in
  `target/validation/garment-preflight-baseline-20261003/`; their manifest is
  the inventory authority. The final G0 outputs and analytic probes are separate
  artifacts under `target/validation/garment-g0-20261003/`.
- A private Wayland session start was automatically rejected before launch:
  “approval required by policy, but AskForApproval is set to Never.” No bypass
  or retry was attempted. GTK behavior remains unverified and this denial does
  not block the authorized headless G0 deliverable. Prior screenshots are
  historical; no current GUI claim is made.

### G1a implementation — 2026-10-03

- The domain stores an optional complete physical size in canonical mm and two
  independent nonnegative thresholds for positive-feature width and negative-gap
  width. Both default to zero (disabled); no minimum PPI or feature-size value is
  invented. Unknown placement leaves PPI unavailable while thresholds remain
  storable. Inch/mm conversion helpers preserve the canonical physical value;
  no UI entry control was added.
- `SetPrintPreparation` is a history-only document edit with exact-base checking,
  revision advancement, undo/redo, stale-token rejection, and no render
  invalidation. Existing full-document dirty tracking includes the settings.
  Source-free Document Presets omit this project intent and preserve the
  destination's intent when applied.
- Projects write container 2/document schema 11 with a required, strict
  `print_preparation` object. The reader accepts strict schema 10 with default
  intent. Opening does not rewrite the source file; saving writes schema 11, which
  the pre-G1a v0.3.3 application cannot read. Document Preset format 3 remains
  frozen at configuration schema 10; Pattern format 5 is unchanged.
- Ten focused tests passed. Scoped checks, formatting, architecture validation,
  and export parity passed; six PNG outputs and both available SVG outputs were
  byte-identical to pre-change outputs. A broader all-targets Clippy command hit
  an unrelated older domain lib-test initializer missing `RandomSites.refinement`;
  strict library/binary Clippy passed. Exact commands and the error location are
  in the [G1a report](../target/validation/garment-g1a-20261003/report.md).
- The user accepted G1a; it is included in this WIP checkpoint. G1a added no
  detector, GTK control, cleanup, or physical print proof.

### G1b implementation — 2026-10-03

- The project-only CLI loads through the authoritative project reader and
  analyzes the selected current frame's final transparent render. Physical
  dimensions and threshold overrides are session-local and never saved.
  Versioned JSON records the selected export backing as metadata; alpha analysis
  remains transparent. G1b adds no document schema or GUI.
- Final alpha support uses alpha ≥1; the alpha ≥128 core is a stronger-coverage
  reference, not an opacity threshold. Eight-connected support inventory runs
  even when both width checks are disabled. Width and gap candidates carry exact
  pixel runs and advisory/unavailable states. Boundary association, unresolved
  low coverage, sampling limits, and cancellation/stale-result outcomes remain
  explicit; no result certifies print safety.
- The engine and CLI add bounded detector work and result sizes. Active
  cancellation during heavy work, stress performance, GTK behavior, supplier
  guidance, and physical print proof remain unverified. The full report records
  the normalized stencil rule, default limits, exact JSON identity, commands,
  review derivatives, raw outputs, and matrix coverage:
  [G1b report](../target/validation/garment-g1b-20261003/report.md).
- Six asset/model projects were inventoried at native AA-on/off and 2× AA-on
  targets; this matrix validates inventory and target identity, not morphology
  across every model and scale. The focused tiny analytic fixture exercises
  positive/negative candidates. Native transparent PNG bytes matched before and
  after preflight in the tested CLI build.
- The user accepted G1b on 2026-10-04; it is included in this WIP checkpoint.
  No G2 desktop UI, cleanup, artistic modification, or print-safety certification
  is included.

### G2a implementation — 2026-10-04

- G2a is implemented and awaiting user review; it is checkpointed as WIP, not
  accepted. The additive engine API returns the exact measured transparent raster and report as one immutable pair; the existing report-only API and CLI
  JSON remain unchanged.
- The app captures one live `DocumentSession`, sources, frame, workspace and
  lifecycle generations before the existing save chooser. PNG options, options
  defaults, worker evaluation, and actual PNG/SVG export use that chooser-time
  capture. Later same-workspace edits do not substitute a new document/frame
  into only one side; workspace/lifecycle replacement rejects old callbacks.
  Runtime target/AA/backing selections are shared by analysis and export, while
  Automatic-versus-explicit backing intent remains distinct from its effective
  background. Ordinary export remains available without a report and preserves
  existing default pixels.
- The widget-free app controller serializes one active worker and latest queued
  successor, checks source freshness off GTK and current tokens on the main
  thread, and clears stale pairs immediately. Physical intent uses the existing
  typed `SetPrintPreparation` history command; Apply/Undo/Redo retires preflight
  while preserving valid render caches. Over-limit results are Unavailable at
  the selected target with no downsampling. No schema/container change, visible
  controls, or dialog rearrangement was added.
- Focused tests passed: three engine pair tests, nine controller tests, and one
  actual app-export-worker capture test. The report records engine/app/CLI
  compilation, strict production Clippy, and exact commands. Strict app test-
  target Clippy still reports two pre-existing warnings in untouched files; a
  rerun allowing only those two lints passed. Native GTK role/readback/dialog
  appearance, real chooser timing, heavy-decode cancellation/latency, and
  physical print behavior remain unverified. The prior GTK launch was not
  retried.
- See the [G2a evidence report](../target/validation/garment-g2a-20261004/report.md)
  and its [permission diagnosis](../target/validation/garment-g2a-20261004/permission-diagnosis.md),
  including the rejected XML recount; denied reads were not retried.

### User reports from Debian v0.3.3 — not reproduced this turn

The user reports successful import, edit/undo, reopen, and 2× output; no bleed
across 18 hidden-RGB cases; pale edges after visible RGB was matted toward white
while partial alpha remained; jagged edges and some fragile tiny marks with AA
off; 2×2 details surviving both AA settings; minimum fill 0.2 adding 8.4%
coverage without size-based removal; a white CMYK PNG export-dialog default; and
missing PNG profile/resolution metadata. Keep these distinct from this turn's
headless results: the generated 18 hidden-RGB outputs matched byte-for-byte,
and the tiny fixture's 0→0.2 coverage increase was 29.3%, not 8.4%. The evidence
does not reproduce the user's former GTK workflow.

Do not turn the reports into blanket hidden-RGB, alpha, color, or export bugs.
Zero-alpha hidden RGB, partially transparent RGB that was already blended
toward white, visible edge coverage, and antialiasing are different cases.
TON-001 and TON-003 remain separate open reports; this proposal does not close
or claim to repair them. TON-008 is resolved in the issue ledger and unrelated.
TON-010 stays paused. Packaging and font-resolution reports also remain separate.

### Supplier guidance checked 2026-10-03

Printful gives a product-dependent general range of 150–300 DPI, recommends an
embedded sRGB IEC61966-2.1 profile, and directs artists to the exact product
file guidelines. Its DTFlex guide warns about small dots, narrow lines, soft
edges, and semi-transparent effects; its DTG guidance describes transparency
behavior on dark fabric. White-underbase use depends on design, product, and
printing technique and may be determined automatically. These are useful
examples of why the app needs user-entered dimensions and qualified warnings;
they are not Toniator-wide defaults or universal thresholds.

## Proposed product and authority boundaries

### Physical intent and resolution

Store the optional complete print width/height in canonical millimetres through
document commands and history. Existing `CanvasSpec` is unitless; never infer
millimetres. Unknown size makes PPI unavailable; thresholds remain independent
and storable. Zero disables each width threshold, and Toniator sets no minimum
PPI or feature-size value. The headless domain provides checked millimetre/inch
conversion helpers; no UI entry/display control is implemented. Opening schema
10 supplies default intent without rewriting the file; saving writes schema 11.
The pre-G1a v0.3.3 application cannot read schema 11.

Use the complete exported canvas, including intentional empty padding, for the
physical-size calculation. Report occupied-artwork bounds separately. For the
selected final output dimensions `Nₓ × Nᵧ` and print dimensions `Wₘₘ × Hₘₘ`:

```text
inches = millimetres / 25.4
PPIₓ   = Nₓ × 25.4 / Wₘₘ
PPIᵧ   = Nᵧ × 25.4 / Hₘₘ
```

Print placement dimensions remain fixed when output resolution or export scale
changes. More output pixels increase effective PPI; they do not enlarge the
printed artwork. If dimensions are unknown, neither printed size nor PPI is
available.

Use actual final PNG dimensions after the chosen output scale. Report unequal
axis densities instead of averaging away anisotropy. A threshold of zero
disables its corresponding warning; all checks start disabled until the user
chooses values. No safe size, PPI, dot, stroke, gap, or alpha threshold is
invented by Toniator.

### Transparency, color, and presentation

Analyze the final transparent artwork composition at its selected output size
and antialiasing setting before any opaque PNG backing is applied. Separately
state when the chosen export background will flatten transparency. Respect the
Addendum's model-sensitive PNG background defaults and explicit user override.
Preview-only light, dark, or custom garment-color backdrops must not alter the
document, canonical render, or exported pixels.

Report source-profile provenance distinctly from encoded pixel values:
verified sRGB, untagged/assumed sRGB, or unsupported/unknown profile where the
decoder can establish that distinction. Later output tagging may claim sRGB
only when the output bytes are sRGB encoded. Embedding a truthful profile and
physical-resolution metadata is a separate export stage; it must not silently
convert or retag source colors. ICC conversion and soft proofing remain out of
scope unless separately specified.

Coverage analysis is based on visible final-composite alpha. RGB inspection is
a separate color/transparency diagnostic; this proposal does not promise a
color-detail detector, including for fine color changes inside opaque regions.
Per-channel or layer diagnostics may be useful, but must be labeled separately
from final-composite findings and as contributions rather than ink separations:
Toniator composition and occlusion can hide channel contributions; actual printer
separations and underbase remain supplier-owned.

### G0 measurement contract baseline — uncertainty remains provisional

The user accepted these measurements as the G1a implementation baseline. The
uncertainty band remains provisional; these measurements are not calibrated
print thresholds, a detector implementation, or print-safety proof.
The exact fixtures, probe equations, command outputs, and limitations are in
the [G0 report](../target/validation/garment-g0-20261003/report.md).

1. For final transparent RGBA8 pixels, define support `S = alpha >= 1` and
   high-coverage reference core `C = alpha >= 128`. Alpha 1–127 is low coverage;
   alpha 1–254 is fractional coverage, so `C` does not mean opaque. Inventory
   every 8-connected `S` component with extent/location and alpha counts. A
   component with no `C` pixels is an isolated low-coverage candidate/unresolved
   regardless of size; never discard it or turn it into “no findings.”
   Apply these thresholds to final rendered/output alpha. Source alpha in
   Source color + alpha maps to mark size and is not a direct final-mask oracle.
2. In a component that contains `C`, classify `S \ C` separately. The rendered
   opaque-AA control had 40 lower-alpha pixels within a provisional two-output-
   pixel Chebyshev band; an attached faint-tail case had 44 lower-alpha pixels
   outside it. Inside-band coverage is boundary-associated, not proven AA;
   farther attached coverage is unresolved, not automatically an AA fringe.
   The band is not calibrated. The alpha-64 and alpha-1 output cases each had
   one support component and no core, establishing why a core-only inventory
   would lose actual faint output marks.
3. Analyze positive features on both `S` and `C`, and negative gaps separately
   on 4-connected background with enough padding to exclude remote exterior.
   For output pitches `dx = Wmm/Nx`, `dy = Hmm/Ny`, and user limit `T`, the
   artifact probe samples `B(T/2) = { (i,j) : (i·dx)^2 + (j·dy)^2 <= (T/2)^2 }`
   inclusively. In pixel coordinates this physical disk is elliptical under
   anisotropic scaling. For each foreground `F`, `E = erode(F,B)`,
   `O = dilate(E,B) ∩ F`, and residual `R = F \ O`. `R` is probe-sensitive
   material, including corners/tips, not exact width. A component with no `E`
   center is a width-limited candidate; two or more `E` components within one
   original component suggest a constriction. These are advisory outcomes.
4. Canvas-touching features and gaps remain boundary candidates because raster
   output does not prove authored intent or clipping provenance. Under-pixel
   features, and measurements inside an uncertainty band, remain unresolved;
   surviving the probe is never a pass. In the 1 mm probe, an actual 0.8 mm
   vertical strip survived at one sampled pixel phase and disappeared at others.
   The script's `1e-12 mm²` disk-inclusion epsilon and the two-pixel band are
   experiment details, not calibrated universal tolerances or guarantees.
   Never report “print safe” or infer no findings from unresolved cases.
5. A zero threshold bypasses only its corresponding warning. Keep the full
   support inventory when sensitivity changes. Tie later reports to document
   and revision, source/frame, visibility/opacity, output target/scale/AA,
   placement, thresholds, alpha policy, and algorithm identity. Cancellation,
   stale-result rejection, and checked mask/label/distance/work budgets remain
   future implementation requirements. No product detector exists in G0.

Every result is tied to document ID/revision, source identity, still/current
frame identity, visibility and opacity snapshot, output target/scale and
antialiasing, placement, thresholds and alpha policy, and algorithm identity.
Cancelled, stale, or mismatched results never appear current. This plan covers
the current still frame only; it promises no temporal batch preflight.

Bound masks, labels, distance buffers, result lists, and total work with checked
arithmetic and explicit memory/work limits, not only the renderer's pixel cap.
Poll cancellation at bounded intervals. Exceeding a limit yields a clear
unavailable result, never a partial “no findings” result.

## G2a authorized implementation contract

G2a is implemented and awaiting user review, checkpointed as WIP, not accepted. It
adds only internal shared
export-selection/settings, undoable physical-intent integration, and one
cancellable/freshness-checked evaluation that returns the exact transparent
raster/report pair. Keep the existing chooser → PNG options order and add no
visible controls or dialog rearrangement; those belong to unapproved G2b.

At `choose_still_export` entry, capture one coherent live document session,
sources, frame, workspace/lifecycle generation, revision, and applied physical
intent before opening the existing save chooser. Resolve the PNG-options canvas
and model-dependent defaults from this capture, and carry the same document
snapshot through options, analysis, and export. Runtime target, AA, and backing
selections made in PNG options must be shared by analysis and export. A later
same-workspace edit or frame change never substitutes newer live content into
only one side of the operation; workspace/lifecycle replacement rejects its
callbacks. Use the existing G1a `SetPrintPreparation` history authority for
undoable physical intent and its freshness; add no document schema or container
change.

Analyze the final transparent composite across channels, not each channel in
isolation. A faint contribution that overlaps a substantial mark from another
channel is not an isolated mark merely because it is faint in its own channel.
The report and analyzed raster must have exact RGBA identity. Transparent PNG
output must retain parity with the existing transparent renderer; white- and
black-backed PNGs must separately match their corresponding current renderer
outputs. View backdrops cannot affect analysis or exported pixels. A report is
optional: ordinary export works without one, findings never gate export, and
the existing default output pixels remain unchanged.

Cancel or reject superseded/stale evaluation, clear its report/highlights, and
cover cancellation, supersession, document/frame changes, and physical-intent
edit/Undo/Redo freshness. If the selected target exceeds a detector or
retained-result budget, report Unavailable at that target and never downsample
analysis or change the export target. The three engine, nine controller, and
one app-export tests passed the exact-pair, final-composite overlap,
transparent/separately backed output parity, backdrop-invariance,
cancellation/supersession/freshness, and oversized-unavailability checks. G2a does not
launch GTK, retry the rejected GTK workflow, read model profiles, or change
profile configuration. The rejected XML recount is recorded in the permission
diagnosis; it was not retried, and no new call was made.
The separately deferred [TON-013 SVG radius issue](../ISSUES.md#ton-013--svg-export-emits-zero-radius-circles-and-rounds-tiny-positive-radii-to-zero)
receives no fix in G2a.

## G2 proposal: optional first preflight review

This remains the proposed G2b desktop workflow; it is not authorized by the
G2a approval. G2b should add an optional **Review print preparation** action in
the shared PNG options window, not an automatic gate on every export. It may
move the save chooser until the user chooses Export, so review can happen before
a destination is selected and no filename or nested modal is needed. If G2b
changes that order, capture one coherent document/session/source/frame/workspace
snapshot at Export click before opening the chooser and use it for both report
and export. G2a keeps the existing chooser-first order and captures at
`choose_still_export` entry.

The main inspector has a collapsed Print preparation summary and optional
Review… action that opens the existing PNG options window with review expanded;
the same Review… action inside PNG options expands its review area in place.
Its single physical-settings form edits the existing complete-canvas
width/height and independent positive-feature/gap thresholds, with mm/in entry
units and zero meaning disabled. Both physical fields blank means Unknown; one
blank is invalid. Validate the complete draft before an explicit Apply. Check
and Refresh consume applied project settings only; an unapplied draft never
changes a report. Apply uses the typed
`SetPrintPreparation` history command and remains a project edit even if the
window is later closed or export is cancelled; Undo/Redo remains available.
Closing with an unapplied draft discards it. Unit display, preview backdrop,
output size, AA, and export background are runtime selections, not project
fields. Always show the actual final `Nₓ × Nᵧ` target; unknown physical size
makes only physical dimensions/PPI unavailable, never the known pixel target.
Show physical width/height and PPI for both axes when available. Reuse the
existing checked canvas-size and export-selection authorities. Unknown
physical size still permits pixel dimensions and alpha inventory. If the
selected final target exceeds the detector or retained-UI budget, show
Unavailable and preserve that selected export target; never downsample analysis
silently or present a smaller-target result as applying to a larger export.

Preview backdrop and PNG export background are separate. Preview Automatic
follows the normative model-sensitive viewer default: RGB black, CMYK white,
and Source color + alpha neutral mid-gray. Offer Checkerboard, Light, Dark, and
Garment as view-only choices without changing pixels or invalidating analysis;
Garment can use a simple accessible hex color entry with a swatch. Export
remains available when review is not checked, stale, unavailable, or advisory;
findings never gate export, and export never consumes a stale report. Existing
export-option validation continues to reject invalid output settings.
Export Automatic independently retains the normative model-sensitive export
default: RGB black, CMYK white, and Source color + alpha transparent. Show the
effective export background and whether it is automatic or explicit; state
when an opaque backing will flatten transparent output. A garment color is a
viewer color, not fabric, underbase, adhesive, or CMYK simulation. Never change
the export background silently.

The G2b review is for the exact full-canvas transparent final composition at the
selected target and AA setting. It must use the report and transparent raster
from one engine evaluation, not crop the main viewport or launch a second
analysis. The current CLI publishes report-only results; propose a narrow
additive engine adapter returning an immutable raster/report pair while keeping
the CLI report-only API and accepted detector semantics unchanged. Follow the
existing app-owned source-worker pattern rather than introducing a scheduler.
Capture the live document session and its revision, source/frame identity,
complete target, AA, backing selection, and applied print settings. Do not
reconstruct the session from saved bytes. Perform decode/evaluation away from
GTK; check the report identity's `is_current` against its captured live
authority before posting the result. Admit it on the main thread only when its
workspace, surface/request epoch, document token, and complete runtime
selection still match. Permit one bounded
job at a time; provide Refresh and Cancel, and cancel/drop work on window close
or workspace change. A document/source/history/frame/target/AA/backing/intent
change cancels the job and immediately hides its raster, highlights, and
current-finding navigation. Show **Out of date** with Refresh; never draw old
geometry over current artwork. Retain category filters only; clear selected
finding IDs and disable report actions until a fresh result arrives. Backdrop,
zoom, and list filtering only repaint. G1a print-intent commands do not
invalidate rendered art, but they must invalidate preflight and retire an old
report token while preserving valid render/cache state. A pending preview for
the old token must not be admitted; a new request can reuse a matching render
cache entry where valid. Bound the retained review raster, masks, and any
`GdkTexture` memory independently from engine detector budgets, and release
them when stale, closed, or replaced.

Present only final-composition alpha findings; do not add a per-channel pass or
label channel contributions as printer separations. Keep support inventory,
boundary-associated partial coverage, and unresolved faint marks distinct.
Alpha-1/64 support and core-free components stay visible even with both width
thresholds zero; attached tails outside the provisional two-pixel band remain
unresolved, and inside-band coverage is not proven antialiasing. Show exact
pixel runs, high-contrast highlights, navigable locations, and record counts
without implying unique physical defects. Provide non-color cues, keyboard
selection/navigation, and a highlight toggle; never increase artwork opacity
or modify source pixels. Use truthful states such as Not checked, Running,
Current advisory, Out of date, Cancelled, Unavailable, Unknown size, Width
check disabled, and Sampling limited. “No candidates found” applies only to
completed checks and never hides unresolved support. Do not use a
safe/pass/clean badge.

Proposed bounded increments are: **G2a**, the immutable report/raster-pair
contract, shared settings authority, worker capture/cancellation, and stale
admission; **G2b**, the complete first usable review window with advisory list,
exact highlights, backdrop controls, export transparency status, and keyboard
and accessible control behavior; **G2c**, scoped GUI and end-to-end acceptance
evidence, only after that verification is separately authorized. Do not retry
or bypass the prior GTK launch rejection. G2a is not a
first-usable-UI acceptance. Preserve schema 11 and the schema-10 read behavior;
do not add persistence fields for runtime presentation or selections.

Later G2b/G2c acceptance should cover exact report/raster alpha identity and exported
pixel parity for native and 2× targets, both AA modes, and custom target; the
accepted alpha-1/64, intentional-white, partial-fringe, unresolved-tail,
tiny-dot, neck/gap, and clipping cases; both immutable assets across all three
models; and races from edit, Undo/Redo, source/frame/workspace/window, target,
AA, backing, print intent, and export selection changes. Verify physical-entry
validation, Apply/discard, persistence,
Undo/Redo, and schema-10/current-11 reopen without schema changes. Backdrop,
selection, and highlights must not alter exported bytes or add overlay pixels.
Exercise bounded engine work and separately bounded retained UI memory,
cancellation without GTK blocking, and the stale policy above. Audit each named
control: inspector Review; physical W/H with mm/in, positive/gap thresholds and
Apply; PNG size/custom size, AA, and export backing; Check, Refresh, and Cancel;
finding categories/list, highlight toggle, and Zoom to location; preview
backdrop and garment hex color/swatch; Export and Close. For each, record the
actual GTK role, state/value, actions, label relation, truthful enablement,
keyboard path, semantic readback, and screenshot/log evidence; do not assume or
invent native roles before inspection. Automated Sway/wlroots
evidence does not claim GNOME/Mutter acceptance; physical-print proof remains a
separate product-specific gate. Existing implementation surfaces to inspect
when authorized are `crates/toniator-app/src/components.rs` (inspector summary),
`crates/toniator-app/src/main.rs` (PNG options, typed history commands, preview
acceptance, and workspace generation),
`crates/toniator-app/src/temporal_preview.rs` (worker precedent), and
`crates/toniator-app/resources/png-export-options.blp`. No mockup image/PDF is
available in this checkout; this proposal is text/source grounded, not a visual
mockup review.

## Proposed staged delivery

Priority 1 is G0 through G2; G3a and G3b are Priority 2. The user accepted G0
headless evidence and the measurement contract as the implementation baseline;
the uncertainty band remains provisional. G1a is accepted and included in this WIP checkpoint.
G1b was accepted on 2026-10-04 and is included in this WIP checkpoint. G2a is implemented,
awaiting user review, checkpointed as WIP, not accepted; G2b, G2c, G2R, and later
stages remain separately gated.

| Stage | Scope and stop condition |
| --- | --- |
| **G0 — Headless baseline and proposed metric contract** | Accepted as the G1a implementation baseline. The bounded evidence and analytic probes remain distinct from prior user reports and unknowns; the measurement uncertainty band stays provisional. The denied GTK launch was not retried and does not block this headless baseline. |
| **G1a — Physical intent and persistence** | User accepted; included in this WIP checkpoint. Adds explicit dimensions and threshold intent through domain commands and history. Writes schema 11, strictly reads schema 10 with default intent, preserves the old file on open, and preserves source/default output behavior. |
| **G1b — Headless detector and CLI evidence** | Accepted by the user on 2026-10-04; included in this WIP checkpoint. Adds the bounded advisory detector, stable result identity, deterministic JSON, cancellation/stale rejection, and a headless CLI report. See the G1b evidence section above. |
| **G2a — Shared internal evaluation/export snapshot** | Implemented; awaiting user review, checkpointed as WIP, not accepted. Preserve the current chooser → PNG options flow. Capture one live session/source/frame/workspace/lifecycle snapshot for options, transparent report/raster evaluation, and export; preserve current default pixels and allow export without a report. See the [G2a report](../target/validation/garment-g2a-20261004/report.md). |
| **G2b — First usable preflight UI** | Proposed only; implementation needs separate authorization. Follow the [G2 proposal](#g2-proposal-optional-first-preflight-review): optional Review action, visible results/highlights, model-sensitive view/export background status, and accessible controls. |
| **G2c — GUI and end-to-end acceptance evidence** | Separately gated. Do not retry or bypass the prior GTK launch rejection; automated Sway/wlroots evidence does not claim GNOME/Mutter acceptance. |
| **G2R — Minimal isolated-island removal (separately gated, unapproved)** | Eligible only after G2 and requires its own approval. Measure the whole support component's physical pixel-footprint bounding box diagonal as a conservative upper bound on span. Candidate removal requires a whole isolated support component whose bounding-box diagonal is strictly below the artist's physical maximum; equality or measurement uncertainty stays advisory and unselected. Alpha 1 and intentional white are never presumed defects. Thresholds only find candidates: the artist selects each candidate and explicitly Applies after seeing the affected feature at the exact current target/AA/placement, with before/after, bypass, and undo. Preserve the source. Key selection to the full report/render identity; any document, target, or AA change invalidates it and forces a fresh preview before Apply. Representation and algorithm remain deferred. Merging, coverage redistribution, and source-matte cleanup are separate decisions. |
| **G3a — Source-color and confidence presentation** | Make the existing **Source color + alpha** mode discoverable for preserving sampled source colors while alpha drives marks. Keep alpha coverage distinct from RGB/profile diagnostics; report source-profile confidence without implying soft proof. Stop. |
| **G3b — Truthful color and physical metadata** | If still accepted, inspect source profiles and confidence states; embed an sRGB profile only for sRGB output bytes and pHYs resolution metadata derived from explicit intent. Verify PNG chunks and decoded RGB/alpha independently. Do not add soft proofing or hidden color conversion. Stop. |
Merging nearby features, redistributing coverage, and source-matte diagnostics or
cleanup remain separate unapproved decisions. Intentional white is ambiguous
and is never auto-removed. Actual-artwork Pattern Wizard previews remain
deferred; they are separate from G2R.

Proposed ownership route: Luna for contained implementation, UI, tests, and
documentation after scope is settled; Sol for difficult detector, persistence,
or numerical work when the evidence warrants escalation; Astra for architecture
reconciliation, conflicting authority, or final integration/review. Preserve
one writer per stage. Routing does not bypass approval gates.

## Acceptance evidence for later stages

- Analytic fixtures include clean fades; matching-alpha clean/white-matted edge
  pairs; opaque matte; intentional white; zero-alpha hidden RGB; isolated dots
  and 2×2 islands; narrow positive strokes/necks; negative holes/gaps; diagonal
  corner contacts; edge-crossing marks; partial alpha; and occluded/color-only
  details.
- Vary pixel phase and orientation, both physical axes, anisotropic dimensions,
  output scale, target size, AA, transforms, clipping, visibility/opacity,
  composition, and selected background. Keep exact expected measurements
  separate from advisory bands.
- Prove deterministic result identity, cancellation, stale-result suppression,
  checked allocation/work limits, unknown-size unavailability, and zero-disabled
  behavior. With detector checks disabled, canonical scene and decoded RGBA
  pixels remain exact; encoded-file byte parity applies before G3b only because
  G3b intentionally adds metadata chunks.
- Verify history/dirty state, undo/redo, save/reopen at the target v0.3.3 input,
  strict current-schema load, and source/content preservation.
- Inspect decoded PNG dimensions, raw RGB and alpha separately, alpha counts,
  background behavior, ICC/sRGB claims, and pHYs rounding. Keep native files as
  primary evidence; label any light/dark composites as derivatives.
  Inspect enlarged edges and comparisons at the intended physical placement
  size; screen previews require known display scaling and are not print proofs.
- Use both immutable assets as source inputs and verify their documented hashes
  for relevant stages. SVG live text needs deterministic font provenance for
  exact pixel comparisons. Run focused stage tests and directly relevant
  current foundational checks only; do not sweep historical test suites.
- For G2c, exercise GTK import → edit → preview → export → reopen and record
  stable accessible product names, GTK role/state/value/actions, label
  relations, truthful applicability, keyboard operation, semantic
  action/readback, and screenshots. Assess new warnings/errors against baseline
  logs instead of requiring every inherited log to be empty. Automated
  Sway/wlroots evidence does not claim manual GNOME/Mutter acceptance.
- A selected product needs supplier-specific file-area dimensions and guidance.
  Printing and evaluating a physical sample is required for claims about actual
  fabric color, underbase, dot adhesion, cracking, or wash durability.

## References checked 2026-10-03

- [Printful: preparing print files](https://help.printful.com/hc/en-us/articles/50264019148177-How-should-I-prepare-my-print-file-for-the-best-results) — product-specific dimensions, 150–300 DPI general range, sRGB, transparency, and small-detail cautions.
- [Printful: RGB or CMYK](https://help.printful.com/hc/en-us/articles/50264003313041-Should-I-use-RGB-or-CMYK-for-Printful-print-files) — embedded sRGB recommendation and limits of screen/print color matching.
- [Printful: DPI and actual size](https://help.printful.com/hc/en-us/articles/50264010605457-What-is-DPI-resolution-and-actual-print-file-size) — DPI depends on real physical print dimensions.
- [Printful: DTFlex preparation](https://www.printful.com/uk/blog/preparing-dtf-print-file) — process-specific warnings for small dots, narrow lines, soft edges, and semi-transparency.
- [Printful: transparency in DTG](https://www.printful.com/transparency-in-dtg-files) — reported DTG fade behavior, especially on dark fabric.
- [Printful: white underbase](https://help.printful.com/hc/en-us/articles/50264409546641-When-is-a-white-underbase-used) — underbase behavior varies with design, material, product, and method.
