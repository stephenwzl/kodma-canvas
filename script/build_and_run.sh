#!/usr/bin/env bash
# S001 local validation host; bundles/logs go in ignored dist/, settings in a temporary directory.
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TASK_MODE=run
TASK_PRESENTATION=--native-canvas
TASK_BUILD=1
TASK_ARGS=()
while (($#)); do
    case "$1" in
        --verify|--debug|--logs) TASK_MODE="${1#--}" ;;
        --reference) TASK_PRESENTATION= ;;
        --canvas-only) TASK_PRESENTATION=--canvas-only ;;
        --no-build) TASK_BUILD=0 ;;
        *) TASK_ARGS+=("$1") ;;
    esac
    shift
done
if [[ "$(uname -s)" != Darwin ]]; then
    echo 'The native validation bundle requires macOS.' >&2
    exit 2
fi
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
cd "$TASK_ROOT"
# Only this validation bundle is stopped; other FigCraft installations are untouched.
pkill -x KodmaCanvas 2>/dev/null || true
for TASK_WAIT in {1..40}; do
    if ! pgrep -x KodmaCanvas >/dev/null; then break; fi
    sleep 0.1
done
if pgrep -x KodmaCanvas >/dev/null; then
    echo 'The previous validation process did not stop.' >&2
    exit 1
fi
if ((TASK_BUILD)); then
    cargo build -p figcraft -p figcraft-cli
fi
TASK_CONTENTS="$TASK_ROOT/dist/KodmaCanvas.app/Contents"
TASK_CONFIG="${KODMA_CANVAS_CONFIG_DIR:-${TMPDIR:-/tmp}/kodma-canvas-validation}"
mkdir -p "$TASK_CONTENTS/MacOS" "$TASK_CONFIG"
# An upgrade rehearsal can supply its isolated binary with --no-build, without replacing main.
cp "${KODMA_CANVAS_BIN:-$TASK_ROOT/target/debug/figcraft}" "$TASK_CONTENTS/MacOS/KodmaCanvas"
cat > "$TASK_CONTENTS/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>KodmaCanvas</string>
<key>CFBundleIdentifier</key><string>work.oldiron.kodma.canvas-validation</string>
<key>CFBundleName</key><string>Kodma Canvas</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>NSPrincipalClass</key><string>NSApplication</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
# Bind the copied executable to this local bundle; the linker's ad-hoc signature has no bundle
# resource envelope. This uses no identity and is not a distribution/notarization signature.
/usr/bin/codesign --force --sign - "$TASK_ROOT/dist/KodmaCanvas.app"
/usr/bin/codesign --verify "$TASK_ROOT/dist/KodmaCanvas.app"
if [[ -n "$TASK_PRESENTATION" ]]; then
    TASK_ARGS=("$TASK_PRESENTATION" "${TASK_ARGS[@]}")
fi
if [[ "$TASK_MODE" == debug ]]; then
    FIGCRAFT_CONFIG_DIR="$TASK_CONFIG" lldb -- "$TASK_CONTENTS/MacOS/KodmaCanvas" "${TASK_ARGS[@]}"
else
    TASK_OPENED=0
    # LaunchServices can briefly retain the old job after its process exits.
    for TASK_ATTEMPT in {1..3}; do
        if /usr/bin/open -n "$TASK_ROOT/dist/KodmaCanvas.app" \
            --env "FIGCRAFT_CONFIG_DIR=$TASK_CONFIG" --env RUST_LOG=info \
            --stdout "$TASK_ROOT/dist/canvas-stdout.log" --stderr "$TASK_ROOT/dist/canvas-stderr.log" \
            --args "${TASK_ARGS[@]}"; then
            TASK_OPENED=1
            break
        fi
        sleep 1
    done
    if (( ! TASK_OPENED )); then exit 1; fi
    if [[ "$TASK_MODE" == verify ]]; then
        sleep 2
        pgrep -x KodmaCanvas >/dev/null
    elif [[ "$TASK_MODE" == logs ]]; then
        tail -f "$TASK_ROOT/dist/canvas-stderr.log"
    fi
fi
