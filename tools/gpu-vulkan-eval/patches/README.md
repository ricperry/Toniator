# Single experimental capacity patch

Base upstream: https://github.com/linebender/vello
Commit: `72c2fb0628dc6f4bbc7eaf3375284be89be03a06`.
Package: `vello_encoding`0.11.0, Apache-2.0 OR MIT.

`vello-encoding-capacity-v1.patch` increases only lines/segment-counts/segments
from2^21 to2^23 (24/8/24bytes per element; total112→448MiB, increment336MiB).
It adds literal responsibility/bounds documentation and an explicit Cargo
`package.workspace="../.."` owner because the isolated Git-patch path otherwise
inherits the wrong enclosing workspace. No shader, geometry, quality, tolerance,
blend, other buffer, automatic retry or production dependency change occurs.
This one predeclared variant is authorized research, not upstream adoption.
External cgroup/host/VRAM/GTT budgets remain unchanged. Require separate actual
allocation-counter validation with failed0/allcapacities before full timing.

The source worktree is ignored research output; keep this reviewed patch and
base pin to reproduce it. Never reset/clean/overwrite an existing worktree.
