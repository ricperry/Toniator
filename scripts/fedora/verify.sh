#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
cd "${REPO_ROOT}"
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:${PATH}"

fail() {
    printf 'Fedora verification failed: %s\n' "$*" >&2
    exit 1
}

if [[ ! -r /etc/os-release ]]; then
    fail 'cannot identify the operating system from /etc/os-release'
fi
# shellcheck disable=SC1091
. /etc/os-release
[[ "${ID:-}" == fedora && "${VERSION_ID:-}" == 44 ]] || \
    fail "requires Fedora 44 (found ${ID:-unknown} ${VERSION_ID:-unknown})"
[[ "$(uname -m)" == x86_64 ]] || \
    fail "requires x86_64 (found $(uname -m))"

RUST_CHANNEL="$(bash "${SCRIPT_DIR}/rust-channel.sh")"
if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="${REPO_ROOT}/target/fedora44"
fi
mkdir -p "${CARGO_TARGET_DIR}"

rpm -q --qf '%{NAME} %{VERSION}-%{RELEASE}.%{ARCH}\n' \
    ffmpeg-free fontconfig dejavu-sans-fonts
command -v ffmpeg >/dev/null || fail 'ffmpeg-free did not provide ffmpeg'
printf 'Qualified FFmpeg: '
ffmpeg -hide_banner -version | sed -n '1p'

ffmpeg_encoders="$(ffmpeg -hide_banner -encoders 2>&1)"
ffmpeg_decoders="$(ffmpeg -hide_banner -decoders 2>&1)"
require_encoder() {
    local codec="$1"
    grep -Eq "^[[:space:]]*V[^[:space:]]{5}[[:space:]]+${codec}([[:space:]]|$)" \
        <<<"${ffmpeg_encoders}" || fail "FFmpeg lacks required video encoder ${codec}"
}
require_decoder() {
    local codec="$1"
    grep -Eq "^[[:space:]]*V[^[:space:]]{5}[[:space:]]+${codec}([[:space:]]|$)" \
        <<<"${ffmpeg_decoders}" || fail "FFmpeg lacks required video decoder ${codec}"
}
for codec in ffv1 libsvtav1 libvpx libvpx-vp9; do
    require_encoder "${codec}"
done
for codec in ffv1 vp8 vp9; do
    require_decoder "${codec}"
done
printf 'Qualified FFmpeg video encoders: ffv1, libsvtav1, libvpx, libvpx-vp9\n'
printf 'Qualified FFmpeg video decoders: ffv1, vp8, vp9\n'

# The immutable SVG sample has live text. Pin its generic sans-serif resolution
# to the installed DejaVu Sans face instead of relying on the host's font order.
export FONTCONFIG_FILE="${CARGO_TARGET_DIR}/validation-fonts.conf"
cat >"${FONTCONFIG_FILE}" <<'FONTCONFIG'
<fontconfig><dir>/usr/share/fonts/dejavu-sans-fonts</dir><alias><family>sans-serif</family><prefer><family>DejaVu Sans</family></prefer></alias></fontconfig>
FONTCONFIG
resolved_sans="$(fc-match -f '%{family}' sans-serif)"
printf 'Qualified generic sans-serif: %s\n' "${resolved_sans}"
[[ "${resolved_sans}" == *'DejaVu Sans'* ]] || \
    fail "generic sans-serif did not resolve to DejaVu Sans (found ${resolved_sans})"

printf 'Using Rust toolchain %s from rust-toolchain.toml\n' "${RUST_CHANNEL}"
cargo "+${RUST_CHANNEL}" build --locked -p toniator-app -p toniator-cli
RUSTUP_TOOLCHAIN="${RUST_CHANNEL}" bash scripts/validate_architecture.sh

# Storage and preset persistence.
for test_target in \
    persistence \
    document_presets \
    personal_library_final \
    stage22_temporal_persistence \
    recent_files; do
    cargo "+${RUST_CHANNEL}" test --locked -p toniator-io --test "${test_target}"
done
cargo "+${RUST_CHANNEL}" test --locked -p toniator-io --lib sequence::tests
cargo "+${RUST_CHANNEL}" test --locked -p toniator-io --lib video_output::tests

# These media tests decode both immutable project-wide raster/SVG samples and
# qualify the required FFV1, AV1, VP8, and VP9 paths on the installed package.
cargo "+${RUST_CHANNEL}" test --locked -p toniator-sampling --lib concurrent_decoders -- --nocapture
cargo "+${RUST_CHANNEL}" test --locked -p toniator-sampling --test stage22_media -- --nocapture
cargo "+${RUST_CHANNEL}" test --locked -p toniator-sampling --test stage22_alpha_sar -- --nocapture
cargo "+${RUST_CHANNEL}" test --locked -p toniator-engine --lib export::video:: -- --nocapture
cargo "+${RUST_CHANNEL}" test --locked -p toniator-engine --lib media_import:: -- --nocapture
cargo "+${RUST_CHANNEL}" test --locked -p toniator-cli --test stage22_frames -- --nocapture
cargo "+${RUST_CHANNEL}" run --locked -p toniator-cli -- --help

# Exercise ALL and named SourceColorAlpha previews through actual rendering,
# plus the retained black-neutral preview for RGB and CMYK.
cargo "+${RUST_CHANNEL}" test --locked -p toniator-app --bin toniator-app \
    stage21b_source_color_alpha_wizard_previews_preserve_sampled_paint_and_render -- --nocapture
cargo "+${RUST_CHANNEL}" test --locked -p toniator-app --bin toniator-app \
    stage21b_review_preview_is_one_black_channel_without_draft_mutation -- --nocapture

# Keep production linting scoped to the changed packages and focused media/IO
# test targets; no historical test suite is included.
cargo "+${RUST_CHANNEL}" clippy --locked -p toniator-io --lib \
    --test persistence --test document_presets --test personal_library_final \
    --test stage22_temporal_persistence --test recent_files -- -D warnings
cargo "+${RUST_CHANNEL}" clippy --locked -p toniator-sampling --lib \
    --test stage22_media --test stage22_alpha_sar -- -D warnings
cargo "+${RUST_CHANNEL}" clippy --locked -p toniator-engine --lib -- -D warnings
cargo "+${RUST_CHANNEL}" clippy --locked -p toniator-cli --bin toniator \
    --test stage22_frames -- -D warnings
cargo "+${RUST_CHANNEL}" clippy --locked -p toniator-app --bin toniator-app -- -D warnings
