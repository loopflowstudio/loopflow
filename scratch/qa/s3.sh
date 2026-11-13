ROOT=/tmp/lf-qa-867
for v in $(env | grep -E '^(LF_|REX_)' | cut -d= -f1); do unset $v; done
export HOME=$ROOT/home LF_HOME=$ROOT/lfhome LF=/tmp/lf-qa-867-bin LF_BIN=/tmp/lf-qa-867-bin
export CODEX_HOME=$HOME/.codex CLAUDE_CONFIG_DIR=$HOME/.claude
cd $ROOT/repo
step() { print "\n### $*"; }
run() { print "\$ lf ${(j: :)@}"; $LF "$@" < /dev/null 2>&1; print "(exit $?)"; }
T1=$(cat $ROOT/task1); T2=$(cat $ROOT/task2)
step "Route Tasks back to this Machine"; run unset machine.tasks
run task claim $T1
step "T2 (created while mini owned the repo) claims here now"; run task claim $T2
step "Broken personal config: claim must refuse before writing"
cp $LF_HOME/config.yaml $ROOT/config.good
print 'machines: [not, a, map' > $LF_HOME/config.yaml
run task unassign $T1
sqlite3 "file:$LF_HOME/loopflow.db?mode=ro" "select kind,count(*) from task_ownership where task_id='$T1' group by kind"
step "Store not left locked: restore config, unassign succeeds"
cp $ROOT/config.good $LF_HOME/config.yaml
run task unassign $T1
step "Two claims at once"
($LF task claim $T1 < /dev/null > $ROOT/c1 2>&1; echo "a exit $?" >> $ROOT/c1) &
($LF task claim $T1 < /dev/null > $ROOT/c2 2>&1; echo "b exit $?" >> $ROOT/c2) &
wait; cat $ROOT/c1 $ROOT/c2
sqlite3 "file:$LF_HOME/loopflow.db?mode=ro" "select kind,machine_id,person_key from task_ownership where task_id='$T1' order by rowid"
