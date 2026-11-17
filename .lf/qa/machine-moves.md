Read when the change moves a Task between owned Machines.

# Moving a Task between owned Machines

The same stand-in `ssh` drives `lf machine add mini` and `lf task move <task>
--machine mini` with no real host: it runs its last argument under mini's own
`HOME` and explicit `LF_HOME` (unset, `lf` finds the real `~/.lf`), with mini's
`~/.local/bin/lf` the branch binary and a `~/.gitconfig` identity. A local bare
origin and `planning: {provider: git, personal: true}` are enough; the move's
check clones the repository on mini. To leave a move `sent, not arrived`, swap
mini's `lf` for a wrapper that exits 1 on `session receive` without `--check`,
then read `lf task status`, `lf task list` and `lf machine list` before
restoring it and repeating the move. A wrapper that runs the real receive and
then exits 255 loses the arrival answer. Both homes share the host name, so mini
labels the laptop's claim by its own host name. For mini to reach the laptop
(retries on mini, moves back), route the stand-in by host: when an argument is
`laptop`, run under the laptop's `HOME` and `LF_HOME`; then `lf machine add
laptop` on mini, from outside any clone. Run inside a clone before the first
move, `lf machine add` and `lf config repo --bind` minted mini's own planning
key, and the move's check then refused; start mini's home over. A marker file the
stand-in checks per host (exit 255, "Connection refused") makes either side
unreachable without touching the other's store.
