#!/usr/bin/env bash
# Open the native S003 review with two synthetic managed drafts.
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$TASK_ROOT"
TASK_MANIFEST="$TASK_ROOT/dist/s003-i5/regression.json"
if [[ "${1:-}" == --resume ]]; then
    shift
    TASK_CONFIG=$(ruby -rjson -e '
      manifest = JSON.parse(File.read(ARGV.fetch(0)))
      config = File.dirname(File.dirname(manifest.fetch("A").fetch("path")))
      abort "Expected an isolated S003 I5 configuration" unless File.basename(File.realpath(config)).start_with?("kodma-s003-i5-")
      print config
    ' "$TASK_MANIFEST")
    KODMA_CANVAS_CONFIG_DIR="$TASK_CONFIG" ./script/build_and_run.sh --workspace --no-build --verify --control 17992 "$@"
    exit
fi
TASK_CONFIG="$(mktemp -d "${TMPDIR:-/tmp}/kodma-s003-i5-XXXXXX")"
export KODMA_CANVAS_CONFIG_DIR="$TASK_CONFIG"
./script/build_and_run.sh --workspace --verify --control 17992 "$@"
ruby script/verify_s003_i5.rb seed 17992 "$TASK_MANIFEST"
