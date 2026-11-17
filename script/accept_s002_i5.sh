#!/usr/bin/env bash
# UI/manual acceptance entrypoint: isolated prefs and unique synthetic files, no auto-pass claim.
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$TASK_ROOT"
if [[ "${1:-}" == --resume ]]; then
    shift
    TASK_CONFIG=$(ruby -rjson -e 'm=JSON.parse(File.read(ARGV.fetch(0))); c=m.fetch("config"); abort "The acceptance configuration is missing; prepare a fresh case." unless c.is_a?(String) && File.directory?(c); print c' "$TASK_ROOT/dist/s002-i5/latest.json")
    # Preserve the existing synthetic files and their recent-file settings across a restart.
    KODMA_CANVAS_CONFIG_DIR="$TASK_CONFIG" ./script/build_and_run.sh --workspace --no-build --verify --control 17982 "$@"
    exit
fi
./script/accept_s002_i4.sh "$@"
ruby script/prepare_s002_i5.rb 17982
