# Pinned i_tree source and opt-in iterator callback

Upstream crate: i_tree 0.19.0, crates.io package checksum
`1e9d4a992a9fe83130f41ceacceac3bb116a4355dfc9c8d6ecea4f1e4b4c6caf`.
Repository: https://github.com/iShape-Rust/iTree . Published VCS commit:
`40572d3ba9bceebdcb68ec3037875b8f36cc9d87`.

Exact published library `src/` and package metadata/README are copied from the
locally resolved crate. External upstream test files are omitted. The original
copied-file hashes are in UPSTREAM-SHA256.json. LICENSE is the exact MIT text
from the pinned upstream commit; the crate archive omitted it.

The only modified source is `src/seg/tree.rs`. `next_with_control` runs the
original scan, expiration cleanup, duplicate suppression and yield ordering,
calling a supplied predicate before every entity inspection. A false predicate
returns None; the caller owns sticky cancellation/error state and discards its
local incomplete work. Ordinary Iterator::next supplies an always-true predicate
and preserves its previous order and semantics. Direct tests cover equivalence
and cancellation inside a long scan of expired entries.

This callback exists only to let the opt-in checked iOverlay split path account
hidden tree scans. Preserve the exact algorithm and no-op default behavior when
rebasing. No other tree API, balancing, insertion, or geometry behavior changes.
