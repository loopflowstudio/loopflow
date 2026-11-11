#!/bin/zsh
# bg.sh <cwd> <out> lf-args... : launch in a PTY; turns are released by touching $Q/go-N.
cwd=$1 out=$2; shift 2
Q=/tmp/lf-qa-821/lf/qa; rm -rf $Q; mkdir -p $Q
cd $cwd && /Users/jack/src/loopflow.loo-821-managed-sessions-hooks/scratch/qa/env.sh QA_SECS=400 /usr/bin/python3 -I /Users/jack/src/loopflow.loo-821-managed-sessions-hooks/scratch/qa/pty.py $out lf "$@" < /dev/null
