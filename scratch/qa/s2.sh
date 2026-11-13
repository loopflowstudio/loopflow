ROOT=/tmp/lf-qa-867
for v in $(env | grep -E '^(LF_|REX_)' | cut -d= -f1); do unset $v; done
export HOME=$ROOT/home LF_HOME=$ROOT/lfhome LF=/tmp/lf-qa-867-bin LF_BIN=/tmp/lf-qa-867-bin
export CODEX_HOME=$HOME/.codex CLAUDE_CONFIG_DIR=$HOME/.claude
cd $ROOT/repo
step() { print "\n### $*"; }
run() { print "\$ lf ${(j: :)@}"; $LF "$@" < /dev/null 2>&1; print "(exit $?)"; }
T1=$(cat $ROOT/task1)
step "Declare Machine mini in personal global config"
print "existing:"; cat $LF_HOME/config.yaml 2>/dev/null
cat >> $LF_HOME/config.yaml <<Y
machines:
  mini:
    id: mach_qa_mini_0000000000000000
Y
run machine list
step "Route this repository's new Tasks to mini"
run set machine.tasks mini
run config explain machine.tasks
step "New Task while mini owns the repository's Tasks"
out=$($LF task create --title 'Runs on mini' --request-id qa-2 --json < /dev/null 2>&1)
T2=$(print -r -- "$out" | python3 -I -c 'import json,sys; d=json.loads(sys.stdin.read()); print(d.get("task",d).get("id"))' 2>&1)
print "TASK2=$T2"; print $T2 > $ROOT/task2
step "Claim it here (writer is mini)"; run task claim $T2
step "Existing owned Task stays writable here"; run task unassign $T1; run task claim $T1
step "Hand T1 to mini"; run task move $T1 --machine mini
run task show $T1 --json
