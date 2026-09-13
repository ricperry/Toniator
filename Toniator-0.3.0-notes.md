# Toniator 0.3.0 Review Notes

## Pattern Recipe / Validation Issues

### Spiral pattern requires an unexposed setting

The Spiral pattern requires `curve.parametric.segment_limit`, but that setting is not exposed anywhere in the wizard and does not appear to be included in the preset.

A valid preset or wizard-generated recipe should contain all required settings. If `segment_limit` is required, it needs either:

* an exposed wizard control;
* a sensible default supplied automatically; or
* inclusion in the preset/recipe without requiring manual intervention.

### Invalid recipes should not be appliable

Some pattern families expose recipe-card settings that can produce an invalid recipe. The user can currently apply those values and only afterward receive a warning that the pattern could not be created.

The wizard should prevent an invalid recipe from being applied. When validation fails, it should identify the specific invalid setting and take the user directly to the relevant control so the problem can be corrected.

---

## Channel Editing / State Propagation

### Bias changes do not consistently propagate

Changing the response bias does not consistently apply the new value to the document.

This appears to be a state synchronization or application problem rather than simply a rendering issue.

### `ALL` channel editing sometimes stops applying to all channels

After making many edits, especially after switching between individual R/G/B channels and then returning to `ALL`, subsequent edits sometimes apply only to the most recently edited individual channel.

When `ALL` is active, edits should reliably apply to every active channel regardless of which individual channel was edited previously.

### Duplicate controls in Advanced Settings

When the active channel selection is `ALL`, expanding an individual channel in Advanced Settings duplicates several controls:

* Minimum thickness
* Maximum thickness
* Curve response bias

A second subgroup labeled `Output [x]` is created containing the same settings.

The hierarchy should expose each effective setting once and make the relationship between global and per-channel overrides clear.

---

## Source Response / Tonal Controls

### Toniator needs better control over source tonal response

There does not appear to be a way to apply curves, levels, or similar tonal adjustments to the source image before it drives the pattern response.

Although source-image editing could theoretically be done in another application, these controls directly affect how Toniator interprets the source. They therefore belong naturally within Toniator's source-response pipeline.

Useful controls would include:

* Levels / black and white points
* Curves
* Contrast
* Gamma
* Per-channel source response
* Channel assignment / mapping

This would make it possible to deliberately control what tonal ranges produce marks, gaps, and maximum coverage without permanently modifying the source artwork in another application.

### True blacks / light-tone suppression

ToniatorLegacy included a `Light-Tone Cutoff` control that automatically removed marks smaller than a specified threshold.

That feature was particularly useful because it:

* eliminated visually insignificant marks in very light regions;
* reduced image noise; and
* could substantially reduce exported SVG/file size.

The greenfield version should restore an equivalent mechanism. It does not necessarily need to use the same name or implementation, but there should be a way to suppress marks below a useful response threshold.

### Artwork Contrast behaves more like brightness

The Advanced `Artwork contrast` control currently appears to affect source brightness rather than contrast.

Expected contrast behavior would be:

* lower contrast: highlights and shadows move toward the midtones;
* higher contrast: midtones are pushed outward toward highlights and shadows.

The current behavior should be checked against that expectation.

### Gamma adjustment

A gamma adjustment should be added to the source-response controls in Advanced Settings.

Gamma would provide a useful nonlinear tonal adjustment without requiring the user to redefine black and white points.

---

## Advanced Settings Scope

The Advanced Settings dialog currently contains too many controls that are fundamentally pattern-editing controls, including:

* Minimum fill
* Maximum fill
* X offset
* Y offset

These controls already belong conceptually to pattern construction and/or document geometry.

I think Advanced Settings should instead focus primarily on **how the source artwork is interpreted**, for example:

* channel curves;
* levels / black and white points;
* contrast;
* gamma;
* source-channel assignment;
* channel-response mapping;
* tonal cutoffs or thresholds;
* other source preprocessing controls.

This would give Advanced Settings a clearer purpose instead of making it a second location for recipe controls.

---

## Main Document Controls

### Promote X/Y offset to the main side panel

X and Y offset should be promoted to the main side panel and grouped with Rotation.

Conceptually, these are all document/pattern transform controls:

* Rotation
* X Offset
* Y Offset

Keeping them together would make transforms easier to understand and would also support intentionally misregistering color channels.

---

## Transform Consistency Across Pattern Families

Some pattern families do not currently respect rotation and/or X/Y offset.

I think every pattern family and every final recipe should support these transforms.

There are useful artistic reasons for allowing this even when the underlying pattern is stochastic. For example, a small per-channel rotation or positional offset can create deliberate registration errors or chromatic-aberration-like effects.

This is especially useful when:

* channels share the same random seed; and
* source-dispersion weighting uses the same values for each channel.

In that situation, rotation and X/Y offset provide a simple, predictable way to introduce controlled spatial differences between otherwise aligned channel sample locations.

For consistency and simplicity, the final transformation layer should therefore apply rotation and offset to every generated pattern rather than making support dependent on the pattern family.

---

## Scatter / Random Sampling

### `UNIFORM` scatter does not appear uniform

The Scatter family's `UNIFORM` mode does not appear to generate a spatially uniform distribution. Visually, it behaves more like unconstrained statistical randomness.

If `UNIFORM` means independent uniformly distributed random coordinates, the terminology may be mathematically defensible but is misleading in the UI because users will generally interpret "uniform" as spatially even.

Either the algorithm or the naming should be reconsidered.

### Switching scatter modes does not fully restore the selected algorithm

There appears to be stale state when switching Scatter styles.

Reproduction:

1. Start with `Even Spacing`.
2. Switch to `Uniform`.
3. Apply the change.
4. Switch back to `Even Spacing`.
5. Apply again.

The resulting pattern does not return to the original `Even Spacing` appearance.

Using Undo restores the original result, which suggests that some underlying recipe or generated state is not being fully refreshed when the scatter algorithm changes.

Changing algorithms should rebuild all algorithm-dependent state from the currently selected recipe.

### Random sample positions do not appear to respond to weighting

I do not see meaningful positional weighting in the random-sample patterns.

Regardless of the weighting settings I use, corresponding sample locations across channels continue to align with one another.

If source-weighted dispersion is enabled, I would expect the spatial distribution of sites to vary according to the selected channel's source response. Different channel data should therefore be capable of producing different site distributions even when other recipe settings are identical.

This should be checked to determine whether:

* the weighting is not being applied to site placement;
* all channels are inadvertently sharing a generated site set; or
* weighting currently affects some later property rather than the actual sample positions.

---

## Pattern Size / Coverage Terminology

ToniatorLegacy used the controls:

* `Sampling Detail`
* `Coverage`

I found these more intuitive than the greenfield rewrite's current terminology.

The current relationship appears to be approximately:

* `Pattern Size` → legacy `Sampling Detail`
* `Coverage` / maximum fill → legacy `Coverage`

`Pattern Size` is somewhat ambiguous because it can sound like the physical dimensions of the entire pattern rather than its sampling density or feature scale.

I would consider returning to terminology closer to `Sampling Detail`, `Feature Size`, or another name that more directly communicates the effect.

### Fill defaults

Minimum fill should remain editable, but I think almost every preset should default to:

* Minimum fill: `0.0`
* Maximum fill: `1.0`

Those values provide the full available response range and are a better baseline unless a particular pattern has a strong technical reason to constrain it.

Preset-specific tonal shaping should generally come from source-response controls rather than unnecessarily narrowing the generated pattern's available fill range.



---

## File Export Operations

### Image and video export are not clearly separated

The current export workflow defaults to video export. Image export to `.png` or `.svg` is only exposed through the `Export current frame` action.

This is poor UX because still-image export and video export are fundamentally different operations, but the interface presents image export as a secondary variation of video export.

The hamburger menu should instead expose two distinct top-level actions:

- `Export image`

- `Export video`

`Export image` should provide the available still-image formats, such as PNG and SVG, while `Export video` should contain the animation/video-specific export options.

This would make the available export paths immediately understandable and avoid implying that video export is the default or primary operation.





### Video export needs explicit duration controls for still-image sources

When exporting video from a still-image source, the Video Export dialog needs a way to define how long the generated video should be.

At minimum, the dialog should expose either:

- frame count; or

- frame rate and duration.

Ideally, these values should be linked so that changing any two determines the third:

`frame count = frame rate × duration`

For a **source image**, these values must be explicitly defined because the source itself provides no inherent timeline or frame count.

For a **source video**, Toniator already has a natural timeline. The export settings should therefore initialize from the source video's:

- frame rate;

- frame count; and

- duration.

In that case, the user should not normally need to specify the export duration manually unless Toniator intentionally supports overriding or trimming the source timeline.

This distinction should be reflected in the Video Export dialog so that exporting animation from a still image does not depend on an implicit or unexplained default duration.
