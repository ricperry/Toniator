#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

fail() {
    printf 'architecture validation failed: %s\n' "$*" >&2
    exit 1
}

rg -q '^exclude = \["ToniatorLegacy"\]$' Cargo.toml || fail 'workspace must exclude ToniatorLegacy'

metadata="$(cargo metadata --format-version 1 --no-deps)"
member_count="$(jq -r '.workspace_members | length' <<<"$metadata")"
[[ "$member_count" == '9' ]] || fail "expected nine workspace members, found $member_count"

expected_members=$'toniator-app\ntoniator-cli\ntoniator-domain\ntoniator-engine\ntoniator-geometry\ntoniator-io\ntoniator-patterns\ntoniator-render\ntoniator-sampling'
actual_members="$(jq -r --arg root "$repo_root" '
    .packages[]
    | select(.manifest_path | startswith($root + "/crates/"))
    | .name
' <<<"$metadata" | sort)"
[[ "$actual_members" == "$expected_members" ]] || fail 'workspace members do not match the Stage 1 crate set'

while IFS=$'\t' read -r package dependency; do
    [[ -z "$package" ]] && continue
    case "$package:$dependency" in
        toniator-geometry:toniator-domain | \
        toniator-sampling:toniator-domain | \
        toniator-sampling:toniator-geometry | \
        toniator-patterns:toniator-domain | \
        toniator-patterns:toniator-geometry | \
        toniator-patterns:toniator-sampling | \
        toniator-render:toniator-domain | \
        toniator-render:toniator-geometry | \
        toniator-io:toniator-domain | \
        toniator-engine:toniator-domain | \
        toniator-engine:toniator-sampling | \
        toniator-engine:toniator-patterns | \
        toniator-engine:toniator-render | \
        toniator-engine:toniator-io | \
        toniator-cli:toniator-domain | \
        toniator-cli:toniator-engine | \
        toniator-cli:toniator-io | \
        toniator-app:toniator-domain | \
        toniator-app:toniator-engine | \
        toniator-app:toniator-io)
            ;;
        *)
            fail "forbidden workspace dependency: $package -> $dependency"
            ;;
    esac
done < <(jq -r --arg root "$repo_root" '
    .packages[]
    | select(.manifest_path | startswith($root + "/crates/"))
    | .name as $package
    | .dependencies[]?
    | select(.path != null)
    | "\($package)\t\(.name)"
' <<<"$metadata")

if rg -n -i --glob '*.rs' --glob 'Cargo.toml' --glob '!crates/toniator-app/**' \
    '(^|[^[:alnum:]_])(gtk4?|libadwaita|adw)([^[:alnum:]_]|$)' crates; then
    fail 'GTK/libadwaita is restricted to toniator-app'
fi

if rg -n -i 'TON-010|Stage[[:space:]]*4\.5|4\.5[A-D]' \
    AGENTS.md .codex/agents .agents/skills; then
    fail 'obsolete TON-010 or Stage 4.5 workflow remains active'
fi

printf 'architecture validation passed\n'
