# Runs inside the destination PTY, never in a Desktop attachment's local shell.
export LF_TERMINAL_ID="$0"
export LF_TERMINAL_TTY="$(tty)"
export SHELL="$1"
bootstrap="$2"
export GHOSTTY_RESOURCES_DIR="$bootstrap/ghostty"
shift 2
# Personal login startup runs before the initial literal argv. Its exit leaves
# the destination shell; losing a viewer never executes this script again.
if [ "$#" -gt 0 ]; then
    "$SHELL" -l -c '"$@"' loopflow "$@"
fi
if [ -d "$GHOSTTY_RESOURCES_DIR/shell-integration" ]; then
    export GHOSTTY_SHELL_FEATURES="${GHOSTTY_SHELL_FEATURES-}"
    case "${SHELL##*/}" in
        zsh)
            if [ "${ZDOTDIR+x}" = x ]; then export GHOSTTY_ZSH_ZDOTDIR="$ZDOTDIR"; fi
            export ZDOTDIR="$bootstrap"
            ;;
        bash)
            export GHOSTTY_BASH_ENV="${ENV-}"
            export GHOSTTY_BASH_INJECT=1
            export ENV="$bootstrap/bash-env"
            exec "$SHELL" --posix -l
            ;;
        fish)
            exec "$SHELL" -l -C 'source "$GHOSTTY_RESOURCES_DIR/shell-integration/fish/vendor_conf.d/ghostty-shell-integration.fish"; if test -n "$LF_BIN"; set -gx PATH (dirname "$LF_BIN") $PATH; end'
            ;;
    esac
fi
exec "$SHELL" -l
