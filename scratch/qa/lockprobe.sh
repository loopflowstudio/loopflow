# usage: lockprobe.sh <lf-binary> <root> <subcommand...>
# Serves $LF_HOME/config.yaml through a FIFO; at each open, checks whether
# another connection can take the store's write lock.
B=$1; ROOT=$2; shift 2
for v in $(env | grep -E '^(LF_|REX_)' | cut -d= -f1); do unset $v; done
export HOME=$ROOT/home LF_HOME=$ROOT/lfhome LF_BIN=$B CODEX_HOME=$ROOT/home/.codex CLAUDE_CONFIG_DIR=$ROOT/home/.claude
cd $ROOT/repo
T=$(sqlite3 "file:$LF_HOME/loopflow.db?mode=ro" "select id from tasks limit 1")
good=$'machines:\n  mini:\n    id: mach_qa_mini_0000000000000000\n'
rm -f $LF_HOME/config.yaml; mkfifo $LF_HOME/config.yaml
$B task "$@" $T < /dev/null > $ROOT/probe.out 2>&1 &
pid=$!
rm -f $ROOT/probe.stop
(trap "" PIPE; while [ ! -e $ROOT/probe.stop ]; do
  exec 3>$LF_HOME/config.yaml
  if sqlite3 -cmd '.timeout 0' $LF_HOME/loopflow.db 'BEGIN IMMEDIATE; ROLLBACK;' >/dev/null 2>&1; then print free; else print HELD; fi
  print -n -- "$good" >&3; exec 3>&-
done) > $ROOT/probe.opens &
feeder=$!
wait $pid; st=$?
touch $ROOT/probe.stop
while kill -0 $feeder 2>/dev/null; do
  cat $LF_HOME/config.yaml > /dev/null & r=$!; sleep 0.3; kill $r 2>/dev/null
done
rm -f $ROOT/probe.stop
print "opens: $(sort $ROOT/probe.opens | uniq -c | tr '\n' ' ')"
rm -f $LF_HOME/config.yaml; print -n -- "$good" > $LF_HOME/config.yaml
print "lf task $* -> exit $st"; tail -3 $ROOT/probe.out
