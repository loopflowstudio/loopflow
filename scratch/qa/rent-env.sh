R=${R:-/tmp/lf-qa-rent}; LF=${LF:-/tmp/lf-qa-867-bin}
for v in $(env | grep -E '^(LF_|REX_)' | cut -d= -f1); do unset $v; done
export HOME=$R/home LF_HOME=$R/lfhome LF_BIN=$LF PATH=$R/stand:$PATH DAYTONA_API_KEY=qa-placeholder
export CODEX_HOME=$HOME/.codex CLAUDE_CONFIG_DIR=$HOME/.claude
cd $R/repo
step() { print "\n### $*"; }
run() { print "\$ lf ${(j: :)@}"; $LF "$@" < /dev/null 2>&1; print "(exit $?)"; }
