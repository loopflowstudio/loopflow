#!/bin/zsh
# 744 more Tasks: every 3rd claimed, every 5th of those completed.
source ${0:A:h}/env.sh; cd $REPO
one() {
  n=$1
  id=$($LF task create --title "Bulk $n" --request-id bulk-$n --json < /dev/null 2>/dev/null | jq -r .task.id)
  [[ -z $id || $id == null ]] && { echo "FAIL create $n"; return; }
  if (( n % 3 == 0 )); then $LF task claim $id < /dev/null > /dev/null 2>&1 || echo "FAIL claim $n"; fi
  if (( n % 15 == 0 )); then $LF task complete $id < /dev/null > /dev/null 2>&1 || echo "FAIL complete $n"; fi
  echo "ok $n"
}
for n in {1..744}; do one $n & (( n % 12 == 0 )) && wait; done; wait
