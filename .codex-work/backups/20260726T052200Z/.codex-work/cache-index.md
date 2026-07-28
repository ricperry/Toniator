# Toniator evidence cache index

This index points agents to reusable evidence under `.codex-work/`. It is
checkout-aware and must not be treated as authoritative over current files.

Add one entry per reusable cache record with:

- Cache key and relative entry path
- Repository absolute path, Git HEAD, and relevant dirty files
- Producing agent, task/subsystem, and timestamp
- Validity status or last validation
- Short scope note and invalidation conditions

Read the linked entry and validate it against the current checkout before use.
The parent thread records read-only-agent updates here after persisting them.
