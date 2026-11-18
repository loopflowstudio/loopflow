# usage: . env.sh <new|old|prev>   (prev: the branch build before the second implement pass)
TAG=$1
ROOT=/tmp/lf-qa-978/$TAG
if [ "$TAG" = old ]; then BIN=$(whence -p lf); else BIN=/tmp/lf-qa-978/bin/lf-$TAG; fi
for v in $(env | grep '^LF_' | cut -d= -f1); do unset $v; done
unset CLAUDE_CODE_CHILD_SESSION
export HOME=$ROOT/home LF_HOME=$ROOT/home/.lf LF_BIN=$BIN
export CODEX_HOME=$HOME/.codex CLAUDE_CONFIG_DIR=$HOME/.claude
L() { "$BIN" "$@" < /dev/null; }
