# Private Machine for LOO-869 QA. Source from zsh.
for v in $(env | grep -E '^(LF_|REX_)' | cut -d= -f1); do unset $v; done
export QA=/tmp/lf-qa-869
export LF=$QA/bin/lf
export HOME=$QA/home LF_HOME=$QA/lfhome LF_BIN=$QA/bin/lf
export CODEX_HOME=$HOME/.codex CLAUDE_CONFIG_DIR=$HOME/.claude
export REPO=$QA/repo
