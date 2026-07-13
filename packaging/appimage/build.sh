#!/usr/bin/env bash
set -Eeuo pipefail

readonly APP_ID="com.toniator.Toniator"
readonly APP_NAME="Toniator"
readonly LINUXDEPLOY_VERSION="1-alpha-20251107-1"
readonly LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage"
readonly LINUXDEPLOY_SHA256="c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d"
readonly APPIMAGETOOL_VERSION="1.9.1"
readonly APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
readonly APPIMAGETOOL_SHA256="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"
readonly APPIMAGE_RUNTIME_SIZE="944632"
readonly APPIMAGE_RUNTIME_SHA256="d30a3ba1388ef57be73faabf606e0d326682f48c87f666106a3c8d6b35b58b4f"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/../.." && pwd -P)"
APPDIR="$ROOT_DIR/target/appimage/${APP_NAME}.AppDir"
EVIDENCE_DIR="$ROOT_DIR/target/appimage/evidence"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/toniator/appimage-tools"
DESKTOP_FILE="$SCRIPT_DIR/${APP_ID}.desktop"
METAINFO_FILE="$SCRIPT_DIR/${APP_ID}.metainfo.xml"
ICON_FILE="$ROOT_DIR/assets/toniatorAnim.svg"

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

notice() {
    printf 'appimage: %s\n' "$*"
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || die "required command '$1' was not found"
}

verify_checksum() {
    local path="$1"
    local expected="$2"
    printf '%s  %s\n' "$expected" "$path" | sha256sum --check --status -
}

resolve_tool() {
    local label="$1"
    local override="$2"
    local filename="$3"
    local url="$4"
    local expected="$5"
    local cached="$CACHE_DIR/$filename"
    local partial="$cached.part"

    if [[ -n "$override" ]]; then
        [[ -f "$override" && -x "$override" ]] || die "$label override is not an executable file: $override"
        verify_checksum "$override" "$expected" || die "$label override failed SHA256 verification: $override"
        printf '%s\n' "$override"
        return
    fi

    if [[ -f "$cached" ]]; then
        if verify_checksum "$cached" "$expected"; then
            chmod 0755 "$cached"
            printf '%s\n' "$cached"
            return
        fi
        [[ "${TONIATOR_OFFLINE:-0}" != "1" ]] || die "$label cache checksum mismatch while TONIATOR_OFFLINE=1: $cached"
        notice "$label cache checksum mismatch; downloading a verified replacement" >&2
        rm -f -- "$cached"
    fi

    [[ "${TONIATOR_OFFLINE:-0}" != "1" ]] || die "$label is unavailable in verified cache and TONIATOR_OFFLINE=1"
    mkdir -p -- "$CACHE_DIR"
    rm -f -- "$partial"
    notice "downloading $label" >&2
    curl --fail --location --retry 3 --output "$partial" "$url"
    verify_checksum "$partial" "$expected" || {
        rm -f -- "$partial"
        die "$label download failed SHA256 verification"
    }
    chmod 0755 "$partial"
    mv -f -- "$partial" "$cached"
    verify_checksum "$cached" "$expected" || die "$label cache verification failed after atomic install"
    printf '%s\n' "$cached"
}

resolve_appimage_runtime() {
    local appimagetool="$1"
    local runtime="$CACHE_DIR/appimagetool-$APPIMAGETOOL_VERSION-runtime-x86_64"
    local partial="$runtime.part"

    if [[ -f "$runtime" ]] && verify_checksum "$runtime" "$APPIMAGE_RUNTIME_SHA256"; then
        printf '%s\n' "$runtime"
        return
    fi
    rm -f -- "$runtime" "$partial"
    head -c "$APPIMAGE_RUNTIME_SIZE" "$appimagetool" >"$partial"
    verify_checksum "$partial" "$APPIMAGE_RUNTIME_SHA256" || {
        rm -f -- "$partial"
        die "could not derive the pinned AppImage runtime from verified appimagetool $APPIMAGETOOL_VERSION"
    }
    mv -f -- "$partial" "$runtime"
    verify_checksum "$runtime" "$APPIMAGE_RUNTIME_SHA256" || die "derived AppImage runtime failed verification after atomic install"
    printf '%s\n' "$runtime"
}

copy_rpm_schemas() {
    local package="$1"
    local destination="$2"
    local -a files=()
    mapfile -t files < <(rpm -ql "$package" | awk '/\/usr\/share\/glib-2.0\/schemas\/.*\.(xml|enums)$/')
    ((${#files[@]} > 0)) || die "$package did not provide any GSettings schema XML/enums files"
    local file
    for file in "${files[@]}"; do
        [[ -f "$file" ]] || die "$package schema listed by RPM is missing: $file"
        install -m 0644 -- "$file" "$destination/$(basename -- "$file")"
    done
}

link_deployed_module() {
    local basename="$1"
    local relative_destination="$2"
    local deployed=""
    local destination="$APPDIR/$relative_destination"

    deployed="$(find "$APPDIR/usr/lib" "$APPDIR/usr/lib64" -type f -name "$basename" -print -quit 2>/dev/null || true)"
    [[ -n "$deployed" ]] || die "linuxdeploy did not stage required module $basename"
    mkdir -p -- "$(dirname -- "$destination")"
    ln -sfn -- "$(realpath --relative-to="$(dirname -- "$destination")" "$deployed")" "$destination"
}

restore_fedora_elfs() {
    local deployed source basename
    install -m 0755 -- "$ROOT_DIR/target/release/toniator" "$APPDIR/usr/bin/toniator"
    while IFS= read -r -d '' deployed; do
        basename="$(basename -- "$deployed")"
        source="/lib64/$basename"
        case "$basename" in
            libdconfsettings.so)
                source="/usr/lib64/gio/modules/libdconfsettings.so"
                ;;
            libim-ibus.so)
                source="/usr/lib64/gtk-4.0/4.0.0/immodules/libim-ibus.so"
                ;;
        esac
        [[ -f "$source" ]] || die "cannot restore linuxdeploy-modified Fedora ELF $basename from its host source"
        cp --dereference --preserve=mode,timestamps -- "$source" "$deployed"
        readelf -h "$deployed" >/dev/null 2>&1 || die "restored Fedora dependency is not a valid ELF: $basename"
    done < <(find "$APPDIR/usr/lib" -maxdepth 1 -type f -print0)
}

remove_host_gpu_loaders() {
    local directory deployed
    for directory in "$APPDIR/usr/lib" "$APPDIR/usr/lib64"; do
        [[ -d "$directory" ]] || continue
        while IFS= read -r -d '' deployed; do
            notice "removing host-provided Vulkan loader ${deployed#"$APPDIR/"}"
            rm -f -- "$deployed"
        done < <(find "$directory" \( -type f -o -type l \) -name 'libvulkan.so*' -print0)
    done
}

audit_appdir() {
    local library_path="$APPDIR/usr/lib:$APPDIR/usr/lib64"
    local audit_file="$EVIDENCE_DIR/elf-dependencies.txt"
    local bundled_file="$EVIDENCE_DIR/bundled-libraries.txt"
    local forbidden_file="$EVIDENCE_DIR/forbidden-bundles.txt"
    : >"$audit_file"
    : >"$bundled_file"
    : >"$forbidden_file"

    find "$APPDIR" -type f -o -type l | sort | while IFS= read -r path; do
        local_name="$(basename -- "$path")"
        if [[ "$local_name" == *.so || "$local_name" == *.so.* ]]; then
            printf '%s\n' "${path#"$APPDIR/"}" >>"$bundled_file"
        fi
        case "$local_name" in
            ld-linux*.so*|libc.so*|libm.so*|libpthread.so*|librt.so*|libdl.so*|libutil.so*|libresolv.so*|libnss_*.so*|libGL.so*|libGLX.so*|libEGL.so*|libGLES*.so*|libOpenGL.so*|libMesa*.so*|libdrm*.so*|libgbm.so*|libvulkan.so*|libwayland-client.so*|libX11.so*|libX11-xcb.so*|libxcb.so*)
                printf '%s\n' "${path#"$APPDIR/"}" >>"$forbidden_file"
                ;;
        esac
    done
    if [[ -s "$forbidden_file" ]]; then
        cat "$forbidden_file" >&2
        die "AppDir contains system, display ABI, or GPU libraries that must remain host-provided"
    fi
    if find "$APPDIR" -type d \( -name fonts -o -name dri \) -print -quit | grep -q .; then
        die "AppDir unexpectedly contains host fonts or GPU driver directories"
    fi

    local elf
    while IFS= read -r -d '' elf; do
        if readelf -h "$elf" >/dev/null 2>&1; then
            printf '\n[%s]\n' "${elf#"$APPDIR/"}" >>"$audit_file"
            LD_LIBRARY_PATH="$library_path" ldd "$elf" >>"$audit_file" 2>&1 || true
        fi
    done < <(find "$APPDIR" -type f -print0)
    if grep -q 'not found' "$audit_file"; then
        grep -B 2 -A 2 'not found' "$audit_file" >&2
        die "AppDir ELF audit found unresolved libraries"
    fi
}

[[ "$(uname -m)" == "x86_64" ]] || die "only x86_64 AppImage builds are supported by this script"

for command in cargo curl sha256sum rpm glib-compile-schemas \
    desktop-file-validate appstreamcli readelf ldd git python3 install find awk \
    grep realpath head cp; do
    require_command "$command"
done
rpm -q gtk4 >/dev/null 2>&1 || die "Fedora package 'gtk4' is required"
rpm -q gsettings-desktop-schemas >/dev/null 2>&1 || die "Fedora package 'gsettings-desktop-schemas' is required"
[[ -f /usr/lib64/gio/modules/libdconfsettings.so ]] || die "required dconf GIO module is missing; install Fedora package 'dconf'"
[[ -f /usr/lib64/gio/modules/giomodule.cache ]] || die "required Fedora GIO module cache is missing"
[[ -f "$DESKTOP_FILE" && -f "$METAINFO_FILE" && -f "$ICON_FILE" ]] || die "packaging metadata or application icon is missing"

desktop-file-validate "$DESKTOP_FILE"
appstreamcli validate --no-net "$METAINFO_FILE"

LINUXDEPLOY="$(resolve_tool \
    "linuxdeploy $LINUXDEPLOY_VERSION" \
    "${TONIATOR_LINUXDEPLOY:-}" \
    "linuxdeploy-$LINUXDEPLOY_VERSION-x86_64.AppImage" \
    "$LINUXDEPLOY_URL" \
    "$LINUXDEPLOY_SHA256")"
APPIMAGETOOL="$(resolve_tool \
    "appimagetool $APPIMAGETOOL_VERSION" \
    "${TONIATOR_APPIMAGETOOL:-}" \
    "appimagetool-$APPIMAGETOOL_VERSION-x86_64.AppImage" \
    "$APPIMAGETOOL_URL" \
    "$APPIMAGETOOL_SHA256")"
APPIMAGE_RUNTIME="$(resolve_appimage_runtime "$APPIMAGETOOL")"

VERSION="$(cargo metadata --manifest-path "$ROOT_DIR/Cargo.toml" --no-deps --format-version 1 | python3 -c '
import json, os, sys
metadata = json.load(sys.stdin)
root = os.path.realpath(sys.argv[1] + "/Cargo.toml")
matches = [p["version"] for p in metadata["packages"] if os.path.realpath(p["manifest_path"]) == root]
if len(matches) != 1:
    raise SystemExit("could not identify the root Cargo package version")
print(matches[0])
' "$ROOT_DIR")"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$ ]] || die "Cargo reported an unsafe package version: $VERSION"

if [[ -z "${SOURCE_DATE_EPOCH:-}" ]]; then
    SOURCE_DATE_EPOCH="$(git -C "$ROOT_DIR" log -1 --format=%ct 2>/dev/null || true)"
    [[ -n "$SOURCE_DATE_EPOCH" ]] || SOURCE_DATE_EPOCH="$(date +%s)"
fi
[[ "$SOURCE_DATE_EPOCH" =~ ^[0-9]+$ ]] || die "SOURCE_DATE_EPOCH must be an integer Unix timestamp"
export SOURCE_DATE_EPOCH

OUTPUT_DIR="$ROOT_DIR/dist"
OUTPUT="$OUTPUT_DIR/${APP_NAME}-${VERSION}-x86_64.AppImage"
TEMP_OUTPUT="$OUTPUT.tmp"
rm -rf -- "$APPDIR"
rm -f -- "$OUTPUT" "$TEMP_OUTPUT"
mkdir -p -- "$APPDIR/usr/bin" "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
    "$APPDIR/usr/share/metainfo" "$APPDIR/usr/share/doc/toniator" "$EVIDENCE_DIR" "$OUTPUT_DIR"

notice "building Toniator $VERSION with the locked dependency graph"
cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --locked --release
install -m 0755 -- "$ROOT_DIR/target/release/toniator" "$APPDIR/usr/bin/toniator"
install -m 0644 -- "$ICON_FILE" "$APPDIR/usr/share/icons/hicolor/scalable/apps/${APP_ID}.svg"

linuxdeploy_args=(
    --appimage-extract-and-run
    --appdir "$APPDIR"
    --executable "$APPDIR/usr/bin/toniator"
    --desktop-file "$DESKTOP_FILE"
    --icon-file "$APPDIR/usr/share/icons/hicolor/scalable/apps/${APP_ID}.svg"
    --library /usr/lib64/gio/modules/libdconfsettings.so
)
if [[ -f /usr/lib64/gtk-4.0/4.0.0/immodules/libim-ibus.so ]]; then
    linuxdeploy_args+=(--library /usr/lib64/gtk-4.0/4.0.0/immodules/libim-ibus.so)
else
    notice "optional GTK4 IBus module is absent; complex input through IBus will use host integration when available"
fi
notice "deploying recursive non-excluded ELF dependencies"
NO_STRIP=1 "$LINUXDEPLOY" "${linuxdeploy_args[@]}"
notice "restoring Fedora RELR ELFs after linuxdeploy dependency discovery"
restore_fedora_elfs
remove_host_gpu_loaders

install -m 0644 -- "$METAINFO_FILE" "$APPDIR/usr/share/metainfo/${APP_ID}.metainfo.xml"
ln -sfn -- "${APP_ID}.metainfo.xml" "$APPDIR/usr/share/metainfo/${APP_ID}.appdata.xml"
install -m 0644 -- "$ROOT_DIR/LICENSE" "$APPDIR/usr/share/doc/toniator/LICENSE"

SCHEMA_DIR="$APPDIR/usr/share/glib-2.0/schemas"
mkdir -p -- "$SCHEMA_DIR"
copy_rpm_schemas gtk4 "$SCHEMA_DIR"
copy_rpm_schemas gsettings-desktop-schemas "$SCHEMA_DIR"
glib-compile-schemas --strict "$SCHEMA_DIR"

link_deployed_module libdconfsettings.so usr/lib64/gio/modules/libdconfsettings.so
if [[ -f /usr/lib64/gtk-4.0/4.0.0/immodules/libim-ibus.so ]]; then
    link_deployed_module libim-ibus.so usr/lib64/gtk-4.0/4.0.0/immodules/libim-ibus.so
fi
grep '^libdconfsettings\.so:' /usr/lib64/gio/modules/giomodule.cache \
    >"$APPDIR/usr/lib64/gio/modules/giomodule.cache"
[[ -s "$APPDIR/usr/lib64/gio/modules/giomodule.cache" ]] || die "Fedora GIO cache has no dconf settings backend entry"

ln -sfn -- "usr/share/applications/${APP_ID}.desktop" "$APPDIR/${APP_ID}.desktop"
ln -sfn -- "usr/share/icons/hicolor/scalable/apps/${APP_ID}.svg" "$APPDIR/${APP_ID}.svg"
ln -sfn -- "usr/share/icons/hicolor/scalable/apps/${APP_ID}.svg" "$APPDIR/.DirIcon"

rm -f -- "$APPDIR/AppRun"
cat >"$APPDIR/AppRun" <<'APP_RUN'
#!/usr/bin/env bash
set -Eeuo pipefail
APPDIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
export LD_LIBRARY_PATH="$APPDIR/usr/lib:$APPDIR/usr/lib64${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export XDG_DATA_DIRS="$APPDIR/usr/share${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
export GSETTINGS_SCHEMA_DIR="$APPDIR/usr/share/glib-2.0/schemas"
export GIO_MODULE_DIR="$APPDIR/usr/lib64/gio/modules"
export GTK_PATH="$APPDIR/usr/lib64/gtk-4.0${GTK_PATH:+:$GTK_PATH}"
exec "$APPDIR/usr/bin/toniator" "$@"
APP_RUN
chmod 0755 "$APPDIR/AppRun"
readelf -h "$APPDIR/usr/bin/toniator" >/dev/null 2>&1 || die "staged Toniator executable is not an ELF binary after AppRun installation"

desktop-file-validate "$APPDIR/usr/share/applications/${APP_ID}.desktop"
appstreamcli validate --no-net "$APPDIR/usr/share/metainfo/${APP_ID}.metainfo.xml"
audit_appdir

notice "creating $OUTPUT"
ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run \
    --runtime-file "$APPIMAGE_RUNTIME" "$APPDIR" "$TEMP_OUTPUT"
[[ -s "$TEMP_OUTPUT" ]] || die "appimagetool did not create the expected output: $TEMP_OUTPUT"
chmod 0755 "$TEMP_OUTPUT"
mv -f -- "$TEMP_OUTPUT" "$OUTPUT"
sha256sum "$OUTPUT" | tee "$EVIDENCE_DIR/appimage.sha256"
notice "created $OUTPUT"
