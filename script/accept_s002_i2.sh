#!/usr/bin/env bash
# Fresh isolated settings and synthetic files for native I2 acceptance. Never reuse private recents.
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$TASK_ROOT"
TASK_CONFIG=$(mktemp -d "${TMPDIR:-/tmp}/kodma-s002-i2.XXXXXX")
KODMA_CANVAS_CONFIG_DIR="$TASK_CONFIG" ./script/build_and_run.sh --workspace --verify --control 17982 "$@"
KODMA_I2_CONFIG_DIR="$TASK_CONFIG" ruby script/prepare_s002_i2.rb 17982
