# Receive one home-relative file from SSH stdin, under the caller's umask 077.
receive_file() (
    path="$HOME/$1"
    size=$2
    mode=$3
    dir=${path%/*}
    mkdir -p "$dir"
    tmp=$(mktemp "$dir/.lf-forward.XXXXXXXX")
    trap 'rm -f "$tmp"' 0
    trap 'exit 1' HUP INT TERM

    dd bs=1 count="$size" of="$tmp" 2>/dev/null
    if [ "$(wc -c < "$tmp")" -ne "$size" ]; then
        echo "incomplete forwarding of ~/$1" >&2
        exit 1
    fi
    if [ "$mode" = replace ]; then
        if [ -d "$path" ]; then
            echo "destination is a directory: ~/$1" >&2
            exit 1
        fi
        mv -f "$tmp" "$path"
    elif [ -e "$path" ] || [ -L "$path" ] || ! ln "$tmp" "$path"; then
        # A concurrent launch can win the link. Keep its complete copy.
        [ -e "$path" ] || [ -L "$path" ] || exit 1
        cmp -s "$tmp" "$path" || printf 'kept ~/%s\n' "$1"
    fi
)
