#!/bin/zsh
# sample.sh <out> <secs>: every 0.2 s record each branch-lf process (pid, ppid, args), deduplicated by pid+args.
out=$1; end=$(( $(date +%s) + $2 )); : > $out
while [ $(date +%s) -lt $end ]; do
  ps -axo pid=,ppid=,command= | grep -E "loo-835-keep-one-server/target/debug/lf|lf-qa-835/bin/lf" | grep -v grep | while read pid ppid cmd; do
    key="$pid $cmd"; grep -qxF "$key" $out.keys 2>/dev/null || { echo "$key" >> $out.keys; echo "$(date +%T) pid=$pid ppid=$ppid $cmd" >> $out; }
  done
  sleep 0.2
done
rm -f $out.keys
