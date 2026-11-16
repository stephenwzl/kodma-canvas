#!/usr/bin/env bash
# Fresh isolated settings and synthetic A/B files; never load personal recent documents.
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$TASK_ROOT"
TASK_CONFIG=$(mktemp -d "${TMPDIR:-/tmp}/kodma-s002-i4.XXXXXX")
KODMA_CANVAS_CONFIG_DIR="$TASK_CONFIG" ./script/build_and_run.sh --workspace --verify --control 17982 "$@"
KODMA_I4_CONFIG_DIR="$TASK_CONFIG" ruby script/prepare_s002_i4.rb 17982
