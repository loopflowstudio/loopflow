#!/bin/zsh
# Compare the history query before/after on a copy of the Machine store:
#   sqlite3 -readonly ~/.lf/loopflow.db ".backup /tmp/loo866/store.db"
#   scratch/loo-866-bench/bench.sh /tmp/loo866/store.db
# Builds <copy>.after with the history_window_indexes migration, diffs full results, times best of 5.
set -e
here=${0:A:h}; store=$1; after=$store.after
[[ -f $after ]] || { cp $store $after; { cat $here/../../rust/loopflow/src/store/migrations/history_window_indexes.sql; echo 'ANALYZE;'; } | sqlite3 $after; }
run() { # db sql since finished limit [count]
  local body=$(cat $2)
  [[ -n $6 ]] && body=".timer on
SELECT count(*) FROM ($body)"
  sqlite3 $1 <<SQL
.parameter init
.parameter set ?5 $3
.parameter set ?6 $4
.parameter set ?8 $5
$body;
SQL
}
best() { for n in 1 2 3 4 5; do run $@ count | sed -n 's/.*real \([0-9.]*\).*/\1/p'; done | sort -n | head -1; }
now=$(date +%s)
for spec in "7 1 NULL" "7 0 50" "1 1 NULL" "1 0 50" "30 1 NULL" "0 0 NULL"; do
  set -- ${=spec}; since=$(( $1 == 0 ? 0 : now-$1*86400 ))
  run $store $here/before.sql $since $2 NULL > /tmp/loo866-before.txt
  run $after $here/after.sql $since $2 NULL > /tmp/loo866-after.txt
  cmp -s /tmp/loo866-before.txt /tmp/loo866-after.txt && same="same $(wc -l < /tmp/loo866-after.txt | tr -d ' ') rows" || same=DIFF
  echo "${1}d finished=$2 limit=$3: $same; before $(best $store $here/before.sql $since $2 $3)s after $(best $after $here/after.sql $since $2 $3)s"
done
