export GHOSTTY_RESOURCES_DIR="$LF_SHELL_RESOURCES/ghostty"
if [ -d "$GHOSTTY_RESOURCES_DIR/shell-integration" ]; then
    export GHOSTTY_SHELL_FEATURES="${GHOSTTY_SHELL_FEATURES-}"
    case "${SHELL##*/}" in
        zsh)
            if [ "${ZDOTDIR+x}" = x ]; then export GHOSTTY_ZSH_ZDOTDIR="$ZDOTDIR"; fi
            export ZDOTDIR="$LF_SHELL_RESOURCES"
            ;;
        bash)
            export GHOSTTY_BASH_ENV="${ENV-}"
            export GHOSTTY_BASH_INJECT=1
            export ENV="$LF_SHELL_RESOURCES/bash-env"
            exec "$SHELL" --posix -l
            ;;
        fish)
            exec "$SHELL" -l -C 'source "$GHOSTTY_RESOURCES_DIR/shell-integration/fish/vendor_conf.d/ghostty-shell-integration.fish"; if test -n "$LF_BIN"; set -gx PATH (dirname "$LF_BIN") $PATH; end'
            ;;
    esac
fi
exec "$SHELL" -l
