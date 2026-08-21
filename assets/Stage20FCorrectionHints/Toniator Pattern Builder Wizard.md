# Toniator Pattern Builder Wizard Specification

## Purpose

This document defines the artist-facing workflow for creating and editing reusable Toniator patterns.

The Pattern Builder should expose Toniator's structural capabilities in a way that is visually discoverable without requiring the artist to understand the internal geometry architecture.

The normal Toniator workflow remains preset-first:

1. Browse a visual preset.
2. Apply it directly.
3. Edit or duplicate it when customization is needed.
4. Enter the Pattern Builder when creating a new pattern or changing the structural definition of an existing preset.

Built-in presets and user-created presets should use the same serialized configuration model. A preset must not invoke hidden renderer behavior unavailable through the Pattern Builder.

---

# 1. Core pattern model

A pattern is built through the following conceptual pipeline:

**Layout Family → Generator → Base Structure → Derivation → Sites / Paths / Regions → Realization → Canonical Geometry → Final Canvas Clip → Preview / Export**

These layers should remain distinct even when the UI combines trivial steps onto a single screen.

## Layout Family

The broad category describing how the pattern is organized.

Initial families:

* Grid
* Dispersion
* Parametric
* Hybrid

## Generator

The algorithm that creates the underlying structure.

Examples:

* Grid → Two Guides
* Dispersion → Poisson Disc
* Parametric → Spiral
* Hybrid → Maze, Two Guide

## Base Structure

The geometry directly produced by the generator.

Typical base structures include:

* guides;
* parametric curves;
* sites;
* procedural paths or topology.

## Derivation

A downstream operation that converts the base structure into another structural primitive.

Examples:

* guide intersections → sites;
* spacing along guides → sites;
* closed guide faces → regions;
* sites → Voronoi regions;
* sites → connections → paths;
* parametric curve → evenly spaced sites;
* curve → repeated or offset paths.

## Realization

The operation that turns structural primitives into visible geometry.

* Sites → marks/shapes
* Paths → visible path geometry
* Regions → scaled or offset region geometry

---

# 2. Canvas invariant

The canvas never participates in pattern construction.

Toniator generates sufficient geometry outside the visible canvas. Canvas clipping is performed only after canonical pattern geometry exists.

The canvas boundary must never be used to:

* terminate a guide;
* terminate a parametric curve;
* close a region;
* create a guide cell;
* complete a Voronoi cell;
* connect sites;
* alter maze topology;
* determine constant-gap path topology;
* manufacture any other pattern structure.

The final sequence is always:

**Generated Geometry → Canonical Geometry → Canvas Clip → Preview / Export**

A site, path, cell, or region may exist partially or entirely outside the canvas and remain part of the generated pattern.

---

# 3. Preset Browser

The Pattern Editor should open to a visual preset browser rather than directly opening the Pattern Builder.

## Primary actions

* Apply
* Edit
* Duplicate
* Create New Pattern

## Recommended family filters

* All
* Grid
* Dispersion
* Parametric
* Hybrid

Optional visual tags can include:

* Dots
* Curves
* Cells
* Maze
* Connected
* Organic
* Geometric
* Radial
* Sparse
* Dense

## Preset card

Each preset card should contain:

* representative thumbnail;
* preset name;
* family;
* optional tags;
* Apply action;
* Edit and Duplicate secondary actions.

---

# 4. Adaptive wizard design

The Pattern Builder should not have a fixed number of mandatory screens.

A simple branch should remain short. A complex branch may introduce additional configuration screens when necessary.

Do not label the interface primarily as "Step 3 of 7."

Instead, maintain an always-visible structural breadcrumb.

Examples:

**Grid › Two Guides › Sites Along Guides › Voronoi Cells**

**Parametric › Spiral › Square › Sites Along Curve**

**Dispersion › Poisson Disc › Connections**

## Persistent wizard elements

Once configuration begins, the UI should retain:

* structural breadcrumb;
* live preview;
* Back;
* Cancel;
* Next or Create Pattern;
* optional Advanced disclosure.

## Selection screens

Use large visual cards.

Each card should contain:

* example thumbnail;
* short name;
* one- or two-line explanation.

Show only choices valid for the current structure unless there is a specific educational reason to display a disabled option.

---

# 5. Overall wizard flow

The general path is:

**Preset Browser**

→ Create New Pattern

→ **Choose Layout Family**

→ **Choose Generator / Variant**

→ **Configure Generator**

→ determine generated structural type

Then branch according to the structure:

### Guides

**Guides**

→ Raw Guides → Paths

or

→ Create Sites
→ Guide Intersections or Sites Along Guides
→ Sites

or

→ Guide Cells
→ Regions

### Sites

**Sites**

→ Marks at Sites → Site Realization

or

→ Connections → Paths → Path Realization

or

→ Voronoi Cells → Regions → Region Realization

### Parametric Curves

**Curve**

→ Raw Curve → Paths

or

→ Sites Along Curve → Sites

or

→ Offset / Repeated Curves → Paths

### Hybrid structures

Hybrid generators expose only the derivations appropriate to that generator.

All branches eventually converge on:

**Site Realization / Path Realization / Region Realization**

→ **Canonical Geometry**

→ **Final Canvas Clip**

→ **Preview / Export**

A preset may then be saved from the completed configuration.

---

# 6. Layout Family screen

## Heading

**Choose a layout family**

## Helper text

Choose the kind of structure that will organize the pattern.

### Grid

Repeating guide structures, intersections, and enclosed regions.

### Dispersion

Sites distributed without a regular lattice.

### Parametric

Curves generated from mathematical forms.

### Hybrid

Procedural structures that combine specialized construction rules.

---

# 7. Grid family

A Grid generator creates one or more guide families.

## Grid variants

### One Guide

A repeating family of parallel or curved guides.

Capabilities:

* editable guide curve;
* Raw Guides;
* Sites Along Guides;
* Voronoi after sites exist.

Not applicable:

* Guide Intersections;
* normal Guide Cells.

### Two Guides

Two crossing guide families.

Capabilities:

* editable Guide A;
* editable Guide B;
* Raw Guides;
* Guide Intersections;
* Sites Along Guides;
* Guide Cells;
* Voronoi after sites exist.

This should be the primary general-purpose grid variant.

### Triagrid

Three fixed straight guide directions forming a triangular lattice.

Capabilities:

* Raw Guides;
* Guide Intersections;
* Sites Along Guides;
* Guide Cells;
* Voronoi after sites exist.

Restriction:

* no guide-curve editor.

The three guide directions remain straight because arbitrary curvature causes their expected intersection structure to diverge.

### Tetragrid

Four-guide special case.

Recommendation:

Keep this out of the primary workflow unless a concrete artistic requirement justifies it.

If retained:

* expose under Advanced;
* no guide editor;
* primarily intended for Guide Cell / region construction.

---

# 8. Grid arrangement screen

## Heading

**Choose the grid arrangement**

### One Guide

A single editable guide family repeated across the pattern.

**Guide editing available**

### Two Guides

Two editable crossing guide families.

**Guide editing available**

### Triagrid

Three fixed straight guide directions forming a triangular lattice.

**Guide curves remain fixed**

### Tetragrid

A specialized four-direction structure intended mainly for region construction.

**Advanced**

---

# 9. Grid generator configuration

## One Guide / Two Guides

### Heading

**Configure the guide layout**

Potential controls include:

* Guide A
* Guide B, for Two Guides
* Guide Spacing
* Phase
* Aspect or density relationships where appropriate
* Open Guide Editor

The Guide Editor modifies the authored guide curve used to generate the repeated family.

Base pattern rotation remains a channel-level setting rather than being duplicated here.

## Triagrid

### Heading

**Configure the triangular grid**

Controls may include:

* Guide Spacing
* Phase
* density/aspect controls that preserve valid topology

Information message:

**Guide editor unavailable**

Triagrid keeps its guide directions straight so its lattice intersections remain structurally valid.

---

# 10. Guide use screen

After the grid itself exists:

## Heading

**Choose how the guides are used**

### Raw Guides

Use the generated guides directly as paths.

**Guides → Paths**

### Create Sites

Generate discrete sites from the guide structure.

This opens the Site Source screen.

### Guide Cells

Use regions directly enclosed by the guide topology.

**Guides → Closed Faces → Regions**

Guide Cells appear only when the generated guide topology actually produces closed faces.

---

# 11. Grid Site Source screen

## Heading

**Choose where sites are created**

### Guide Intersections

Create a site where eligible guide families cross.

Available only when the current grid produces intersections.

### Sites Along Guides

Place sites at measured intervals along the guides.

Configuration may include:

* Site Interval
* positional jitter
* offset bias where appropriate
* deterministic jitter seed

After either choice, Toniator possesses Sites and proceeds to the common Site Use workflow.

---

# 12. Dispersion family

Dispersion generators create Sites directly.

## Initial generators

### Poisson Disc

Randomized sites with controlled minimum separation.

Suggested controls:

* density or target site count;
* minimum spacing;
* exclusion/margin;
* seed;
* weighting influence.

### Stochastic

Unconstrained random site placement.

Suggested controls:

* density;
* seed;
* exclusion/margin.

### Noise Weighted

A site distribution whose density is influenced by a continuous noise field.

Suggested controls:

* density;
* noise scale;
* noise strength;
* seed;
* exclusion/margin.

Perlin and similar noise formulations should preferably be configuration or presets of the applicable generator rather than isolated renderer-specific pattern types.

## Dispersion flow

**Dispersion Generator → Sites**

Then:

* Sites → Marks
* Sites → Connections
* Sites → Voronoi Cells

---

# 13. Parametric family

Parametric generators create mathematical curves.

## Initial recommended generators

* Spiral
* Rosette
* Lissajous
* Trochoid
* Radial Wave

Potential future generators:

* Lemniscate
* Superformula
* Cycloid
* Harmonograph

---

# 14. Spiral generator

Spiral is the generator class.

Round, triangular, square, pentagonal, and similar spirals are configurations of Spiral rather than separate generators.

## Shape

Suggested initial options:

* Round
* Triangle
* Square
* Pentagon
* Hexagon
* N-gon

A squiral therefore becomes:

**Parametric › Spiral › Shape: Square**

rather than a separate Squiral generator.

## Spiral controls

Potential controls include:

* Shape
* Turns
* Radial Spacing
* Phase
* Winding Direction
* shape-specific parameters

Base rotation remains controlled by channel settings.

---

# 15. Other parametric generators

## Rosette

Suggested controls:

* lobes/petals;
* inner radius;
* outer radius;
* phase.

## Lissajous

Suggested controls:

* horizontal frequency;
* vertical frequency;
* phase;
* X extent;
* Y extent.

## Trochoid

Suggested controls:

* rolling mode;
* radius relationship;
* tracing offset;
* phase.

## Radial Wave

Suggested controls:

* base radius;
* frequency/lobe count;
* wave amplitude;
* phase.

---

# 16. Parametric Curve Use screen

## Heading

**Choose how the curve is used**

### Raw Curve

Use the generated curve directly as a path.

**Curve → Path**

### Sites Along Curve

Create sites at measured intervals along the curve.

**Curve → Sites**

Configuration can include:

* Site Interval
* positional jitter
* deterministic seed

### Offset / Repeated Curves

Create a related path family from the generated curve.

Only show this choice for generators supporting that operation.

**Curve → Path Family**

---

# 17. Hybrid family

Hybrid generators may define specialized structural rules instead of being forced into Grid or Dispersion abstractions.

## Initial generators

### Maze — Two Guide

Maze based on a two-direction guide structure.

### Maze — Three Guide

Maze based on a three-direction triangular structure.

Possible outputs include:

* raw maze paths;
* sites sampled along maze paths;
* regions when the generated topology contains useful closed faces.

Future Hybrid candidates may include recursive or space-filling structures.

---

# 18. Common Site Use screen

Any algorithm that produces Sites can enter this workflow.

That is a capability relationship, not a named-preset relationship.

## Heading

**Choose how the sites are used**

### Marks at Sites

Draw a shape at each site.

**Sites → Site Realization**

### Connections

Connect eligible sites to create paths.

**Sites → Connection Policy → Paths**

Connection options appear only when supported.

The existing "Poisson Disc with 0–2 random connections" therefore becomes:

**Dispersion › Poisson Disc › Connections › 0–2 Connections**

It is not a separate Connected Poisson generator.

### Voronoi Cells

Create a Voronoi partition from the sites.

**Sites → Voronoi → Regions**

Any valid site-generating algorithm can feed Voronoi.

Examples:

* Poisson Disc Sites → Voronoi
* Stochastic Sites → Voronoi
* Grid Intersections → Voronoi
* Sites Along Guides → Voronoi
* Sites Along Spiral → Voronoi

---

# 19. Voronoi Cells vs Guide Cells

Both ultimately create Regions, but their source is different.

## Voronoi Cells

Voronoi cells are site-derived.

**Sites → Voronoi Partition → Regions**

Artist-facing description:

### Voronoi Cells

Create a region around each generated site.

Availability rule:

Voronoi is available whenever valid Sites exist.

## Guide Cells

Guide Cells are guide-derived.

**Guides → Closed Guide Faces → Regions**

Artist-facing description:

### Guide Cells

Use the regions enclosed directly by the guide structure.

Availability rule:

Guide Cells are available only when the current guide topology forms valid closed faces.

## Example: Two Guide grid

The same Two Guide generator can produce regions by two different routes.

### Guide Cells

**Two Guides → Closed Faces → Regions**

The spaces enclosed between the guide families become the cells.

### Voronoi Cells

**Two Guides → Guide Intersections → Sites → Voronoi → Regions**

The guide intersections become sites, and the Voronoi algorithm partitions space around those sites.

These produce different regions.

Once Regions exist, however, the same Region Realization workflow applies regardless of their origin.

---

# 20. Site Realization

Site realization answers:

**What should be drawn at each site?**

## Heading

**Configure site marks**

### Shape

Options may include:

* Circle
* Polygon
* built-in authored shape
* custom authored shape
* Open Shape Editor

The Shape Editor belongs here because it defines how a Site becomes visible geometry.

### Rotation Jitter

Adds deterministic per-site orientation variation.

Base orientation remains controlled by channel settings.

Conceptually:

**Final Mark Rotation = Channel Rotation + Site Rotation Jitter**

Additional site controls can include:

* nominal mark size;
* shape-specific parameters;
* variation seed.

---

# 21. Path Realization

Path realization answers:

**How should structural paths become visible geometry?**

## Heading

**Configure paths**

## Thickness

Controls nominal path thickness.

## Centerline Bias

Range:

**-1.0 ← 0.0 → +1.0**

Meaning:

* 0.0 = neutral, centered on structural centerline
* -1.0 = fully biased toward one side
* +1.0 = fully biased toward the opposite side

Use the UI label:

**Centerline Bias**

rather than simply "Bias."

---

# 22. Path spacing method

## Stacked

Preserve the generated path positions.

Neighboring paths may become naturally closer or farther apart as the structure bends.

Artist-facing description:

### Stacked

Preserve the generated path positions.

## Constant Gap

Use the Bezziator-derived offset/grow/shrink behavior to maintain approximately uniform visible spacing between neighboring paths.

Artist-facing description:

### Constant Gap

Offset neighboring paths to maintain a uniform gap.

When selected, expose:

* Path Gap
* Endpoint Behavior

---

# 23. Constant-gap endpoint behavior

When generating a constant-gap path family, an offset path endpoint may move toward or into the portion of the pattern that will eventually be visible.

The artist chooses how the generated topology handles that endpoint.

The canvas itself still plays no role in creating the geometry.

## Extend Beyond Canvas

Continue the underlying path sufficiently far that the endpoint remains outside the eventual visible result.

Artist-facing description:

### Extend Beyond Canvas

Continue the path so its endpoint remains outside the rendered area.

## Wrap Around Endpoint

Preserve the endpoint and curve subsequent constant-gap paths around it.

This produces a fingerprint-like structure.

Artist-facing description:

### Wrap Around Endpoint

Preserve the endpoint and curve neighboring paths around it.

This is a topological choice, not merely a stroke style, and should be prominently exposed whenever Constant Gap is selected.

---

# 24. Region Realization

Region realization applies equally to:

* Voronoi Cells;
* Guide Cells;
* any future algorithm that produces Regions.

## Heading

**Configure regions**

## Scale

Resize the region proportionally relative to its own reference.

This does not guarantee a constant distance from the original boundary.

Artist-facing description:

### Scale

Resize each region proportionally.

## Constant Gap

Use the Bezziator shrink/grow operation to offset the region boundary by an approximately constant distance.

Artist-facing description:

### Constant Gap

Shrink or grow region boundaries by a uniform distance.

## Amount control

The UI should make Shrink and Grow explicit.

Recommended conceptual control:

**Grow ← Neutral → Shrink**

The underlying model may use a signed numeric value if appropriate.

---

# 25. Distinct spacing concepts

The UI should avoid using the generic term "Spacing" for several unrelated quantities.

Three different concepts exist.

## Guide Spacing

Distance between generated guides or other structural members.

## Site Interval

Distance between sites sampled along a guide, curve, or path.

## Realization Gap

Distance maintained between final visible geometry.

Examples:

* Path Gap
* Region Gap

Example:

| Control       | Value |
| ------------- | ----: |
| Guide Spacing | 40 px |
| Site Interval | 12 px |
| Path Gap      |  3 px |

These terms should remain consistent across the application.

---

# 26. Capability decision table

## Grid

| Variant    | Guide Editor | Raw Guides | Intersections | Sites Along Guides | Guide Cells | Voronoi From Sites |
| ---------- | -----------: | ---------: | ------------: | -----------------: | ----------: | -----------------: |
| One Guide  |          Yes |        Yes |            No |                Yes |          No |                Yes |
| Two Guides |          Yes |        Yes |           Yes |                Yes |         Yes |                Yes |
| Triagrid   |           No |        Yes |           Yes |                Yes |         Yes |                Yes |
| Tetragrid  |           No |   Advanced |      Advanced |           Advanced | Primary Use |     If Sites Exist |

Tetragrid should remain hidden or Advanced unless a specific required result depends on it.

## Dispersion

| Generator      | Produces Sites | Marks |      Connections | Voronoi |
| -------------- | -------------: | ----: | ---------------: | ------: |
| Poisson Disc   |            Yes |   Yes |              Yes |     Yes |
| Stochastic     |            Yes |   Yes | Capability-based |     Yes |
| Noise Weighted |            Yes |   Yes | Capability-based |     Yes |

## Parametric

| Generator   | Raw Curve | Sites Along Curve | Offset Curves | Voronoi After Sites |
| ----------- | --------: | ----------------: | ------------: | ------------------: |
| Spiral      |       Yes |               Yes |           Yes |                 Yes |
| Rosette     |       Yes |               Yes |  If Supported |                 Yes |
| Lissajous   |       Yes |               Yes |  If Supported |                 Yes |
| Trochoid    |       Yes |               Yes |  If Supported |                 Yes |
| Radial Wave |       Yes |               Yes |           Yes |                 Yes |

## Hybrid

| Generator          | Raw Paths | Sites Along Paths | Closed-Face Regions |
| ------------------ | --------: | ----------------: | ------------------: |
| Maze — Two Guide   |       Yes |               Yes |     When meaningful |
| Maze — Three Guide |       Yes |               Yes |     When meaningful |

Hybrid capabilities remain generator-owned.

---

# 27. Wizard screen map

The following are logical screens. The wizard may merge adjacent screens when there is little to configure.

## Common screens

### Choose a layout family

Choose the kind of structure that will organize the pattern.

### Choose a generator

Choose the structure Toniator should generate.

### Configure the generator

Adjust the geometry that defines the pattern.

---

## Grid screens

### Choose the grid arrangement

Choose how many guide directions define the grid.

### Configure the guide layout

Adjust the generated guide structure.

### Choose how the guides are used

Choices:

* Raw Guides
* Create Sites
* Guide Cells

### Choose where sites are created

Choices:

* Guide Intersections
* Sites Along Guides

---

## Dispersion screens

### Choose a distribution

Choose how sites should be dispersed.

### Configure the distribution

Adjust density, exclusion, weighting, and deterministic variation.

Then proceed directly to the common Site Use screen.

---

## Parametric screens

### Choose a parametric form

Choose the mathematical curve used to organize the pattern.

### Configure the form

Adjust the parameters that define the curve.

### Choose how the curve is used

Choices:

* Raw Curve
* Sites Along Curve
* Offset / Repeated Curves

---

## Site screens

### Choose how the sites are used

Choices:

* Marks at Sites
* Connections
* Voronoi Cells

### Configure connections

Potential controls:

* Minimum Connections
* Maximum Connections
* Maximum Connection Distance
* nearest/random connection bias
* seed

### Configure site marks

Potential controls:

* Shape
* Shape Editor
* Mark Size
* Rotation Jitter
* variation seed

---

## Path screen

### Configure paths

Controls:

* Thickness
* Centerline Bias
* Path Spacing Method

  * Stacked
  * Constant Gap
* Path Gap, when Constant Gap
* Endpoint Behavior, when Constant Gap

  * Extend Beyond Canvas
  * Wrap Around Endpoint

---

## Region screen

### Configure regions

Controls:

* Region Method

  * Scale
  * Constant Gap
* Scale Amount, when Scale
* Shrink / Grow Amount, when Constant Gap

This screen is identical whether the Regions originated from Voronoi or Guide Cells.

---

# 28. Review and Save

## Heading

**Review your pattern**

Show:

* live preview;
* complete structural breadcrumb;
* major generator settings;
* derivation choices;
* realization choices;
* preset name;
* optional tags;
* Save as reusable preset.

Example breadcrumbs:

**Grid › Two Guides › Sites Along Guides › Voronoi Cells › Constant Gap Regions**

**Dispersion › Poisson Disc › Connections › Constant Gap Paths**

**Parametric › Spiral › Square › Sites Along Curve › Shape Marks**

Actions:

* Back
* Cancel
* Create Pattern / Save Preset

---

# 29. Discovery rules

## Visual-first choices

Use the existing SVG and PNG examples wherever possible as the canonical thumbnails for structural choices.

An artist should generally be able to recognize the result before reading the explanatory text.

## Progressive disclosure

Show only valid choices.

Do not populate screens with disabled operations that cannot work for the selected structure.

An exception is useful educational context such as explaining why Triagrid does not permit guide editing.

## Preserve compatible state

Going Back must preserve configuration.

If the user changes an upstream choice:

* preserve downstream settings that remain semantically valid;
* reset only settings that became incompatible;
* do not silently reinterpret an old value as a different concept.

## Advanced controls

Uncommon mathematical parameters should normally live under an Advanced disclosure instead of creating more wizard pages.

## Live preview

The preview should update immediately from authoritative document state.

Preview and export must use the same canonical geometry pipeline.

---

# 30. Capability-driven UI rule

The wizard should decide what is available based on the structural primitive and its supported derivations, not by checking a named preset.

Examples:

**Voronoi is available because Sites exist.**

Not:

"Voronoi is available because this is Poisson Disc."

**Guide Cells are available because closed guide faces exist.**

Not:

"Guide Cells are available because this is a Two Guide preset."

**Shape realization is available because Sites exist.**

**Path realization is available because Paths exist.**

**Region realization is available because Regions exist.**

This capability model should be reflected in both the domain architecture and the adaptive wizard.

---

# 31. Recommended remaining work boundaries

These are architectural work slices rather than proposed replacement stage numbers. They can be mapped onto the project's existing stage plan.

## A. Adaptive wizard shell

* structural breadcrumb;
* visual choice cards;
* valid-next-option queries;
* Back/Next state preservation;
* live preview integration;
* preset Edit/Duplicate entry.

## B. Grid authoring workflow

* One Guide;
* Two Guides;
* Triagrid;
* Guide Editor restrictions;
* Raw Guides;
* Guide Intersections;
* Sites Along Guides;
* Guide Cell derivation.

Tetragrid should not block completion of the primary workflow.

## C. Shared Sites pipeline

* Sites as reusable structural output;
* Marks at Sites;
* Connections;
* Voronoi from any valid site source;
* Shape Editor integration;
* Rotation Jitter.

## D. Shared Path realization

* thickness;
* Centerline Bias;
* Stacked;
* Constant Gap;
* Bezziator offset behavior;
* Extend Beyond Canvas;
* Wrap Around Endpoint.

## E. Shared Region realization

* common Region representation;
* Guide Cells → Regions;
* Voronoi → Regions;
* Scale;
* Bezziator Constant Gap shrink/grow.

## F. Dispersion generators

* Poisson Disc;
* Stochastic;
* Noise Weighted;
* reuse common Site downstream operations.

## G. Parametric generators

Initial:

* Spiral;
* shape parameterization;
* Raw Curve;
* Sites Along Curve;
* Offset Curves where appropriate.

Then expand independently with:

* Rosette;
* Lissajous;
* Trochoid;
* Radial Wave.

All should reuse shared downstream Site/Path/Region behavior.

## H. Hybrid generators

* Maze — Two Guide;
* Maze — Three Guide;
* generator-specific structural rules;
* common downstream realization wherever output types match.

## I. Preset authoring polish

* visual browser;
* family filters;
* tags;
* Edit;
* Duplicate;
* user preset names;
* identical serialization rules for built-in and user presets.

---

# 32. Acceptance criteria

The Pattern Builder design is coherent when:

1. An artist can apply a preset without entering the wizard.
2. A user can create a pattern through visual choices without needing to understand internal geometry terminology.
3. The wizard displays only the screens required by the active branch.
4. One Guide and Two Guides support authored guide curves.
5. Triagrid does not expose guide-curve editing.
6. Any valid Site source can feed Voronoi.
7. Guide Cells are created only from actual closed guide faces.
8. Voronoi Cells and Guide Cells converge on the same downstream Region model.
9. Site realization owns mark Shape and the Shape Editor.
10. Site realization supports Rotation Jitter.
11. Base mark rotation remains controlled by channel settings.
12. Path realization supports Centerline Bias from -1.0 to +1.0.
13. Paths support Stacked and Constant Gap realization.
14. Constant-gap paths support both Extend Beyond Canvas and Wrap Around Endpoint behavior.
15. Region realization supports proportional Scale.
16. Region realization supports Bezziator-based constant-distance Shrink/Grow.
17. Guide Spacing, Site Interval, Path Gap, and Region Gap remain distinct concepts.
18. No pattern-building algorithm uses the canvas boundary as geometry.
19. Canvas clipping occurs only after canonical geometry is complete.
20. Preview and export use the same canonical geometry.
21. Built-in presets contain no hidden pattern behavior unavailable through authored configuration.
22. Wizard availability is capability-driven rather than preset-name-driven.
23. Downstream realization changes reuse upstream generated structure whenever the dependency model permits.

