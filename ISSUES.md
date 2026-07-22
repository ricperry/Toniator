# Toniator Issue Tracker

This file converts the current Toniator wish list and known problems into discrete, actionable issues for Codex orchestration.

## Conventions

- **Status:** `Open`, `In progress`, `Blocked`, or `Done`
- **Priority:**
  - **P0 — Defect:** Existing output can be incomplete, incorrect, or unreliable.
  - **P1 — Core UX / capability:** High-value improvement that materially affects normal use.
  - **P2 — Expansion:** Important new workflow or pattern support.
  - **P3 — Low priority:** Useful enhancement that can wait until the core experience is stable.
- Preserve existing project files and exported output wherever practical.
- Preview and export must use the same layout and rendering rules.
- UI work should be reviewed with the `creative_tester` agent from the perspective of an experienced creative-tool user.
- Do not mark an issue complete solely because the feature exists. Its behavior must also be understandable, discoverable, and reliable.

---

## Recommended implementation order

1. Fix incomplete repeated-motif coverage.
2. Audit terminology and add contextual help.
3. Correct the source-mapping hint icons and move the side panel.
4. Rework the application shell toward GNOME/libadwaita conventions.
5. Add document-background and transparency controls.
6. Add RGB display mode.
7. Add DTF mode.
8. Expand shape and curve-layout patterns.
9. Add mixed shape-and-curve documents.

---

# Defects and UX problems

## TON-001 — Fix incomplete coverage in repeated-motif curve layouts

- **Status:** Done
- **Priority:** P0
- **Area:** Curves / repeated motif layout
- **Type:** Rendering defect

### Problem

When the row count is at or near its maximum and row spacing is low, Toniator cannot always draw enough repeated motif paths to cover the full image. This leaves unfilled areas, particularly along edges and in corners.

### Required outcome

Repeated-motif layouts must cover the complete intended artwork area at all supported row-count and spacing combinations.

### Acceptance criteria

- The generated layout covers the full source-image bounds, including all four edges and corners.
- Extreme combinations of maximum row count and minimum practical row spacing do not leave accidental gaps.
- Coverage accounts for motif width, stroke width, transforms, rotation, and any displacement that can extend or retract the visible artwork.
- Paths may extend beyond the document bounds when necessary to guarantee clipped edge coverage.
- Preview and exported output show the same coverage.
- Cancellation and stale-preview suppression continue to work during dense renders.
- Regression coverage includes:
  - maximum row count;
  - minimum row spacing;
  - wide and tall aspect ratios;
  - rotated motifs;
  - large stroke widths;
  - transparent source edges;
  - all supported document sizes.
- Any remaining blank area must be caused by the source image or an explicit user setting, not by insufficient path generation.

### Implementation notes

Investigate whether the current layout computes rows only from the nominal canvas bounds rather than from an expanded coverage region. The fix should derive the required path extent from the rendered motif footprint rather than adding an arbitrary fixed number of rows.

### Completion evidence

- Replaced nominal-axis heuristic counts with transformed lattice coverage derived from the actual motif advance, row advance, rotation, pivot, offsets, stagger, bleed, and maximum rendered width.
- Custom Grid column and row counts now act as requested minimums; symmetric off-canvas guard copies are generated when the artboard requires more coverage.
- Collapsed or sub-1 px legacy lattice advances remain bounded instead of generating thousands of coincident paths; the supported 1 px minimum still receives full coverage guards.
- Added deterministic regressions for wide and tall artboards, rotation, heavy curve weight, maximum rows with minimum spacing, nonzero transforms, all four artboard edges, transparent source edges, degenerate spacing, and canonical preview/SVG geometry.
- Verified with 110 automated tests, the bundled repeated-motif stress preset exported to both SVG and PNG, formatting/diff checks, and an independent regression review with no remaining actionable finding. One unrelated GTK widget test still requires a graphical session and cannot initialize in the headless test shell.

---

## TON-002 — Replace implementation-oriented terminology with creator-friendly language

- **Status:** Done
- **Priority:** P1
- **Area:** Entire application
- **Type:** UX / content design

### Problem

Many labels use terms that are confusing, overly technical, or inconsistent with what a typical artist or creative-tool user would assume they mean.

### Required outcome

The interface should describe creative intent and visible results rather than internal rendering concepts.

### Scope

Audit all visible terminology, including:

- section headings;
- control labels;
- mode names;
- menu items;
- tooltips;
- status messages;
- error messages;
- export terminology;
- saved-preset descriptions;
- curve and shape pattern names;
- source-mapping controls;
- layout controls.

### Acceptance criteria

- Every visible term has a clear meaning to a user who understands general graphics applications but is new to Toniator.
- Internal implementation names are not exposed unless they are also accepted creative-industry terms.
- The same concept uses the same name throughout the application.
- Similar concepts are clearly distinguished.
- Labels describe the result of a control, not merely the variable being changed.
- Ambiguous terms are tested with the `creative_tester` agent.
- Existing serialized project keys may remain unchanged internally; this issue concerns user-facing language.
- A short terminology reference is added to the project documentation for concepts that remain specialized.

### Deliverable

Include a terminology-change table in the implementation notes or pull request description:

| Existing term | Replacement | Reason |
|---|---|---|
| Example | Example | What misunderstanding it prevents |

### Completion evidence

- Replaced implementation-shaped labels with creator-facing terminology across the shipping GTK UI, dynamic status text, CLI help, preset/export wording, and accessibility labels while preserving serialized keys.
- Added the required terminology-change table and specialized-term reference to `README.md`.
- Verified mode-specific wording for inks versus directional Crosshatch layers, including current screen angles and consistent Shapes/Curves terminology.
- Reviewed against refreshed main and Crosshatch screenshots by the `creative_tester`; no remaining actionable terminology issue was found.

---

## TON-003 — Add contextual popup help for controls

- **Status:** Done
- **Priority:** P1
- **Area:** Entire application
- **Type:** UX / discoverability

### Problem

Labels alone do not adequately explain many controls, especially where Toniator uses specialized layout, sampling, color-separation, or motif concepts.

### Required outcome

Users should be able to understand a control without consulting source code or external documentation.

### Acceptance criteria

- Every non-obvious control provides contextual help through a tooltip, popover, help button, or another GNOME-appropriate mechanism.
- Help text explains:
  1. what the control changes;
  2. what increasing or decreasing it does;
  3. any interaction with related controls;
  4. whether it affects preview, export, or both.
- Help text uses creator-friendly language established by TON-002.
- Tooltips do not merely repeat the label.
- Keyboard-only users can access equivalent information.
- Help remains readable at normal UI scaling and does not obscure the control being explained.
- Controls with nonlinear, conditional, or mode-dependent behavior explicitly state those conditions.
- The `creative_tester` agent reviews the final help text for clarity and usefulness.

### Non-goal

This does not require a full manual embedded inside the application.

### Completion evidence

- Added a declarative help catalog with focusable GNOME help buttons, GTK-owned popovers, substantive tooltips, and accessible descriptions for non-obvious controls.
- Help copy covers effect, increase/decrease behavior, related controls or mode conditions, and preview/export scope; dynamic popovers update fully between ink and Crosshatch-layer modes.
- Added completeness, realized-widget, keyboard activation, hidden-control, accessibility, and capture-lifecycle regressions.
- Verified a zero-warning build, 113 automated tests, successful bounded main/Crosshatch screenshot capture, non-hanging terminal artifact failures, and independent creative/test review passes.

---

## TON-004 — Rework the UI toward the GNOME/libadwaita HIG

- **Status:** Done
- **Priority:** P1
- **Area:** Application shell and interaction design
- **Type:** UI/UX overhaul
- **Depends on:** TON-002, TON-003
- **Related:** TON-005

### Problem

The current interface does not consistently follow GNOME/libadwaita conventions and contains rough edges that make the application feel less coherent and less predictable.

### Required outcome

Adopt GNOME/libadwaita patterns to the greatest practical extent without breaking existing functionality or reducing the efficiency of creative workflows.

### Scope

Review and improve:

- window and header-bar structure;
- side-panel organization;
- spacing and grouping;
- adaptive layout behavior;
- primary versus secondary actions;
- buttons, toggles, rows, dropdowns, and dialogs;
- keyboard navigation and focus;
- mode visibility;
- destructive-action handling;
- progress, cancellation, and error presentation;
- empty states;
- save, reopen, export, and unsaved-change behavior;
- disabled-control explanations;
- status and preview feedback.

### Acceptance criteria

- The application uses native or appropriately styled libadwaita components where practical.
- Related controls are visually grouped and ordered according to workflow.
- The current mode, active channel, selected object, and pending operation are always apparent.
- Primary actions are visually distinguishable without excessive emphasis.
- The interface remains usable at reduced window widths and common display scaling factors.
- Keyboard focus order follows the visible workflow.
- Disabled controls either have an obvious reason or provide an explanation.
- Existing functionality remains available after the rework.
- Existing project files continue to open.
- Dense controls do not cause clipping, overlap, or inaccessible content.
- A `creative_tester` review identifies no remaining blocker-level confusion in the main import-to-export workflow.

### Implementation approach

Perform this incrementally. Avoid a single unreviewable rewrite that changes architecture, terminology, layout, and rendering behavior simultaneously.

### Completion evidence

- Reworked the native GTK/libadwaita shell without changing renderer or document semantics: contextual header actions, `AdwWindowTitle` save/operation state, fixed editing-context summary, workflow-grouped inspector, persistent preview/export feedback, and accurate disabled/action explanations.
- Retained the resizable left controls pane on wide windows and moved the same inspector into a Controls-triggered overlay below the narrow breakpoint; focus, visibility, saved width, and canvas allocation remain synchronized.
- Added creator-facing context for Shapes, Curves, inks, and directional Crosshatch layers, plus standard new-project, controls, zoom, undo/redo, save, open, and export shortcuts.
- Added real cooperative cancellation for preview and export work. Controlled sampling, generation, curve, raster, SVG, PNG, and atomic-write paths checkpoint cancellation; cancel/commit state is race-safe, cancelled exports cannot replace an existing destination, and close remains inhibited until export cleanup acknowledges.
- Added realized layout/focus/allocation regressions, cancellation/state-machine tests, refreshed start/Shapes/Crosshatch/narrow artifacts, and independent creative/test review passes.
- Verified formatting, warnings-as-errors linting, diff checks, and 119 automated tests. Image resize and PNG encoding remain indivisible third-party calls, with cancellation checked immediately before and after them.

---

## TON-005 — Move the primary side panel to the left

- **Status:** Done
- **Priority:** P1
- **Area:** Application layout
- **Type:** UI change
- **Related:** TON-004

### Problem

The primary controls are currently positioned on the right side of the window. The preferred Toniator workflow places the controls on the left and the artwork/preview area to the right.

### Acceptance criteria

- The primary side panel is on the left side of the main preview.
- Panel resizing, minimum width, and collapse behavior remain functional.
- The preview remains centered or otherwise sensibly positioned in the remaining workspace.
- Popovers, tooltips, and dropdowns open within visible screen bounds.
- Existing keyboard navigation and shortcuts continue to work.
- No control ordering or grouping is accidentally changed merely as a side effect of moving the panel.
- The layout remains usable on narrow windows.

### Completion evidence

- Moved the primary controls into the left/start pane while preserving the existing control order, vertical scrolling, resizable divider, saved width, canvas centering, and keyboard shortcuts.
- Added an explicit, keyboard-focusable `Controls` toggle in the left side of the header; its pressed state, tooltip, and accessible description track panel visibility, and the panel restores its saved width when reopened.
- Added realized GTK regression coverage for pane ownership, drag persistence, constrained narrow widths, collapse/restore, toggle activation, and state synchronization.
- Verified normal, narrow, collapsed, Crosshatch, light, and dark layouts; the final labeled-toggle revision passed independent creative review. A fresh post-revision screenshot remains a compositor-session verification gap, not an observed defect.

---

## TON-006 — Use SVG source-mapping hint icons instead of PNG versions

- **Status:** Done
- **Priority:** P2
- **Area:** Source mapping hints
- **Type:** Visual-quality defect

### Problem

The application uses PNG versions of source-mapping hint icons even though SVG assets are available. The raster icons scale less cleanly and are inconsistent with the intended asset source.

### Acceptance criteria

- All source-mapping hints use the SVG assets.
- Icons remain crisp at all supported display scaling factors.
- Icon aspect ratio, padding, alignment, and baseline placement are consistent.
- Dark and light appearance modes remain legible.
- No obsolete PNG fallback is loaded when a valid SVG asset is present.
- Packaging and installed builds include the required SVG assets.
- Missing-asset errors fail visibly and do not silently substitute unrelated imagery.

### Completion evidence

- Confirmed all six source-mapping hints are compile-time embedded SVG assets with no PNG load or fallback path; installed builds require no runtime asset lookup.
- Made SVG rasterization follow the widget scale factor and refresh when that factor changes, with consistent square framing, padding, alignment, and a neutral tile that remains legible in light and dark appearances.
- Added parsing, table-identity, accessibility, aspect, and 1x/2x texture-dimension regressions for every mapping pair.
- Verified a zero-warning build, 113 automated tests, representative Shapes/Crosshatch and light/dark artifacts, and independent creative and regression review passes. A live 2x-compositor capture remains a verification gap rather than a confirmed defect.

---

# Core capabilities

## TON-007 — Add configurable document background and transparency controls

- **Status:** Done
- **Priority:** P1
- **Area:** Document / preview / export
- **Type:** New capability
- **Related:** TON-008, TON-009

### Problem

Transparent artwork is difficult to evaluate without being able to define the intended document or garment background. This is especially important for DTF artwork.

### Required outcome

Allow the user to preview transparent artwork against a chosen background color while clearly distinguishing preview-only background settings from exported artwork.

### Acceptance criteria

- The user can choose a document-preview background color.
- The color picker supports transparency where applicable.
- A checkerboard or equivalent transparency indication remains available.
- The UI clearly distinguishes:
  - transparent artwork;
  - preview background;
  - an actual exported background object or layer.
- The preview background is not exported by default.
- The user can explicitly choose to include a background in export when supported.
- Background settings are saved and restored with the document or project.
- Changing the background does not alter source-image sampling or channel values unless the user explicitly chooses a compositing option.
- The control works consistently in CMYK, RGB, and DTF workflows.
- Preview and export behavior are documented in contextual help.

### DTF use case

The selected background can represent the garment color so that white underbase and knockout behavior can be evaluated before export.

### Completion evidence

- Added saved, alpha-capable Preview Surface and Export Background settings with checkerboard support, undo/redo, dirty-state tracking, and v1-v3 migration that preserves legacy white output.
- Kept Preview Surface strictly canvas-only and composition-only; mark and curve generation remain unchanged. SVG emits an optional named bottom background layer, while PNG uses the saved background by default with explicit transparent and white one-export overrides.
- Added visible contextual help that explains preview-only transparency, saved export defaults, PNG overrides, and alpha preservation. The controls remain reachable in the narrow adaptive inspector.
- Verified transparent and semi-transparent PNG/SVG artifacts, successful editor screenshots, strict failing artifact arguments, and exact full-document preview/export equivalence for both Shapes and Curves.
- Passed formatting, zero-warning Clippy, 128 automated tests, diff validation, and independent creative and regression reviews.

---

## TON-008 — Add RGB mode for display-oriented halftone artwork

- **Status:** In progress
- **Priority:** P1
- **Area:** Color modes / rendering / export
- **Type:** New capability
- **Depends on:** TON-007
- **Related:** TON-009

### Problem

Toniator is currently oriented toward print-style color separation. Users also need a display-oriented RGB workflow.

### Required outcome

Add an RGB mode designed for artwork viewed on screens rather than separated for CMYK printing.

### Acceptance criteria

- The user can explicitly choose between CMYK and RGB document modes.
- RGB mode provides red, green, and blue channels with clear creator-friendly terminology.
- The compositing model is explicitly defined and reflected accurately in the preview.
- Default RGB behavior produces a sensible result for display against an appropriate background.
- Channel visibility, ordering, color, and per-channel settings remain independently controllable.
- Source mapping and tonal interpretation are appropriate for RGB output rather than reusing CMYK assumptions without review.
- Exported SVG or raster output visually matches the preview within normal rendering tolerances.
- Transparency and the configurable document background work correctly.
- Save and reopen preserve the selected color mode and all RGB channel settings.
- Existing CMYK projects continue to open and render unchanged.
- Mode switching either preserves compatible settings or clearly warns which settings will be reset.
- Help text explains when to use RGB versus CMYK.

### Design question to resolve

Determine whether RGB artwork uses additive blending, precomputed composite colors, or another documented model. Do not leave this as an accidental consequence of SVG renderer behavior.

---

## TON-009 — Add DTF output treatment with merged white base and optional black knockout

* **Status:** Open
* **Priority:** P1
* **Area:** Export / DTF production
* **Type:** New output workflow
* **Depends on:** TON-007
* **RGB integration depends on:** Completion and stabilization of TON-008
* **Related:** TON-008

### Problem

Toniator can generate halftone artwork using CMYK Print or RGB Screen construction, but artwork intended for direct-to-film production requires additional preview and export behavior.

CMYK halftone artwork assumes a light, reflective substrate beneath and between the printed dots. On a dark garment, exposing the fabric through the negative spaces between CMYK marks changes the intended color mixing, brightness, and contrast. A white artwork base may therefore need to support the complete intended halftone region rather than only the colored mark geometry.

RGB Screen artwork has different substrate assumptions. It may intentionally rely on a dark garment showing between colored marks, while still requiring white beneath the printed marks themselves to preserve their brightness.

Black or nearly black regions may optionally be omitted from a transfer intended for a black or sufficiently dark garment. In those areas, the garment supplies the visible black through transparent knockout regions.

DTF preparation is an output treatment applied after artwork generation. It must not replace CMYK Print or RGB Screen as the document’s artwork-generation model.

### Required outcome

Provide a DTF output workflow that can be applied to supported CMYK and RGB artwork.

The workflow must provide:

* an accurate garment preview;
* one derived and merged white-base layer;
* model-appropriate white-base coverage;
* optional garment-supplied black knockout;
* transparent sRGB POD export;
* explicit layered production export where supported.

### Artwork and output model

Toniator must keep these concepts separate.

#### Artwork-generation model

* CMYK Print;
* RGB Screen.

#### Output treatment

* Standard Artwork;
* DTF Transfer.

Selecting DTF Transfer must not discard, reinterpret, or overwrite the document’s CMYK or RGB generator settings.

Disabling DTF Transfer must restore the normal document preview and export behavior without losing the saved DTF settings.

Existing CMYK and RGB documents must continue to render unchanged when DTF treatment is disabled.

### DTF document settings

The saved DTF treatment must include:

* DTF treatment enabled or disabled;
* intended garment or preview color;
* white-base coverage policy;
* white-base visibility for inspection;
* knockout enabled or disabled;
* knockout rule or preset;
* knockout threshold where applicable;
* POD-ready or layered-production export target.

These settings must participate in:

* save and reopen;
* undo and redo;
* dirty-state tracking;
* project migration;
* mode switching.

### Garment preview

The existing Preview Surface setting may represent the intended garment color.

The garment color must:

* remain preview-only unless explicitly exported through an unrelated standard background option;
* not enter source sampling;
* not alter channel values;
* not alter shape or curve generation;
* not alter the saved source artwork;
* be used when evaluating the final DTF composite and knockout preview.

The final visual preview order is:

```text
Garment
→ merged white base
→ printable color artwork
```

The interface must allow inspection of:

* printable color artwork only;
* merged white base only;
* knockout mask;
* final garment composite.

Inspection toggles must not change generated geometry or exported content unless the corresponding production setting is explicitly changed.

### Merged white-base behavior

DTF treatment generates one merged white-base layer beneath the printable color artwork.

The merged base must be calculated as a coverage mask, not as four separate CMYK underbase channels.

Overlapping color layers must not:

* increase white opacity;
* create seams;
* create multiple stacked white layers;
* produce different white density merely because several color channels overlap.

Continuous alpha coverage should be combined as a coverage union rather than as an additive sum.

The white base must not affect:

* source sampling;
* channel values;
* shape generation;
* curve generation;
* placement patterns;
* tonal interpretation.

#### CMYK Print coverage

For CMYK Print artwork, the default white-base policy must preserve the light-substrate assumption of the halftone.

The base should therefore derive from the intended printable artwork support region, including white negative space that is intended to participate in the CMYK halftone appearance.

It must not automatically be limited to the union of the cyan, magenta, yellow, and black mark geometry when doing so would expose a dark garment through spaces that were intended to appear as white substrate.

The support region should normally derive from:

* the source artwork alpha or explicit artwork mask;
* the document clipping region;
* any deliberate transparent holes;
* the transformed bounds of the intended artwork.

It must not simply flood the entire document or source-image rectangle when those areas are intentionally transparent.

#### RGB Screen coverage

For RGB Screen artwork, the default white-base policy may derive from the merged coverage of the actual printable red, green, and blue marks.

This allows the garment to remain visible between marks where that dark background is part of the intended additive-looking result.

RGB Screen plus DTF must resolve Screen blending into deterministic printable sRGB colors before export. It must not rely on the POD service or external SVG renderer to reproduce internal blend modes.

#### Coverage policy

If both behaviors are exposed to users, the interface should use creator-facing choices such as:

* **Preserve White Substrate** — supports the intended artwork region, including halftone negative space;
* **Under Printed Marks Only** — places white only beneath printable colored marks.

CMYK Print should default to Preserve White Substrate.

RGB Screen should default to Under Printed Marks Only unless testing demonstrates a more appropriate default.

Contextual help must explain the visible consequence rather than expose mask-generation terminology.

### Black knockout

Knockout produces one mask derived from an explicit and documented qualifying rule.

The initial required scope is garment-supplied black or near-black on black or sufficiently dark garments. Matching arbitrary colored artwork to arbitrary garment colors may be deferred.

The knockout rule must not rely solely on:

* the CMYK black channel;
* source pixel values before compositing;
* a filename or mapping label.

Qualification must be evaluated from the final intended printable color before garment-preview compositing.

The rule may consider:

* perceptual lightness;
* chroma;
* neutrality;
* closeness to neutral black;
* a conservative user-controlled threshold.

The initial interface may provide:

* Off;
* True Black Only;
* Near Black.

Any threshold must use creator-facing language and provide immediate visual feedback.

When knockout is enabled, the knockout mask is subtracted from both:

* printable color artwork;
* merged white-base coverage.

Knocked-out regions therefore contain neither color artwork nor white-base artwork and allow the garment to show through.

Removing only the white base while retaining printed black does not count as garment knockout.

Dark saturated colors that do not qualify as garment-supplied black must continue to print normally.

Antialiased boundaries must not create:

* white fringes;
* dark halos;
* accidental holes;
* abrupt threshold stair-stepping;
* unintended partial transparency.

Conceptually:

```text
Knockout mask =
    qualifying garment-supplied black regions

Printable color =
    resolved generated artwork
    minus knockout mask

Merged white base =
    selected white-base support coverage
    minus knockout mask
```

### POD-ready sRGB export

POD-ready export must produce a flattened sRGB PNG appropriate for services that accept normal transparent artwork rather than explicit production separations.

The PNG must:

* be transparent outside the intended artwork;
* be transparent in knockout regions;
* omit the preview garment color;
* resolve all internal blend behavior;
* preserve antialiased edges appropriately;
* contain the final visible white artwork required to preserve the intended CMYK negative space.

For CMYK Preserve White Substrate output, areas of the white-base artwork visible between halftone marks must be flattened as visible white sRGB pixels. They cannot remain as an invisible hidden production layer in an ordinary PNG.

The POD provider may still generate its own physical printer underbase beneath all opaque artwork pixels. Toniator’s visible white substrate artwork and the provider’s printer-generated underbase are related but distinct concepts.

The interface and help must not imply that an ordinary PNG can carry an invisible independent white-ink plate.

### Layered production export

Where supported, layered production output should contain:

```text
Color Artwork
White Base
Knockout Mask
```

* **Color Artwork** contains the final printable CMYK- or RGB-generated color artwork.
* **White Base** contains one merged white-support layer.
* **Knockout Mask** identifies regions removed from both color and white coverage.

Layer names and ordering must be:

* stable;
* clear;
* deterministic;
* independent of the order in which controls were edited.

The document-layer order should represent the intended final garment appearance:

```text
Color Artwork
White Base
Garment or transparent destination
```

Device-specific film print order, mirroring, and RIP conventions remain deferred unless explicitly required by a supported production export.

### Acceptance criteria

* DTF Transfer can be enabled independently of CMYK Print or RGB Screen.
* Enabling or disabling DTF does not discard generator settings.
* CMYK plus DTF is supported.
* RGB plus DTF is supported after TON-008 is stable.
* One merged white-base layer is generated.
* No per-channel white-underbase layers are created.
* Overlapping channels do not accumulate white opacity.
* CMYK Preserve White Substrate coverage includes intended halftone negative space.
* Intentionally transparent source areas remain transparent.
* RGB marks-only coverage preserves intended dark garment gaps.
* White-base coverage can be inspected independently.
* Black knockout can be enabled or disabled.
* The knockout rule is explicit, predictable, and documented.
* Knockout removes both printable color and white-base coverage.
* Dark saturated non-black colors remain printable.
* Garment color can be used for preview.
* Garment color does not alter source sampling or generated geometry.
* Preview, PNG, and layered export agree within documented rendering tolerances.
* POD PNG output is consistently encoded as sRGB.
* POD PNG output contains no preview garment background.
* POD PNG output uses transparency for knockout and outside-artwork regions.
* Visible white substrate areas required by the artwork remain visible white in POD output.
* Layered production output uses one merged White Base layer.
* Layer names and order remain stable.
* Save and reopen preserve all DTF settings.
* Undo and redo preserve DTF changes.
* Existing project files continue to open.
* Standard CMYK and RGB output remains unchanged when DTF is disabled.
* Long DTF preview and export operations remain cancellable.
* Cancelled or superseded renders cannot replace newer results.
* Failed or cancelled exports cannot damage an existing destination.
* Normal settings do not create obvious white fringes, dark halos, or transparent seams.
* Contextual help clearly explains:

  * artwork-generation model;
  * DTF output treatment;
  * white substrate support;
  * marks-only white support;
  * printed black;
  * garment-supplied black knockout;
  * preview garment color;
  * POD-ready flattened export;
  * layered production export.

### Required regression coverage

Include deterministic tests and representative artifacts for:

* CMYK artwork on white, black, and colored garment previews;
* RGB artwork with DTF treatment after TON-008 stabilization;
* white base enabled and disabled;
* CMYK Preserve White Substrate coverage;
* RGB Under Printed Marks Only coverage;
* one merged white-base layer;
* overlapping CMYK channel geometry without white accumulation;
* white negative space between CMYK marks;
* transparent source edges;
* internal transparent holes;
* antialiased boundaries;
* true-black knockout;
* near-black knockout;
* dark saturated colors that must remain printed;
* knockout disabled;
* knockout removal from both color and white coverage;
* save and reopen;
* undo and redo;
* CMYK → RGB → CMYK mode switching with saved DTF settings;
* preview versus flattened PNG parity;
* preview versus layered SVG parity;
* cancellation and stale-result suppression;
* failed-export destination protection;
* unchanged legacy CMYK and RGB output with DTF disabled.

### Implementation sequence

Implement TON-009 as separate testable slices:

1. DTF output-treatment model, migration, persistence, and undo/redo.
2. Garment preview and inspection views.
3. One merged white-base mask with CMYK Preserve White Substrate behavior.
4. RGB marks-only white-base behavior after TON-008 stabilization.
5. POD-ready flattened sRGB export.
6. Black and near-black knockout applied to both color and white masks.
7. Layered production export.
8. Full creative and regression review.

Do not implement all slices as one unreviewable change.

### Deferred enhancements

Track these separately unless required by the current rendering pipeline:

* arbitrary garment-color matching and knockout;
* underbase choke or spread;
* trapping;
* edge-color decontamination;
* editable knockout masks;
* manually painted white-base corrections;
* printer- or RIP-specific separations;
* provider-specific export presets;
* spot-color naming conventions;
* film mirroring and device print orientation;
* ink-limit controls;
* powder, curing, and transfer settings.

---

# Shape-pattern expansion

## TON-010 — Add an extensible halftone-pattern framework and user-defined patterns

* **Status:** Open
* **Priority:** P1
* **Area:** Pattern generation / Shapes / Curves
* **Type:** Foundational capability
* **Depends on:** Stable Shapes and Curves rendering pipelines
* **Related:** TON-008, TON-017
* **Replaces:** Previous pattern-specific TON-010 through TON-016 issues

### Problem

Toniator currently represents shape placement and curve layouts as a small number of hard-coded application modes.

Adding a new pattern currently risks requiring coordinated changes to:

* document enums and settings;
* dropdown indexes;
* UI callbacks;
* reverse UI synchronization;
* persistence and migration;
* preview generation;
* PNG and SVG export;
* contextual help;
* tests and visual fixtures.

That approach does not scale to a broad pattern library and provides no practical route for users to create, save, exchange, or embed their own patterns.

Many apparently distinct halftone styles are combinations of reusable concepts rather than entirely separate renderers. Examples include:

* rectangular, triangular, or stochastic placement;
* dots, polygons, lines, or custom motifs;
* rotation, phase, wave, jitter, or mesh deformation;
* size-, density-, spacing-, or width-based source modulation;
* deterministic or seeded generation.

Toniator needs a pattern framework rather than another expanding collection of hard-coded pattern enums.

### Required outcome

Create a versioned, extensible halftone-pattern framework through which:

* built-in patterns are registered using stable identifiers;
* user-defined patterns use the same discovery and execution path;
* pattern metadata and parameter definitions drive the interface;
* mark-based and path-based patterns are both supported;
* deterministic and stochastic patterns use shared framework services;
* new patterns can be added without redesigning the document model;
* supported pattern components can be combined into reusable user recipes;
* projects can embed custom definitions so they remain portable;
* preview, PNG, and SVG consume the same generated geometry.

The framework must support future expansion toward pattern families such as:

* rectangular and triangular grids;
* dot and line screens;
* angled grids and line fields;
* pointillism and grain;
* Poisson or blue-noise distributions;
* weighted stippling;
* cellular, pebble, and paver patterns;
* clustered and aperiodic patterns;
* mesh and dot waves;
* curved line fields;
* spiral and maze-like paths;
* procedural contour patterns.

Implementing every pattern in that list is not required for the initial framework.

---

# Pattern architecture

## Stable pattern identity

Each pattern must have:

* a stable machine-readable identifier;
* a creator-facing display name;
* a category;
* a schema version;
* a generator or recipe version;
* a declared output kind;
* a declared parameter schema;
* documented compatibility with Shapes, Curves, CMYK, and RGB workflows;
* deterministic migration behavior.

Pattern selection must not depend on:

* numeric dropdown position;
* display-name text;
* source filename;
* insertion order in the registry.

Example stable identifiers might follow a namespace convention such as:

```text
builtin.grid.rectangular
builtin.grid.triangular
builtin.lines.wave
builtin.points.poisson
user.rich.custom-dot-wave
```

The exact naming convention may differ, but identifiers must remain stable and distinct from translated display names.

Renaming a pattern in the interface must not break saved projects.

## Pattern registry

Built-in and imported patterns must be discoverable through a common registry.

The registry must provide:

* metadata lookup;
* stable identifier resolution;
* schema-version validation;
* parameter definitions;
* generator or recipe construction;
* capability checks;
* migration support;
* clear errors for missing or incompatible patterns.

Built-in patterns must not bypass the registry through private dropdown branches or renderer-specific switches.

The exact Rust API may differ, but the architectural boundary should resemble:

```rust
trait PatternGenerator {
    fn metadata(&self) -> &PatternMetadata;

    fn generate(
        &self,
        context: &PatternContext,
        parameters: &PatternParameters,
    ) -> Result<PatternOutput, PatternError>;
}

enum PatternOutput {
    Marks(Vec<MarkInstance>),
    Paths(Vec<GeneratedPath>),
}
```

Mark and path output may share framework services, but they must not be forced into an unsuitable common geometry representation.

## Canonical pattern output

Pattern generation must produce canonical geometry before raster or SVG rendering.

The same canonical output must feed:

* live preview;
* PNG export;
* SVG export;
* artifact validation.

Preview and export must not independently reimplement the pattern algorithm.

Color compositing may differ by output mode, but generated positions, paths, transforms, and clipping must remain equivalent.

## Pattern context

The framework must provide a common generation context containing the services needed by patterns, such as:

* document bounds;
* clipping and crop bounds;
* source-value sampling;
* document and channel transforms;
* active color model;
* stable channel identity;
* preview or export quality level;
* deterministic random state;
* cancellation token;
* edge-coverage requirements;
* geometry tolerances;
* source alpha or artwork mask where relevant.

Individual pattern generators must not independently recreate these systems.

## Pattern output kinds

The initial framework must support at least:

### Mark-based output

For patterns composed from discrete repeated elements, including:

* dots;
* polygons;
* custom SVG marks;
* stippling;
* pointillism;
* cellular marks;
* clustered marks.

A mark instance should be able to describe relevant properties such as:

* position;
* size;
* rotation;
* transform;
* source response;
* primitive or custom motif reference.

### Path-based output

For patterns composed from continuous or repeated paths, including:

* parallel lines;
* waves;
* spirals;
* routed or maze-like layouts;
* repeated motifs following larger paths.

A generated path should retain sufficient information for:

* continuous rendering;
* predictable motif orientation;
* variable width where supported;
* clipping and edge coverage;
* clean SVG output.

Supporting mark and path output does not require mixed Shapes and Curves within one document. Mixed-generator documents remain part of TON-017.

---

# Composable pattern model

The framework should support reusable stages where they are appropriate. A pattern does not need to use every stage.

Conceptually:

```text
Placement or topology
→ primitive or motif
→ deformation
→ source modulation
→ clipping and coverage
→ canonical marks or paths
```

## Placement or topology

Supported or future placement systems may include:

* rectangular lattice;
* triangular lattice;
* parallel-line field;
* staggered lattice;
* Poisson or blue-noise distribution;
* weighted stippling;
* cellular or Voronoi distribution;
* clustered distribution;
* spiral topology;
* routed or maze-like topology.

## Primitive or motif

Supported or future primitives may include:

* circle or dot;
* regular polygon;
* line;
* existing user-defined SVG mark;
* imported SVG motif;
* repeated curve motif.

Placement and primitive must remain separate concepts. For example:

```text
Triangular placement + Circle
Triangular placement + Custom SVG Mark
Poisson placement + Polygon
Wave deformation + Parallel Lines
```

must not require unrelated hard-coded pattern types when they can be represented as recipes.

## Deformation

Supported or future deformation stages may include:

* none;
* rotation;
* phase or offset;
* sine-wave displacement;
* mesh deformation;
* controlled jitter;
* flow-field displacement;
* bend or warp.

## Source modulation

A pattern must explicitly declare which properties can respond to source values.

Supported or future modulation targets may include:

* mark size;
* mark density;
* spacing;
* line width;
* path displacement;
* threshold;
* cluster size;
* cluster density;
* rotation;
* motif scale.

The interface must not expose a modulation control that the active pattern ignores.

## Pattern-specific algorithms

Not every pattern can be represented as a simple recipe.

Algorithms such as:

* Poisson-disc sampling;
* weighted Voronoi stippling;
* plasma or reaction-style fields;
* aperiodic clustering;
* maze routing;

may require dedicated generators.

Those generators must still integrate through the same registry, parameter, cancellation, persistence, and output contracts.

---

# Parameter schema

## Schema-driven controls

Each pattern must declare its configurable parameters through a versioned schema.

A parameter definition should include, where applicable:

* stable parameter key;
* creator-facing label;
* contextual help;
* parameter type;
* default value;
* valid range;
* unit;
* step or precision;
* choice values;
* whether it affects geometry, source response, or appearance;
* whether it requires regeneration;
* visibility conditions;
* parameter scope;
* serialization version.

Supported parameter types should include, as needed:

* Boolean;
* integer;
* floating-point value;
* angle;
* distance;
* percentage;
* enumeration;
* seed;
* color;
* mark or motif reference.

The inspector should construct ordinary parameter controls from this schema rather than maintaining a large hand-written match statement for every pattern.

Specialized editors remain permitted when generic controls cannot provide an adequate creative workflow.

## Parameter scope

The framework must distinguish parameter scope.

A parameter may be:

* document-wide;
* shared by all channels;
* independently stored per channel;
* specific to a custom recipe component.

The initial framework does not have to allow a completely different pattern generator on every channel, but its model must not prevent that future extension.

## Conditional controls

Pattern controls must appear only when applicable.

Examples:

* deterministic patterns do not show seed controls;
* a wave pattern shows amplitude and wavelength;
* a pattern without rotation does not show an ineffective rotation control;
* custom-mark controls appear only when the active recipe uses a custom mark;
* independent seed fields appear only under Independent Arrangements.

Hidden controls must not leave:

* empty rows;
* orphan help buttons;
* inaccessible focus targets;
* active state that silently affects the result.

---

# Seeded generation and channel synchronization

Patterns that use randomness must receive deterministic random state through the common framework.

Pattern implementations must not create:

* uncontrolled process-global random generators;
* time-seeded generators during ordinary rendering;
* preview-only random streams that differ from export;
* random streams that advance merely because a preview was cancelled.

## Channel randomness policy

The framework must support two creator-facing policies.

### Shared Arrangement

All enabled channels use:

* one shared seed;
* one shared ordered stochastic basis.

The shared basis may be represented by:

* candidate points;
* an ordered site sequence;
* a noise field;
* a random scalar field;
* another pattern-appropriate shared structure.

Each channel may then independently:

* filter that basis;
* modulate sizes;
* apply its own source mapping;
* apply channel transforms;
* control visibility;
* use different colors or opacity.

Shared Arrangement must mean more than independently running each channel with the same integer seed. The framework must create a reusable stochastic basis where the pattern supports one.

This allows channels to remain visually coordinated even when their source values produce different accepted marks or sizes.

### Independent Arrangements

Each channel has:

* its own editable seed;
* its own stochastic basis;
* its own regenerate action.

Regenerating one channel must not change:

* another channel’s seed;
* another channel’s geometry;
* unrelated pattern parameters.

A separate **Regenerate All** action may generate new seeds for every applicable channel.

## Seed storage

Seeds must be associated with stable semantic channel identifiers, not visible positions.

Examples include:

```text
cmyk.cyan
cmyk.magenta
cmyk.yellow
cmyk.black

rgb.red
rgb.green
rgb.blue
```

The framework should conceptually preserve state similar to:

```rust
enum ChannelRandomness {
    Shared {
        seed: u64,
    },
    Independent {
        channel_seeds: BTreeMap<ChannelId, u64>,
    },
}
```

The actual model may differ.

Switching between Shared Arrangement and Independent Arrangements must preserve the inactive policy’s stored seeds so switching back restores the prior state.

## Deterministic random implementation

The pattern definition or schema must identify the random-stream algorithm or version used.

The same combination of:

* pattern identifier;
* generator version;
* parameter values;
* source state;
* document state;
* channel identity;
* seed;
* randomness policy;

must reproduce equivalent geometry.

Application updates must not silently alter saved stochastic patterns without an explicit migration or generator-version change.

## Seed interface

For stochastic patterns, the interface should expose:

```text
Channel Randomness
  Shared Arrangement
  Independent Arrangements
```

Shared Arrangement should provide:

```text
Pattern Seed
Regenerate
```

Independent Arrangements should provide:

```text
Cyan / Red Seed
Magenta / Green Seed
Yellow / Blue Seed
Black Seed where applicable

Regenerate Active Channel
Regenerate All
```

Numeric seeds must be manually editable through an advanced but accessible control.

## Seed behavior

* Fixed seeds reproduce fixed geometry.
* Regenerate changes only the applicable seed or seeds.
* Regenerate does not alter unrelated parameters.
* Hiding a channel does not change its seed.
* Reordering UI controls does not reassign seeds.
* Temporarily disabling a channel does not lose its seed.
* Save and reopen preserve exact seed state.
* Undo and redo restore exact seed values and generated geometry.
* Presets preserve seed policy and values where appropriate.
* Imported custom patterns preserve seed configuration.
* Cancelled generation does not advance or replace the saved seed.
* A stale result cannot replace geometry generated from a newer seed.

Custom recipes must declare whether they:

* use randomness;
* support Shared Arrangement;
* require independent channel streams;
* are deterministic without a seed.

---

# User-defined patterns

## Initial scope

The initial custom-pattern system must support declarative pattern recipes built from registered and safe pattern components.

It must not initially execute arbitrary:

* Rust;
* Python;
* JavaScript;
* shell commands;
* native shared libraries.

A user-defined pattern may combine supported components, for example:

```text
Triangular Lattice
+ Imported SVG Mark
+ Source-Controlled Size
+ Wave Deformation
```

or:

```text
Parallel Line Field
+ Angled Orientation
+ Sine Deformation
+ Source-Controlled Width
```

## User capabilities

Users must be able to:

* begin from a built-in pattern or an empty supported recipe;
* select compatible placement, primitive, deformation, and modulation stages;
* configure exposed parameters;
* import an SVG mark or motif where supported;
* assign a creator-facing pattern name;
* save the recipe to their local pattern library;
* duplicate and modify a recipe;
* export a versioned Toniator pattern definition;
* import a compatible pattern definition;
* use the saved pattern through the ordinary pattern selector;
* embed custom definitions and required assets in a project.

## Project portability

A project using a non-built-in pattern must store enough information to reproduce it.

The project should embed:

* pattern definition;
* schema version;
* component identifiers and versions;
* parameter values;
* seed policy and seeds;
* imported SVG marks or motifs where legally and technically appropriate.

Opening a project must not silently substitute a different pattern when:

* the local pattern is missing;
* a component is unavailable;
* a schema is incompatible;
* an embedded asset is invalid.

The user must receive a visible, actionable error.

A missing pattern may be restored from an embedded project definition when that definition is valid.

## Pattern-library files

Pattern import and export must use a documented, versioned format.

The format must be:

* deterministic;
* inspectable;
* validated before use;
* bounded against pathological input;
* portable across supported platforms;
* independent of absolute local filesystem paths.

Imported assets must be copied or embedded according to a clear ownership model rather than referenced through fragile external paths.

---

# Existing behavior and migration

## Existing patterns

Current Shapes and Curves behavior must be represented through the framework without changing output.

At minimum, migrate the existing equivalents of:

* rectangular shape grid;
* full-width curve layout;
* repeated-motif curve layout;
* existing mark selection;
* existing custom SVG mark behavior.

Migration may initially use built-in generator adapters, but those adapters must enter through the same registry and output contracts as future generators.

## Compatibility

Existing projects must:

* open without user intervention;
* retain their current pattern behavior;
* render equivalent geometry;
* retain undo and redo behavior;
* save into the current document version;
* continue to export matching PNG and SVG output.

Legacy enum values may remain internally supported for migration, but newly saved projects should use stable pattern identifiers and versioned parameters.

Unknown legacy values must fail visibly rather than choosing an arbitrary default.

## Output preservation

Before visible new patterns are added, establish deterministic fixtures demonstrating that framework migration does not change current output.

Where byte-identical comparison is practical, use it.

Otherwise compare:

* mark count;
* mark positions;
* path commands;
* transformed bounds;
* clipping;
* SVG grouping;
* raster output within documented tolerances.

---

# Initial proof patterns

The framework must be validated with patterns from meaningfully different families.

These proofs must use the registry, parameter schema, canonical output, persistence, help, cancellation, and export contracts. They must not use private shortcuts.

## Proof 1 — Triangular Dot Grid

A deterministic mark-based pattern.

Required parameters should include, where appropriate:

* horizontal or nominal spacing;
* rotation;
* phase or offset;
* source-controlled mark size;
* mark selection;
* shared versus independent channel transforms where already supported.

Required behavior:

* geometrically consistent triangular spacing;
* no cumulative drift;
* complete and intentional edge coverage;
* correct behavior under document transforms;
* support for circles and compatible custom marks;
* preview, PNG, and SVG parity;
* save and reopen;
* undo and redo;
* no irrelevant seed controls.

## Proof 2 — Wave Line Field

A deterministic path-based pattern.

Required parameters should include, where appropriate:

* line spacing;
* orientation;
* amplitude;
* wavelength;
* phase;
* rotation;
* source-controlled line width.

Required behavior:

* horizontal, vertical, and angled configurations through ordinary parameters rather than separate hard-coded patterns;
* continuous paths;
* predictable orientation;
* no accidental self-intersections at normal settings;
* paths extend beyond crop bounds where needed;
* full edge and corner coverage;
* preview, PNG, and SVG parity;
* save and reopen;
* undo and redo;
* no irrelevant seed controls.

## Proof 3 — Evenly Spaced Pointillism

A stochastic mark-based pattern used to validate seeded generation.

The implementation may use a Poisson-disc, blue-noise, or another documented even-spacing method.

Required parameters should include, where appropriate:

* density;
* minimum spacing;
* source-density response;
* uniform or source-weighted mark size;
* randomness policy;
* seed values.

Required behavior:

* random-looking placement without obvious grid repetition;
* controlled avoidance of accidental collisions or excessive clumping;
* deterministic fixed-seed output;
* Shared Arrangement support;
* Independent Arrangements support;
* Regenerate Active Channel;
* Regenerate All;
* responsive cancellation during dense generation;
* no stale-preview replacement;
* predictable edge treatment;
* preview, PNG, and SVG parity.

The first implementation may deliver the proofs in separate reviewable stages. All three are required before the complete framework issue is marked Done.

---

# Pattern-library direction

After the framework and proof patterns are complete, additional pattern-library work should be tracked as separate issues or batches.

Likely families include:

## Lattice and screen patterns

* rectangular dots;
* angled dots;
* staggered grids;
* triangular grids;
* vertical lines;
* horizontal lines;
* angled lines.

## Stochastic patterns

* pointillism;
* grain;
* Poisson or blue-noise distributions;
* weighted Voronoi stippling;
* controlled clusters.

## Cellular and contour patterns

* pavers;
* pebbles;
* cellular islands;
* petroglyph-like contours;
* plasma or field contours.

## Deformed patterns

* mesh waves;
* dot mesh waves;
* dot waves;
* horizontal line waves;
* vertical line waves;
* angled line waves.

## Routed path patterns

* spirals;
* mazes;
* serpentine layouts;
* contour-following paths.

Variants that differ only by ordinary parameters should not become separate hard-coded patterns.

For example:

```text
Horizontal Line Wave
Vertical Line Wave
Angled Line Wave
```

should normally be one Wave Line Field pattern with an orientation parameter.

---

# User interface

## Pattern selection

The interface must distinguish:

* the pattern or placement system;
* the primitive or motif;
* source modulation;
* channel settings.

For Shapes, a likely conceptual organization is:

```text
Pattern
Mark
Pattern Settings
Source Response
Channel Settings
```

For Curves:

```text
Pattern or Layout
Line Shape or Motif
Pattern Settings
Source Response
Channel Settings
```

Pattern selection must display:

* creator-facing name;
* category where useful;
* built-in or user-defined status;
* clear missing-pattern errors;
* contextual help.

## Dynamic controls

Pattern-specific controls must be generated from the active schema.

The UI must correctly handle:

* conditional rows;
* advanced settings;
* per-channel parameters;
* shared parameters;
* seed-policy controls;
* custom assets;
* narrow adaptive layouts;
* keyboard navigation;
* accessible labels and descriptions.

Switching patterns must not:

* leave orphan controls;
* preserve incompatible visible values as though they apply;
* destroy the inactive pattern’s saved parameters unnecessarily;
* trigger callback loops;
* create dirty-state changes caused only by UI synchronization.

Where practical, switching away from a pattern and back should restore its prior parameter values.

## Pattern creation and management

The initial user-defined workflow must provide understandable actions for:

* Duplicate as Custom Pattern;
* Save Pattern;
* Rename Pattern;
* Export Pattern;
* Import Pattern;
* Delete Custom Pattern;
* Restore Built-in Defaults where applicable.

Destructive actions must follow the established application conventions.

---

# Performance and cancellation

Pattern generation must remain responsive at dense settings.

Generators must:

* use cooperative cancellation;
* checkpoint inside expensive loops or iterations;
* stop superseded work promptly;
* bound iteration counts;
* bound memory growth;
* avoid unbounded candidate generation;
* avoid process-global caches that grow without release;
* preserve atomic export behavior.

Cancellation must not:

* mutate saved seeds;
* partially replace a preview;
* damage an existing export destination;
* leave the interface stuck in a cancelling state.

Pattern metadata should identify potentially expensive parameters so the interface can provide suitable warnings or limits where necessary.

---

# Acceptance criteria

## Framework

* Patterns use stable identifiers rather than numeric dropdown indexes.
* Pattern definitions and parameters are versioned.
* Built-in patterns are registered through the common registry.
* Imported user patterns use the same discovery and execution path.
* Mark-based and path-based outputs are supported.
* Preview, PNG, and SVG consume equivalent canonical geometry.
* Existing Shapes and Curves projects open and render unchanged.
* Existing custom SVG marks remain supported.
* Pattern migration is deterministic and tested.
* Unknown or incompatible patterns fail visibly.
* The interface is driven by pattern parameter schemas.
* Unsupported controls are not shown as functional.
* Pattern help follows TON-002 and TON-003 conventions.
* Pattern generation is cancellable.
* Stale results cannot replace newer previews or exports.

## Proof patterns

* Triangular Dot Grid works through the common framework.
* Wave Line Field works through the common framework.
* Evenly Spaced Pointillism works through the common framework.
* Proof patterns do not use private UI, persistence, or renderer bypasses.
* Mark and path geometry remain correct under transforms.
* Edge and corner coverage is intentional.
* Save and reopen preserve pattern choice and parameters.
* Undo and redo restore exact pattern state.
* Preview, PNG, and SVG agree within documented tolerances.

## Seeds

* Deterministic patterns do not show seed controls.
* Stochastic patterns use the framework random-state service.
* Shared Arrangement produces a reusable shared stochastic basis.
* Independent Arrangements store independent per-channel seeds.
* Fixed shared seeds reproduce the same shared basis.
* Fixed independent seeds reproduce the applicable channel geometry.
* Different channel seeds produce different valid arrangements.
* Regenerate Active Channel leaves other seeds unchanged.
* Regenerate All changes all applicable seeds.
* Shared and independent seed state survives save and reopen.
* Switching randomness policy does not discard inactive seed values.
* Undo and redo restore exact seeds and geometry.
* Channel visibility and ordering do not reassign seeds.
* Cancelled generation does not advance a seed.
* Preview, PNG, and SVG use the same seeded geometry.
* The random-stream version prevents silent output changes after upgrades.

## User-defined patterns

* A user can create a declarative pattern from supported components.
* A user can use an imported SVG mark or motif.
* A custom pattern can be saved to the pattern library.
* A custom pattern can be exported and imported.
* A project can embed its custom pattern definition and assets.
* Opening the embedded project reproduces the pattern without depending on an absolute local path.
* Invalid recipes and assets fail safely with actionable errors.
* Custom recipes declare their randomness requirements.
* At least one proof pattern can be duplicated and saved as a working custom variation.

## User experience

* Pattern names and parameters use creator-facing terminology.
* Dynamic controls remain usable in wide and narrow layouts.
* Keyboard focus follows the visible workflow.
* Hidden controls leave no orphan widgets or help buttons.
* Disabled controls explain why they are unavailable.
* Built-in and custom patterns are clearly distinguishable.
* Seed synchronization is understandable without requiring knowledge of pseudorandom generators.
* The `creative_tester` finds no blocker-level confusion in selecting, adjusting, saving, importing, or regenerating patterns.

---

# Required regression coverage

Include deterministic tests and representative visual artifacts for:

* migration of every existing pattern or layout;
* unchanged legacy Shapes output;
* unchanged legacy Curves output;
* stable pattern identifiers;
* renamed display labels;
* pattern schema migration;
* missing pattern definitions;
* invalid parameter values;
* invalid custom SVG assets;
* mark-based output;
* path-based output;
* pattern switching;
* preservation of inactive pattern settings;
* shared parameters;
* per-channel parameters;
* CMYK channels;
* RGB channels;
* hidden and disabled channels;
* shared seeds;
* independent seeds;
* active-channel regeneration;
* regenerate all;
* save and reopen;
* undo and redo;
* preset save and load where supported;
* pattern import and export;
* embedded project definitions;
* wide and tall artwork;
* rotated and transformed artwork;
* edge and corner coverage;
* transparent source edges;
* internal transparent holes;
* preview versus PNG parity;
* preview versus SVG parity;
* dense-generation cancellation;
* stale-result suppression;
* repeated preview memory behavior;
* failed-export destination protection;
* narrow adaptive interface;
* keyboard and accessibility behavior.

Representative artifacts must include:

* migrated rectangular grid;
* migrated repeated-motif curve layout;
* Triangular Dot Grid;
* horizontal Wave Line Field;
* vertical or angled Wave Line Field;
* Evenly Spaced Pointillism with Shared Arrangement;
* Evenly Spaced Pointillism with Independent Arrangements;
* one imported or duplicated custom pattern;
* equivalent CMYK and RGB examples where supported.

---

# Implementation sequence

Implement TON-010 as separate reviewable stages.

## Stage 1 — Registry, identifiers, and model migration

Implement:

* pattern registry;
* stable identifiers;
* schema and generator versions;
* versioned parameter storage;
* canonical mark/path output boundary;
* document migration;
* adapters for existing Shapes and Curves behavior;
* deterministic output regressions.

Do not add new visible patterns in this stage.

Completion evidence must show that existing output remains unchanged.

## Stage 2 — Parameter-schema-driven interface

Implement:

* schema-defined controls;
* parameter scope;
* dynamic visibility;
* contextual help;
* pattern selection by stable identifier;
* restoration of per-pattern settings;
* narrow-layout and accessibility behavior.

Do not add broad new algorithm families in this stage.

## Stage 3 — Triangular Dot Grid

Implement the deterministic mark-based proof through the common framework.

Verify:

* geometry;
* source modulation;
* transforms;
* coverage;
* persistence;
* preview and export parity.

## Stage 4 — Wave Line Field

Implement the deterministic path-based proof through the common framework.

Verify:

* orientation;
* amplitude;
* wavelength;
* phase;
* path continuity;
* transformed coverage;
* persistence;
* preview and export parity.

## Stage 5 — Seed framework and Evenly Spaced Pointillism

Implement:

* explicit deterministic random-stream version;
* Shared Arrangement;
* Independent Arrangements;
* stable channel seed storage;
* seed editing;
* Regenerate Active Channel;
* Regenerate All;
* Evenly Spaced Pointillism as the stochastic proof.

Verify cancellation, repeatability, channel synchronization, persistence, and export parity.

## Stage 6 — Declarative custom-pattern recipes

Implement:

* supported component composition;
* custom naming;
* custom SVG primitive or motif;
* parameter persistence;
* randomness declarations;
* duplicate-as-custom workflow.

Do not introduce arbitrary executable code.

## Stage 7 — Pattern library import, export, and embedding

Implement:

* versioned pattern-library file format;
* validation;
* import;
* export;
* local library management;
* project embedding;
* missing-pattern recovery from embedded definitions.

## Stage 8 — Full review and follow-up catalog

Perform:

* complete regression validation;
* real preview, PNG, and SVG artifact comparison;
* performance and cancellation evaluation;
* memory-use evaluation;
* creative workflow review;
* accessibility review;
* migration review.

After the framework passes, create separate follow-up issues for additional pattern families. Do not silently add the full target catalog to this issue.

---

# Completion reporting

For every stage, report:

* files changed;
* architecture introduced or changed;
* migration behavior;
* user-visible behavior;
* tests performed;
* artifacts produced;
* performance findings;
* known limitations;
* follow-up issues discovered;
* independent review findings.

Do not mark TON-010 Done when:

* only the registry exists;
* only the interface exists;
* only one output kind works;
* user-defined patterns cannot be saved and restored;
* seeded channel behavior remains undefined;
* preview and export differ;
* existing projects change output;
* cancellation is incomplete.

TON-010 is complete only when the common framework, mark/path proofs, stochastic seed behavior, declarative custom recipe workflow, and project portability have all passed their respective review gates.

---

# Deferred capabilities

Track these separately unless they become necessary for the initial safe recipe system:

* arbitrary scripting;
* native-code pattern plugins;
* Python-based generators;
* sandboxed WASM generators;
* node-based visual pattern authoring;
* live mathematical-expression evaluation;
* third-party online pattern repositories;
* downloadable pattern marketplace;
* cloud synchronization;
* unrestricted external asset references;
* combined mark-and-path output from one generator;
* per-channel selection of entirely different generator types;
* mixed Shapes and Curves documents;
* printer-specific production patterns;
* backward-compatibility guarantees for unpublished experimental schemas.

---

# Lower-priority workflow expansion

## TON-011 — Assign halftone patterns independently per channel

* **Status:** Open
* **Priority:** P1
* **Area:** Document model / channels / pattern generation / rendering
* **Type:** Foundational workflow capability
* **Depends on:** TON-010 extensible halftone-pattern framework
* **Related:** TON-008, TON-009
* **Replaces:** Former TON-017 mixed Shapes and Curves issue

### Problem

Toniator currently treats Shapes and Curves as document-wide modes. A document chooses one generator family, and all color channels follow that global choice.

That prevents workflows in which different channels use different halftone structures. For example:

* Cyan uses a dot grid;
* Magenta uses a wave-line field;
* Yellow uses pointillism;
* Black uses a custom SVG mark;
* Red and Green share one stochastic arrangement while Blue uses a separate pattern;
* one channel uses continuous curves while another uses discrete marks.

After TON-010, Shapes, Curves, layouts, stochastic distributions, custom motifs, and user-defined recipes will all be represented as registered halftone patterns. The document model must therefore stop treating Shapes versus Curves as a global document choice.

### Required outcome

Allow every printable color channel or ordinary artwork layer to select and configure its own TON-010 halftone pattern.

Each channel must be able to use any compatible registered pattern, including:

* mark-based patterns;
* path-based patterns;
* built-in patterns;
* imported custom patterns;
* user-defined pattern recipes;
* deterministic patterns;
* stochastic patterns;
* patterns using custom SVG marks or motifs.

A single document may therefore combine dots, polygons, custom marks, line fields, waves, pointillism, stippling, cellular patterns, spirals, routed paths, and other TON-010 pattern types across its channels.

The initial implementation should use **per-channel pattern assignment** rather than introducing an unrestricted layer-compositing system.

The architecture must not prevent a later general generator-group or artwork-layer model.

---

# Document model

## Channel pattern instances

Each printable channel must own a pattern instance containing at least:

* stable pattern identifier;
* pattern schema version;
* generator or recipe version;
* pattern parameter values;
* source mapping;
* source modulation settings;
* primitive or motif selection where applicable;
* transforms;
* visibility;
* opacity or intensity;
* seed policy and seed values where applicable;
* clipping or artwork-mask reference where supported;
* embedded custom pattern definition where required for portability.

Conceptually:

```rust
struct ChannelPatternInstance {
    channel_id: ChannelId,
    pattern_id: PatternId,
    pattern_schema_version: u32,
    parameters: PatternParameters,
    source_mapping: SourceMapping,
    transform: PatternTransform,
    visibility: bool,
    opacity: f32,
    randomness: Option<ChannelRandomnessState>,
    embedded_definition: Option<EmbeddedPatternDefinition>,
}
```

The exact Rust representation may differ.

## Stable channel identity

Pattern assignments must use stable semantic channel identifiers rather than visible indexes.

Examples include:

```text
cmyk.cyan
cmyk.magenta
cmyk.yellow
cmyk.black

rgb.red
rgb.green
rgb.blue
```

Reordering channel controls, hiding a channel, or changing the active channel must not reassign pattern state.

## No global Shapes/Curves restriction

The document must no longer require a single global choice between Shapes and Curves.

Instead:

```text
Document
  Cyan  → Triangular Dot Grid
  Magenta → Wave Line Field
  Yellow → Evenly Spaced Pointillism
  Black → Custom SVG Pattern
```

or:

```text
Document
  Red → Wave Line Field
  Green → Rectangular Dot Grid
  Blue → Imported User Pattern
```

The active pattern determines whether the channel produces marks or paths through the TON-010 canonical output contract.

## Pattern state isolation

Each channel’s pattern state must remain isolated.

Changing one channel’s:

* pattern;
* parameters;
* custom mark;
* source mapping;
* transform;
* seed;
* visibility;

must not alter another channel unless an explicit linking or shared-setting feature is active.

Switching a channel from one pattern to another should preserve the inactive pattern’s previous settings where practical, so switching back restores the earlier configuration.

## Compatibility with output treatments

DTF settings from TON-009 are output-treatment settings and must remain separate from ordinary channel pattern assignment.

The following are production or derived layers, not normal user-selectable halftone channels:

* merged white base;
* knockout mask;
* garment-preview surface.

They must derive from the final resolved artwork and must not independently receive ordinary TON-010 patterns unless a future issue explicitly introduces that behavior.

---

# Pattern compatibility

## Common pattern selector

Every channel must select patterns through the TON-010 registry.

The selector may include:

* built-in patterns;
* user-defined patterns;
* imported patterns;
* recent patterns;
* compatible pattern categories.

Pattern selection must use stable identifiers rather than numeric dropdown positions.

## Mark and path patterns

A channel may use either:

```text
PatternOutput::Marks
```

or:

```text
PatternOutput::Paths
```

without changing the document-wide mode.

The renderer must accept both output kinds in one document and composite them deterministically.

Mark- and path-based channels must use the same:

* source sampling;
* cancellation;
* stale-result suppression;
* clipping;
* preview;
* PNG export;
* SVG export;
* persistence;
* undo and redo infrastructure.

## Compatibility declarations

A pattern must declare whether it is compatible with:

* CMYK channels;
* RGB channels;
* Crosshatch or directional layers where applicable;
* mark output;
* path output;
* custom motifs;
* shared stochastic arrangements;
* per-channel stochastic arrangements.

The interface must not allow an incompatible pattern to appear usable.

An incompatible pattern should be:

* omitted from the normal compatible list; or
* visibly disabled with an explanation.

It must not silently fall back to another pattern.

---

# Shared and independent channel configuration

## Independent patterns

By default, every channel may select and configure its pattern independently.

Example:

```text
Cyan
  Pattern: Triangular Dot Grid
  Spacing: 12 px
  Rotation: 15°

Magenta
  Pattern: Wave Line Field
  Spacing: 9 px
  Amplitude: 18 px

Yellow
  Pattern: Evenly Spaced Pointillism
  Minimum Spacing: 5 px
  Seed: 148273

Black
  Pattern: Custom Anchor Mark
  Grid Rotation: 45°
```

## Linked pattern settings

To avoid forcing users to configure the same pattern repeatedly, Toniator should support explicit linking.

Creator-facing options may include:

```text
Pattern Assignment
  Independent per Channel
  Same Pattern for All Channels
```

When the same pattern is assigned to multiple channels, the user should be able to choose which settings are linked.

Potential scopes include:

* pattern identity only;
* placement and geometry;
* primitive or motif;
* deformation;
* source response;
* transforms;
* stochastic arrangement;
* all compatible pattern settings.

The initial implementation may provide a smaller, understandable set such as:

```text
Use Same Pattern
Link Pattern Settings
Link Arrangement
```

Do not expose a highly granular linking matrix unless the simpler model proves insufficient.

## Linking semantics

Linked settings must be explicit.

Changing a linked value must update all participating channels as one undoable operation.

Unlinking channels must copy the current linked values into each channel’s independent state so the visible result does not change merely because linking was disabled.

Relinking must not silently discard differing channel settings without confirmation or a documented deterministic policy.

## Mixed pattern assignments

Linking must not prevent mixed documents.

A user must be able to:

* link Cyan and Magenta;
* leave Yellow independent;
* assign Black an entirely different pattern.

The model should support channel groups conceptually, even if the initial interface only exposes common presets such as All Channels or Independent.

---

# Randomness and channel arrangements

TON-010 defines shared and independent stochastic arrangements. TON-011 must integrate that behavior with per-channel pattern assignment.

## Same stochastic pattern across channels

When multiple channels use the same compatible stochastic pattern, users may choose:

* **Shared Arrangement**
* **Independent Arrangements**

### Shared Arrangement

Participating channels use:

* the same pattern generator;
* a shared stochastic basis;
* a shared seed;
* independent source modulation and channel appearance.

The channels may still produce different accepted marks or sizes because their source values differ, but their randomness must originate from the same candidate field or site sequence.

### Independent Arrangements

Each channel retains:

* its own seed;
* its own candidate field;
* its own regenerated geometry.

Regenerating one channel must not change another.

## Different stochastic patterns

Channels using different stochastic pattern generators cannot necessarily share a meaningful random basis.

In that case:

* their seeds remain independently stored;
* the interface must not imply that Shared Arrangement applies across incompatible algorithms;
* a shared master seed may be used only as a deterministic source for deriving independent pattern-specific streams, if clearly documented.

## Seed preservation

Changing a channel’s pattern must not destroy the saved seed state for the previous pattern where practical.

Seed state must survive:

* save and reopen;
* undo and redo;
* channel visibility changes;
* CMYK/RGB mode switching;
* pattern switching;
* linking and unlinking;
* pattern duplication;
* project embedding.

---

# User interface

## Channel-centered workflow

The main editing workflow should become channel-centered rather than document-mode-centered.

A likely conceptual hierarchy is:

```text
Active Channel
Pattern
Pattern Settings
Source Response
Channel Appearance
```

For example:

```text
Channel: Magenta

Pattern
  Wave Line Field

Pattern Settings
  Orientation
  Line Spacing
  Amplitude
  Wavelength
  Phase

Source Response
  Stroke Width from Value

Channel
  Visibility
  Opacity
  Mapping
```

## Active context

The interface must always make clear:

* active output model;
* active channel;
* active pattern;
* whether settings are shared or independent;
* whether a stochastic arrangement is shared;
* which channels are linked;
* whether the selected pattern is built-in or custom.

Changing the active channel must update all pattern controls without callback loops or unintended document edits.

## Pattern overview

Users need a compact way to understand the complete document assignment.

A channel overview may resemble:

```text
Cyan       Triangular Dot Grid
Magenta    Wave Line Field
Yellow     Evenly Spaced Pointillism
Black      Custom Anchor Pattern
```

The overview should communicate:

* channel visibility;
* pattern name;
* custom or built-in status;
* linking state;
* warning or missing-pattern state.

It should not require opening every channel individually merely to determine what the document contains.

## Pattern assignment actions

Useful actions may include:

* Apply Pattern to Active Channel;
* Apply Pattern to All Channels;
* Copy Pattern Settings;
* Paste Pattern Settings;
* Duplicate Pattern to Other Channels;
* Link Selected Channels;
* Unlink Selected Channels;
* Reset Active Channel Pattern;
* Replace Pattern While Preserving Compatible Settings.

Actions affecting multiple channels must preview or clearly describe their scope.

## Dynamic controls

The inspector must be driven by the active pattern’s TON-010 parameter schema.

Switching channels or patterns must not leave:

* controls belonging to the previous pattern;
* orphan help buttons;
* empty containers;
* invalid focus targets;
* enabled controls that the pattern ignores;
* stale seed controls;
* mark controls visible for path-only patterns;
* path controls visible for mark-only patterns.

## Wide and narrow layouts

Per-channel pattern assignment must remain usable in:

* the normal wide inspector;
* the adaptive narrow overlay;
* keyboard-only operation;
* common display scaling factors.

The interface must not require an excessively wide window merely because channels use different patterns.

---

# Rendering and compositing

## Deterministic render order

Channels must render in a stable, documented order.

For CMYK, the default order should remain consistent with the existing CMYK pipeline unless explicitly changed.

For RGB Screen, ordering and Screen compositing must remain consistent with TON-008.

The result must not depend on:

* active channel;
* edit history;
* registry insertion order;
* asynchronous worker completion order;
* UI ordering.

## Unified render plan

The renderer should construct a deterministic document render plan containing one entry per enabled channel.

Conceptually:

```rust
struct ChannelRenderPlan {
    channel_id: ChannelId,
    pattern_output: PatternOutput,
    color: RenderColor,
    opacity: f32,
    blend_mode: BlendMode,
    clip: Option<ClipGeometry>,
}
```

The exact structure may differ.

All channel outputs must be resolved before final compositing in stable order.

## Canonical geometry

Each channel’s selected TON-010 pattern generates canonical marks or paths.

The same channel geometry must feed:

* live preview;
* PNG export;
* SVG export.

Mixed pattern output must not cause one export path to regenerate or reinterpret geometry differently.

## SVG organization

Exported SVG must remain organized and editable.

At minimum, the SVG should contain clearly named groups such as:

```text
Toniator Artwork
  Cyan — Triangular Dot Grid
  Magenta — Wave Line Field
  Yellow — Evenly Spaced Pointillism
  Black — Custom Anchor Pattern
```

Group identifiers must be:

* stable;
* unique;
* safe for SVG/XML;
* independent of translated display labels where necessary.

Custom pattern metadata may be included in a non-disruptive form where useful.

## PNG output

PNG export must composite all enabled channel outputs in the same order and with the same blending rules as the live preview.

Transparent areas must remain transparent unless an explicit Export Background is enabled.

## DTF integration

When DTF treatment is enabled later:

1. all ordinary channel patterns generate and composite;
2. knockout is resolved;
3. merged white-base coverage is derived;
4. garment preview or production export is produced.

DTF must not independently rerun channel pattern generation under different rules.

---

# Persistence and migration

## Legacy document migration

Existing documents that use one global Shapes or Curves mode must migrate without changing their output.

### Existing Shapes document

Migration should assign the equivalent registered shape pattern and settings to every existing color channel.

Shared settings should migrate into an explicit linked or shared configuration where that preserves current behavior.

### Existing Curves document

Migration should assign the equivalent registered curve pattern or layout to every existing color channel.

Existing:

* path geometry;
* motif settings;
* layout settings;
* per-channel transforms;
* source mappings;

must remain intact.

### Crosshatch and specialized modes

Existing specialized layer modes must either:

* migrate into compatible per-layer pattern assignments; or
* remain represented through a documented compatibility adapter until fully integrated.

Migration must not silently reinterpret directional Crosshatch layers as ordinary CMYK or RGB channels.

## Document versioning

The new per-channel assignment model requires an explicit document-version migration.

New documents must serialize:

* channel identifiers;
* pattern identifiers;
* pattern versions;
* channel parameters;
* linking state;
* seed state;
* embedded custom definitions.

Older documents must continue to open.

Newer documents using per-channel patterns must fail visibly and safely when opened by unsupported older builds where such detection is possible.

## Save and reopen

Save and reopen must restore exactly:

* pattern assigned to each channel;
* all per-channel parameters;
* linked settings;
* pattern-specific inactive state;
* custom definitions;
* seeds;
* visibility;
* rendering order;
* active channel where appropriate.

---

# Undo and redo

Undo and redo must preserve channel-specific operations.

Examples include:

* changing one channel’s pattern;
* changing one parameter;
* applying a pattern to all channels;
* linking channels;
* unlinking channels;
* copying settings between channels;
* regenerating one seed;
* regenerating all seeds;
* importing a custom pattern;
* replacing a missing pattern;
* switching artwork model.

A multi-channel operation must normally be one coherent undo step.

Undo must not restore only the visible UI while leaving generated geometry or hidden channel state inconsistent.

---

# Performance and cancellation

Per-channel pattern assignment may cause multiple fundamentally different generators to run during one preview.

The implementation must therefore:

* preserve cooperative cancellation;
* checkpoint inside each generator;
* stop remaining channel generation when the whole preview is cancelled;
* suppress results from superseded document generations;
* avoid allowing a slow channel to replace a newer complete preview;
* bound memory retained by inactive pattern state;
* release canonical geometry from stale previews;
* avoid recomputing unchanged channels where safe caching is available.

## Channel-level invalidation

Changing one channel should invalidate only the necessary work where practical.

For example:

* changing Cyan’s pattern need not regenerate unchanged Magenta, Yellow, and Black geometry;
* changing an Export Background need not regenerate channel geometry;
* changing a shared linked parameter must invalidate every affected channel;
* changing the shared stochastic basis must invalidate every participating channel.

Optimization must not compromise correctness. A full-document fallback is acceptable initially if it remains responsive and deterministic.

## Cancellation

Cancellation must leave:

* saved pattern assignments unchanged;
* seeds unchanged;
* the previous valid preview intact where appropriate;
* export destinations undamaged;
* the interface out of any permanent cancelling state.

---

# Error handling

The document may contain different failure conditions on different channels.

Examples include:

* missing custom pattern;
* incompatible pattern version;
* invalid imported SVG mark;
* unsupported pattern for the active color model;
* invalid parameter;
* generator failure;
* resource limit exceeded.

Errors must identify:

* affected channel;
* affected pattern;
* cause;
* available recovery action.

Toniator must not silently replace a failed channel pattern with:

* the first registered pattern;
* a rectangular grid;
* the pattern assigned to another channel;
* an empty result presented as success.

Where possible, unaffected channels may continue to preview while the failed channel is clearly marked. Export behavior must be explicit about whether partial export is prohibited or deliberately permitted.

---

# Acceptance criteria

## Per-channel assignment

* Every CMYK channel can select its own TON-010 pattern.
* Every RGB channel can select its own compatible TON-010 pattern.
* Mark-based and path-based patterns coexist in one document.
* Built-in and user-defined patterns can coexist.
* One channel’s pattern changes do not corrupt another channel’s state.
* Pattern selection uses stable identifiers.
* The active channel and active pattern are always clear.
* The complete channel-to-pattern assignment can be inspected without opening every channel.
* Incompatible patterns are not presented as silently usable.

## Shared and independent settings

* Channels may use independent patterns and parameters.
* Compatible channels may share the same pattern.
* Compatible parameters may be linked.
* Unlinking preserves the current visible result.
* Linking and unlinking are undoable.
* Mixed linked and independent channel groups are supported.
* Shared stochastic arrangements behave according to TON-010.
* Independent channel seeds remain independent.
* Regeneration scope is explicit.

## Rendering

* Mark and path outputs render together correctly.
* Preview order is deterministic.
* PNG compositing order is deterministic.
* SVG group order is deterministic.
* Preview, PNG, and SVG use equivalent canonical geometry.
* Transparent output remains transparent.
* Existing CMYK compositing remains unchanged.
* Existing RGB Screen compositing remains unchanged.
* DTF treatment can consume the resolved mixed-pattern artwork without regenerating it differently.

## Persistence

* Save and reopen restore every channel assignment.
* Save and reopen restore all parameters.
* Custom pattern definitions remain portable.
* Linked settings are restored.
* Seed policy and values are restored.
* Inactive per-pattern settings are preserved where supported.
* Existing single-mode Shapes documents migrate without visible output changes.
* Existing single-mode Curves documents migrate without visible output changes.
* Existing project files continue to open.

## User experience

* Irrelevant controls are hidden.
* Hidden controls leave no orphan widgets.
* Keyboard navigation follows the active workflow.
* Wide and narrow layouts remain usable.
* Multi-channel actions clearly state their scope.
* Built-in and custom patterns are distinguishable.
* Missing or incompatible patterns produce actionable errors.
* The `creative_tester` finds no blocker-level confusion in assigning and editing different patterns per channel.

## Reliability

* Undo and redo restore exact mixed-pattern state.
* Dense mixed-pattern previews remain cancellable.
* Stale results cannot replace newer previews.
* Failed exports cannot damage existing destinations.
* Repeated preview generation does not cause unbounded memory growth.
* Errors identify the affected channel and pattern.
* Unaffected legacy output remains unchanged.

---

# Required regression coverage

Include deterministic tests and representative artifacts for:

* legacy Shapes migration;
* legacy Curves migration;
* CMYK with four different patterns;
* RGB with three different patterns;
* marks and paths in one document;
* built-in and custom patterns together;
* same pattern on all channels;
* different pattern on every channel;
* linked pattern identity;
* linked parameters;
* unlinking;
* partially linked channel groups;
* shared stochastic arrangement;
* independent per-channel seeds;
* regenerate active channel;
* regenerate all;
* channel visibility changes;
* channel reordering in the UI;
* pattern switching and restoration;
* custom SVG marks;
* missing custom patterns;
* invalid embedded definitions;
* unsupported pattern/color-model combinations;
* save and reopen;
* undo and redo;
* preview versus PNG parity;
* preview versus SVG parity;
* stable SVG grouping and naming;
* transparent source edges;
* internal transparent holes;
* wide and tall documents;
* transformed documents;
* dense-generation cancellation;
* stale-result suppression;
* failed-export destination protection;
* repeated-preview memory behavior;
* narrow adaptive UI;
* keyboard navigation;
* accessibility labels and descriptions.

Representative artifacts should include at least:

```text
CMYK Mixed Pattern
  Cyan — Triangular Dot Grid
  Magenta — Wave Line Field
  Yellow — Evenly Spaced Pointillism
  Black — Custom Pattern

RGB Mixed Pattern
  Red — Wave Line Field
  Green — Rectangular Grid
  Blue — Pointillism
```

Also include:

* one document with all channels linked to one pattern;
* one document with Shared Arrangement;
* one document with Independent Arrangements;
* one migrated legacy Shapes document;
* one migrated legacy Curves document.

---

# Implementation sequence

Implement TON-011 as separate reviewable stages.

## Stage 1 — Per-channel document model and migration

Implement:

* channel-owned pattern instances;
* stable channel identifiers;
* document-version migration;
* migration of global Shapes documents;
* migration of global Curves documents;
* persistence;
* undo and redo;
* unchanged legacy output fixtures.

Do not implement a broad new interface in this stage beyond what is necessary for testing.

## Stage 2 — Per-channel pattern selection UI

Implement:

* active-channel selector;
* per-channel pattern selector;
* schema-driven active-pattern controls;
* channel assignment overview;
* dynamic control replacement;
* narrow-layout behavior;
* keyboard and accessibility support.

Do not add linking yet unless it is inseparable from preserving migrated shared behavior.

## Stage 3 — Mixed mark and path rendering

Implement:

* deterministic multi-channel render plan;
* mark and path coexistence;
* stable compositing;
* preview integration;
* PNG integration;
* SVG grouping and naming;
* cancellation and stale-result suppression.

Verify one CMYK and one RGB mixed-pattern document.

## Stage 4 — Shared and linked channel settings

Implement:

* same pattern across selected channels;
* linked compatible settings;
* unlinking with state preservation;
* grouped multi-channel undo;
* assignment copying;
* clear linking-state UI.

Keep the initial linking model understandable rather than exposing every possible parameter relationship.

## Stage 5 — Stochastic coordination

Integrate TON-010 randomness with per-channel assignments:

* Shared Arrangement;
* Independent Arrangements;
* stable per-channel seeds;
* regenerate active channel;
* regenerate all;
* compatible and incompatible stochastic patterns;
* persistence and undo/redo.

## Stage 6 — Custom-pattern portability

Verify:

* different embedded custom patterns on different channels;
* custom SVG assets;
* import and export;
* missing-pattern recovery;
* invalid-definition errors;
* stable project portability.

## Stage 7 — Performance and final review

Evaluate:

* channel-level invalidation;
* caching;
* repeated previews;
* dense mixed generators;
* cancellation;
* memory behavior;
* preview/PNG/SVG parity;
* creative workflow;
* accessibility;
* migration;
* regression safety.

Do not implement all stages as one unreviewable change.

---

# Completion reporting

For every stage, report:

* files changed;
* document-model changes;
* migration behavior;
* user-visible workflow;
* render-plan changes;
* persistence behavior;
* tests performed;
* artifacts produced;
* performance findings;
* review findings;
* known limitations;
* follow-up issues discovered.

Do not mark TON-011 Done when:

* only per-channel pattern fields exist;
* only the UI exists;
* mark and path outputs cannot coexist;
* preview and export differ;
* migration changes legacy output;
* custom patterns are not portable;
* seed behavior is undefined;
* linking corrupts independent settings;
* cancellation is incomplete;
* only CMYK or only RGB has been tested.

TON-011 is complete only when channels can independently use any compatible TON-010 pattern, mixed mark/path documents render consistently, persistence and migration are reliable, stochastic coordination works, and the complete workflow passes creative and regression review.

---

# Deferred capabilities

Track these separately unless required by the initial per-channel implementation:

* unlimited arbitrary artwork layers;
* multiple generator groups assigned to the same channel;
* per-generator masks;
* nested pattern groups;
* layer folders;
* arbitrary user-controlled blend modes;
* cross-channel clipping relationships;
* multiple source images in one document;
* adjustment layers;
* pattern masks painted directly in Toniator;
* combined mark-and-path output from one pattern generator;
* pattern assignment to DTF white-base or knockout production layers;
* general-purpose vector compositing;
* fully node-based document construction.

The smallest stable first architecture is **one TON-010 pattern instance per printable channel**, with explicit linking where needed. That delivers the mixed Shapes/Curves result while avoiding an immediate expansion into a full general-purpose layer system.

---

# TON-012 — Separate artwork source sampling, output model, and channel assignment

* **Status:** Open
* **Priority:** P1
* **Area:** Artwork mapping / source sampling / color models
* **Type:** Model and workflow refactor
* **Depends on:** TON-008 RGB stabilization
* **Blocks:** TON-010 pattern framework and TON-011 Advanced Pattern Mixing
* **Related:** TON-009 DTF output treatment
* **Implementation order:** Complete after TON-008 and before TON-010

## Problem

Toniator currently combines several unrelated decisions into one **Artwork Mapping** dropdown.

Current choices such as:

```text
Color → CMYK Inks
Brightness → One Ink
Brightness → All Inks
Brightness → Crosshatch
RGB Color → Screen
```

simultaneously determine:

* which source information is sampled;
* whether output is CMYK or RGB;
* how many channels or layers receive the sampled values;
* whether a specialized Crosshatch workflow is active.

This model is difficult to understand and creates unnecessary coupling between source interpretation, output color model, channel routing, and pattern selection.

It also causes source mappings to change the output model, even though choosing a source channel should not inherently determine whether the document uses CMYK Print or RGB Screen output.

The architecture will become increasingly difficult to maintain when TON-010 adds registered patterns and TON-011 adds optional per-channel pattern assignment.

## Required outcome

Replace the combined Artwork Mapping preset model with separate, explicit controls for:

1. **Artwork Source**
2. **Source Alpha behavior**
3. **Output Model**
4. **Channel Assignment**, where applicable

These concepts must be represented independently in the document model, interface, persistence, rendering pipeline, and migration code.

Changing one concept must not silently change another unless the user loads a full document preset that explicitly contains both settings.

---

# Conceptual model

The source-to-output pipeline should be represented conceptually as:

```text
Imported artwork
→ Artwork Source sampling
→ Source Alpha handling
→ Channel Assignment
→ CMYK or RGB output channels
→ Active halftone pattern
→ Preview and export
```

Each stage must have a clear responsibility.

## Artwork Source

Artwork Source determines which information Toniator samples from the imported artwork.

Recommended creator-facing choices are:

* **Full Color** — default;
* **Red Channel**;
* **Green Channel**;
* **Blue Channel**;
* **Value**;
* **Perceptual Lightness**;
* **Alpha**.

Internally these may use stable identifiers such as:

```text
source.full_color
source.red
source.green
source.blue
source.value
source.perceptual_lightness
source.alpha
```

The selected Artwork Source must not encode or imply the output model.

For example:

```text
Full Color + CMYK Print
Full Color + RGB Screen
Perceptual Lightness + CMYK Print
Perceptual Lightness + RGB Screen
```

must all be representable.

## Full Color

Full Color uses the source image’s RGB color information.

Its interpretation depends on the selected Output Model:

### Full Color with CMYK Print

The source RGB color is converted into the documented CMYK channel values used by Toniator’s CMYK rendering pipeline.

### Full Color with RGB Screen

The source RGB components map to the Red, Green, and Blue output channels according to TON-008.

The Artwork Source setting remains `Full Color` in both cases. Changing the Output Model changes the destination interpretation, not the source selection.

## Red, Green, and Blue channels

These source modes produce a scalar field from the selected RGB component.

For example:

```text
Red Channel
→ scalar red-component values
→ assigned to the selected output channel or channels
```

Selecting Red Channel must not automatically select RGB Screen output.

A Red source channel may intentionally drive:

* one CMYK ink;
* all CMYK inks;
* one RGB channel;
* all RGB channels;
* a later per-channel assignment under TON-011.

## Value

Value must have an exact documented definition.

Recommended definition:

```text
Value = maximum of the source red, green, and blue components
```

This corresponds to the Value component of HSV.

The interface help must explain that Value emphasizes the strongest RGB component and is not perceptually uniform.

## Perceptual Lightness

Perceptual Lightness must use one explicitly documented perceptual model.

The recommended definition is the lightness component of the color model already used by Toniator for perceptual color operations, such as OKLab `L`.

It must not silently alternate between:

* HSL Lightness;
* HSV Value;
* Rec. 601 luma;
* Rec. 709 luma;
* linear-light luminance;
* OKLab lightness.

The UI may display:

```text
Perceptual Lightness
```

with contextual help identifying the actual underlying model.

## Alpha

Alpha uses source opacity as the scalar artwork source.

Examples include:

* generating marks from the silhouette of transparent artwork;
* ignoring color and responding only to coverage;
* using transparency as a mask source.

Selecting Alpha must not cause alpha to be applied twice.

When Alpha is the Artwork Source, the rendering pipeline must distinguish:

* alpha as sampled tonal data;
* alpha as an artwork-coverage mask.

The exact interaction must be explicit and tested.

---

# Source Alpha behavior

Alpha handling should be separate from Artwork Source rather than represented by separate `RGBA` and `RGB` mapping presets.

Recommended choices are:

* **Preserve Source Alpha** — default;
* **Ignore Source Alpha**.

## Preserve Source Alpha

Source alpha limits or scales generated artwork coverage according to the documented sampling model.

Fully transparent source regions must not generate unintended marks or paths.

Partially transparent regions must behave consistently between preview, PNG, and SVG.

## Ignore Source Alpha

Source alpha does not suppress artwork generation.

Because transparent pixels may contain arbitrary hidden RGB values, choosing Ignore Source Alpha for artwork containing transparency must:

* provide clear contextual help;
* avoid silently treating undefined hidden colors as trustworthy;
* use a documented interpretation;
* warn when the source contains transparency if necessary.

A future source-matte option may be added separately if needed.

## Alpha as the selected Artwork Source

When Artwork Source is Alpha:

* the alpha values become the sampled scalar field;
* the source-alpha policy must not accidentally multiply the same alpha values into the result a second time;
* fully transparent source pixels may still produce zero source values without being independently clipped twice;
* behavior must be documented through tests and contextual help.

---

# Output Model

Output Model determines the document’s printable or display-oriented color-channel system.

Initial choices are:

* **CMYK Print** — default;
* **RGB Screen**.

Stable identifiers should be used internally, such as:

```text
output.cmyk
output.rgb_screen
```

Output Model must control:

* available output channels;
* channel names;
* channel colors;
* channel count;
* compositing behavior;
* applicable controls;
* export interpretation.

It must not determine which Artwork Source is selected.

## CMYK Print

CMYK Print provides:

* Cyan;
* Magenta;
* Yellow;
* Black.

It retains Toniator’s documented subtractive print-oriented behavior.

## RGB Screen

RGB Screen provides:

* Red;
* Green;
* Blue.

It retains TON-008’s documented Screen compositing behavior.

Changing Output Model must:

* preserve the selected Artwork Source where it remains valid;
* preserve mode-specific channel state;
* update channel controls immediately;
* not infer a different source from a display label;
* not rely on mapping dropdown indexes;
* not silently reinterpret incompatible saved settings.

## Automatic mode switching

Selecting an Artwork Source must not automatically change Output Model.

For example:

* selecting Full Color does not imply CMYK;
* selecting Red Channel does not imply RGB;
* selecting Perceptual Lightness does not imply monochrome output.

A complete preset may intentionally set both Artwork Source and Output Model, but the preset must visibly represent both settings.

Loading an explicitly RGB document or full RGB preset may select RGB Screen.

Loading a source-only mapping must not.

---

# Channel Assignment

Artwork Source determines what values are sampled. Channel Assignment determines where those values are applied when the source produces a scalar field.

Recommended choices include:

* **Automatic Color Separation**;
* **Active Channel**;
* **All Channels**.

Additional choices may be added later through TON-011.

## Automatic Color Separation

Automatic Color Separation is available when Artwork Source is Full Color.

With CMYK Print:

* source color is converted into CMYK channel values.

With RGB Screen:

* source red, green, and blue components map to their corresponding RGB output channels.

Automatic Color Separation should normally be the default for Full Color.

## Active Channel

The selected scalar source drives only the active output channel.

Examples:

```text
Perceptual Lightness
→ Active Channel
→ Black
```

or:

```text
Red Channel
→ Active Channel
→ Cyan
```

This replaces the current bundled concept:

```text
Brightness → One Ink
```

without hard-coding Brightness as the only scalar source.

## All Channels

The selected scalar source drives all enabled output channels.

Each channel may retain its own:

* pattern transform;
* angle;
* visibility;
* opacity;
* color;
* pattern settings where supported.

This replaces:

```text
Brightness → All Inks
```

without tying that routing behavior to one source interpretation.

## Conditional presentation

Channel Assignment should be shown only where it has a meaningful effect.

For example:

* Full Color with Automatic Color Separation does not require an Active/All choice.
* Scalar source modes expose Active Channel and All Channels.
* Irrelevant routing controls must not remain visible and enabled.
* Hidden routing controls must leave no orphan help widgets or focus targets.

---

# Crosshatch handling

Crosshatch is not an Artwork Source and must not remain represented as:

```text
Brightness → Crosshatch
```

Crosshatch describes a generated pattern or output-layer structure, not source sampling.

## Before TON-010

Until TON-010 provides the appropriate registered-pattern architecture:

* existing Crosshatch projects must continue to open and render;
* a compatibility adapter may preserve the legacy behavior;
* migration must not change existing Crosshatch output;
* the new Artwork Source model may represent its source as Value or Perceptual Lightness according to the existing actual behavior;
* the compatibility adapter remains responsible for the directional Crosshatch layers.

## After TON-010

Crosshatch should migrate to one of the following, based on the final pattern architecture:

* a registered path-based pattern;
* a registered multi-directional pattern recipe;
* a documented specialized compatibility pattern.

TON-012 must not invent the complete TON-010 pattern implementation.

It must, however, remove the architectural assumption that Crosshatch is a source mapping.

---

# Standard and advanced behavior

## Standard workflow

The standard interface should expose a concise structure such as:

```text
Artwork Source
  Full Color

Output Model
  CMYK Print
```

When the selected source is scalar:

```text
Artwork Source
  Perceptual Lightness

Apply To
  Active Channel
```

The interface should not present users with combined technical presets that must be decoded.

## TON-011 advanced workflow

When TON-011 Advanced Pattern Mixing is eventually enabled, each printable channel may have its own:

* pattern;
* source selection;
* source modulation;
* channel assignment where applicable.

TON-012 must establish a model that TON-011 can extend without duplicating or replacing it.

TON-012 does not implement per-channel pattern mixing.

---

# Presets

Presets must distinguish between:

## Source presets

A source preset changes only:

* Artwork Source;
* Source Alpha behavior;
* related source-sampling parameters.

It must not silently change Output Model.

## Output presets

An output preset changes only:

* Output Model;
* output-channel defaults;
* compositing-related defaults.

It must not silently change Artwork Source.

## Complete workflow presets

A complete workflow preset may intentionally contain:

* Artwork Source;
* Source Alpha behavior;
* Output Model;
* Channel Assignment;
* pattern;
* channel configuration.

Its name and preview must make that broader scope clear.

Loading a complete preset must be one coherent undoable operation.

---

# Document model and persistence

The document must store these concepts independently.

Conceptually:

```rust
struct ArtworkSamplingSettings {
    source: ArtworkSource,
    alpha_policy: SourceAlphaPolicy,
    channel_assignment: ChannelAssignment,
}

struct DocumentColorSettings {
    output_model: OutputModel,
}
```

The exact representation may differ.

The model must not derive Output Model by examining Artwork Source or vice versa.

## Stable identifiers

Serialized values must use stable semantic identifiers rather than:

* dropdown indexes;
* translated labels;
* enum declaration order where that would make migration unsafe.

## Save and reopen

Save and reopen must preserve:

* Artwork Source;
* Source Alpha behavior;
* Output Model;
* Channel Assignment;
* mode-specific channel settings;
* active channel where appropriate;
* compatibility state for legacy Crosshatch documents.

## Undo and redo

The following must be independently undoable:

* changing Artwork Source;
* changing alpha policy;
* changing Output Model;
* changing Channel Assignment;
* loading a source preset;
* loading an output preset;
* loading a complete workflow preset.

UI synchronization must not create extra undo entries.

---

# Legacy mapping migration

Existing mapping values must migrate deterministically.

Recommended conceptual migration:

| Legacy mapping          | Artwork Source                 | Output Model                 | Channel Assignment               |
| ----------------------- | ------------------------------ | ---------------------------- | -------------------------------- |
| Color → CMYK Inks       | Full Color                     | CMYK Print                   | Automatic Color Separation       |
| Brightness → One Ink    | Existing brightness definition | Preserve current model       | Active Channel                   |
| Brightness → All Inks   | Existing brightness definition | Preserve current model       | All Channels                     |
| Brightness → Crosshatch | Existing brightness definition | Preserve compatibility state | Crosshatch compatibility adapter |
| RGB Color → Screen      | Full Color                     | RGB Screen                   | Automatic Color Separation       |

Before migration, determine whether the legacy “Brightness” calculation actually uses:

* HSV Value;
* HSL Lightness;
* luma;
* luminance;
* another formula.

Do not silently rename it to Value or Perceptual Lightness until its actual behavior is identified.

Preserve legacy rendering where necessary, even if the legacy formula is retained temporarily under a compatibility identifier.

---

# User interface requirements

The new interface must:

* place Artwork Source and Output Model in separate clearly labeled controls;
* show Channel Assignment only when applicable;
* use creator-facing terminology;
* provide contextual help;
* update immediately when settings change;
* work in wide and narrow layouts;
* remain keyboard accessible;
* avoid callback loops;
* avoid dirty-state changes caused only by synchronization;
* hide irrelevant controls cleanly;
* clearly identify compatibility behavior for legacy Crosshatch documents.

The active Output Model should remain visible without requiring the user to infer it from channel names or source-mapping text.

A likely organization is:

```text
Artwork

  Artwork Source
    Full Color

  Source Alpha
    Preserve

Output

  Output Model
    CMYK Print
```

For scalar sources:

```text
Artwork Source
  Perceptual Lightness

Apply To
  All Channels
```

---

# Acceptance criteria

## Model separation

* Artwork Source is stored independently from Output Model.
* Source Alpha behavior is stored independently.
* Channel Assignment is stored independently.
* Selecting an Artwork Source does not silently change Output Model.
* Selecting an Output Model does not silently replace Artwork Source.
* Source-only presets do not change Output Model.
* Output-only presets do not change Artwork Source.
* Complete workflow presets clearly and intentionally change both.

## Artwork sources

* Full Color works with CMYK Print.
* Full Color works with RGB Screen.
* Red Channel works as a scalar source.
* Green Channel works as a scalar source.
* Blue Channel works as a scalar source.
* Value uses one documented formula.
* Perceptual Lightness uses one documented formula.
* Alpha works as a scalar source without accidental double application.
* Transparent source behavior follows the selected alpha policy.
* Preview, PNG, and SVG interpret every source consistently.

## Output models

* CMYK Print remains the default for new documents.
* RGB Screen remains available and preserves TON-008 behavior.
* Output-model changes update channels and controls immediately.
* CMYK-specific state survives CMYK → RGB → CMYK switching.
* RGB-specific state survives RGB → CMYK → RGB switching.
* Existing CMYK and RGB projects continue to render unchanged after migration.

## Channel assignment

* Full Color supports Automatic Color Separation.
* Scalar sources support Active Channel.
* Scalar sources support All Channels.
* Active Channel affects only the selected output channel.
* All Channels affects every enabled output channel.
* Irrelevant assignment controls are not shown as functional.
* Channel Assignment survives save/reopen and undo/redo.

## Crosshatch compatibility

* Existing Crosshatch documents continue to open.
* Existing Crosshatch output remains unchanged.
* Crosshatch is no longer conceptually treated as an Artwork Source.
* TON-012 does not implement or preempt the broader TON-010 pattern framework.

## User experience

* Users can identify the current Artwork Source directly.
* Users can identify the current Output Model directly.
* Users do not need to decode combined labels such as `Color → CMYK Inks`.
* Help explains Value versus Perceptual Lightness.
* Help explains Preserve versus Ignore Source Alpha.
* Standard import-to-export workflow remains straightforward.
* Narrow and wide layouts remain usable.
* The `creative_tester` finds no blocker-level confusion between source selection, output model, and channel assignment.

---

# Required regression coverage

Include deterministic tests and representative artifacts for:

* every legacy mapping migration;
* Full Color to CMYK;
* Full Color to RGB;
* Red source;
* Green source;
* Blue source;
* Value source;
* Perceptual Lightness source;
* Alpha source;
* preserved source alpha;
* ignored source alpha;
* partially transparent artwork;
* fully transparent edges;
* internal transparent holes;
* Active Channel assignment;
* All Channels assignment;
* Automatic Color Separation;
* CMYK → RGB → CMYK switching;
* RGB → CMYK → RGB switching;
* mode-specific state preservation;
* save and reopen;
* undo and redo;
* source preset behavior;
* output preset behavior;
* complete workflow preset behavior;
* legacy Crosshatch compatibility;
* preview versus PNG parity;
* preview versus SVG parity;
* wide and narrow interface;
* keyboard navigation;
* contextual help and accessibility;
* unchanged legacy output.

Representative artifacts should include:

```text
Full Color + CMYK Print
Full Color + RGB Screen
Perceptual Lightness + Active Channel
Perceptual Lightness + All Channels
Alpha + Active Channel
Legacy Crosshatch Migration
```

---

# Implementation sequence

Implement TON-012 as separate reviewable stages.

## Stage 1 — Audit and model definitions

Determine:

* the actual formulas used by every legacy mapping;
* how source alpha currently participates;
* where output model is currently inferred from mapping;
* how Crosshatch is represented;
* which preset formats contain mapping state.

Implement:

* independent Artwork Source model;
* Source Alpha policy;
* independent Output Model;
* Channel Assignment;
* stable identifiers;
* document-version migration structures.

Do not broadly redesign the interface yet.

## Stage 2 — Rendering and migration

Implement:

* source samplers;
* Full Color conversion into CMYK and RGB;
* scalar channel sampling;
* alpha behavior;
* deterministic migration from legacy mappings;
* Crosshatch compatibility adapter;
* unchanged-output regression fixtures.

## Stage 3 — Interface refactor

Replace the combined Artwork Mapping dropdown with:

* Artwork Source;
* Source Alpha where applicable;
* Output Model;
* Apply To where applicable.

Implement:

* dynamic visibility;
* contextual help;
* wide and narrow layouts;
* keyboard and accessibility behavior;
* callback-loop prevention;
* correct dirty-state behavior.

## Stage 4 — Presets and persistence

Separate:

* source presets;
* output presets;
* complete workflow presets.

Verify:

* save and reopen;
* undo and redo;
* migration;
* mode-specific state restoration;
* no silent cross-setting changes.

## Stage 5 — Final review

Perform:

* complete regression validation;
* preview/PNG/SVG comparisons;
* legacy project migration checks;
* Crosshatch compatibility checks;
* creative review;
* accessibility review;
* performance review where source sampling changes affect render cost.

Do not begin TON-010 until TON-012’s source and output boundaries are stable.

---

# Scope warning for orchestration

TON-012 is an artwork-source and output-model refactor.

It must not broaden into:

* implementing new halftone patterns;
* implementing TON-010’s registry;
* implementing TON-011 per-channel pattern assignment;
* implementing TON-009 DTF treatment;
* replacing the Crosshatch generator;
* adding arbitrary color-management profiles;
* redesigning the entire application shell.

Before changing labels such as Brightness, inspect the actual current formula.

Do not claim Value or Perceptual Lightness behavior unless the implementation uses the documented calculation.

Do not mark TON-012 Done merely because the new controls exist. The document model, migration, preview, PNG, SVG, presets, save/reopen, and legacy output must all agree.

---

# Deferred capabilities

Track separately unless required to preserve current behavior:

* arbitrary mathematical source expressions;
* imported auxiliary masks;
* source-channel blending;
* channel mixer matrices;
* LAB or LCh component sources;
* HSV Hue or Saturation sources;
* source-matte selection for ignored alpha;
* multiple source images;
* per-channel source selection in the standard workflow;
* ICC-profile conversion controls;
* printer-profile simulation;
* node-based source-processing graphs.

The main correction is:

> **Artwork Source says what Toniator reads. Output Model says what Toniator generates. Channel Assignment says where scalar source values go.**

---

# Cross-cutting completion checklist

Apply this checklist to every issue where relevant:

- [ ] Existing project files still open.
- [ ] Save and reopen preserve the new settings.
- [ ] Preview and export match.
- [ ] Undo and redo behave correctly.
- [ ] Long operations can be cancelled.
- [ ] Cancelled or superseded work cannot replace a newer preview.
- [ ] Errors are visible and actionable.
- [ ] Transparent output remains transparent.
- [ ] Destination files are not damaged by failed exports.
- [ ] Large-image responsiveness is evaluated.
- [ ] Memory use does not grow without release after repeated previews.
- [ ] UI terminology follows TON-002.
- [ ] Contextual help follows TON-003.
- [ ] The `creative_tester` agent reviews the resulting workflow.
- [ ] Verified defects are distinguished from speculative concerns in the completion report.

---

# Orchestrator guidance

For each issue:

1. Reproduce or inspect the current behavior before changing it.
2. Identify the smallest coherent implementation slice.
3. Preserve unrelated behavior.
4. Add automated checks where the behavior is deterministic.
5. Add visual fixtures or screenshots where correctness is primarily visual.
6. Run the existing test and artifact-validation suites.
7. Ask the `creative_tester` agent to review user-facing changes.
8. Report:
   - files changed;
   - behavior changed;
   - tests performed;
   - known limitations;
   - follow-up issues discovered.
9. Do not silently broaden an issue into a general rewrite.
10. Do not mark an issue complete when only the UI exists but the exported result is still incomplete or inconsistent.
