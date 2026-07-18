# Architecture control-spine review

## What was implemented

- Added the durable `Work → Epoch → Run → Launch → optional Turn` spine, one
  monotonic Basis per Epoch, typed Waits, stable Home identity, and one opaque
  `LF_RUN_LEASE` that resolves the exact active Run without caller-supplied
  Work or Session identity.
- Made Steer the authored-direction record. User and active parent Runs append
  through the same Basis-fenced mutation; immutable Send attempts record live
  transport outcomes without claiming incorporation.
- Deleted stored `InteractionReview`, `InteractiveHandoff`, and `ChildCommand`
  aggregates. Review now derives from the interactive flow position, a live
  Launch, its route, and nullable pending attention.
- Completed the Review turn-taking handshake: a parent Steer parks pending
  attention without closing Review, the child's next terminal Turn re-arms it
  once and advances the parent's evidence Basis, and close advances the flow
  under its existing fence.
- Gave Wave and Project the same oldest-child-first control projection. Live
  delivery interrupts the repurposed background body; seed-only delivery
  preserves the saved playhead and services child attention before more
  background work.
- Made Turn the sole additive usage grain and persisted root assistant output
  for reconstruction and parent control. `usage`, `top`, `runs`, `doctor`, JSON,
  and Swift read the normalized Turn path.
- Replaced provider-wide steerability with the exact active Turn's
  `Sent | NotSteerable | Failed | Unknown` outcome and added public `lf work`
  and `lf launch` surfaces.
- Added `lf prompt`, focused prompt methods, and the short universal Evidence
  Loop without injecting the full authoring guide into every execution.

## Key choices

1. **Authority is a capability, not ambient identity.** Agent mutations require
   the exact active Run lease. Missing or stale credentials fail closed; only
   authenticated external entrypoints construct User authority.
2. **Transport acceptance is not application.** A live provider response can
   reduce latency, but only a later successful boundary Basis proves that a
   Steer was honored.
3. **Review is an interval; attention is one unanswered turn.** Keeping the
   route while clearing `attention_at` prevents duplicate parent delivery
   without inventing a Review row or conversation ledger.
4. **Reconstruction reads durable facts.** Work truth, Steers, flow position,
   workspace and external evidence are authoritative. Provider transcripts and
   continuation tokens are optional Launch hints.
5. **Do not mask the remaining controller split.** The gate deliberately did
   not restore a Session interrupt fallback when a mirrored Run has no product
   Launch. That red test is evidence of the unfinished authority cut, not a
   fixture inconvenience.

The Mitchell-style review changed the branch in four concrete places:

- user docs and builtin skills no longer teach deleted handoff, review,
  directive-receipt, acknowledgment, or decision commands; a regression test
  scans every embedded builtin skill for those retired surfaces;
- integration and smoke tests clear inherited Run and legacy Session authority
  and use isolated homes, so an agent's real development ledger cannot decide
  test behavior;
- successor tests now prove that predecessor and successor Session-era ids map
  to one stable Work and that only the current Epoch enters a boundary seed;
- the Wave-resolution matrix now supplies the missing `--channel` value instead
  of mistaking a clap error for stale-identity behavior.

## How it fits together

```text
authenticated User / active parent Run
                 │
                 ▼
          Steer + Basis revision ──────► Send(exact Turn)
                 │                         │
                 └──── next seed ◄────────┘ transport outcome only

Work ─► Epoch ─► Run ─► Launch ─► optional Turn ─► usage + root output
                   └──► typed Wait

interactive flow + live Launch + route + attention_at ─► derived Review
```

Stable Work owns identity and input history. Run owns execution authority;
Launch owns provider/process continuity; Turn owns observed exchange and spend.
Wave and Project query the same child-attention facts before advancing their
own background flow.

## Risks and bottlenecks

### Blocking before this architecture can land

- **Task and Project still execute through the Session/body controller.** Their
  runners, stores, statuses, generations, `ChildWriteLease`, and legacy env
  credentials remain load-bearing. The normalized Run currently mirrors those
  bodies instead of conducting them.
- The consequence is pinned by
  `task_github_cache_tests::rest_failure_opens_one_durable_circuit_while_local_controls_continue`:
  the fixture has a live legacy Task body and mirrored active Run but no product
  Launch, so direct Run interrupt returns `Query returned no rows`. The correct
  fix is to make every Task/Project executor a Run Launch, not to add a fallback.
- Migrations `0.11.029`–`0.11.035` are still registered as canonical ordinals.
  Main documents dependency-ordered drafts, but the Rust `DRAFTS` registry does
  not exist. The final controller/schema cut must reconcile this once rather
  than create a second migration ledger.
- Rust source is 122,944 Tokei code lines, 1,125 above the 121,819 acceptance
  ceiling. Nineteen production files still contain 567 references to
  `ProjectSessionStatus`, `TaskSessionStatus`, or `ChildWriteLease`; deleting
  that duplicate controller is the intended reduction.

### Non-blocking follow-ons

- OpenCode Task/Project launches still do not deliver usage into captured
  Turns. Absence is now reported honestly rather than replaced with zero.
- Persisting every assistant delta keeps partial output crash-visible but
  rewrites a growing Turn row. Batch only after preserving partial terminal
  evidence.
- Credentialed provider smoke tests were not run during this headless gate.

## What's not included

- Session-free Task and Project `reserve | advance | stop` execution;
- keeper recovery through that shared controller and the final schema drop;
- the final draft-migration cutover;
- OpenCode's single end-to-end usage parser/producer path;
- transcript-free recovery drills across every credentialed provider;
- hosted Mac UI behavior (`--ui-host` remains a separate host gate).

## Validation

Full gate command: `uv run python scripts/test.py --all`.

| Suite | Result |
| --- | --- |
| Python | 112 passed |
| Rust fmt / clippy | passed, warnings denied |
| Rust, no fail-fast | 1,814 passed, 1 architecture blocker failed, 2 skipped |
| Website | 66 passed, 3 skipped |
| Swift package | 191 passed; multiplatform boundary check passed |
| E2E smoke | passed with an isolated Loopflow home |
| Mac app build-for-testing | passed |
| Hosted UI | not run; separate host gate |

Additional checks:

- `InteractionReview`, `InteractiveHandoff`, `ChildCommand`, `AwaitingHuman`,
  and `Author::Human` have zero production Rust references.
- Focused Review handshake, Wave live/seed scheduling, successor resolution,
  land isolation, builtin prompt guard, and Wave resolution tests pass.
- `git diff --check` passes.

Disposition: **not ready to submit or land**. The branch has one precise red
execution-path proof and has not met the controller-deletion, migration, or
line-count acceptance criteria.
