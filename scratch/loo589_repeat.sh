#!/bin/zsh
# Usage: loo589_repeat.sh <iterations> <filter> ; runs under 16 busy loops, 30 s per-run cap.
cd "${0:A:h}/.."
for v in $(env | grep ^LF_ | cut -d= -f1); do unset $v; done
n=$1; filter=$2
pids=(); for i in $(seq 16); do (while :; do :; done) & pids+=$!; done
pass=0 fail=0 hang=0 leak=0
for i in $(seq $n); do
  out=$(perl -e 'alarm 30; exec @ARGV' swift test --package-path swift --skip-build --filter "$filter" < /dev/null 2>&1); rc=$?
  [[ $out == *"leaked its continuation"* ]] && leak=$((leak+1))
  if [[ $rc == 142 ]]; then hang=$((hang+1)); elif [[ $rc == 0 ]]; then pass=$((pass+1)); else fail=$((fail+1)); echo "$out" | tail -20; fi
done
kill $pids 2>/dev/null
echo "iterations=$n pass=$pass fail=$fail hang=$hang runs_with_leaked_continuation=$leak"
