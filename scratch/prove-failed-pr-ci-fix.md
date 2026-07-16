# Prove failed-PR ci-fix recovery as one deterministic lifecycle

## Problem

A Task sleeping on an open PR should wake into exactly one ci-fix turn when a
required check fails, repair the same PR, push, rearm on the new head, and
return to waiting — automatically, idempotently, and restart-safe. The wake
machinery exists (PR #916 truthful observation, #967 narrow bounded wake), but
no test drives the **full lifecycle** end-to-end as one deterministic state
machine. Each piece has unit tests; the chain does not.

Today the proof is split across three places that never meet:

- `task/mod.rs:1372-1470` — pure unit tests of `wake_warranted` / `fresh_ci`, no store, no gh.
- `ops/child.rs:1246` — `ci_fix_wake_refuses_an_open_pr_without_a_warranted_failure`, a store-level in-crate test with a hand-written `CiObservation`, no gh.
- `tests/task_github_cache_tests.rs` — real fake-`gh` reconcile, but its `pr checks` **always returns `[]`** (green). No test has ever driven a failing required check through reconcile.

So the one transition the feature exists for — a red check becoming an armed
wake — has no coverage at any layer.

W2-156 (the parent product task) stays open until this proof and its three
sibling prerequisites (W2-230 audited ledger, W2-231 infra-blocked, W2-232
bounded settlement — all three confirmed still open) hold. This task delivers
the behavioral proof against what is on main today.

Who benefits: maintainers who need to trust that a red PR heals itself without
duplication, without human intervention, and without losing state across
restarts.

Why now: the wake code is on main (#967) but has no integration proof. The
longer the gap between "shipped" and "proven," the harder a regression is to
diagnose.

## The demo

```bash
cargo test --lib ci_fix_lifecycle    # the lifecycle state machine
cargo test --lib ci_observation      # the wire contract
```

A deterministic sequence prints green, driving one fake PR from pending to
green through a real SQLite store and a fake `gh` on `PATH`:

1. Required checks **pending** → reconcile stores `CiState::Pending`; `arm_ci_fix_wake` returns false.
2. Checks go **failing** → reconcile stores `CiState::Failing` with the failing leaf names; `wake_warranted()` is true.
3. `arm_ci_fix_wake` → **true exactly once**; a second call returns false (the `mark_woken` stamp).
4. Reconcile again on the same head with the same failing set → still no wake (the dedup marker carries forward).
5. The fake PR gets a **new head** (the simulated push) → the old reading goes stale (`fresh_ci()` → `None`), reconcile takes a fresh reading and re-arms.
6. The new head is **green** → `CiState::Passing`, no wake, the Task is back to waiting.
7. **Infra-blocked**: `gh` gone → the PR read degrades, no new CI reading is invented, no false green, local control survives.

## Approach

**The proof lives in-crate, not in `tests/`.** This is the one structural
change from the prior draft, and it is forced: every function the lifecycle
needs is private or `pub(crate)`, so an integration test under `tests/` — which
links `loopflow` as an external consumer — can reach none of them.

| Item | Visibility | Reachable from `tests/`? |
|---|---|---|
| `arm_ci_fix_wake` (`task/runner.rs:1823`) | private | no |
| `reconcile_task_pr_with_authority` (`ops/task.rs:2337`) | private | no |
| `observe_required_checks` (`ops/task.rs:2262`) | private | no |
| `read_check_set` (`ops/pr.rs:526`) | private | no |
| `reconcile_task_pr_for_lease` (`ops/task.rs:2249`) | `pub(crate)` | no |
| `wake_task_ci_fix` (`ops/child.rs:485`) | `pub(crate)` | no |
| `ChildWriteLease` (`child_session.rs:138`) | `pub(crate)` | no |

Making that surface `pub` purely so a test can call it is exactly what
CLAUDE.md forbids ("Never reshape production code for tests"). The in-crate
placement needs **zero visibility changes** and follows the pattern this area
already uses — `ci_fix_wake_refuses_an_open_pr_without_a_warranted_failure` is
an in-crate `#[tokio::test]` on a real tempdir store.

### Where the module goes

```rust
// rust/loopflow/src/task/runner.rs
#[cfg(test)]
mod ci_fix_lifecycle_tests;     // -> src/task/runner/ci_fix_lifecycle_tests.rs
```

A child module of `task::runner` sees `super::arm_ci_fix_wake` (private items
are visible to descendants) and `crate::ops::task::reconcile_task_pr_for_lease`
(`pub(crate)`). `tempfile` is a normal dependency; `loopflow-test-support` is a
dev-dependency of the `loopflow` crate itself, so `TestRepo` is available to
in-crate unit tests. Per CLAUDE.md, explicit imports — no `use super::*`.

### What each phase actually calls

The lifecycle is driven by two real functions plus one fake-`gh` state change.
Nothing is mocked; the store is real SQLite in a `TempDir`.

| Phase | Test does | Production code under test |
|---|---|---|
| observe | write the fake's state files, call `reconcile_task_pr_for_lease` | `observe_pr_by_number` → `merge_gate_state` → `read_check_set` → `observe_required_checks` |
| arm | call `arm_ci_fix_wake` | `fresh_ci` → `wake_warranted` → `mark_woken` → `update_task_pr_for_lease` |
| push | flip `head.sha` in the fake's PR state, reconcile | `fresh_ci` staleness rule |
| restart | re-call `reconcile_task_pr_for_lease` at each checkpoint | `observe_required_checks` dedup carry-forward |

### The lifecycle state machine

```
                    reconcile (fake gh: pending on h1)
Waiting ───────────────────────────────────────────► Waiting
  │   ci_observation = Pending(h1), arm_ci_fix_wake = false
  │
  │   reconcile (fake gh: failing[fmt] on h1)
  ├──────────────────────────────────────────────────► Waiting (warranted)
  │   ci_observation = Failing(h1, [fmt]), wake_warranted = true
  │
  │   arm_ci_fix_wake  ── the body's startup stamp
  ├──────────────────────────────────────────────────► Starting (ONE body)
  │   woken_failure_set = [fmt]  → returns true
  │
  │   arm_ci_fix_wake again (duplicate delivery)
  ├──────────────────────────────────────────────────► Starting (no second body)
  │   wake_warranted = false     → returns false
  │
  │   reconcile again, same head, same failing set (restart)
  ├──────────────────────────────────────────────────► Starting (still one)
  │   woken_failure_set carried forward → arm = false
  │
  │   push: fake gh head.sha h1 → h2; reconcile
  ├──────────────────────────────────────────────────► rearmed on h2
  │   old reading stale (fresh_ci = None), fresh reading taken
  │
  │   reconcile (fake gh: passing on h2)
  └──────────────────────────────────────────────────► Waiting
      ci_observation = Passing(h2), arm_ci_fix_wake = false
```

### Fake `gh` with scripted state files

One script on `PATH`, reading state files the test rewrites between phases.
This is the natural hook: `read_check_set` **deliberately ignores exit status**
(`pr checks` exits non-zero while red), so only stdout matters, and
`task_github_cache_tests.rs:48` already proves the `api` + `pr checks` dispatch
shape — it just always answers `[]`.

```sh
#!/bin/sh
[ "$1" = "--version" ] && { echo "gh version 2.0.0"; exit 0; }
case "$1 $2" in
  'api')       cat "$LF_TEST_GH_DIR/pr.json"; exit 0;;      # repos/{nwo}/pulls/{n}
  'pr checks') for a in "$@"; do [ "$a" = "--required" ] && \
                 { cat "$LF_TEST_GH_DIR/required.json"; exit 0; }; done
               cat "$LF_TEST_GH_DIR/full.json"; exit 0;;
esac
echo "unexpected gh invocation: $@" >&2; exit 1
```

Three refinements over the prior draft:

- **No `jq`.** The fake `cat`s pre-rendered files the test writes with `serde_json`. A fake that shells out to `jq` fails on a host without it, breaking "runs in normal CI."
- **Fail loudly on unexpected invocations**, like `release_tests.rs:13`, not `exit 0` like `pr_tests.rs:13`. A silent `exit 0` lets a wrong `gh` call parse as empty JSON and read as **green** — the exact failure this proof exists to catch.
- **Honor `--required`, to exercise leaf-dropping.** `read_check_set(repo, branch, required)` is called twice per reading. `MergeGateReading::from_checks` (`pr.rs:551`) reads the gate (failing/pending) from the **required** set, but seeds `failing_leaves` from the **full** set *minus the required names* — dropping the required aggregate in favour of the actual broken jobs, with fallbacks (`full_failing`, then `gate.failing_checks`) so a real gate failure never yields an empty seed.

  This mirrors production exactly: in this repo the only merge-gating check is the **`tests-result` roll-up**; the leaf jobs aren't required. An observer that read only the required set would seed ci-fix with "tests-result failed" — the roll-up's link, not the broken job. The full-set read is what makes the seed actionable.

  So the fake serves two different lists:

  ```
  --required : [ {tests-result, fail} ]                         -> gate: failing
  (full)     : [ {tests-result, fail}, {cargo-fmt, fail},       -> leaves: [cargo-fmt, clippy]
                 {clippy, fail}, {docs, pending} ]                 (aggregate dropped)
  ```

  The proof asserts `failing_checks == [cargo-fmt, clippy]` — that the aggregate is **absent**. That's the assertion with teeth.

### The `pushurl` trick for `github_repo_nwo`

`observe_pr_by_number` resolves owner/repo from `remote.origin.url`, and
`github_repo_nwo` (`engine/worktrees.rs:269`) only parses `github.com` URLs.
`TestRepo`'s origin is a local bare path, so it returns `None` and the whole
read degrades before CI is ever consulted.

The existing idiom (`pr_tests.rs:75`, `point_origin_at_github`) does `git remote
set-url origin https://github.com/loopflowstudio/loopflow.git`, which
**destroys push**. This proof never pushes (the "push" is a fake-`gh` head
change), so that idiom suffices and is what we use. Recorded for the next
person who needs both: setting `remote.origin.pushurl` to the bare path while
`remote.origin.url` reads as GitHub keeps both working — verified locally:

```
url:  https://github.com/test/repo.git      # github_repo_nwo parses test/repo
push OK; bare has: b8d0a8e... refs/heads/main
```

## Wire surfacing: `CiObservation` on `PrSnapshot`

The task requires "CiObservation wire fields are explicit in Rust, fixtures,
and mirrored consumers; no serde default hides schema drift." Today that is
**unmet, but not for the reason the prior draft gave**. CI already reaches the
wire — lossily, as prose:

```rust
// waves.rs:1727
fn ci_failure_reason(ci: &CiObservation) -> String {
    format!("required checks failed: {}", names.join(", "))   // -> NextMove.reason
}
```

A consumer wanting "which checks are red" must **parse an English sentence**.
`PrSnapshot` (`waves.rs:372`) carries no CI field and no `head_sha`; Swift's
`GithubPrSnapshot` is only `{ number, url }`. So the fix is to carry the
structure, not to introduce CI to the wire for the first time.

- Add `ci_observation: Option<CiObservationSnapshot>` to `PrSnapshot`.
- `CiObservationSnapshot` is **wire-only**: `{ head_sha, state, failing_checks, observed_at }`. It **excludes** `woken_failure_set` — the internal dedup marker, and the one field carrying `#[serde(default)]` (`task/mod.rs:307`, a stand-in for a migration on the JSON column). Excluding it keeps that default off the wire while the storage type keeps it.
- `PrSnapshot::new` (`waves.rs:400`) maps `TaskPr.ci_observation` → `CiObservationSnapshot`.
- `NextMove`/`ci_failure_reason` stay as-is — prose is fine *as a rendered summary*; it is only wrong as the *only* representation.

### The "no serde default" guard — two layers, because serde is asymmetric

Attributes can't be reflected on, so the guard asserts the **observable
consequence** instead. But that consequence differs by field shape, and getting
this wrong is how the drift gets in. Measured, not assumed:

| Field shape | Key absent | Can serde require the key? |
|---|---|---|
| `head_sha: String`, `state: CiState`, `failing_checks: Vec<CiCheck>`, `observed_at: OffsetDateTime` | **parse error** | yes, inherently |
| `ci_observation: Option<CiObservationSnapshot>` | **silently `None`** | **no** |

**serde gives every `Option<T>` an implicit `None` default** — `#[serde(default)]`
is not required for a missing key to decode as `None`. Swift's synthesized
`Codable` does the same via `decodeIfPresent`. So a fixture that simply omits
`ci_observation` decodes clean in *both* languages and pins nothing: precisely
the silent split-brain the DTO rule exists to kill.

That splits the guard in two:

**Layer 1 — the snapshot's own fields (serde enforces it).** This is the test
that catches someone adding `#[serde(default)]`, however they spell it:

```rust
#[test]
fn ci_observation_snapshot_requires_every_field() {
    for missing in ["head_sha", "state", "failing_checks", "observed_at"] {
        // the full object minus one key must fail to parse
        assert!(serde_json::from_value::<CiObservationSnapshot>(without(missing)).is_err());
    }
}
```

**Layer 2 — the `ci_observation` key itself (only the fixture can enforce it).**
No serde attribute makes an `Option` key mandatory, so the guard moves to the
pinned wire contract: parse the fixture as raw `serde_json::Value` and assert
every `active_pr` object *contains* the key.

```rust
#[test]
fn every_fixture_pr_snapshot_states_its_ci_observation() {
    for pr in active_prs_in(FIXTURE) {           // raw Value, not PrSnapshot
        assert!(pr.as_object().unwrap().contains_key("ci_observation"),
                "a PrSnapshot fixture must state ci_observation explicitly, null or not");
    }
}
```

Without layer 2, "add `ci_observation` to the wire" and "forget it in the
fixture" are indistinguishable — both green. This is the test the task's "no
serde default hides schema drift" requirement is actually asking for.

### Fixtures that must change

Every fixture carrying a `PrSnapshot` gains an explicit `"ci_observation": null`.
Note this is **not** required to make the decode pass (serde would default it to
`None`) — it is required so the fixture *pins the field*, enforced by layer 2
above. Surveyed — exactly two, plus one false positive:

| Fixture | Path | Action |
|---|---|---|
| `task_attention_states.json` | `dead_authored_commits/active_pr` | add `"ci_observation": null` |
| `task_attention_states.json` | *new* `ci_failing` scenario | populated `ci_observation`, `next_move.owner: "ci"` |
| `roadmap_snapshot.json` | `waves[0]/projects/items[0]/tasks[1]/active_pr` | add `"ci_observation": null` |
| `wave_detail.json` | `projects[0]/tasks[0]/active_pr` | **no change** — it's a PR *id string*, a different DTO |

`shared_attention_fixture_pins_every_desktop_state` (`waves.rs:2926`) asserts
`tasks.len() == 8` → becomes 9. Swift's `WorkAttentionTests.swift:90` and
`WaveLensTests.swift:177` decode the same file, so the Swift mirror picks the
scenario up automatically once `CiObservationSnapshot` exists in
`swift/Loopflow/Models/WaveWorkMap.swift:325`.

## De-risking

Every finding below was verified against this tree at **`895c5cd1d`** — this
branch's actual base — not assumed.

Re-confirmed at that base after review flagged the doc citing `42cd883cd`, a
commit main has since moved past (#1006, #1002, #999). The findings all held;
**five line citations had drifted** and are corrected above
(`reconcile_task_pr_with_authority` 2247 → **2337**, `observe_required_checks`
2172 → **2262**, `reconcile_task_pr_for_lease` 2158 → **2249**, the dedup window
2206-2212 → **2296-2302**, `pr_tests.rs` 222 → **248**). The re-check also
surfaced a substantive miss — `ops/pr.rs:1243` already asserts the
aggregate-dropping behaviour this design had promoted to its headline test
(decision 6). Verifying at the wrong commit is not a cosmetic error.

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Can an integration test in `tests/` drive the lifecycle? | **No — this killed the prior design.** `arm_ci_fix_wake`, `reconcile_task_pr_with_authority`, `observe_required_checks`, `read_check_set` are private; `reconcile_task_pr_for_lease`, `wake_task_ci_fix`, `ChildWriteLease` are `pub(crate)`. A `tests/` binary links the crate externally and sees none of them. | The proof moves in-crate to `src/task/runner/ci_fix_lifecycle_tests.rs`. Zero visibility changes, zero production reshaping. |
| Is there any `pub` route to reconcile? | Yes — `ops::task::task_status` (used by `pr_tests.rs:248`). But it reconciles by Linear identifier and gives no access to arming. | Insufficient alone. The observe half is publicly reachable; the dedup stamp is not. In-crate covers both. |
| Can an in-crate test reach `arm_ci_fix_wake`? | Yes, if the module is a **descendant of `task::runner`**. Private items are visible to descendant modules. A `#[cfg(test)] mod` in `ops/task.rs` could not. | Module placement is load-bearing: `src/task/runner/ci_fix_lifecycle_tests.rs`, declared from `runner.rs`. |
| Are `TestRepo` / `tempfile` available in-crate? | Yes. `loopflow-test-support` is a dev-dependency of `loopflow` (`Cargo.toml:79`); `tempfile` is a **normal** dependency (`Cargo.toml:76`). | No new dependency. |
| Is `tests/support/`'s `EnvGuard` reachable in-crate? | No — `tests/support/mod.rs` compiles into each test binary, not the lib. | The in-crate test needs its own guard. **Correction (review):** I wrote that it "does not need `EnvGuard`'s job" because the test builds its store via `open_store(tempdir)`. That was wrong, and measurably so — see the row below. The guard must do `EnvGuard`'s job, so it is a near-copy, not a smaller primitive. |
| Does the ambient Session env reach this test? | **Yes, and it fails the test.** Reproduced, not reasoned: `cargo test --test task_github_cache_tests` inside this Session panics `Wave 6155f18a… cannot control Task INF-123 owned by Wave 4ca22205…`; the same command with the `LF_*` vars cleared passes. The ambient ids reach **production** code on the exact paths this proof drives — `ops/task.rs:395` reads `WAVE_ID_ENV`, and `resolve_task_authority` (`ops/task.rs:1131`) reads `LF_TASK_SESSION_ID`. The Task runner exports all ten; CI exports none. | The guard clears the ambient vars, and the doc no longer calls it PATH-only. The trap here is the *obvious* fix: making production code satisfy the test would be reshaping production around a test-environment artifact, in reverse. Nothing is wrong with the code. |
| Why not just clear `PATH` and let the store isolate itself? | Because `resolve_task_authority` calls `open_registry_for_authority()`, which resolves the **global** registry from `LF_CONTROL_HOME`/`LF_CONTROL_DB_PATH`. Left ambient, an in-crate test driving authority reads — and could write — the developer's live control DB. | Decisive. The guard redirects the store home to a temp dir as well. This is exactly `EnvGuard`'s job, which is why the guard converges on it. |
| Can fake-`gh` serve a scripted fail→pass sequence? | Yes. `read_check_set` calls `gh pr checks <branch> [--required] --json name,bucket,link` and **ignores exit status** — only stdout parses. `GhCheck` fields are `#[serde(default)]` (a deliberately lenient CLI parser). | State-file fake works. |
| Must the fake distinguish `--required`? | **Yes — though not for the reason I first wrote.** I claimed a flag-ignoring fake yields empty `failing_checks`; traced `from_checks` (`pr.rs:551`) and it doesn't: `failing_leaves` falls back to `full_failing`, then to `gate.failing_checks`, so it's never empty on a real gate failure. The real reason is fidelity: this repo's only required check is the **`tests-result` aggregate**, and the full-set read exists to drop that roll-up in favour of the broken leaves. A single-list fake never exercises that path. | The fake serves distinct required/full lists, and the proof asserts the aggregate is **dropped** (`failing_checks == [cargo-fmt, clippy]`, no `tests-result`) — the assertion that actually has teeth. |
| Does `observe_pr_by_number` need a GitHub remote URL? | Yes. It runs `gh api repos/{owner}/{name}/pulls/{number}` and resolves nwo from `remote.origin.url`; `github_repo_nwo` only strips `git@github.com:` / `https://github.com/`. `TestRepo`'s origin is a local bare path → `Degraded`. | Reuse `point_origin_at_github`'s idiom. `pushurl` split verified as the alternative if a future test needs push too. |
| Does the wake path spawn a process? | `wake_task_ci_fix` → `child.launch(store, LaunchIntent::CiFix)` — yes. `arm_ci_fix_wake` — no; it is pure store I/O. | The proof calls `arm_ci_fix_wake`, never `wake_task_ci_fix`. The transitions are proven without a provider. |
| Does the dedup survive a reconcile (restart)? | Yes, and it's subtle: `observe_required_checks` (`ops/task.rs:2296-2302`) carries `prior.woken_failure_set` forward **only when** `prior.head_sha == new.head_sha` **and** `prior.woken_failure_set == Some(new.failure_set())`. Any head move or failing-set change re-arms. | Restart safety is a real assertion, not a tautology. The test drives reconcile *after* arming and asserts no re-arm — and asserts a *changed* failing set **does** re-arm. |
| What is infra-blocked today? | `gh` absent → `observe_pr_by_number` returns `Degraded { reason: "gh CLI not found" }` and reconcile returns on the degraded branch. `observe_required_checks` returns `None` when `merge_gate_state` is `None` (gh gone / no required checks / unparseable), and **reconcile only assigns when `Some`** — a `None` read leaves the prior observation standing. | Assertable today: degraded PR read, `ci_observation` **unchanged**, no invented green, local control intact. The W2-231 `Blocked` transition is gated. |
| Does a gh outage strand a stale wake? | Consequence of the above: with gh gone the head can't move, so the last failing reading stays fresh and *would* still warrant a wake. Defensible (it is a real, unrepaired failure) but a genuine behavioral claim. | Name it explicitly and assert it, rather than let it be discovered later. Changing it is W2-231's territory; this proof pins today's truth. |
| Is `CiObservation` on the wire today? | Not structurally. It is rendered to prose by `ci_failure_reason` (`waves.rs:1727`) into `NextMove.reason`. `PrSnapshot` has no CI field; Swift `GithubPrSnapshot` is `{number, url}` only. | The prior draft's "internal-only" claim was wrong. The design is unchanged in substance; the rationale becomes "carry structure, not prose." |
| Does a plain `Option<T>` (no `#[serde(default)]`) require its key? | **No — I assumed it did, and measured otherwise.** serde gives every `Option<T>` an implicit `None` default; `{"name":"a"}` decodes clean as `ci: None`. Swift's synthesized `Codable` does the same (`decodeIfPresent`). Non-`Option` fields *are* genuinely required — an absent `head_sha`/`state`/`failing_checks`/`observed_at` is a hard parse error. | Splits the guard in two. The behavioral no-default test works for `CiObservationSnapshot`'s four fields but **cannot** work for `PrSnapshot.ci_observation`. Layer 2 moves that guard to the fixture: assert the raw JSON object contains the key. |
| Which fixtures break when the field lands? | **None break** — a fixture omitting `ci_observation` silently decodes as `None` in both languages. That is the drift, not a safety net. Populated `PrSnapshot`s: `task_attention_states.json` (1) and `roadmap_snapshot.json` (1). `wave_detail.json`'s `active_pr` is a PR **id string** — a different DTO, unaffected. | Both get explicit `"ci_observation": null`, enforced by the layer-2 presence guard rather than by the decoder; fixture count assertion 8 → 9. |
| Is the branch behind main? | No. Base is now **`895c5cd1d`**, `0` behind, on PR 2's branch. Main moved twice during this kickoff (`42cd883cd` → `9613efbb2` → `895c5cd1d`), which is what stranded the doc's citations. | Prerequisite discharged — but re-check citations against the base at implementation time, not at design time. See `questions.md` for the `lf rebase`/`lf pr publish` trap that forced the branch rotation. |
| Are the three prerequisites landed? | No — W2-230, W2-231, W2-232 all confirmed open in Linear, ranked *below* W2-229. | Prove today's behavior; gate W2-231's assertion. Do not block on siblings. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Integration test in `tests/ci_fix_lifecycle_tests.rs` (**the prior design**) | Matches the repo's flat `<area>_tests.rs` convention; reuses `EnvGuard`/`register_task`. | **Not buildable.** Every function it must call is private or `pub(crate)`. Impossible without widening a swath of internals. |
| Widen `arm_ci_fix_wake` et al. to `pub`, keep the test in `tests/` | Preserves the file convention. | CLAUDE.md: "Never reshape production code for tests." Widening wake internals so an external test binary can see them is that rule's exact target, and it exports a surface no real consumer wants. |
| Drive it publicly via `task_status` only | Uses a real public entry point; no visibility questions. | Proves observe but not arming — and "exactly one body" *is* the arming stamp. Would prove the easy half and quietly skip the claim. |
| Full E2E shell test (like `tests/e2e/test_rebase_efficiency.sh`) | Exercises the real binary end to end. | Needs a real provider and real `gh`: not deterministic, doesn't run in normal CI. Violates the task's "local deterministic fakes" requirement. |
| Test only the pure functions (`wake_warranted`, `fresh_ci`, `mark_woken`) | Fast; already exists at `task/mod.rs:1372`. | Already done, and it's the status quo that left the gap. Doesn't prove the chain — and the untested link is precisely reconcile→failing→arm. |
| Mock the provider spawn to test `wake_task_ci_fix` | Covers the launch path too. | Requires a factory trait or cfg gate existing only for tests. The state-machine proof is stronger *and* cheaper: dedup is decided in `arm_ci_fix_wake`, before any process. |
| Put `CiObservation` on the wire as-is | One type, no mapping. | Ships `#[serde(default)] woken_failure_set` to the wire, violating the DTO rule and the task's own "no serde default" requirement. The marker is internal; no consumer wants it. |
| Extend `task_github_cache_tests.rs`'s fake to answer failing `pr checks` | Reuses an existing, working fake. | Right instinct, wrong layer — that file's tests reach reconcile via `task_status` and still can't arm. Worth doing *later* as a public-surface companion; it is not the proof. |

## Key decisions

1. **In-crate, because the alternative is unbuildable.** `src/task/runner/ci_fix_lifecycle_tests.rs`. This overrides the prior draft's `tests/` placement. It departs from the flat `tests/<area>_tests.rs` convention, and that is the correct trade: the convention serves tests of the *public* surface, and this lifecycle has none. The area's own precedent (`ops/child.rs:1246`) is already in-crate for the same reason.

2. **State-machine proof, not process proof.** Drive `reconcile_task_pr_for_lease` (observe) + `arm_ci_fix_wake` (arm) + fake-`gh` state changes (push). Never spawn a provider. "Exactly one body" is proven where the system actually decides it — the `mark_woken` stamp, which `runner.rs:118` documents as the idempotency point: *"Marking the observation woken here — before any body starts — makes the wake idempotent."* Testing the stamp tests the claim.

3. **`CiObservationSnapshot` as a separate wire type**, excluding `woken_failure_set`. Storage keeps its `#[serde(default)]` for the JSON column; the wire gets none. Mirrors the existing `TaskPr` → `PrSnapshot` storage→wire selection.

4. **The no-default guard is behavioral, and two-layered because serde is asymmetric.** Attributes aren't reflectable; consequences are. For `CiObservationSnapshot`'s four non-`Option` fields, an absent key is a parse error — that test catches any spelling of `#[serde(default)]`. For `PrSnapshot.ci_observation`, serde *cannot* require the key (every `Option<T>` defaults to `None` implicitly, in Rust and in Swift), so the guard moves to the fixture: assert the raw JSON states the key. Layer 2 is the one the task's "no serde default hides schema drift" clause actually needs — without it, omitting the field from the wire is indistinguishable from carrying it.

5. **The fake `gh` fails loudly** on unexpected invocations (`release_tests.rs` style, not `pr_tests.rs`'s `exit 0`). A permissive fake answers a wrong call with empty JSON, which reads as green — silently inverting the test's verdict.

6. **The fake honors `--required` — but the aggregate-dropping assertion is already written, so don't write it again.** Two corrections, in order:

   First: not because a single-list fake would go vacuous (it wouldn't — `from_checks` has fallbacks), but because this repo's only merge-gating check is the `tests-result` roll-up.

   Second, found while re-verifying at the real base: **`ops/pr.rs` already unit-tests exactly this**, landed by #967 alongside the feature — `merge_gate_seeds_actionable_leaves_not_the_required_aggregate` (`ops/pr.rs:1243`), `merge_gate_keeps_a_required_leaf_when_it_is_the_only_failure` (`:1273`), `merge_gate_falls_back_to_required_when_the_full_read_is_empty` (`:1289`). The first asserts `failing_leaves == ["rust-test"]`, that `tests-result` is absent, and that the seed carries the leaf's own job link. That was my "assertion with teeth" — already ringing, for months.

   So `MergeGateReading::from_checks` is *well* covered as a pure function, and this proof must not duplicate it. What those tests do **not** touch is `gh` itself: they construct `Vec<GhCheck>` directly, bypassing `read_check_set`'s parsing. Combined with the earlier finding that every fake `gh` in the suite returns `[]` for `pr checks`, the real gap is narrower and sharper than the doc claimed:

   - **covered:** classification logic (pure, 3 tests)
   - **uncovered:** `gh` output → `read_check_set` → `merge_gate_state` (no fake ever returns a failing check)
   - **uncovered:** failing gate → `arm_ci_fix_wake` → one body, deduped across reconcile

   The fake still serves distinct required/full lists — that is what makes it *realistic* — but the proof asserts the **lifecycle**, not the classification. Its check-name assertion exists only to prove the seed survives the `gh`→observation wiring, and should be one line, not the centrepiece.

7. **Infra-blocked pins today's truth, including the uncomfortable part.** `gh` gone → degraded read, observation untouched, no false green — *and* the last failing reading stays wake-warranted. Assert it rather than let W2-231 discover it.

8. **Rearm is `fresh_ci()` going stale.** A second wake on a *new* failing head is W2-232's bounded settlement, not this proof. This proves: head moves → old reading stale → fresh reading → green → waiting. The test additionally asserts that a **changed failing set on the same head re-arms**, since that is `observe_required_checks`'s carry-forward condition and is cheap to pin here.

9. **The env guard is not PATH-only, and is a near-copy of `EnvGuard` by force.** It must:

   - prepend the fake-`gh` temp bin dir to `PATH`;
   - **clear the ambient Session vars** `LF_WAVE_ID`, `LF_TASK_SESSION_ID`, `LF_TASK_GENERATION`, `LF_TASK_LEASE_TOKEN`, `LF_RUN_ID`, `LF_PROCESS_ID`;
   - **redirect the store home** — `LF_WAVE_HOME`, `LF_CONTROL_HOME`, `LF_CONTROL_DB_PATH`, `LF_HOME`, `LF_DB_PATH` — at a temp dir, so `open_registry_for_authority()` can never reach the developer's live control DB;
   - hold a process-wide `Mutex` (the lib test binary runs tests in threads, and env is process-global);
   - restore every previous value on `Drop`.

   That is `tests/support/mod.rs`'s `EnvGuard` almost exactly, including its `env_lock()` mutex and its temp-bin-dir fake-executable writer. It cannot be reused: `tests/support/mod.rs` compiles into each *integration test binary*, and this proof is in-crate. A `#[cfg(test)]` module in `src/` cannot see it, and a lib test binary is a separate process from every `tests/` binary, so even the mutex would not be shared. **The duplication is forced by Rust's test architecture, not chosen** — and CLAUDE.md's "keep one implementation" is about production code, while its testing section explicitly sanctions test-only modules. Say so in the module's header comment so the next reader doesn't try to DRY the two together.

   Precedent for the necessity, and for the exact var list: `EnvGuard`'s own `AMBIENT_TASK_ENV` now clears five (`LF_TASK_SESSION_ID`, `LF_TASK_GENERATION`, `LF_TASK_LEASE_TOKEN`, `LF_WAVE_ID`, `LF_PROJECT_SESSION_ID`) — `LF_WAVE_ID` and `LF_PROJECT_SESSION_ID` added by #1003 *during this kickoff*, which is why `cargo test` is now green inside a Session (see `questions.md`). `handoff_tests.rs:22` independently does `.env_remove("LF_WAVE_ID")` on its subprocess. Copy that list rather than re-deriving it, and re-check it at implementation time — it grew twice this month.

## Scope

- In scope:
  - `rust/loopflow/src/task/runner/ci_fix_lifecycle_tests.rs` — the deterministic lifecycle proof
  - `#[cfg(test)] mod ci_fix_lifecycle_tests;` declaration in `task/runner.rs`
  - An in-module state-file fake `gh` + an env guard that shims `PATH` **and clears the ambient Session env** (see below)
  - `CiObservationSnapshot` wire type + `PrSnapshot.ci_observation` + `PrSnapshot::new` mapping
  - Behavioral no-serde-default guard test
  - Fixtures: `task_attention_states.json` (null + new `ci_failing` scenario), `roadmap_snapshot.json` (null); fixture count 8 → 9
  - Swift `CiObservationSnapshot` mirror + decode assertion

- Out of scope:
  - W2-230 (audited command ledger) — alternative wake routing, unmerged
  - W2-231 (infra-blocked `Blocked` transition) — proof pins current behavior; the `Blocked` assertion is gated
  - W2-232 (bounded settlement) — second-wake-on-new-failing-head
  - The ci-fix skill's internal behavior (reproduce → fix → push) — that's the body, not the lifecycle
  - Refactoring `tests/support/`'s `EnvGuard` or unifying the two `write_gh_script` helpers — real duplication, but touching every test file is a separate change

## Done when

```bash
cargo test --lib ci_fix_lifecycle          # lifecycle state machine
cargo test --lib ci_observation_snapshot   # wire contract + no-default guard
cargo test --lib shared_attention_fixture  # fixture round-trip, 9 scenarios
cargo fmt --check && cargo clippy -- -D warnings
xcodebuild test -scheme Loopflow -only-testing:LoopflowTests/WorkAttentionTests
```

Observable outcomes:

- [ ] Pending → failing → armed once → duplicate refused → reconcile-after-arm refused → new head → green → waiting, in one test
- [ ] A changed failing set on the same head re-arms (carry-forward condition pinned)
- [ ] Restart (re-run reconcile) after observe, after arm, after push → same outcome
- [ ] Infra-blocked (`gh` absent) → degraded read, `ci_observation` unchanged, no false green (Blocked assertion gated on W2-231)
- [ ] `failing_checks` carries the leaf names parsed from the fake `gh`'s **full** set — proving the `gh` → `read_check_set` → `merge_gate_state` wiring, which no existing test drives. (The *classification* is already proven by `ops/pr.rs:1243`; do not restate it.)
- [ ] `PrSnapshot.ci_observation` populated and round-trips; both null-carrying fixtures updated
- [ ] Each `CiObservationSnapshot` field absent → parse error (layer 1)
- [ ] Every fixture `active_pr` object states `ci_observation` explicitly (layer 2 — the guard serde cannot give us)
- [ ] Swift mirror decodes `ci_observation` from the shared fixture

## Measure

| Metric | Baseline | Target |
|--------|----------|--------|
| Tests driving a **failing** required check through reconcile | **0** — every fake-`gh` `pr checks` in the suite returns `[]` | ≥1, the full lifecycle |
| Lifecycle integration proof | 0 (three disjoint unit-test islands) | 1 module, 6–8 tests, one state machine |
| `CiObservation` structural wire presence | prose only (`ci_failure_reason` → `NextMove.reason`) | typed `Option<CiObservationSnapshot>` + fixture + Swift mirror |
| `#[serde(default)]` on wire CI fields | n/a (not on wire) | 0, guarded behaviorally (layer 1) |
| Fixtures pinning `ci_observation`'s presence | n/a | every `active_pr`, guarded on raw JSON (layer 2) |
| Wake dedup proof | unit-level on `wake_warranted`/`mark_woken` | arm → re-arm → reconcile → assert one |

## Prerequisites

- **Rebase onto main** — **done.** Base `895c5cd1d`, 0 behind. Getting here was not free: `lf rebase` classifies a scratch-only branch as disposable and *resets* it, which discarded the design commit and closed PR #1008. The work now rides PR #1009 on a rotated branch. Full write-up and recovery in `questions.md`.
- **W2-231 (infra-blocked)** — the `Blocked` assertion is gated. The proof compiles and passes without it.
