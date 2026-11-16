#!/usr/bin/env bash
# One entry point for the S001 personal acceptance draft. Does not alter reference-app settings.
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$TASK_ROOT"
./script/build_and_run.sh --verify --control 17981 "$@"
ruby script/prepare_s001.rb 17981
