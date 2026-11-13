ROOT=/tmp/lf-qa-867
for v in $(env | grep -E '^(LF_|REX_)' | cut -d= -f1); do unset $v; done
export HOME=$ROOT/home LF_HOME=$ROOT/lfhome LF=/tmp/lf-qa-867-bin LF_BIN=/tmp/lf-qa-867-bin
export CODEX_HOME=$HOME/.codex CLAUDE_CONFIG_DIR=$HOME/.claude
cd $ROOT/repo; T1=$(cat $ROOT/task1)
run() { print "\$ lf ${(j: :)@}"; $LF "$@" < /dev/null 2>&1; print "(exit $?)"; }
print "### malformed personal config"; cp $LF_HOME/config.yaml $ROOT/config.good
print 'machines: [not, a, map' > $LF_HOME/config.yaml
run task unassign $T1
cp $ROOT/config.good $LF_HOME/config.yaml
print "### store writable afterwards"; run task claim $T1; run task unassign $T1
print "### two claims at once"
($LF task claim $T1 < /dev/null > $ROOT/c1 2>&1; echo "a exit $?" >> $ROOT/c1) &
($LF task claim $T1 < /dev/null > $ROOT/c2 2>&1; echo "b exit $?" >> $ROOT/c2) &
wait; tail -1 $ROOT/c1; tail -1 $ROOT/c2
print "### last log rows"; $LF task claim $T1 < /dev/null 2>&1 | tail -3
