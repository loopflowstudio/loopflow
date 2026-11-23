#!/bin/zsh
# usage: measure.sh <worktree> <label> <cargo args...>
wt=$1; label=$2; shift 2
out=/Users/jack/src/loopflow.loo-922-reduce-fresh-worktree/scratch/probe/$label
cd $wt || exit 1
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') /usr/bin/time -l cargo "$@" -v < /dev/null > $out.log 2> $out.err
echo "exit=$?" > $out.sum
grep -E 'real|user|sys|maximum resident' $out.err | tail -4 >> $out.sum
echo "compiling=$(grep -c '^ *Compiling' $out.err) fresh=$(grep -c '^ *Fresh' $out.err) running=$(grep -c '^ *Running .*build-script' $out.err)" >> $out.sum
cat $out.sum
