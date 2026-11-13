# usage: count.sh <lf-binary> <root>
B=$1; ROOT=$2; rm -rf $ROOT; mkdir -p $ROOT/home $ROOT/lfhome
for v in $(env | grep -E '^(LF_|REX_)' | cut -d= -f1); do unset $v; done
export HOME=$ROOT/home LF_HOME=$ROOT/lfhome LF_BIN=$B CODEX_HOME=$ROOT/home/.codex CLAUDE_CONFIG_DIR=$ROOT/home/.claude
git init -q -b main $ROOT/repo && cd $ROOT/repo
git config user.email qa@example.com; git config user.name 'QA Person'
mkdir -p .lf; printf 'default_agent: codex\nplanning: {provider: git, personal: true}\n' > .lf/config.yaml
git add -A; git commit -qm init
T=$($B task create --title count --json < /dev/null 2>/dev/null | python3 -I -c 'import json,sys; d=json.load(sys.stdin); print(d.get("task",d)["id"])')
$B task claim $T < /dev/null > /dev/null 2>&1
print 'machines: [not, a, map' > $LF_HOME/config.yaml
for c in unassign claim; do
  n=$($B task $c $T < /dev/null 2>&1 | grep -c 'skipping malformed'); print "$c warnings: $n"
done
n=$($B task show $T --json < /dev/null 2>&1 >/dev/null | grep -c 'skipping malformed'); print "show warnings: $n"
