# Toniator

Toniator is a GPL-3.0-only native Linux creative tool in greenfield rewrite
Stage 1. This checkout currently provides only a Rust workspace boundary:
nine empty crates, a placeholder app binary, and a headless CLI that exposes
standard help and version output. It does not yet implement document state,
pattern generation, rendering, persistence, GTK resources, or exports.

The normative design is in [Architecture Schema](Project%20Specification/ArchitectureSchema.md),
[Pattern Schema](Project%20Specification/PatternSchema.md),
[Channel Schema](Project%20Specification/ChannelSchema.md),
[Module Structure](Project%20Specification/ModuleStructure.md), and the
precedence-setting [Addendum](Project%20Specification/Addendum.md).

Planned stages will implement the shared engine pipeline and frontends. The
headless CLI and the future GTK app are separate frontends; neither is a
dependency of the core crates.
