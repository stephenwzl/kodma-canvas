#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 3 ]]; then
    echo 'Usage: sign-canvas-development.sh <source.app> <new-output.app> <Apple Development identity>' >&2
    exit 2
fi

source_app=$1
output_app=$2
identity=$3
signing_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
profile="$signing_dir/Canvas_macOS_Sandbox_Development.provisionprofile"
entitlements="$signing_dir/canvas-development-entitlements.plist"

if [[ ! -d "$source_app" || "$output_app" != *.app || -e "$output_app" || -L "$output_app" || ! -r "$profile" ]]; then
    echo "Expected an existing Canvas app, a new .app output path, and $profile." >&2
    exit 2
fi

bundle_id=$(/usr/bin/plutil -extract CFBundleIdentifier raw "$source_app/Contents/Info.plist")
if [[ "$bundle_id" != app.kodma.canvas ]]; then
    echo "Expected Canvas bundle ID app.kodma.canvas, found $bundle_id." >&2
    exit 2
fi

staging_dir=$(mktemp -d "$(dirname "$output_app")/.canvas-development.XXXXXX")
trap 'rm -rf "$staging_dir"' EXIT
staged_app="$staging_dir/$(basename "$output_app")"

/usr/bin/ditto "$source_app" "$staged_app"
cp "$profile" "$staged_app/Contents/embedded.provisionprofile"
/usr/bin/codesign --force --sign "$identity" --entitlements "$entitlements" "$staged_app"
/usr/bin/codesign --verify --deep --strict \
    -R='anchor apple generic and certificate leaf[subject.OU] = "WBTHC22CU7"' "$staged_app"
/bin/mv -n "$staged_app" "$(dirname "$output_app")/"
if [[ -e "$staged_app" ]]; then
    echo "Output already exists: $output_app" >&2
    exit 2
fi
