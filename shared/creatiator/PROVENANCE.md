# Source and license provenance

This crate is extracted from the local Bezziator Rust GTK4 application and
Chromiator color code on 2026-11-21. Both use `GPL-3.0-or-later`; the full GPL v3
text is included in `LICENSE`. It may be used under GPL v3 by Toniator's
`GPL-3.0-only` distribution without changing Toniator's license declaration.

| Original path in Bezziator | SHA-256 before extraction | Use here |
| --- | --- | --- |
| `crates/bezziator-app/src/theme.rs` | `3ead67b097449914466861014e17410f2b5630ca5832d22a32417b78d691baed` | `src/theme.rs`; imports and behavior retained, responsibility documentation added |
| `crates/bezziator-app/resources/creative-suite.css` | `9a6f7db1b90600ba4688697711696b75564a26bcdb37b3902271181d6f5dcf33` | `resources/creative-suite.css`; byte-identical shared baseline |
| `LICENSE` | `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986` | `LICENSE`; byte-identical license text |

The component arithmetic of `src/color.rs` is extracted from
`Chromiator/src/color.rs`, `encoded_to_linear` and `linear_to_encoded`, with the
array mapping left to each consumer. That source's SHA-256 before extraction is
`a9741490bb53f9d3ba70f2f038550682209ea7baa88e3c6116642ed832e1deaf`.
`Chromiator/Cargo.toml` declares `GPL-3.0-or-later`; `Chromiator/LICENSE.md`
has the same license-text hash as the included Bezziator license.

The original theme source carries `SPDX-License-Identifier: GPL-3.0-or-later`.
The suite stylesheet is also byte-identical in Toniator and Chromiator. The new
`install_styles` helper installs this baseline below application-specific CSS;
it does not copy any application's resource names or UI templates. No source
is extracted from Toniator's protected specifications or legacy implementation.
The new threshold/extended-range tests preserve the extracted transfer policy;
the portal tuple test independently exercises the standard `(v)` reply shape.

The source repositories contain existing uncommitted work. These hashes identify
the exact extracted inputs without asserting that they match a Git checkpoint.
