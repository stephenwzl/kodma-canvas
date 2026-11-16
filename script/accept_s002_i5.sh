#!/usr/bin/env bash
# UI/manual acceptance entrypoint: isolated prefs and unique synthetic files, no auto-pass claim.
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$TASK_ROOT"
./script/accept_s002_i4.sh "$@"
ruby script/prepare_s002_i5.rb 17982
