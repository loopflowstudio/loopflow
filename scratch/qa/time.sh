#!/bin/zsh
# Median wall time (ms) of 7 warm runs of each selection read.
source ${0:A:h}/env.sh; cd $REPO; LF=${1:-$LF}; echo "binary: $($LF --version < /dev/null)"
for args in "desktop view list --json" "desktop view show kind:task --json" "desktop view show query:qa --json"; do
  $LF ${=args} < /dev/null > /dev/null 2>&1
  ts=()
  for i in {1..7}; do s=$(perl -MTime::HiRes=time -e 'printf "%d", time*1000'); $LF ${=args} < /dev/null > /dev/null 2>&1; e=$(perl -MTime::HiRes=time -e 'printf "%d", time*1000'); ts+=$((e-s)); done
  echo "$args: median $(printf '%s\n' $ts | sort -n | sed -n 4p) ms (${ts})"
done
for args in "task list --json"; do s=$(perl -MTime::HiRes=time -e 'printf "%d", time*1000'); $LF ${=args} < /dev/null > /dev/null 2>&1; e=$(perl -MTime::HiRes=time -e 'printf "%d", time*1000'); echo "$args: $((e-s)) ms"; done
