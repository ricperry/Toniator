# Toniator

Toniator is a GPL-3.0-only native Linux creative tool in a greenfield rewrite.
The foundation is committed at Stage 1 (`567d307`); Stage 2's authoritative
document, validation, invalidation, revision, stale-token, and headless
`validate` contracts are committed at `e842a8a`. Stage 3's deterministic
straight-guide family output, analytical off-canvas coverage, intersection
sites, and headless `inspect grid` JSON path are committed at `f60eb65`.
Marks, rendering/export, persistence, source decoding, and GTK behavior are
not implemented yet.

The approved execution roadmap is [GREENFIELD_REWRITE_PLAN.md](docs/GREENFIELD_REWRITE_PLAN.md),
and the current checkpoint ledger is [ProgressTracker.md](ProgressTracker.md).
The normative design is in [Architecture Schema](Project%20Specification/ArchitectureSchema.md),
[Pattern Schema](Project%20Specification/PatternSchema.md),
[Channel Schema](Project%20Specification/ChannelSchema.md),
[Module Structure](Project%20Specification/ModuleStructure.md), and the
precedence-setting [Addendum](Project%20Specification/Addendum.md).

The headless CLI and GTK app are separate peer frontends over the shared
`toniator-engine` boundary; neither frontend owns document or pattern state.
Stage 3 exposes headless point/site inspection only: its JSON sites are not
plotted, so point-site correctness has not been visually confirmed. Visible
marks, rendering/export, source/sampling, persistence, and GTK workflows
remain planned work.
