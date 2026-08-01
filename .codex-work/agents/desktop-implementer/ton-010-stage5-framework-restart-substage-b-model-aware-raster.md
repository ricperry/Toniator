# TON-010 Stage 5 Framework Restart — Substage B model-aware raster compositor

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: `2026-08-01T08:15:20-04:00`
- Git HEAD: `54a8e37d2433781eb4b11f1aa2e4cc989de385be` on `TON-010-Stage5-Framework-Restart`.
- Producing agent: `desktop-implementer`.
- Task: TON-010 Stage 5 Framework Restart, Substage B only — model-aware raster composition for canonical semantic regions with channel-local subtraction.
- Working-tree assumption: pre-existing preserved dirty state was `ISSUES.md`, `assets/CMYKexpected.png`, `assets/RGBexpected.png`, `nextPrompt.md`, `.codex-work/evidence/ton-010-stage5-manual/`, the accepted Substage A source changes/evidence, and the parent Substage A review entry. This substage modifies only `src/render.rs` and adds this evidence entry; `src/png_export.rs`, SVG, UI, persistence, presets, Shapes/Curves generation, site distribution, and Voronoi geometry are untouched.

## Implementation decisions and reused abstractions

- `render_canonical_pattern_output_cancellable` now detects region outputs whose every layer has a semantic `OutputChannelId` and belongs to one output model. Layers with no channel identity, or mixed RGB/CMYK semantic identities, retain the existing generic region algebra.
- `SemanticRegionModel`, `semantic_region_model`, `render_semantic_region_output_cancellable`, and `render_region_coverage_into_pixmap` group semantic regions by model-indexed channel. Every semantic channel has an isolated antialiased coverage pixmap; positive geometry paints coverage and `GeometryPolarity::Subtractive` removes coverage only from that channel pixmap.
- `compose_semantic_channel_coverages` combines completed coverage deterministically in fixed RGB or CMYK channel order. RGB uses additive/screen-style component accumulation; CMYK multiplies each ink's transmittance. Incoming canonical layer order cannot alter cross-channel output.
- With `white_background = false`, output preserves premultiplied semantic artwork coverage and transparent uncovered pixels. With `white_background = true`, transparent uncovered coverage is composed explicitly over white. Preview Surface and Export Background remain outside canonical artwork rendering.
- Generic region/network rendering stays on `render_regions_into_pixmap`/`render_network_into_pixmap`, preserving existing Shapes/Curves paths and general canonical subtraction behavior.

## Exact changed files and symbols

- `src/render.rs`: `SemanticRegionModel`, `SemanticChannelCoverage`, `semantic_region_model`, `semantic_channels`, `render_semantic_region_output_cancellable`, `render_region_coverage_into_pixmap`, `compose_semantic_channel_coverages`, model-aware dispatch in `render_canonical_pattern_output_cancellable` and composite-region dispatch, plus four synthetic compositor tests.
- `.codex-work/agents/desktop-implementer/ton-010-stage5-framework-restart-substage-b-model-aware-raster.md`: this implementation evidence.

## Verified findings

- RGB synthetic canonical regions produce red+green yellow, red+blue magenta, green+blue cyan, and all three white; reversing input layer order produces identical pixels. Transparent output remains transparent outside artwork; white-background output is opaque white outside and preserves the expected mixed color inside.
- CMYK cyan plus magenta yields a blue multiplicative result, remains identical under input order reversal, and a cyan subtractive hole reveals underlying magenta instead of erasing it or becoming white. This confirms sibling-channel subtraction cannot remove another channel.
- A nonzero-gap two-cell fixture leaves its raw-bisector center transparent while retaining visible cells; a zero-gap same-channel fixture has continuous opaque coverage across the shared boundary.
- Synthetic canonical preview rendering and `canonical_pattern_png_bytes` produce byte-identical model-aware pixels. The existing generic, no-semantic-channel subtraction path remains covered and passes.

## Commands and results

- `cargo fmt --check` — passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — passed.
- `cargo test --locked render::tests` — passed: 40 library tests, 0 failures.
- `cargo test --locked png_export` — passed: 7 library tests, 0 failures.
- `cargo test --locked weighted_voronoi` — passed: 7 library tests, 0 failures.
- `cargo test --locked pattern::tests` — passed: 13 library tests, 0 failures.
- `git diff --check` — passed.

## Artifacts, limitations, and follow-up review

- No runtime screenshot, GTK launch, or manual composition artifact was produced because this slice modifies only the canonical raster compositor. No PNG export source change was required.
- SVG still has its existing separate serializer/mask behavior and was intentionally not modified or claimed equivalent for the new semantic compositing contract. Multi-channel reference-artifact validation and actual GTK preview inspection remain later-stage review work.
- Documentation likely affected after milestone review: the Stage 5 framework/architecture and manual acceptance records should describe the model-aware channel-local raster contract. No durable documentation was changed in this substage.
- Invalidate this evidence if `src/render.rs`, canonical region/layer/polarity definitions, PNG routing, Weighted Voronoi producer output, SVG serializer, Git HEAD, or the recorded dirty-worktree assumptions change.
