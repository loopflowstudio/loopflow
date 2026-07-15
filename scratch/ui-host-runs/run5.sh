#!/usr/bin/env bash
# Drive the required --ui-host gate 5 times on this permissioned macOS host.
# Durable: each run's outcome (gate exit + whether the hosted UI test actually
# executed) is appended to summary.tsv as it completes, so progress survives a
# session teardown mid-sequence.
set -u
cd "$(dirname "$0")/../.." || exit 2
RUNS_DIR="scratch/ui-host-runs"
SUMMARY="$RUNS_DIR/summary.tsv"
if [[ ! -f "$SUMMARY" ]]; then
  printf 'run\tgate_exit\tui_executed\tui_test_result\tgate_result\tstarted\telapsed_s\n' > "$SUMMARY"
fi

# Which run index to start at = count of data rows already recorded + 1.
done_rows=$(($(wc -l < "$SUMMARY") - 1))
for i in $(seq $((done_rows + 1)) 5); do
  log="$RUNS_DIR/run-$i.log"
  started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  t0=$SECONDS
  uv run python scripts/test.py --ui-host > "$log" 2>&1
  gate_exit=$?
  elapsed=$((SECONDS - t0))
  # Did the hosted LoopflowUITests actually execute (not skip/hang)?
  ui_line=$(grep -E "Test Suite 'LoopflowUITests(\.xctest)?' .*(passed|failed)" "$log" | tail -1)
  exec_line=$(grep -E "Executed [0-9]+ test" "$log" | tail -1)
  ui_executed=no
  if echo "$exec_line" | grep -qE "Executed [1-9][0-9]* test"; then ui_executed=yes; fi
  gate_result=$(grep -E "^Result: (PASS|FAIL)" "$log" | tail -1)
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$i" "$gate_exit" "$ui_executed" "${ui_line:-NONE}" "${gate_result:-NONE}" "$started" "$elapsed" >> "$SUMMARY"
  echo "RUN $i done: gate_exit=$gate_exit ui_executed=$ui_executed elapsed=${elapsed}s"
done
echo "ALL RUNS COMPLETE"
