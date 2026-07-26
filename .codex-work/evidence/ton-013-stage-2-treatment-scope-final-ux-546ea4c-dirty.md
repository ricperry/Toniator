# TON-013 Stage 2 final treatment-scope UX review

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Producing agent: UX reviewer
- Timestamp: 2026-07-26
- Verdict: Stage 2 gate passes

## Verified findings

- Source, Output, and Channel Settings are the actual top inspector sections.
- At 1000x760, Source and Output read cleanly and the expanded Treatment
  Editing Scope control remains usable without horizontal clipping.
- Treatment Editing Scope is the sole visible treatment-recipient control. It
  synchronizes Shapes and Curves targets without changing Output routing.
- Legacy Adjust Ink/Adjust Channel rows are hidden in both treatment panels and
  are absent from the screenshot; no later code path re-shows them.
- The real-channel and aggregate resources remain separate and honest: one is a
  semantic channel template and one is an All Inks/All Channels/All Layers
  status/context panel, never a fake channel or duplicate editor.
- Source and Output start expanded; Channel Settings and Treatment Settings
  start collapsed. The artifact intentionally expands Channel Settings.
- XML and Cambalache hashes remain valid.

## Remaining verification gap

No dedicated assistive-technology tree or focus-order capture was run for the
hidden compatibility widgets. Static review found no visual or normal keyboard
focus artifact because their parent rows are invisible. Narrow-window and
screen-reader behavior remain follow-up checks, not Stage 2 blockers.

## Evidence

Reviewed `src/ui.rs`, all four UI resources, `Toniator.cmb`, docs, issue ledger,
and `test-artifacts/ton-013/stage2-treatment-scope-correction.png` (1000x760).
The reviewer used targeted `rg`/`sed`/`sha256sum`/`file` inspection.

## Invalidation

Invalidate after changes to Stage 2 source/UI resources, CMB hashes, scope or
pipeline callbacks, GTK accessibility behavior/version, screenshot artifact,
Git HEAD, or relevant dirty-tree state.
