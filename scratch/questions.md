# Open questions (LOO-858)

- Desktop shells now need the Machine server on their Machine (auto-started
  like other managed launches). Today they deliberately ran without it on
  headless Linux. Docs assume the systemd server is acceptable there.
- `-b` bare runs end with their caller (in-process); `-b` Work commands
  survive it (detached, output tailed). Docs assume this split.
- A piped interactive launch (`lf resume X < /dev/null`, no tty) now runs
  here with pipes instead of an owner's PTY; a provider that needs `/dev/tty`
  exits. Only tests did this; assumed acceptable.
- Desktop shell Close ends the shell at once (close notice → kill its
  session). The owner-era wait for "requested work" in the shell is gone:
  work started from a shell is either bare (ends with the shell) or Work
  (held or detached elsewhere).
- An earlier build's owner-hosted *bare* Session restarts where any resume of
  it runs: held from a Desktop pane, in the terminal for `lf resume` (so it
  now ends with that terminal, where the owner outlived it). The steer said
  "resume held from provider history"; docs now describe the code. Holding
  it from a terminal too would mean a special case in `transport::delegate`.
