#!/bin/zsh
source ${0:A:h}/env.sh; cd $REPO
typeset -A name
for f in $QA/create-*.json; do t=${${f:t:r}#create-}; name[$(jq -r .task.id $f)]=$t; done
for s in open unstarted idle backlog done; do
  ids=$($LF desktop view show "kind:task state:$s" --json < /dev/null 2>/dev/null | jq -r '.resolution.members[].work.id')
  printf '%-10s' $s; for i in ${(f)ids}; do printf ' %s' ${name[$i]}; done; echo
done
echo ---
for i in ${(k)name}; do $LF task status $i --json < /dev/null 2>/dev/null | jq -c --arg n ${name[$i]} '{n:$n} + (to_entries | map(select(.key|test("state|section|owner"))) | from_entries)'; done
