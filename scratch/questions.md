# Open questions and assumptions (LOO-709)

- **Grace default: 2 minutes.** Jack Heart named 2m and 30m as candidates and
  left the choice open. 2m is chosen because the goal is that an idle
  conversation costs nothing and `lf resume` pays only a provider restart.
  Change with `session.end_detached_after` in `~/.lf/config.yaml`.
- **Restart set: viewed only.** The brief says `lf resume --all` resumes
  "the conversations that had a viewer when the machine went down (or that
  recorded no exit)". Implemented as: owner had a viewer AND never recorded
  its own exit AND is gone. A crashed owner that was busy but unviewed is not
  resumed; its cut-off turn would not be replayed anyway.
- **Snapshot location, not a DTO field.** `lf resume --prepare --json` does
  not name the screen file: `SessionRecord` is a mirrored DTO and adding a
  field means touching every Swift fixture, and Desktop (LOO-672) is out of
  scope. Readers use `$LF_HOME/screens/<session>.vt`, named in the transport
  README and `transport::last_screen_path`. Jack Heart's 2026-11-02 steer
  asked this Task and LOO-710 to agree on one path and one reader before
  either publishes. Agreed 2026-11-02: LOO-710 adopted this branch's
  directory and Session key (`<home>/screens/<session>`), keeps its own
  format (`.ghostsnp`), adds no DTO field, and owns the reader in `lf resume`;
  the second branch to land deletes the `.vt` writer. The writer runs on
  every end the owner decides, idle, executable gone, LOO-705's superseded,
  through the one `ending` arm, and on its command's exit. An external
  SIGTERM writes none.
- **Owners from before the rule.** Jack's 2026-11-02 steer: LOO-705's
  `lf monitor rollover` ends a superseded owner that cannot answer its socket
  and reports the cut turn. The PR points the 27 stale owners there instead
  of listing hand signals.
- **Turn-in-progress for native terminals.** Loopflow's Claude hooks deliver
  context only, so an unviewed native Claude leaves no "turn running" fact in
  the store. The owner treats its command's own terminal output within the
  grace as a turn in progress (Claude's and Codex's spinners redraw every
  second mid-turn). A provider that stays silent mid-turn for the whole grace
  with no store fact is ended and reopened from history.
- **"Advance the boot time" in the restart test.** Integration tests cannot
  move the kernel boot time, and with every PID killed the boot time changes
  no judgement. The test instead closes the dead owner rows the way retention
  does after a reboot (`completed_at` set, no `outcome`) and shows
  `lf resume --all` still resumes the viewed set.
- **`s.interactive=0` in `owner_waits_for_person` stays.** A pipe owner
  never reads a detached limit, so the headless Run test passes without it,
  but every owner watches its executable, and that path asks the same
  question: a Run whose lf build is replaced mid-turn must keep running, and
  `interactive=0` is what keeps it. Resolved 2026-11-02 (compress).
