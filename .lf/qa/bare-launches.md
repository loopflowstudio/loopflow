Read when the change touches a bare `lf` launch on a terminal: job control and foreground groups.

# Bare terminal launches

A bare `lf : "<prompt>"` on a PTY runs the provider as `lf`'s child and
the terminal's foreground group leader; `ps -o pid,ppid,pgid,tpgid,tty`
shows it. Drive Ctrl+Z/`fg` from `zsh -f -i` on the PTY, not bare `lf`.
A stand-in `claude` reads its first turn from stdin only when stdin is not a
terminal (`[ -t 0 ] || first=$(cat)`), then keys from `/dev/tty`; on resume
stdin is the terminal. Pi retitles itself `pi`: read its argv from
`processes.command`. A Session the installed `lf` made can be reopened by
the branch only when the branch's migrations are a superset; otherwise it
refuses with "a newer lf migrated this store".
