# 5b3da9 demo: headless, October 30, 2026

No one was present; the agent stood in for Jack Heart at the demo decision. Design: `scratch/5b3da9-design.md`. Build: this branch's debug `lf` at 3807847743, against a `sqlite3 .backup` copy of Jack Heart's live store (`LF_HOME` pointed at the copy, inherited `LF_*` cleared). The copy was deleted afterwards.

## Observed

- **A reader ends with its parent.** `lf desktop plan --json --all --watch` (Desktop's planning part) ran under a shell whose stdin was a FIFO that a separate `sleep` held open. After `kill -9` on the parent shell, the reader exited within about 1 s while its stdin stayed open.
- **Reads write nothing in steady state.** `lf monitor ps --json`, `lf task status 5b3da9 --json` and `lf desktop plan --json --all` left every `store_revisions` domain unchanged and wrote no Process row. A temporary insert/update/delete trigger on `processes` recorded nothing. The first few CLI reads on the fresh copy did move `processes` (+14 per read) and `planning`, and one read added 1 to `tasks.ship_restarts`. That was `ops::recovery::recover` (lazy recovery in `lf.rs`, which runs on `task status`, `monitor`, `desktop plan` and similar reads) settling the copy's backlog. This behavior already exists on main, and the in-app library never runs it.
- **Idle CPU, debug build:** 7.0 s of CPU over 120 s (≈5.8% of one core) for one `desktop plan --all --watch` reader. The copy's revisions stayed still, but the reader follows real checkouts, and live agents were changing them. It emitted 3 frames: one Task's `local_progress.dirty` flipped, and each frame was the full ≈1.3 MB plan. Because a debug build overstates CPU, this doesn't test the 2% target. The installed-build measurement stays with gate, as planned.
- **Not observed:** idle Desktop starting no `lf`, and the Working Tasks sidebar latency. Both need a running Dev app, and this demo does not launch one unasked.

## Interpretation (agent, not Jack Heart)

- The parent-exit and quiet-read done-whens hold on the CLI path.
- Done-when wording: `lf monitor work` no longer exists as a command (`lf monitor` has `ps`, `top`, `rollover`, `prune`). Desktop's readers are the per-part watches (`desktop plan --all --watch` and the others under `Read::subscribe`). The design's title and done-whens still say `lf monitor work`. A later realign should name the current readers.
- A frame is the whole plan even when one Task's flag changes. The reading may be cheap, but each change still serializes about 1.3 MB to stdout. That matters only for CLI `--watch` clients; in-app pulls skip stdout. Record it if the installed-build profile shows it.
- Demo hazard: CLI reads against a store copy run lazy recovery, which can start `lf --task <id> ship` with that build in real Task worktrees. No such process was found afterwards. The 03:57 `Clear added scratch files` commit in `manabot.892c47` came from the installed app's `lf ops pr land` for Jack Heart's own requested ship of 892c47 (live store Process `c1f85293…`), not from this demo. Future store-copy demos should use read-only commands outside the recovery list, or an empty Machine row.
- Live observation outside this Task: Task f871a8 has `ship_restarts` 493 on the live store, still climbing. Its ship keeps failing and restarting.

## Decision

Advance. The implement pass's lifecycle fix and write slices hold under the headless checks above. No concrete gap within the implemented scope needs another implement pass. Still owned by others: gate (installed-build idle/busy CPU, Xcode build, full matrix) and an interactive demo (idle Desktop starts no `lf`; Working Tasks latency).
