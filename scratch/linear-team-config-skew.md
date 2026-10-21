# Why LOO-520 stalled on `unknown field \`team\``

## What failed

On October 21, 2026, Jack ran `lf task run LOO-520 pursue -a claude` in
`loopflow.loo-520-use-asana-or`. Installed lf (0.14.12) died with:

    Error: invalid flow: Config validation error: unknown field `team`, there are no fields

It had already failed the same way the day before. The store's `processes`
table (read-only query, `cwd` in the LOO-520 worktree, matching that error)
holds five failures:

| When (PDT) | Who ran it | What it ran |
|---|---|---|
| Oct 20 13:31:20 / :24 / :26 | `lf task run LOO-520 pursue` (`c58d0ac1…`), installed `lf-6c7e562b…` | three pursue Flows (`190a4add`, `43dc3676`, `d367f3a0`), each dying at its first step |
| Oct 21 11:20:52 / 11:21:00 | Jack, installed `lf-eb54eaac…` (0.14.12) | `session connect` and the pursue relaunch |

LOO-520 made no progress for about 22 hours. Its status showed the error
only as "latest Flow: implement: invalid flow: …".

**Status: shim reverted.** `7d0c4047b8` read the released `linear.team` so
installed lf kept working here. Jack rejected it on October 21: no
compatibility shims. The branch keeps `planning.team`; Flows in this
worktree need the branch-built lf until the release, and lf is deployed with
the merge.

## Causal chain

1. **Installed lf rejected the worktree's `.lf/config.yaml`.** The file said
   `planning: {provider: linear, team: …}`. In released lf, `PlanningConfig` is
   `#[serde(tag = "provider", deny_unknown_fields)]` and its `Linear` variant
   has no fields. Any extra key there is fatal ("there are no fields").

2. **The branch had changed that file.** `d42c5e33bd` (Oct 20 13:02, "Name the
   planning team for every provider") moved `linear.team` to `planning.team`.
   That is the intended shape for LOO-520's Asana and Notion providers. The
   first failure came 29 minutes later.

3. **Flows in a branch's worktree run the installed lf against that branch's
   config.** Every step command was `~/.lf/bin/lf-<installed digest>`, run
   with the worktree as its cwd. Repository config is read by whichever lf
   runs in the checkout. Here that means the released binary reading
   unreleased config, so the branch broke its own pipeline. The store has a
   guard for exactly this skew: the reader floor, plus draft builds that work
   on a copy of the store. Config has neither.

4. **The branch also refused the released shape, so no file satisfied both
   builds.** `d42c5e33bd` added a guard in `load_yaml_file` that errored on
   any top-level `linear:` block ("moved to `planning.team`"). The installed
   build needed `linear.team`, the branch build refused it, and each rejected
   the other's form. AGENTS.md's Flexibility section names this pattern: a
   guard that refuses the context instead of adapting to it.

5. **Nothing ran the released reader against the edited config before the
   step ended.** The branch's config tests write temporary files. The
   implementing agent's focused checks ran the branch build. The first
   released-lf read of the file was the next Flow's first step.

### Side branch: why it went unnoticed for a day

- **Automatic retries replayed a failure that could not change.** `lf task
  run` restarts a failed Flow up to `TASK_RUN_ATTEMPTS` times
  (`rust/loopflow/src/lf/commands/flow.rs:112-169`). Three Flows each died in
  about two seconds on the same config load, before any agent turn. A retry
  cannot fix a config parse error.
- **The error doesn't explain itself.** It is labelled `invalid flow`
  (`LoadError::InvalidFlow`), names no file, and says nothing about version
  skew. "There are no fields" is serde wording that means nothing to the
  reader. Jack couldn't tell from the message that the cause was a released
  binary reading branch config.
- **Nothing told Jack the Task had stopped.** No message reached Jack until
  Jack ran it again.

## Unanswered

- The October 20 13:31 failures were under `lf-6c7e562b…`, not 0.14.12.
  Which release that digest is was not checked. The same strict enum shipped
  in both, so the analysis does not depend on it.
- Flow `2e27945c`'s implement step (`a4ce5851`) ran from October 19 19:58 to
  October 21 11:20 and exited 1 with no recorded error. It ended at the
  moment of Jack's relaunch, so it is probably unrelated to this failure, but
  it was not traced.
- Whether a step that ran after `d42c5e33bd` inside the same Flow also loaded
  config was not checked.

## Prevention (proposed, not accepted)

Jack rejected the fold in `7d0c4047b8` (reverted): no compatibility shims.
Proposal 1 below is rejected with it.

1. **Treat repository config as a format that two versions read at once.**
   This addresses links 3–4. While a branch is in flight, the released lf and
   the branch build both read the same file. A config shape change keeps the
   released form readable until it releases, as LOO-520 now does. After the
   release, a later change moves the file to the new form and deletes the
   fallback. Proposed as an AGENTS.md rule beside the SQLite schema rules.
   - **Proof:** a gate check that parses the branch's `.lf/config.yaml` with
     the installed lf whenever the diff touches that file.
2. **Name the file and the skew in config errors.** This addresses the
   side-branch messaging. Give config failures their own error, not
   `InvalidFlow`. Include the path, and when the field is unknown, say "this
   config may be for a newer lf (installed X)".
   - **Proof:** a unit test feeding an unknown planning field asserts the
     path and the hint appear in the message.
3. **Don't retry a Flow that failed before any agent turn on a load error.**
   This addresses the side-branch retries. Stop on the first such failure and
   report it as blocked.
   - **Proof:** a flow-runner test with a broken config sees exactly one
     attempt.
4. **Consider letting unknown keys in repository planning config warn rather
   than fail.** This addresses link 1 for purely additive fields. It would
   not have saved this rename: Linear would have lost its team quietly
   instead. So it is weaker than 1 and needs Jack's judgment against the
   DTO rule's preference for strictness.

Recommended next action: proposals 1 and 2. They are small and they directly
address the release-versus-branch skew that will recur with the next config
rename.
