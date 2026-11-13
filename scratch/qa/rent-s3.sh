# Race: T1 (repo) is held by qa-2. `lf machine end qa-2` reads repo's
# .lf/config.yaml (served through a FIFO) for its planning facts; at the first
# open, before answering, T3 from repo2 is handed to qa-2.
source ${0:A:h}/rent-env.sh
T1=$(cat $R/t1)
git init -q -b main $R/repo2 && (cd $R/repo2 && git config user.email qa@example.com && git config user.name 'QA Person' \
  && mkdir -p .lf && printf 'default_agent: codex\nplanning: {provider: git, personal: true}\n' > .lf/config.yaml \
  && git add -A && git commit -qm init && git init -q --bare $R/origin2.git && git remote add origin $R/origin2.git && git push -q origin main)
T3=$(cd $R/repo2 && $LF task create --title 'Check the totals' --request-id qa-r3 --json < /dev/null 2>/dev/null | python3 -I -c 'import json,sys; d=json.loads(sys.stdin.read()); print(d.get("task",d).get("id"))')
print "T3=$T3"; print $T3 > $R/t3
step "Rent qa-2 and move T1 there"; run machine ondemand rent qa; run task move $T1 --machine qa-2
good=$(cat .lf/config.yaml)
mv .lf/config.yaml $R/repo-config.yaml; mkfifo .lf/config.yaml
: > $R/calls.log; rm -f $R/opens $R/stop
$LF machine end qa-2 < /dev/null > $R/end.out 2>&1 &
pid=$!
(trap "" PIPE; n=0; while [ ! -e $R/stop ]; do
  exec 3>$R/repo/.lf/config.yaml
  n=$((n+1))
  if sqlite3 -cmd '.timeout 0' $LF_HOME/loopflow.db 'BEGIN IMMEDIATE; ROLLBACK;' >/dev/null 2>&1; then lock=free; else lock=HELD; fi
  if [ $n = 1 ]; then
    (cd $R/repo2 && $LF task move $T3 --machine qa-2 < /dev/null > $R/move.out 2>&1; echo "move exit $?" >> $R/move.out)
  fi
  print "open $n lock=$lock $(tail -1 $R/calls.log | cut -c1-40)" >> $R/opens
  print -rn -- "$good" >&3; exec 3>&-
done) &
feeder=$!
wait $pid; st=$?
touch $R/stop
while kill -0 $feeder 2>/dev/null; do cat $R/repo/.lf/config.yaml > /dev/null & r=$!; sleep 0.3; kill $r 2>/dev/null; done
rm -f .lf/config.yaml; mv $R/repo-config.yaml .lf/config.yaml
step "Hand-off during the stall"; cat $R/move.out
step "Config opens while ending (lock state at each open)"; cat $R/opens
step "lf machine end qa-2 -> exit $st"; cat $R/end.out
for P in repo:$T1 repo2:$T3; do
  T=${P#*:}; step "Ownership of $T"
  (cd $R/${P%%:*}; $LF task show $T --json < /dev/null 2>&1 | python3 -I -c 'import json,sys; o=json.load(sys.stdin)["ownership"]; print("owner:", o["owner"]["machine"]["label"]); [print(" ", e["kind"], e["machine_label"]) for e in o["log"]]')
done
step "git status in repo"; git status --short
