# Open questions and assumptions

## Resolved during kickoff (2026-07-16)

The prior draft's three open questions are all answered against the tree at
**`7c4d4e965`** — this branch's real base. Kept here because each answer changed
the design. Their citations (`engine/worktrees.rs:269`, `pr_tests.rs:75`,
`runner.rs:1848`, `waves.rs:2926`, `task/mod.rs:307`) were re-checked at that
base and all still hold; the `ops/task.rs` ones did not, and are corrected below
and in the design's De-risking table.

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

4. **Corrected mid-kickoff: serde `Option<T>` does not require its key.** The
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

5. **Infra-blocked pins today's behavior, not the desired one.** Without W2-231:
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

2. **Nothing stops a *merged* PR from waking a ci-fix body except one line in
   reconcile. Flagging for W2-232's owner.** Found while writing the
   source-of-truth section; not a bug today, but a single-point defense.

   `wake_warranted` (`task/mod.rs:329`) tests only `state == CiState::Failing`
   and the dedup stamp. `arm_ci_fix_wake` is `fresh_ci().is_some_and(
   wake_warranted)`. **Neither consults `merge_commit` or `abandoned_at`.** On a
   merged PR the head does not move, so `fresh_ci()` would return the last
   failing reading as fresh and `wake_warranted()` would be true.

   The only thing preventing a wake on a merged PR is that
   `reconcile_task_pr_with_authority` sets `ci_observation = None` on the
   `merged` (`ops/task.rs:2427`) and `closed` (`:2470`) branches. That clear is
   load-bearing with no defense in depth: a refactor that drops it — or any path
   that arms without reconciling first — reintroduces a ci-fix body on a merged
   PR.

   Not asserted here: terminal settlement is W2-232's scope, and this proof's
   state machine deliberately ends at green→waiting. But it is cheap to pin
   (flip the fake's `pr.json` to `state: "merged"`, reconcile, assert
   `ci_observation` is `None` and `arm_ci_fix_wake` is false) and the fake this
   design specifies already has everything needed. Worth W2-232 taking, or worth
   a guard in `wake_warranted` itself — which would be the honest fix, since the
   invariant it encodes is "don't repair a PR that no longer exists."

3. **The "green in CI, red in a Session" gotcha is fixed — by #1003 on main, not
   by me. Retire the workaround.** Worth recording precisely, because this
   Project has carried it as a known-unfixable fact of life ("don't fix the code,
   it's the environment").

   The "don't fix the code" instinct was right about *production* code and wrong
   about the cause. `tests/support/mod.rs`'s `AMBIENT_TASK_ENV` cleared
   `LF_TASK_SESSION_ID`, `LF_TASK_GENERATION`, `LF_TASK_LEASE_TOKEN` — but not
   `LF_WAVE_ID`, which `resolve_child_command_source` (`ops/util.rs:63`, called
   from `ops/task.rs:397`) reads to classify who is issuing a command:

   ```
   Wave 6155f18a… cannot control Task INF-123 owned by Wave 4ca22205…
        ^ ambient LF_WAVE_ID from my Session      ^ the test's own temp Wave
   ```

   `EnvGuard` was already doing this job and simply missed a var. I reproduced
   the failure, fixed it by adding `LF_WAVE_ID`, and verified all 11
   support-using suites (85 tests) green in-Session — then a rebase revealed
   **#1003 had already landed the same fix on main**, with `LF_PROJECT_SESSION_ID`
   too (5 vars), while this kickoff was running. My commit was redundant and the
   rebase dropped it. Main's version is a superset; keep it.

   Two things follow. First, the workaround is obsolete: `cargo test -p loopflow`
   is green inside a Session on current main, so stop reaching for the `env -u`
   incantation and stop treating those two tests as expected-red. Second, this is
   the staleness trap from the review, live: the fix landed in the very commit
   this branch was one behind. I was fixing a bug that main had already fixed,
   and only noticed because the rebase dropped my patch.

   (#1003 left a spliced doc comment on that constant — `…store. Every EnvGuard`
   dangling into the next sentence. Repaired here, since it is the comment that
   explains the whole trap.)

   The `env -u` form, if a future var escapes `EnvGuard`'s list:

   ```bash
   env -u LF_WAVE_ID -u LF_TASK_SESSION_ID -u LF_TASK_GENERATION -u LF_TASK_LEASE_TOKEN \
       -u LF_RUN_ID -u LF_PROCESS_ID -u LF_WAVE_HOME -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH \
     cargo test -p loopflow
   ```

4. **`lf pr publish` can loop forever on a scratch-only branch — worth filing
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

5. **Two `write_gh_script` helpers already diverge** (`pr_tests.rs:13` is
   permissive and `exit 0`s on unknown calls; `release_tests.rs:13` fails loudly),
   and this proof adds a third fake in-crate. The in-crate one cannot reuse
   either — they are per-test-binary modules. Unifying them into
   `loopflow-test-support` is the right end state but touches many test files;
   out of scope here, worth filing.
