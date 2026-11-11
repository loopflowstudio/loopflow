#!/bin/zsh
# Run a command in the private QA Machine with the branch lf.
QA=/tmp/lf-qa-821
LF=/Users/jack/src/loopflow.loo-821-managed-sessions-hooks/target/debug/lf
exec env -i HOME=$QA/home LF_HOME=$QA/lf LF_BIN=$LF USER=qa \
  CLAUDE_CONFIG_DIR=$QA/home/.claude CODEX_HOME=$QA/home/.codex \
  PATH=$QA/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin TERM=xterm-256color \
  GIT_ALLOW_PROTOCOL=file TMPDIR=/tmp "$@"
