# Toniator

Toniator is a GPL-3.0-only native Linux creative tool in a greenfield rewrite.
The accepted Stage 9 headless authoritative multi-channel render path is
checkpointed at `67e831a`, after the Stage 1 foundation, authoritative document
boundary, deterministic guide family, source sampling, rendering, scheduling,
cache, channel-model, compositor, and CLI integration stages. It provides
authoritative RGB, CMYK, and SourceColorAlpha PNG/SVG rendering over both
baseline sources, with native review artifacts under `target/validation/`.

Stage 10's view-only GTK preview is planned. Persistence, command-bound
editing, and later export controls remain planned. Stage 9E direct-source CLI
rendering still requires an explicit `--canvas`; source-native default sizing
and PNG antialiasing control are future contracts, not shipped Stage 9E
behavior.

The approved execution roadmap is [GREENFIELD_REWRITE_PLAN.md](docs/GREENFIELD_REWRITE_PLAN.md),
and the current checkpoint ledger is [ProgressTracker.md](ProgressTracker.md).
The normative design is in [Architecture Schema](Project%20Specification/ArchitectureSchema.md),
[Pattern Schema](Project%20Specification/PatternSchema.md),
[Channel Schema](Project%20Specification/ChannelSchema.md),
[Module Structure](Project%20Specification/ModuleStructure.md), and the
precedence-setting [Addendum](Project%20Specification/Addendum.md).

The headless CLI and GTK app are separate peer frontends over the shared
`toniator-engine` boundary; neither frontend owns document or pattern state.
The accepted Stage 9 CLI path is headless; GTK preview, persistence, and
command-bound editing remain separately planned work.
