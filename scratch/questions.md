# Assumptions — LOO-394

2026-10-07. Written from research and a conversation with Jack Heart that
settled the holder and the credential rules. Jack authorized only the rename slice and publication; later slices remain intent.

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
- GitHub's handling of a custom ref namespace (accepted, hidden in the UI, no
  Actions, not in default clones) is general knowledge, not verified here.
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
read released cron JSON via an explicit field alias; discard version-1 Desktop
caches per Jack Heart’s review. Retain `~/.lf-machine/install` and the existing
promotion lock because released gates, receipts and jobs pin those paths and
no installed relocation proof exists. These are persisted-data
contracts, not alternate commands or runtime owners. No installed store is touched.

Context: `lf context --skill implement --json` reports authored memory at
15,986/16,000 tokens and scratch at 6,325/12,000. The generated launch goal is
23,644/16,000 (7,644 over), from the broad committed file inventory and stored
steers. Authored-note edits cannot reduce that generated source; limits remain
unchanged and submitted input fits. The complete launch source was inspected;
the archived LOO-393 patch and superseded PR walkthrough remain in Git.
