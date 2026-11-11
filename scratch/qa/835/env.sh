#!/bin/zsh
# Run a command in the private QA Machine with the branch lf.
QA=/tmp/lf-qa-835
LF=/Users/jack/src/loopflow.loo-835-keep-one-server/target/debug/lf
exec env -i HOME=$QA/home LF_HOME=$QA/lf LF_BIN=$QA/bin/lf USER=qa \
  CLAUDE_CONFIG_DIR=$QA/home/.claude CODEX_HOME=$QA/home/.codex \
  PATH=$QA/bin:/Users/jack/.nvm/versions/node/v24.22.2/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin TERM=xterm-256color \
  GIT_ALLOW_PROTOCOL=file TMPDIR=/tmp "$@"
