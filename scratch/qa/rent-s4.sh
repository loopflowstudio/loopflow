# Baseline for rent-s3.sh: same FIFO, no hand-off during the stall.
source ${0:A:h}/rent-env.sh
T1=$(cat $R/t1)
step "Rent qa-3 and move T1 there"; run machine ondemand rent qa; run task move $T1 --machine qa-3
good=$(cat .lf/config.yaml)
mv .lf/config.yaml $R/repo-config.yaml; mkfifo .lf/config.yaml
: > $R/calls.log; rm -f $R/opens $R/stop $R/move.out; : > $R/move.out
$LF machine end qa-3 < /dev/null > $R/end.out 2>&1 &
pid=$!
(trap "" PIPE; n=0; while [ ! -e $R/stop ]; do
  exec 3>$R/repo/.lf/config.yaml
  n=$((n+1))
  if sqlite3 -cmd '.timeout 0' $LF_HOME/loopflow.db 'BEGIN IMMEDIATE; ROLLBACK;' >/dev/null 2>&1; then lock=free; else lock=HELD; fi
  if false; then
    (cd $R/repo2 && $LF task move $T3 --machine qa-3 < /dev/null > $R/move.out 2>&1; echo "move exit $?" >> $R/move.out)
  fi
  print "open $n lock=$lock $(tail -1 $R/calls.log | cut -c1-40)" >> $R/opens
  print -rn -- "$good" >&3; exec 3>&-
done) &
feeder=$!
wait $pid; st=$?
touch $R/stop
while kill -0 $feeder 2>/dev/null; do cat $R/repo/.lf/config.yaml > /dev/null & r=$!; sleep 0.3; kill $r 2>/dev/null; done
rm -f .lf/config.yaml; mv $R/repo-config.yaml .lf/config.yaml
step "Hand-off during the stall (none)"; cat $R/move.out
step "Config opens while ending (lock state at each open)"; cat $R/opens
step "lf machine end qa-3 -> exit $st"; cat $R/end.out
for T in $T1; do
  step "Ownership of $T"
  (cd $R/repo; $LF task show $T --json < /dev/null 2>&1 | python3 -I -c 'import json,sys; o=json.load(sys.stdin)["ownership"]; print("owner:", o["owner"]["machine"]["label"]); [print(" ", e["kind"], e["machine_label"]) for e in o["log"]]')
done
step "git status in repo"; git status --short
