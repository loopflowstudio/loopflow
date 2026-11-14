# Runs only on creation inside the destination PTY.
export LF_TERMINAL_ID="$0"
export LF_TERMINAL_TTY="$(tty)"
export SHELL="$1"
bootstrap="$2"
export LF_SHELL_RESOURCES="$3"
shift 3
if [ -n "$LF_BIN" ]; then export PATH="${LF_BIN%/*}:$PATH"; fi
# Personal login startup precedes literal initial argv. Reattachment never runs it.
if [ "$#" -gt 0 ]; then
    "$SHELL" -l -c '"$@"' loopflow "$@"
fi
if [ -n "$bootstrap" ]; then exec /bin/sh -c "$bootstrap"; fi
unset LF_SHELL_RESOURCES
exec "$SHELL" -l
