#!/bin/zsh
# Alternate base and branch for N rounds per read; print min and median of each.
source ${0:A:h}/env.sh; cd $REPO
now() { perl -MTime::HiRes=time -e 'printf "%d", time*1000'; }
for q in "desktop view list --json" "desktop view show query:qa --json"; do
  b=(); r=()
  for i in {1..15}; do
    s=$(now); $QA/bin/lf-base ${=q} < /dev/null > /dev/null 2>&1; b+=$(( $(now)-s ))
    s=$(now); $LF ${=q} < /dev/null > /dev/null 2>&1; r+=$(( $(now)-s ))
  done
  sb=($(printf '%s\n' $b | sort -n)); sr=($(printf '%s\n' $r | sort -n))
  echo "$q  base min ${sb[1]} med ${sb[8]}  branch min ${sr[1]} med ${sr[8]}  (ms)"
done
