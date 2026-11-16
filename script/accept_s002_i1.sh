#!/usr/bin/env bash
# I1 framework verification only; the full S002 A/B acceptance is delivered in later iterations.
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$TASK_ROOT"
./script/build_and_run.sh --workspace --verify --control 17982 "$@"
ruby script/verify_s002_i1.rb 17982
