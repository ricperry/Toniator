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

- **Status:** Open
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

---

## TON-002 — Replace implementation-oriented terminology with creator-friendly language

- **Status:** Open
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

---

## TON-003 — Add contextual popup help for controls

- **Status:** Open
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

---

## TON-004 — Rework the UI toward the GNOME/libadwaita HIG

- **Status:** Open
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

---

## TON-005 — Move the primary side panel to the left

- **Status:** Open
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

---

## TON-006 — Use SVG source-mapping hint icons instead of PNG versions

- **Status:** Open
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

---

# Core capabilities

## TON-007 — Add configurable document background and transparency controls

- **Status:** Open
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

---

## TON-008 — Add RGB mode for display-oriented halftone artwork

- **Status:** Open
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

## TON-009 — Add DTF mode with white underbase and optional black knockout

- **Status:** Open
- **Priority:** P1
- **Area:** Color modes / DTF production
- **Type:** New capability
- **Depends on:** TON-007
- **Related:** TON-008

### Problem

DTF artwork often requires a white underbase beneath printed colors. On dark garments, black artwork may instead be represented by transparent knockout areas so that the garment supplies the black.

### Required outcome

Provide a dedicated DTF workflow with an explicit white layer and optional black knockout behavior.

### Acceptance criteria

- The user can select a dedicated DTF mode.
- DTF mode can generate a white underbase layer behind the color artwork.
- The white layer is visible and inspectable independently.
- The user can enable or disable black knockout.
- When black knockout is enabled, qualifying black regions remove or suppress the white underbase so the garment/background shows through.
- The knockout threshold or qualifying rule is explicit, predictable, and documented.
- The configurable document background can be used to preview the intended garment color.
- Layer order and layer names are clear in exported files.
- Transparency is preserved where knockout is intended.
- Preview and export match.
- Save and reopen preserve all DTF settings.
- Existing CMYK and RGB workflows are not altered.
- Edge behavior does not introduce obvious white halos or accidental transparent seams under normal settings.
- The UI explains the difference between:
  - printed black;
  - transparent black knockout;
  - white underbase;
  - preview garment color.

### Deferred enhancements

Underbase choke/spread, trapping, and printer-specific production controls may be tracked separately unless they are already required by the current export pipeline.

---

# Shape-pattern expansion

## TON-010 — Add weighted Voronoi stippling with optional uniform shape size

- **Status:** Open
- **Priority:** P2
- **Area:** Shapes mode / pattern generation
- **Type:** New pattern

### Required behavior

Create a stippling pattern based on variable-spacing weighted Voronoi distribution. Image tone should be represented primarily through point density, with an option to keep the rendered shapes at a uniform size.

### Acceptance criteria

- The pattern is selectable by a creator-friendly name.
- Site density follows the mapped source values.
- The user can choose between:
  - density-driven spacing with uniform rendered shape size;
  - density plus variable shape size, when supported.
- Distribution avoids obvious unintended grid alignment.
- Controls expose meaningful creative parameters rather than raw solver internals.
- A fixed seed produces repeatable output.
- A new seed can generate a different valid arrangement.
- Preview generation is cancellable and does not display stale results.
- Export matches preview.
- Large images and dense outputs remain responsive enough to use.
- Edge coverage is intentional and does not leave unexplained borders.
- Help text explains the visual effect of each parameter.

---

## TON-011 — Add random but evenly spaced shape distribution

- **Status:** Open
- **Priority:** P2
- **Area:** Shapes mode / pattern generation
- **Type:** New pattern

### Required behavior

Add a random distribution that avoids clumping and maintains visually even spacing, similar to a blue-noise or Poisson-disc result.

### Acceptance criteria

- The result appears random without obvious clusters, collisions, or grid repetition.
- Minimum spacing is user-controllable.
- Density can respond to the source image.
- Seeded generation is repeatable.
- The user can regenerate the arrangement without changing unrelated settings.
- Edge treatment is predictable.
- Uniform-size and source-weighted-size behavior are clearly distinguished when both are available.
- Preview and export match.
- Dense settings remain cancellable and do not freeze the interface.

---

## TON-012 — Add an aperiodic cluster pattern

- **Status:** Open
- **Priority:** P2
- **Area:** Shapes mode / pattern generation
- **Type:** New pattern

### Required behavior

Add a clustered pattern that produces intentional local groupings without a visibly repeating grid or simple periodic screen.

### Acceptance criteria

- The pattern has no obvious repeating tile at normal viewing sizes.
- Clusters read as intentional rather than as accidental random clumping.
- Source values can influence cluster density, cluster size, or both.
- The user has a small set of understandable controls for the major visual characteristics.
- A fixed seed is repeatable.
- Preview and export match.
- Boundary behavior does not create a conspicuous empty frame.
- The implementation includes representative visual fixtures or screenshots for regression review.

### Design clarification

Before implementation, document the intended visual model for “aperiodic cluster” with one or more reference outputs. Do not complete this issue with a generic random distribution renamed as a cluster pattern.

---

## TON-013 — Add triangular-grid shape layout

- **Status:** Open
- **Priority:** P2
- **Area:** Shapes mode / pattern generation
- **Type:** New pattern

### Required behavior

Add a triangular lattice as an alternative to rectangular or other existing grids.

### Acceptance criteria

- The pattern is selectable from the shape-layout controls.
- Spacing is geometrically consistent in both lattice directions.
- Rotation and phase/offset can be controlled where consistent with existing grid patterns.
- Source weighting works consistently with other shape layouts.
- Edge coverage remains complete and intentional.
- Preview and export match.
- The grid does not drift or distort under document transforms.
- Saved projects restore the triangular-grid settings correctly.

---

# Curve-layout expansion

## TON-014 — Add maze path layout for repeated motifs

- **Status:** Open
- **Priority:** P2
- **Area:** Curves mode / repeated motif path layout
- **Type:** New layout pattern
- **Depends on:** TON-001

### Required behavior

Add a maze-like path system that repeated motif curves can follow. This concerns the larger path layout, not the design of the repeated motif itself.

### Acceptance criteria

- The maze layout is selectable independently of the motif curve.
- The user can control the major visual characteristics with creator-friendly controls.
- The layout provides useful coverage of the document area.
- Paths do not contain unintended discontinuities, self-intersections, or inaccessible isolated regions unless explicitly part of the selected design.
- Motif orientation and continuity remain predictable through corners.
- Preview and export match.
- Dense layouts remain cancellable and suppress stale previews.
- Edge and corner coverage satisfy the same reliability requirements as TON-001.

---

## TON-015 — Add spiral path layout for repeated motifs

- **Status:** Open
- **Priority:** P2
- **Area:** Curves mode / repeated motif path layout
- **Type:** New layout pattern
- **Depends on:** TON-001

### Acceptance criteria

- The spiral layout is selectable independently of the motif curve.
- The spiral can cover the intended document area without unexplained corner gaps.
- The user can control spacing, center, direction, and rotation where appropriate.
- The motif follows the spiral with stable orientation and predictable continuity.
- The center does not produce invalid geometry or uncontrolled overlap.
- Outer turns extend far enough to cover clipped document edges.
- Preview and export match.
- Save and reopen preserve all spiral settings.

---

## TON-016 — Add zig-zag path layout for repeated motifs

- **Status:** Open
- **Priority:** P2
- **Area:** Curves mode / repeated motif path layout
- **Type:** New layout pattern
- **Depends on:** TON-001

### Acceptance criteria

- The zig-zag layout is selectable independently of the motif curve.
- The user can control row spacing, zig-zag width or amplitude, angle, and phase where appropriate.
- Direction changes preserve intentional motif continuity.
- The full document area is covered at supported settings.
- No edge or corner gaps occur because the path system terminates too early.
- Preview and export match.
- Saved projects restore the layout exactly.

---

# Lower-priority workflow expansion

## TON-017 — Allow shapes and curves to be mixed in one document

- **Status:** Open
- **Priority:** P3
- **Area:** Document model / channels / rendering
- **Type:** New capability
- **Depends on:** Stable shape and curve pipelines

### Problem

A document currently requires the user to choose between shapes mode and curves mode. Some artwork would benefit from combining both approaches.

### Required outcome

Allow a Toniator document to contain shape-generated and curve-generated halftone elements at the same time.

### Minimum acceptable scope

At minimum, allow each color channel or output layer to choose its own generator type: shapes or curves.

### Preferred scope

Allow multiple independently configured generator groups within one document, each with its own:

- source mapping;
- color or channel assignment;
- clipping/mask;
- layout settings;
- visibility;
- export group.

### Acceptance criteria

- Shape and curve generators can coexist without corrupting each other’s settings.
- The active generator and active channel/layer are always visually clear.
- Preview order and export order are deterministic.
- Undo and redo preserve generator-specific changes.
- Save and reopen restore all mixed-mode content.
- Exported SVG groups remain organized and identifiable.
- Performance does not degrade disproportionately when both generator types are active.
- The interface does not expose irrelevant controls for the selected generator.
- Existing single-mode documents continue to open unchanged.

### Open design question

Decide whether the first implementation is per-channel generator selection or a more general layer/group model. Prefer the smallest architecture that does not block the eventual preferred scope.

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
