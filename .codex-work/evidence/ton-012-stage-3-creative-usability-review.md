# TON-012 Stage 3 creative usability review

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `bac55f70e7a77ec638b8033d7801fa07141d4d7e`
- Scope: bounded review of the Stage 3 semantic controls and current GTK artifacts.
- Reviewer: `ux_reviewer`, 2026-07-26
- Positive findings: Output Model uses CMYK Print/RGB Screen terminology; scalar Apply To Active Channel versus Apply To All Channels is clear; Active Channel is conditional; RGB treatment labels switch from inks to channels; Crosshatch is separate from source routing.
- Required corrections: add concise source guidance distinguishing Value, Perceptual Lightness, and Alpha; hide or replace Source Alpha when Alpha is the source with explicit no-double-alpha guidance; make Full Color automatic separation a static/insensitive summary rather than an apparently ineffective single-option control; add inline explanation for temporary Crosshatch state and exit behavior.
- Visual uncertainty: current screenshots did not clearly expose the Document section, so a current-build artifact should be captured after correction.
- No files were edited by the reviewer.
- Invalidation: changes to semantic control layout/labels/sensitivity, synchronization, or Crosshatch transition behavior.
