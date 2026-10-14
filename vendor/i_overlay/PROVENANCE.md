# Pinned iOverlay source and local checked patch

Upstream crate: i_overlay 9.0.0, crates.io package checksum
`a518dce2d7cf7e3758159747e7175c8b86ccdcd8d333dfcb7dbeb3eac2e324fc`.
Repository: https://github.com/iShape-Rust/iOverlay . Published VCS commit:
`26782e3b93f9b7d2516e7653f3d56eebc31af798`, package subdirectory `iOverlay`.

The exact published library `src/` and package metadata/README/changelog are copied
from the locally resolved crate. Upstream external test data and README images
are omitted; they are not runtime sources. UPSTREAM-SHA256.json records every
original copied file before the local patch. LICENSE-MIT and LICENSE-APACHE are
exact upstream repository texts from the pinned commit (the crate archive omitted
them). Upstream offers MIT OR Apache-2.0; both notices are retained.

Local modifications, 2026-10-13:

- `core/checked.rs` and `core/mod.rs`: opt-in i64 control, errors, fixed radii
  1/2/4, at most 16 split rounds per attempt shared across strategy fallback,
  common request-work accounting, and exact absolute split-mutation certificates.
- `float/overlay.rs`: opt-in checked method with cleanup disabled. The default
  floating overlay method and its cleanup behavior remain available unchanged.
- `split/{solver,solver_list,solver_tree,solver_fragment,grid_layout}.rs` and
  `segm/merge.rs`: thread compile-time optional controls through the original
  enumeration, ordering, central split application, deduplication and merge.
  Every actual edge split certifies its current directed remainder before mutation.
  Existing ordinary routes supply a no-op control. No replacement all-pairs engine
  or copied geometry solver is introduced.
- The existing pinned i_tree 0.19.0 has a companion opt-in iterator callback;
  see its separate provenance. Fragment column scans, expired/duplicate tree
  entities, merges, deduplication and submark traversal/reversal are charged and
  polled in their expensive loops.

The doubled-area certificate bounds only local split-mutation winding changes
relative to already-quantized integer input. It does not certify float input
quantization, curve flattening, graph extraction, or unrelated geometry code.
Discarded attempts contribute work but contribute no accepted geometry/certificate.
The resource allowance is not a complexity proof. Synchronous upstream allocation,
sort, fill/graph construction, extraction and float conversion retain cancellation
latency gaps; pre/post checks and phase allowances do not prove in-phase polling.

Maintenance: retain the exact dependency pin, compare changed files against the
original SHA inventory, and reapply/review this small opt-in patch when upgrading.
Run the current renderer checked/area/coverage/SVG regressions and the direct
certificate/fragment/tree callback tests; do not treat a new upstream version as
automatically equivalent. Do not run omitted historical upstream fixture suites
from this source-only vendor package.
