#!/bin/zsh
# turns.sh <cwd> <n-turns> <out> -- lf args...  Runs a headless launch, releasing N turns then quitting.
cwd=$1 n=$2 out=$3; shift 4
Q=/tmp/lf-qa-821/lf/qa; mkdir -p $Q; find $Q -maxdepth 1 \( -name "go-*" -o -name "done-*" -o -name "hold-*" -o -name quit -o -name provider.jsonl \) -delete
E=/Users/jack/src/loopflow.loo-821-managed-sessions-hooks/scratch/qa/env.sh
( for t in $(seq 1 $n); do
    [ -e $Q/hold-$t ] && while [ -e $Q/hold-$t ]; do sleep 0.1; done
    touch $Q/go-$t
    for i in $(seq 1 150); do [ -e $Q/done-$t ] && break; sleep 0.1; done
  done; touch $Q/quit ) &
cd $cwd && $E /usr/bin/python3 -I /Users/jack/src/loopflow.loo-821-managed-sessions-hooks/scratch/qa/pty.py $out lf "$@" < /dev/null
echo "exit $?" >> $out
wait
