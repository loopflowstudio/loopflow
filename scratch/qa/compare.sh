#!/bin/zsh
# Same store, base and branch binaries: selection output must match.
source ${0:A:h}/env.sh; cd $REPO
for q in "desktop view list --json" "desktop view show kind:task --json" "desktop view show query:qa --json" "desktop view show state:open --json" "desktop view show state:backlog --json" "desktop view show state:done --json" "desktop view show state:idle --json"; do
  $QA/bin/lf-base ${=q} < /dev/null 2>/dev/null | jq -S . > $QA/base.out
  $LF ${=q} < /dev/null 2>/dev/null | jq -S . > $QA/branch.out
  n=$(jq '[.. | objects | select(has("work")) ] | length' $QA/branch.out)
  if cmp -s $QA/base.out $QA/branch.out; then echo "same  ($n members) $q"; else echo "DIFF $q"; diff $QA/base.out $QA/branch.out | head -10; fi
done
