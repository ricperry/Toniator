#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

fail() {
    printf 'architecture validation failed: %s\n' "$*" >&2
    exit 1
}

rg -q '^exclude = \["ToniatorLegacy", "shared/creatiator"\]$' Cargo.toml || fail 'workspace must exclude ToniatorLegacy and the standalone shared crate'

metadata="$(cargo metadata --format-version 1 --no-deps)"
member_count="$(jq -r '.workspace_members | length' <<<"$metadata")"
[[ "$member_count" == '10' ]] || fail "expected ten workspace members, found $member_count"

expected_members=$'toniator-app\ntoniator-cli\ntoniator-domain\ntoniator-engine\ntoniator-geometry\ntoniator-io\ntoniator-patterns\ntoniator-render\ntoniator-sampling\ntoniator-windows-fs'
actual_members="$(jq -r --arg root "$repo_root" '
    .packages[]
    | select(.manifest_path | startswith($root + "/crates/"))
    | .name
' <<<"$metadata" | sort)"
[[ "$actual_members" == "$expected_members" ]] || fail 'workspace members do not match the core and Windows infrastructure crate set'

while IFS=$'\t' read -r package dependency; do
    [[ -z "$package" ]] && continue
    case "$package:$dependency" in
        toniator-app:creatiator | \
        toniator-domain:creatiator | \
        toniator-sampling:creatiator | \
        toniator-render:creatiator | \
        toniator-geometry:toniator-domain | \
        toniator-sampling:toniator-domain | \
        toniator-sampling:toniator-geometry | \
        toniator-sampling:toniator-windows-fs | \
        toniator-patterns:toniator-domain | \
        toniator-patterns:toniator-geometry | \
        toniator-patterns:toniator-sampling | \
        toniator-render:toniator-domain | \
        toniator-render:toniator-geometry | \
        toniator-io:toniator-domain | \
        toniator-io:toniator-windows-fs | \
        toniator-engine:toniator-domain | \
        toniator-engine:toniator-sampling | \
        toniator-engine:toniator-patterns | \
        toniator-engine:toniator-render | \
        toniator-engine:toniator-io | \
        toniator-cli:toniator-domain | \
        toniator-cli:toniator-engine | \
        toniator-cli:toniator-io | \
        toniator-app:toniator-domain | \
        toniator-app:toniator-geometry | \
        toniator-app:toniator-engine | \
        toniator-app:toniator-io | \
        toniator-app:toniator-patterns)
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

# The vendored library stays headless unless the application explicitly enables GTK.
jq -e --arg path "$repo_root/shared/creatiator" '
    [.packages[] | .name as $package | .dependencies[]?
     | select(.name == "creatiator")
     | (.path == $path and .uses_default_features == false
        and .features == (if $package == "toniator-app" then ["gtk"] else [] end))]
    | length == 4 and all
' <<<"$metadata" >/dev/null || fail 'creatiator must use its local path and enable GTK only in toniator-app'

if rg -n -i --glob '*.rs' --glob 'Cargo.toml' --glob '!crates/toniator-app/**' \
    '(^|[^[:alnum:]_])(gtk4?|libadwaita|adw)([^[:alnum:]_]|$)' crates; then
    fail 'GTK/libadwaita is restricted to toniator-app'
fi

if rg -n -i --glob '*.rs' --glob 'Cargo.toml' \
    '(toniator_sampling|toniator_patterns|resvg|usvg|tiny_skia|gtk4?|libadwaita)' crates/toniator-render; then
    fail 'toniator-render must consume canonical geometry without sampling, patterns, or GTK'
fi

if rg -n --glob '*.rs' '(DocumentSession|apply_command|DocumentCommand|&mut[[:space:]]+Document)' crates/toniator-render; then
    fail 'toniator-render must not own writable document state'
fi

# Optional local agent guidance is not distributed with the public source tree.
local_guidance=()
for path in AGENTS.md .codex/agents .agents/skills; do
    [[ ! -e "$path" ]] || local_guidance+=("$path")
done
if (( ${#local_guidance[@]} )) && rg -n -i 'TON-010|Stage[[:space:]]*4\.5|4\.5[A-D]' \
    "${local_guidance[@]}"; then
    fail 'obsolete TON-010 or Stage 4.5 workflow remains active'
fi

printf 'architecture validation passed\n'
