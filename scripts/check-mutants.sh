#!/usr/bin/env bash
#
# Run mutation testing and fail if anything survives that is not on the
# baseline in `.cargo/mutants-baseline.txt`.
#
# Author: PratikP1
#
# cargo-mutants reports a surviving mutant as a line of the form
#
#     src/plan.rs:179:9: replace RemovalPlan::is_non_empty -> bool with true
#
# A survivor means some behaviour is not pinned by any test: the code could be
# changed that way and the suite would still pass. The baseline holds the few
# survivors that are *equivalent* mutants (where the mutated program is
# genuinely identical), and every one carries a comment saying why.
set -euo pipefail

readonly BASELINE=".cargo/mutants-baseline.txt"
readonly OUTPUT_DIR="mutants.out"

# cargo-mutants exits non-zero when mutants survive or time out; that is the
# normal path here, so judge by the report rather than the exit code.
cargo mutants --no-shuffle "$@" || true

if [ ! -f "$OUTPUT_DIR/outcomes.json" ]; then
    echo "error: cargo-mutants produced no report; the run itself failed" >&2
    exit 1
fi

# Line and column numbers shift whenever anything above them is edited, so
# compare on the description alone.
survivors="$(mktemp)"
expected="$(mktemp)"
trap 'rm -f "$survivors" "$expected"' EXIT

sed -E 's/:[0-9]+:[0-9]+:/:/' "$OUTPUT_DIR/missed.txt" 2>/dev/null | sort >"$survivors"
grep -v '^#' "$BASELINE" | grep -v '^[[:space:]]*$' | sort >"$expected"

if diff -u --label "$BASELINE" "$expected" --label "surviving mutants" "$survivors"; then
    echo "Mutation testing clean: survivors match the baseline."
    exit 0
fi

cat >&2 <<'MESSAGE'

error: the set of surviving mutants changed.

  Lines marked + survived but are not on the baseline. Each one is behaviour no
  test pins down. Write the test that fails without it. Only add an entry to
  .cargo/mutants-baseline.txt if the mutant is provably equivalent, and say why.

  Lines marked - are on the baseline but no longer survive. That is good news:
  drop them from the baseline.
MESSAGE
exit 1
