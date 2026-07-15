#!/usr/bin/env bash
# Drive the required --ui-host gate 5x, capturing per-run proof that the hosted
# LoopflowUITests actually EXECUTED (not just compiled, not hung on a missing
# permission). Detached so it survives session teardown; poll SUMMARY.md.
set -u
cd /Users/jack/src/loopflow.verify-the-required-ui-host || exit 2
RUNDIR=scratch/ui-host-runs
SUMMARY="$RUNDIR/SUMMARY.md"
: > "$SUMMARY"
echo "# --ui-host gate: 5x real-host run" >> "$SUMMARY"
echo "" >> "$SUMMARY"
echo "host: $(hostname) / macOS $(sw_vers -productVersion) / $(xcodebuild -version | head -1)" >> "$SUMMARY"
echo "start: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$SUMMARY"
echo "" >> "$SUMMARY"
echo "| run | gate result | LoopflowUITests executed | test outcome | elapsed |" >> "$SUMMARY"
echo "|-----|-------------|--------------------------|--------------|---------|" >> "$SUMMARY"

pass_count=0
for i in 1 2 3 4 5; do
  LOG="$RUNDIR/run-$i.log"
  t0=$(date +%s)
  uv run python scripts/test.py --ui-host > "$LOG" 2>&1
  rc=$?
  t1=$(date +%s)
  elapsed=$((t1 - t0))

  # Gate summary line
  gate=$(grep -E "^Result: (PASS|FAIL)" "$LOG" | tail -1)
  [ -z "$gate" ] && gate="(no Result line; rc=$rc)"

  # Did the hosted UI suite actually execute? xcodebuild prints an "Executed N
  # test(s)" line only when the runner connected and the tests ran.
  executed=$(grep -E "Executed [0-9]+ test" "$LOG" | tail -1)
  if [ -n "$executed" ]; then
    execnote="YES ($executed)"
  else
    execnote="NO"
  fi

  # Test outcome / capability classification
  if grep -q "MISSING CAPABILITY" "$LOG"; then
    outcome="MISSING CAPABILITY (permission gap)"
  elif grep -q "\*\* TEST SUCCEEDED \*\*" "$LOG"; then
    outcome="TEST SUCCEEDED"
  elif grep -q "\*\* TEST FAILED \*\*" "$LOG"; then
    outcome="TEST FAILED"
  elif grep -q "TIMEOUT: phase 'ui-host'" "$LOG"; then
    outcome="TIMEOUT (budget kill)"
  else
    outcome="unknown (rc=$rc)"
  fi

  if echo "$gate" | grep -q "Result: PASS" && [ -n "$executed" ]; then
    pass_count=$((pass_count + 1))
    mark="PASS"
  else
    mark="FAIL"
  fi

  echo "| $i | ${gate#Result: } | $execnote | $outcome | ${elapsed}s |" >> "$SUMMARY"
done

echo "" >> "$SUMMARY"
echo "clean runs (gate PASS + LoopflowUITests executed): $pass_count/5" >> "$SUMMARY"
echo "end: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$SUMMARY"
echo "DRIVER_DONE pass=$pass_count" >> "$SUMMARY"
