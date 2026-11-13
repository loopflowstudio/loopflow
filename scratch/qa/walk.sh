#!/bin/zsh
# LOO-867 ownership walkthrough against a private Machine.
set -u
LF=${LF:-/Users/jack/src/loopflow.loo-867-keep-placement-config/target/debug/lf}
ROOT=${ROOT:-/tmp/lf-qa-867}
rm -rf $ROOT; mkdir -p $ROOT/home $ROOT/lfhome
for v in $(env | grep -E '^(LF_|REX_)' | cut -d= -f1); do unset $v; done
export HOME=$ROOT/home LF_HOME=$ROOT/lfhome LF_BIN=$LF
export CODEX_HOME=$HOME/.codex CLAUDE_CONFIG_DIR=$HOME/.claude
git init -q --bare $ROOT/origin.git
git init -q -b main $ROOT/repo && cd $ROOT/repo
git config user.email qa@example.com; git config user.name 'QA Person'
mkdir -p .lf; printf 'default_agent: codex\nplanning: {provider: git, personal: true}\n' > .lf/config.yaml
git add -A; git commit -qm init; git remote add origin $ROOT/origin.git; git push -q origin main
step() { print "\n### $*"; }
run() { print "\$ lf ${(j: :)@}"; $LF "$@" < /dev/null 2>&1; print "(exit $?)"; }
