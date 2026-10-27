# LOO-609 open choices (kickoff 2026-10-26, reversible)

- Resolved: Jack Heart (October 26) put the identifier at the end —
  `<Task title> · <identifier>`.
- cmux tabs keep following the Session name and standing; only the workspace is
  Task-owned. Jack's "always stay that" was read as about the workspace.
- `destination_name` changes for every host's create name (cmux, Herdr, tmux), not
  only cmux, to keep one naming rule; tmux's runtime window naming is unchanged.
- Taskless Sessions in cmux stop renaming their workspace at all.
- Kickoff trimmed product Wave memory by ~90 tokens (compressed shipped-history
  pointers) to meet the 16000-token target; no live decision removed.
- Resolved: Jack Heart (October 26, after concept review) — don't overwrite a cmux
  workspace name someone edited by hand, when lf can tell. Before renaming, read the
  workspace's current name (`cmux workspace list`); rename only a name lf wrote
  (its last published name, the create name, or the old `<identifier> · …` shape).
  The read only gates lf's own rename; it never writes to a Task or Session.
- Resolved: Jack Heart (October 26) — receipts elsewhere also lead with the title:
  `lf open` receipts and other `<identifier> · <title>` labels become
  `<title> · <identifier>`. Session names themselves are unchanged.
- Assumed (pr-publish, October 26): "names lf wrote" also covers earlier lf's
  runtime shape `<identifier> <purpose>` (e.g. `LOO-609 cmux titles`) and any
  `<title> · <identifier>` (a create name under an older title), since lf keeps no
  record of them across restarts. An unreadable `cmux workspace list` skips the rename.
