# TON-010 pattern parameter schema contract

Recorded 2026-08-02. This is the completed corrective Gate 2 contract on the
current dirty checkout; it is not TON-010 acceptance.

## Scope and format

`PatternDefinition.parameters[*].authoring` is the typed, inspectable
authoring contract for every recipe parameter. It is required and is either
`Creator` (eligible for its ownership-appropriate creator-facing consumer) or
`Internal` (never shown).
Creator metadata supplies a category, unit, increment, precision, ordered
group, ownership, applicability, validation, serialization, invalidation, and
the optional explicit two-dimensional relation. The layout lists every creator
parameter once, in its declared group and display order.

Definitions use strict `.tnpattern` format version 1 and recipe version 2.
Recipe version 1 is rejected. There is no migration or defaulting path for an
obsolete definition: assets, fixtures, and callers must use the current
definition. Metadata is serialized with the definition and validated before a
recipe executes.

## Numeric storage and display semantics

The category and unit are a semantic pair, not a generic widget hint. The
generic editor must preserve stored values and use these display rules.

| Category | Value type and unit | Required domain or display meaning |
| --- | --- | --- |
| `BoundedNumber` | number; `Unitless` or `Pixels` | General bounded scalar; percentages and normalized effects must use their dedicated semantic categories. |
| `IntegerCount` | integer; `Count` | A cardinality, with integer increment and zero displayed precision. |
| `IntegerValue` | integer; `None` | A discrete value that is not a count, such as a deterministic seed; it has no physical or display unit. |
| `Angle` | number; `Degrees` | Angular value in degrees. |
| `Percentage` | number; `Percent` | Stored as a normalized fraction in `[0, 1]`; display as `stored * 100` with a percent sign. Its stored increment is also multiplied by 100 for display, and `precision` is exactly the decimal precision of that displayed increment. |
| `NormalizedInfluence` | number; `Normalized` | A genuine normalized effect in `[0, 1]`, not a convenient name for arbitrary response strength. |
| `ResponseExponent` | number; `Unitless` | A response/exponent control whose declared constraint may exceed 1; it is distinct from normalized influence. |
| `DocumentRelativeDistance` | number or integer; `DocumentRelativeDistance` | A distance or coordinate in canonical document/artboard units, independent of device pixels and viewport scaling. |
| `TwoDimensionalOffset` | number; `DocumentRelativeDistance` | A canonical document/artboard offset. It must declare a pair ID and X or Y axis, and both axes must be present exactly once. |
| `QualityTolerance` | number; `Unitless` or `Pixels` as declared | A quality/tolerance scalar; its declared unit distinguishes a unitless algorithmic tolerance from a pixel-space tolerance. |
| `Boolean`, `Enumeration`, `Text`, `SvgAsset` | matching nonnumeric type; `None` | Typed nonnumeric values with category-specific validation. |

All numeric metadata increments must match the numeric constraint step. Integer
categories require an integer increment and precision zero. Percentage and
normalized influence require numeric constraints bounded by `[0, 1]`; the
percentage precision rule deliberately concerns the displayed percentage,
whereas other numeric precision describes the stored increment. Generic UI
must not silently convert document-relative values to pixels, infer a unit for
a seed, or collapse response exponent into normalized influence.

## Current classification examples

Bundled definitions are executable schema artifacts as well as recipes. Their
current metadata makes the intended distinctions concrete:

* Weighted Voronoi `seed` is `IntegerValue` with unit `None`.
* Weighted Voronoi `density-strength` and `response-strength`, and the
  channel weight response metadata, are `ResponseExponent` with unit
  `Unitless`.
* Percent thresholds, opacity, random-size response, and minimum cell scale
  use normalized stored percentages and the display-precision rule above.
* Jitter factor is the retained genuine `NormalizedInfluence` example.
* Grid pivots and offsets, and Weighted Voronoi boundary gap, use
  `DocumentRelativeDistance` in canonical artboard units.

## Ownership and future consumers

`DefinitionParameterScope`, `ParameterOwnership`, and applicability stay
separate: scope identifies pattern or output-channel storage, ownership
identifies the semantic owner fixed by Gate 1, and applicability decides when a
valid parameter is relevant. A `PatternDefinition`-owned creator parameter may
be edited by Gate 4 Pattern Editor Guided controls; a `ChannelInstance`-owned
creator parameter belongs in the main-window channel surface. No Pattern Editor
action may mutate a channel-owned value. Invalidation declares whether a change
affects geometry, source sampling, mapping, or presentation. Serialization
states whether a value is persisted; the current format never invents missing
values.

Gate 2 adds no GTK controls, callback, recipe operation, or recipe semantic.
Schema-generated Guided controls and the shared non-lossy Graph editor remain
Gate 4 work. Gate 3, Spiral authoring through the same strict recipe and
operation boundary, is the exact next gate. The existing `point_sampler`
compatibility projection remains documented by Gate 1; its retained
compatibility value is not a second schema authority.

## Verification boundary

The parser/serializer tests reject incompatible category-unit pairs, incorrect
percentage display precision, and non-normalized influence ranges. They also
round-trip the response-exponent metadata. Bundled Shapes, Curves, and
Weighted Voronoi definitions parse and serialize deterministically with this
contract. These `.tnpattern` resources are the representative artifacts for
this contract; no visual artifact applies because this gate changes metadata
semantics only and intentionally leaves canonical output and GTK rendering
unchanged.
