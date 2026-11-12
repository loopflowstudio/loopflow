# Open questions (LOO-858)

- Should `lf resume` keep attaching to owners an earlier build started
  (`lf session serve`, still running until their agents quit)? Docs assume yes:
  the viewer side (`lf session attach OWNER`, `session.attach
  {owner_lf_process_id}`) stays while the serving side is deleted. Deleting it
  too would leave those conversations running but unviewable; `lf resume`
  would then start a second agent on the same thread.
- Does `lf process rollover` go entirely? Docs assume yes (holders keep their
  build; quit + `lf resume` moves a conversation). It is the only bulk way to
  move live conversations to a new build.
- Desktop shells now need the Machine server on their Machine (auto-started
  like other managed launches). Today they deliberately run without it on
  headless Linux. Docs assume the systemd server is acceptable there.
- `-b` bare runs end with their caller (in-process); `-b` Work commands
  survive it (detached, log-tailed). Docs assume this split.
