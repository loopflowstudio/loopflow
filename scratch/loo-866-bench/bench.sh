#!/bin/zsh
# Compare the history query before/after on a copy of the Machine store:
#   sqlite3 -readonly ~/.lf/loopflow.db "VACUUM INTO '/tmp/loo866/store.db'"   # .backup restarts forever on a busy store
#   scratch/loo-866-bench/bench.sh /tmp/loo866/store.db
# Builds <copy>.after with the history_window_indexes migration, diffs full results, times best of 5.
set -e
here=${0:A:h}; store=$1; after=$store.after
[[ -f $after ]] || { cp $store $after; { cat $here/../../rust/loopflow/src/store/migrations/history_window_indexes.sql; echo 'ANALYZE;'; } | sqlite3 $after; }
run() { # db sql since finished limit task [count]
  local body=$(cat $2)
  [[ -n $7 ]] && body=".timer on
SELECT count(*) FROM ($body)"
  sqlite3 $1 <<SQL
.parameter init
.parameter set ?5 $3
.parameter set ?6 $4
.parameter set ?8 $5
.parameter set ?3 $6
$body;
SQL
}
best() { for n in 1 2 3 4 5; do run $@ count | sed -n 's/.*real \([0-9.]*\).*/\1/p'; done | sort -n | head -1; }
now=$(date +%s)
# Task drills (Desktop task history, task.context) read since=0 with only the Task set:
# the Task with the most attributed rows, and one at the 10th rank.
drills=($(sqlite3 -readonly $store "SELECT task_id FROM session_events WHERE task_id IS NOT NULL
  GROUP BY task_id ORDER BY count(*) DESC LIMIT 10" | sed -n '1p;10p'))
specs=("7 1 NULL" "7 0 50" "1 1 NULL" "1 0 50" "30 1 NULL" "0 0 NULL")
for drill in $drills; do specs+=("0 1 NULL '$drill'"); done
for spec in $specs; do
  set -- ${=spec}; since=$(( $1 == 0 ? 0 : now-$1*86400 )); task=${4:-NULL}
  run $store $here/before.sql $since $2 NULL $task > /tmp/loo866-before.txt
  run $after $here/after.sql $since $2 NULL $task > /tmp/loo866-after.txt
  cmp -s /tmp/loo866-before.txt /tmp/loo866-after.txt && same="same $(wc -l < /tmp/loo866-after.txt | tr -d ' ') rows" || same=DIFF
  echo "${1}d finished=$2 limit=$3 task=$task: $same; before $(best $store $here/before.sql $since $2 $3 $task)s after $(best $after $here/after.sql $since $2 $3 $task)s"
done
