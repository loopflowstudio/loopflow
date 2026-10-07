# Assumptions — LOO-411 and later remote-work slices

2026-10-07. Written from research and a conversation with Jack Heart that
settled the holder and the credential rules. Jack authorized the rename and then
LOO-411's machine record through publication and review; later slices remain intent.

- Jack Heart's original mini command and error from 2026-10-06 were never
  captured. The design assumes `lf ssh mini --task <ISSUE> <skill>`, which fails
  today with `Task "…" is not registered`. Reproduce before treating that as the
  incident.
- A target machine is assumed to have the repository cloned. Cloning it is out
  of scope.
- Provider facts come from docs and issue trackers, not experiments. Refresh
  rotation for Claude rests on user reports and loopflow's own comments;
  `setup-token` scope and revocation are partly unverified. No command was run
  against real credentials.
- The Keychain behaviour in `store/token_crypto.rs` over ssh is read from code,
  not observed.
- "Do not restore that service" (product Wave memory) is read as ruling out a
  shared resident process. Jack Heart accepted a per-Session holder on
  2026-10-07 and chose the transparent relay.
- The list of terminal modes the relay must restore on reattach is from general
  knowledge of terminals, not from testing Claude Code or Codex.
- LOO-393's worktree and Task were removed on 2026-10-07 after its commit was
  saved as a patch here. LOO-395 was cancelled as merged the same day.

2026-10-07 rename preservation: keep opaque IDs and installed plist/cache keys;
read released cron JSON via an explicit field alias. These are persisted-data
contracts, not alternate commands or runtime owners. No installed store is touched.

LOO-411, 2026-10-07: the migration helper mistakes the stacked parent's draft for
this Task's. Keep the parent's rename immutable and add this Task's one draft,
dependent on it; the parent renames an existing released table, not a new owner.
Jack Heart's October 7 PR review now permits a first-install offer during
interactive add, default yes; existing lf is never replaced. Batch, JSON and
nonterminal use report the install command. A 60-second idle lifetime is the
reversible choice for private OpenSSH sharing.
