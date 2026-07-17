# Bound daemon logs

The first live install found `~/.lf/logs/lfd.log` at 385 MiB. launchd and
systemd were configured to append stdout and stderr forever, so worktree
recovery itself carried an unbounded file.

Make `lfd` own one 8 MiB current log and one predecessor. Service managers send
their duplicate stdout/stderr streams to the null sink. Rotation stays inside
the process, works identically under launchd and systemd, and honors `LF_HOME`.
