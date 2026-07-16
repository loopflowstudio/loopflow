# Open questions and assumptions

## Resolved during kickoff (2026-07-16)

The prior draft's three open questions are all answered against the tree at
`42cd883cd`. Kept here because each answer changed the design.

1. **Does `TestRepo` set up a GitHub origin remote?** — **No.** `TestRepo::new`
   points origin at a local bare temp path, so `github_repo_nwo`
   (`engine/worktrees.rs:269`, parses only `git@github.com:` /
   `https://github.com/`) returns `None` and `observe_pr_by_number` degrades
   before CI is read. The repo already has the idiom: `point_origin_at_github`
   (`pr_tests.rs:75`) rewrites origin to a GitHub URL, which breaks push — fine
   here, since this proof never pushes. Verified that `remote.origin.pushurl`
   preserves both if a future test needs push *and* nwo.

2. **Should wire `CiObservationSnapshot` include `failing_checks` with URLs?** —
   **Yes.** `CiCheck { name, url: Option<String> }`. `ci_fix_seed`
   (`runner.rs:1848`) already renders both into the body's prompt, and the Mac
   app showing "fixing CI" wants the name and the log link. Carrying only names
   would force consumers back to prose parsing — the exact problem being fixed.

3. **Which DTO fixtures need a `ci` scenario?** — **Surveyed; exactly two files
   change.** Every populated `PrSnapshot` gets an explicit `"ci_observation":
   null`. Note the reason is *not* that the decode would otherwise break — see
   the serde correction below; it is so the fixture pins the field, enforced by
   a raw-JSON presence guard.
   - `task_attention_states.json` → `dead_authored_commits/active_pr` (null) plus
     a new populated `ci_failing` scenario; the count assertion at `waves.rs:2926`
     goes 8 → 9.
   - `roadmap_snapshot.json` → `waves[0]/projects/items[0]/tasks[1]/active_pr` (null).
   - `wave_detail.json` → **no change.** Its `active_pr` is a PR *id string*, a
     different DTO. (This was a false positive in the prior draft.)

## Assumptions

1. **In-crate placement is right, despite breaking the file convention.** The
   repo puts integration tests in flat `tests/<area>_tests.rs`. This proof can't
   live there — every function it drives is private or `pub(crate)`. The choice
   is in-crate placement vs. widening production visibility for tests; CLAUDE.md
   settles it ("Never reshape production code for tests"), and `ops/child.rs:1246`
   already tests this exact area in-crate.

2. **State-machine proof satisfies "exactly one ci-fix body."** The Done-when
   says "drives pending to failed head to exactly one ci-fix body." The proof
   drives observe → arm → push → rearm without spawning a provider. Dedup is
   decided in `arm_ci_fix_wake` *before* any process starts — `runner.rs:118`
   says so outright — so the stamp is where the claim lives. Spawning a real
   body would prove less and require mocking `child.launch`.

3. **`CiObservationSnapshot` excludes `woken_failure_set`.** It's the internal
   dedup marker and the only `#[serde(default)]` field on the storage type
   (`task/mod.rs:307`, standing in for a JSON-column migration). Excluding it
   keeps that default off the wire; storage keeps it.

5. **Corrected mid-kickoff: serde `Option<T>` does not require its key.** The
   first draft of this design assumed a plain `Option<T>` without
   `#[serde(default)]` makes the key mandatory. Measured — it does not: serde
   gives every `Option<T>` an implicit `None`, and Swift's synthesized `Codable`
   does the same via `decodeIfPresent`. Non-`Option` fields *are* genuinely
   required. Consequence: no fixture "breaks" when `ci_observation` lands; an
   omitted field decodes as `None` in both languages and pins nothing. The
   behavioral no-default guard therefore only works for
   `CiObservationSnapshot`'s four non-`Option` fields, and the presence of
   `ci_observation` itself must be guarded on the raw fixture JSON instead.
   Worth knowing repo-wide: the DTO rule's "no defaults on wire types" cannot be
   enforced by the type system for `Option` fields — only by a fixture guard.

4. **Infra-blocked pins today's behavior, not the desired one.** Without W2-231:
   `gh` gone → PR read `Degraded`, `observe_required_checks` → `None`, and
   reconcile leaves the prior observation standing. The `Blocked` assertion is
   gated on W2-231 landing.

## Open

1. **A gh outage leaves the last failing reading wake-warranted.** With `gh`
   gone the head can't move, so `fresh_ci()` still returns the failing reading
   and `wake_warranted()` stays true — a wake can fire while GitHub is
   unreachable. Defensible (the failure is real and unrepaired) and arguably
   the point of W2-231's `Blocked` transition. This proof **asserts today's
   behavior** rather than quietly assuming it is correct. Flagging for W2-231's
   owner: if the intent is that an outage suppresses the wake, that is a
   behavior change, not a test fix.

2. **`cargo test` was red inside a Task Session because `EnvGuard`'s clear-list
   was one var short — fixed here (1 line).** Not a question; a resolved finding,
   recorded because this Project has been treating it as a known-unfixable
   gotcha ("green in CI, red in a Session; don't fix the code").

   The "don't fix the code" instinct was right about *production* code and wrong
   about the cause. `tests/support/mod.rs`'s `AMBIENT_TASK_ENV` cleared
   `LF_TASK_SESSION_ID`, `LF_TASK_GENERATION`, `LF_TASK_LEASE_TOKEN` — but not
   `LF_WAVE_ID`, which `ops/task.rs:395` reads to decide Wave control. So
   `EnvGuard` was already doing this job and simply missed a var:

   ```
   Wave 6155f18a… cannot control Task INF-123 owned by Wave 4ca22205…
        ^ ambient LF_WAVE_ID from my Session      ^ the test's own temp Wave
   ```

   Adding `"LF_WAVE_ID"` to that array turns both long-red tests green *inside a
   live Session with every ambient var still set*. Verified: all 11 support-using
   suites (85 tests) pass in-Session, and in CI the vars are unset so clearing
   them is a no-op — zero CI risk. `handoff_tests.rs:22` had already patched the
   same leak ad hoc with `.env_remove("LF_WAVE_ID")` on its subprocess.

   To reproduce CI exactly (still useful for the vars `EnvGuard` doesn't own):

   ```bash
   env -u LF_WAVE_ID -u LF_TASK_SESSION_ID -u LF_TASK_GENERATION -u LF_TASK_LEASE_TOKEN \
       -u LF_RUN_ID -u LF_PROCESS_ID -u LF_WAVE_HOME -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH \
     cargo test -p loopflow
   ```

3. **`lf pr publish` can loop forever on a scratch-only branch — worth filing
   under Developer Efficiency.** Hit live during this kickoff (2026-07-16).
   A branch whose only authored commit touches `scratch/` classifies as
   `generated_only` → `reset_to_base`:

   ```
   $ lf rebase --plan          # with 1 authored commit containing the design
   class: generated_only    strategy: reset_to_base    unique_commits: 1
   ```

   `lf pr publish` auto-rebases when behind, so it resets the branch, discards
   the authored commit, restores `scratch/` from the stash as *untracked* files,
   and then refuses: "Task worktree still has uncommitted changes; commit them
   before publishing the PR." Committing and retrying re-enters the same loop.
   Observed exactly once here; escaped only because the branch happened to be at
   main's tip on the retry, so no rebase fired.

   No data is lost (the scratch stash restores content), but the commit is, and
   the loop is unbreakable while the branch is behind. This bites the kickoff
   phase specifically, whose entire deliverable *is* `scratch/`. It contradicts
   the project KR "a week of normal development … requires zero manual git
   surgery."

   The classifier's premise — scratch-only branches are disposable — is right for
   a *checkpoint* commit and wrong for an *authored design*, and it can't
   currently tell them apart. Candidate fixes: don't reset a branch whose commits
   are authored (non-checkpoint) even if scratch-only; or have `pr publish` skip
   the disposability reset when the branch has an open PR.

   **The mechanism, confirmed by accident.** Once this branch carried a one-line
   change to `tests/support/mod.rs`, the same branch re-classified:

   ```
   class: generated_only  strategy: reset_to_base   # 1 authored commit, scratch/ only
   class: clean_authored  strategy: direct_rebase   # + 1 line of Rust
   ```

   So the verdict keys on **which files the commits touch**, not on whether a
   human authored them. A design doc is disposable; the same doc plus one line of
   Rust is not. That is the whole bug in two lines, and it suggests the narrow
   fix: authorship, not file path, should decide disposability.

   **It escalated on the second occurrence.** Publishing again while 1 commit
   behind reset the branch to `origin/main`, which left the *published* PR #1008
   with no commits — so `lf` abandoned it, closed it on GitHub, and detached the
   Task (`branch: none`, `PR 1: abandoned`). Publish then refused with "Task
   W2-229 has no active PR", which is unrecoverable through `lf pr publish`
   alone. So the failure is not only "the commit is discarded" but "a published
   PR is closed and the Task loses its branch."

   Recovery that worked, for the next person:

   ```bash
   cp scratch/*.md ~/.lf/tmp/backup/     # rotation does NOT carry committed work
   lf pr next <slug>                     # rotate to a fresh serial PR + branch
   cp ~/.lf/tmp/backup/*.md scratch/     # restore, then commit and publish at tip
   ```

   Two traps inside the recovery: `lf pr next` starts the new branch from a
   **clean tree**, so a *committed* design does not travel (its help says it
   carries "preserved follow-up edits" — that means uncommitted ones); and
   `lf pr next <slug>` concatenates the slug onto the existing branch name,
   yielding `jack-heart/prove-failed-pr-ci-fix-prove-failed-pr-ci-fix`.

   Publishing only ever succeeded when the branch was exactly at `origin/main`'s
   tip. The workaround is `git fetch && git rebase origin/main` (plain git
   replays the commit instead of resetting it) followed immediately by
   `lf pr publish` — i.e. precisely the manual git surgery the KR says should
   never be necessary.

3. **Two `write_gh_script` helpers already diverge** (`pr_tests.rs:13` is
   permissive and `exit 0`s on unknown calls; `release_tests.rs:13` fails loudly),
   and this proof adds a third fake in-crate. The in-crate one cannot reuse
   either — they are per-test-binary modules. Unifying them into
   `loopflow-test-support` is the right end state but touches many test files;
   out of scope here, worth filing.
