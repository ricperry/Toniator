# TON-010 Stage 5 architecture map

| Consumer or boundary | Current path | New framework status | Intended next integration |
| --- | --- | --- | --- |
| Weighted Voronoi | `pattern_state` -> `render.rs` -> `weighted_voronoi.rs` -> positive canonical inset regions | Migrated: site distribution, clipped Voronoi geometry, direct final inset cells, and model-aware channel consumers | Extend only with future response/geometry services as needed |
| Shapes | `pattern_state` -> Shapes compatibility adapter -> canonical marks | Not migrated; unchanged Stage 4 path | Consume guide primitives or ordered points when a Shapes family is scheduled |
| Curves | `pattern_state` -> Curves compatibility adapter -> canonical paths | Not migrated; unchanged Stage 4 path | Consume sampled/connected paths or intersections when a Curves family is scheduled |
| Preview | `render_document_preview_cancellable` -> canonical output renderer | Shared canonical route | No separate generator; retain stale-result suppression |
| PNG export | `png_export.rs` -> canonical output renderer | Shared canonical route | No separate generator |
| SVG export | `svg_export.rs` -> canonical output serializer | Shared canonical route | Preserve explicit canonical relationships and editable structure |
| Persistence | `PatternDocumentState` and strict versioned parameters | Weighted settings persisted in existing current envelope | Reject obsolete generator versions; no migration path |
| Presets | scoped current-format `.tntr` sections | Weighted treatment/channel/complete sections supported | Keep current schema strict |
| UI editing | Blueprint selector/panel -> `DocumentEditor` -> `pattern_state` | Weighted selector and specialized inspector migrated | Generic schema controls remain future work; avoid broad IA reorganization |

Only the Weighted Voronoi row is a shipped consumer of the new neutral
services. The other rows intentionally record their previous path rather than
claiming a framework migration.

Weighted Voronoi's canonical output contains the final boundary-derived inset
polygons as positive regions. Raw cells and cell-sizing boundary rings do not
survive the producer boundary. Preview and PNG render semantic channels through
isolated coverage surfaces before RGB additive or CMYK multiplicative
composition; SVG keeps named channel layers and emits one compound positive
path per channel. The shared canonical algebra still supports genuine
subtractive regions, which retain channel/layer-local masks in SVG.
