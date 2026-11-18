#!/usr/bin/env bash
# Make the Linux desktop bundles portable to hosts other than the build image.
#
# `dx bundle --package-types appimage` copies the build host's shared libraries
# into `usr/lib` and links the binary with `RUNPATH=$ORIGIN/../lib`, but it does
# not copy the WebKit helper processes. The UI process then runs the bundled
# WebKit while every helper it spawns runs the host's copy, which only agrees
# when the host is the build image; elsewhere the window opens but never paints.
# The same AppDir ships libwayland-client built against a newer Mesa than the
# one on the host, so EGL fails before the web process starts.
#
# Usage:
#   package-linux-desktop.sh appimage <input.AppImage> <output.AppImage>
#   package-linux-desktop.sh deb      <input.deb>      <output.deb>
#   package-linux-desktop.sh smoke    <input.AppImage>
#
# WEBKIT_LIBEXEC overrides the WebKit helper directory
# (default: /usr/lib/<multiarch>/webkit2gtk-4.1).
set -euo pipefail

readonly SCRIPT_NAME="${0##*/}"

die() {
    printf '%s: %s\n' "$SCRIPT_NAME" "$*" >&2
    exit 1
}

usage() {
    printf 'usage: %s {appimage <in> <out> | deb <in> <out> | smoke <AppImage>}\n' \
        "$SCRIPT_NAME" >&2
    exit 2
}

multiarch() {
    dpkg-architecture -qDEB_HOST_MULTIARCH 2>/dev/null \
        || gcc -print-multiarch 2>/dev/null \
        || die "cannot determine the multiarch triplet; install dpkg-dev or gcc"
}

# The display and GPU stack belongs to the host: Mesa, the compositor and the
# kernel driver are one unit. A bundled copy makes EGL fail with
# EGL_BAD_PARAMETER or the WebKit process die with "symbol lookup error".
is_host_owned() {
    case "${1##*/}" in
        libwayland-*|libxkbcommon*|libEGL*|libGLES*|libGL.so*|libGLX*|libgbm*|libdrm*|libvulkan*) return 0 ;;
        *) return 1 ;;
    esac
}

# WebKit resolves its helper processes from a compile-time LIBEXECDIR that only
# exists where the matching WebKit package is installed. Rewrite those three
# strings to paths relative to the directory AppRun changes into.
relocate_webkit() {
    local library="$1" multi="$2"
    python3 - "$library" "/usr/lib/$multi" <<'PY'
import sys

path, base = sys.argv[1], sys.argv[2].encode()
replacements = [
    (base + b"/webkit2gtk-4.1/injected-bundle/", b"./webkit2gtk-4.1/injected-bundle/"),
    (base + b"/webkit2gtk-4.1", b"./webkit2gtk-4.1"),
    (base, b"."),
]
data = open(path, "rb").read()
for old, new in replacements:
    count = data.count(old)
    if count != 1:
        sys.exit(f"{path}: expected one {old.decode()!r}, found {count}; "
                 "WebKit changed its libexec layout")
    data = data.replace(old, new + b"\x00" * (len(old) - len(new)))
open(path, "wb").write(data)
PY
}

write_apprun() {
    local appdir="$1" multi="$2"
    rm -f "$appdir/AppRun"
    {
        printf '#!/bin/sh\n'
        printf 'set -eu\n'
        printf 'HERE="$(dirname "$(readlink -f "$0")")"\n'
        printf 'MULTIARCH=%s\n' "$multi"
        printf 'export LD_LIBRARY_PATH="$HERE/usr/lib:$HERE/usr/lib/$MULTIARCH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"\n'
        printf 'export WEBKIT_INJECTED_BUNDLE_PATH="$HERE/usr/lib/$MULTIARCH/webkit2gtk-4.1/injected-bundle"\n'
        printf 'export XDG_DATA_DIRS="$HERE/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"\n'
        printf 'cd "$HERE/usr/lib/$MULTIARCH"\n'
        printf 'exec "$HERE/usr/bin/address-explorer" "$@"\n'
    } > "$appdir/AppRun"
    chmod 0755 "$appdir/AppRun"
}

verify_tree() {
    local appdir="$1" multi="$2"
    local libexec="$appdir/usr/lib/$multi/webkit2gtk-4.1" name

    for name in WebKitWebProcess WebKitNetworkProcess WebKitGPUProcess; do
        [ -x "$libexec/$name" ] || die "missing $libexec/$name"
    done
    [ -f "$libexec/injected-bundle/libwebkit2gtkinjectedbundle.so" ] \
        || die "missing the injected WebKit bundle"

    for name in "$appdir/usr/lib"/*; do
        [ -e "$name" ] || continue
        if is_host_owned "$name"; then
            die "host-owned library still bundled: ${name##*/}"
        fi
    done

    if grep -q -a -- "/usr/lib/$multi" "$appdir/usr/lib/libwebkit2gtk-4.1.so.0"; then
        die "libwebkit2gtk still resolves helpers from /usr/lib/$multi"
    fi
    head -n 1 "$appdir/AppRun" | grep -q '^#!/bin/sh' || die "AppRun is not a script"
}

appimage() {
    local input="$1" output="$2"
    local multi="${MULTIARCH:-$(multiarch)}"
    local libexec="${WEBKIT_LIBEXEC:-/usr/lib/$multi/webkit2gtk-4.1}"

    [ -f "$input" ] || die "no such AppImage: $input"
    [ -x "$libexec/WebKitWebProcess" ] \
        || die "no WebKit helpers in $libexec; install libwebkit2gtk-4.1-0"
    command -v mksquashfs >/dev/null || die "mksquashfs is required (squashfs-tools)"

    ( cd "$WORKDIR" && "$input" --appimage-extract >/dev/null )
    local appdir="$WORKDIR/squashfs-root"
    [ -x "$appdir/usr/bin/address-explorer" ] || die "$input is not a Dioxus AppDir"

    local entry
    for entry in "$appdir/usr/lib"/*; do
        [ -e "$entry" ] || continue
        if is_host_owned "$entry"; then
            rm -f "$entry"
        fi
    done
    mkdir -p "$appdir/usr/lib/$multi"
    cp -a "$libexec" "$appdir/usr/lib/$multi/"
    rm -f "$appdir/usr/lib/$multi/webkit2gtk-4.1/MiniBrowser"

    relocate_webkit "$(readlink -f "$appdir/usr/lib/libwebkit2gtk-4.1.so.0")" "$multi"
    write_apprun "$appdir" "$multi"
    verify_tree "$appdir" "$multi"

    head -c "$("$input" --appimage-offset)" "$input" > "$output"
    mksquashfs "$appdir" "$WORKDIR/root.squashfs" \
        -comp gzip -b 131072 -all-root -noappend -no-progress >/dev/null
    cat "$WORKDIR/root.squashfs" >> "$output"
    chmod 0755 "$output"
    printf 'appimage: %s (%s bytes, WebKit from %s)\n' \
        "$output" "$(stat -c %s "$output")" "$libexec"
}

deb() {
    local input="$1" output="$2"
    local depends='libwebkit2gtk-4.1-0, libgtk-3-0 | libgtk-3-0t64, libxdo3, libsoup-3.0-0'

    [ -f "$input" ] || die "no such package: $input"

    dpkg-deb --raw-extract "$input" "$WORKDIR/root"
    local control="$WORKDIR/root/DEBIAN/control"
    [ -f "$control" ] || die "$input has no control file"
    if grep -q '^Depends:' "$control"; then
        die "$input already declares Depends; update this script instead"
    fi
    printf 'Depends: %s\n' "$depends" >> "$control"
    dpkg-deb --build --root-owner-group "$WORKDIR/root" "$output" >/dev/null

    dpkg-deb -f "$output" Depends | grep -q 'libwebkit2gtk-4.1-0' \
        || die "the rebuilt .deb lost its Depends field"
    printf 'deb: %s (%s bytes)\n' "$output" "$(stat -c %s "$output")"
}

smoke() {
    local input="$1" log="$WORKDIR/app.log" helper root="" attempt status=0 exe pid
    local runner

    [ -f "$input" ] || die "no such AppImage: $input"
    command -v xvfb-run >/dev/null || die "xvfb-run is required for the smoke test"

    set +e
    xvfb-run -a --server-args="-screen 0 1280x800x24" \
        env GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 \
        WEBKIT_DISABLE_DMABUF_RENDERER=1 APPIMAGE_EXTRACT_AND_RUN=1 \
        "$input" > "$log" 2>&1 &
    runner=$!
    set -e

    for attempt in $(seq 1 40); do
        for pid in /proc/[0-9]*; do
            helper="$(readlink "$pid/exe" 2>/dev/null || true)"
            case "$helper" in
                */usr/lib/*/webkit2gtk-4.1/WebKitWebProcess) break 2 ;;
            esac
            helper=""
        done
        if ! kill -0 "$runner" 2>/dev/null; then
            break
        fi
        sleep 1
    done

    if [ -z "$helper" ]; then
        echo "smoke: no WebKitWebProcess after ${attempt}s" >&2
        status=1
    else
        root="${helper%%/usr/lib/*}"
        case "$helper" in
            /usr/lib/*)
                echo "smoke: WebKitWebProcess runs the host library: $helper" >&2
                status=1
                ;;
            *)
                echo "smoke: WebKitWebProcess runs from $root"
                ;;
        esac
        if ! kill -0 "$runner" 2>/dev/null; then
            echo "smoke: the application exited" >&2
            status=1
        fi
    fi

    if grep -qE 'Received invalid message|Could not create default EGL display|symbol lookup error' "$log"; then
        echo "smoke: fatal WebKit error" >&2
        status=1
    fi

    pkill -P "$runner" 2>/dev/null || true
    kill "$runner" 2>/dev/null || true
    for pid in /proc/[0-9]*; do
        exe="$(readlink "$pid/exe" 2>/dev/null || true)"
        case "$exe" in
            *appimage_extracted*) kill "$pid" 2>/dev/null || true ;;
        esac
    done
    wait "$runner" 2>/dev/null || true

    if [ "$status" -ne 0 ]; then
        echo "--- application log" >&2
        sed 's/^/    /' "$log" >&2
        exit 1
    fi
    printf 'smoke: %s renders with the bundled WebKit\n' "$input"
}

main() {
    [ "$#" -ge 1 ] || usage
    local command="$1"
    shift
    WORKDIR=""
    trap '[ -z "$WORKDIR" ] || rm -rf "$WORKDIR"' EXIT
    WORKDIR="$(mktemp -d)"
    case "$command" in
        appimage) [ "$#" -eq 2 ] || usage; appimage "$1" "$2" ;;
        deb) [ "$#" -eq 2 ] || usage; deb "$1" "$2" ;;
        smoke) [ "$#" -eq 1 ] || usage; smoke "$1" ;;
        *) usage ;;
    esac
}

main "$@"
