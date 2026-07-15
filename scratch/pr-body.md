## Try it!

```bash
uv run python scripts/test.py --all

cd swift
./dev run-debug
```

Open **Go → Context Lab**, select a repository and 30-day window, then move
between **Aggregate flame**, **Session lanes**, and **Table**. Select one source
revision, inspect its representative-session metadata, and use **Open trace** to
cross the explicit prompt-body boundary. **Refine source…** requires an idle
Intelligence Task that already owns a Task Session and durable worktree.

Query the same atomic reader directly:

```bash
cargo run -q -p loopflow --bin lf -- \
  context --days 30 --repo "$PWD" --json > /tmp/context-lab.json

jq '{totals, coverage, root_tokens: .aggregate_root.attributed_tokens}' \
  /tmp/context-lab.json

cargo run -q -p loopflow --bin lf -- \
  context --days 30 --repo "$PWD" --steered-only --json
cargo run -q -p loopflow --bin lf -- \
  context --days 30 --repo "$PWD" --current-revision-only --json
```

On the July 15 live Loopflow ledger, 55 sessions, 131 launches, and 139 turns
produced 1,053,450 attributed tokens. The aggregate root and sum of its children
both equal 1,053,450; eight provider-total-only turns remain visible only in
coverage. **Observed steering only** reduces that population to two sessions,
three launches, 11 turns, and 31,318 tokens. **Contains current file
instruction** returns three sessions, three launches, three turns, and 30,982
tokens. The captured `LOOPFLOW.md` revision is now historical and is correctly
excluded as a matching revision rather than mislabeled current.

## Intent

Add Context Lab to the Mac app as a native research workspace over real local
Loopflow sessions. Rust returns one reconciled session-set snapshot with totals,
coverage, context flames, prompt-ordered lanes, canonical revisions, and exact
trace addresses. Swift links those views and can hand one editable revision to a
fresh `refine` session in an existing Task worktree without mutating the
historical trace.

## Assumptions

- The local trace/context ledger is the source of truth; missing token,
  conversation, steering, or cost capture remains missing rather than zero.
- Canonical source identity and effective revision hashes are computed in Rust.
  Swift only checks the returned raw-file receipt against canonical and Task
  worktree bytes immediately before launch.
- Refinement uses an existing idle Intelligence Task Session and its registered
  worktree. Context Lab does not create control-plane ownership.
- Migration `0.11.006_context_launch_work` attributes new launches to durable
  Project and Task identities. Historical launches remain unattributed.

## Key decisions

- Keep the session set as the primary object; there is no instruction-admin
  catalog or copied prompt database.
- Keep prompt and conversation bodies closed until **Open trace**.
- Build flame identity as context kind → canonical source → content revision,
  with provider-total-only capture excluded from attributed geometry.
- Treat observed steering and current-file identity as launch predicates, then
  keep each qualifying launch whole so every snapshot still reconciles.
- Block revision comparison when capture, provider/model mix, or non-zero
  observation spans are materially imbalanced.
- Select representative evidence from distinct outer sessions, falling back to
  the next-best session for each role rather than duplicating or dropping it.
- Refresh source and Task identity before refinement, then rehash canonical and
  worktree files at the last safe moment before command dispatch.
- Reuse Task workspaces, terminals, diffs, and the normal commit/PR lifecycle.

## Not included

- Creating a Linear Task from the refinement sheet.
- A second editor, agent host, git path, remote telemetry store, or LLM-authored
  quality score.
- Proof of the continuous real Intelligence Task journey. The available local
  PM state has no registered Intelligence Wave or selectable W2-71 Task Session,
  so this review did not claim a refinement launch, source diff, backlink, or
  natural post-edit revision.
- Hosted UI and installed-app keyboard walkthrough proof; the headless gate ran
  the full matrix at the gated base, then proportional Rust, website, Swift,
  and Mac build-for-testing checks for this slice instead.
