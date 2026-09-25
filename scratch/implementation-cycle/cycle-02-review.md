# Cycle 2 — review-slice: named native Sessions after continuation integration

2026-09-25. Disposition: **the slice advances the full design after one contract
correction and one added behavioral proof.** Compression's single `SessionTarget`
holds. Publication is withheld by the human's current direction (keep building
before the two-loop UI discussion). No commit, PM, install, live Session or
store action. The full design is not done: frame/Wave/Task/Flow/comments remain.

## Claim matrix

| Claim | Implemented | Proof (this review unless noted) | Result |
|---|---|---|---|
| Canonical Run-owned name: skill, else generator; human beats suggestion | `session-name.json` per Run, one `rename` writer under the Run-record lock | 7 CLI tests (`session_cli_tests`), lib race/golden tests (cycle 1, unchanged) | pass |
| Raw pre-history rename | `find_session` no longer requires provider history; `open`/`complete` read it via `provider_history` | `raw_sessions_are_named_by_a_stable_word_pair`, `session_names_are_shared_and_human_names_win` | pass |
| `$LF_RUN_ID` reaches the waiting boundary (rename, open, complete) | `find_session` resolves standalone id → boundary Run → interactive → Ask → managed Flow | Rename: existing CLI test. **Complete by Ask Run ID: new CLI assertions** (below) | pass |
| Name carry-over and locking through replacement Run | Boundary rename takes the launch lock and re-resolves via `boundary_id` | `boundary_names_follow_run_ids_and_replacement_runs` (rename while provider runs) | pass |
| Exact Flow membership, distinguishing current / earlier-in-active-run / past run | **Corrected here**: `occurrence: current \| earlier \| past` replaces `current: bool` | `flow_sessions_are_named_through_their_run_and_keep_exact_membership` now walks current → earlier → restart; DTO fixtures in Rust and Swift | pass after fix |
| Ask's own Run separate from caller ancestry | `prepare_ask_run`: `Independent`, `parent_run_id` = caller; `start_prepared` keeps prepared `flow` (`manifest.flow.or(...)`) | Source trace; `every_session_kind_requires_its_own_run_reference` | pass (source + fixture) |
| Missing evidence explicit | Old manifests → `unknown`; unreadable Flow → `unknown`; remote title → `unavailable` | Fixtures; remote rename refusal source-only | pass / remote gap |
| Complete-based human review before decision edges | Human reviews return feedback; loop-decide owns Advance/Iterate | 3 `flow_session` tests in the 21-test lib run | pass (simulated provider) |
| Native breadcrumb, rename, held navigation, retained surfaces | `WorkspaceBreadcrumbBar`, generation-guarded readback | Swift 18 tests / 3 suites incl. `namedSessionDrillDownRetainsTerminal` (real Ghostty, three `/bin/cat` PTYs) | pass (fixture registry, real PTYs) |
| Large headless Claude context | `engine/agent.rs`: fresh anonymous tempfile as stdin per attempt, auto only; other harnesses/interactive keep argv | Source read; parent's before/after stand-in receipts `/tmp/loo291-large-prompt-{before,after}.log` | pass (prior receipt) |
| Configured app, remote Home, frame/Wave/Task/Flow/comments | Not in this slice | — | gap |

## Findings

1. **Fixed — restarted invocations were labeled "earlier".** `SessionFlowMembership::Step`
   carried `current: bool`, and Swift rendered `false` as "· earlier" and "earlier Flow
   step". A Session from a *restarted* Task Flow (new invocation id) or a finished
   standalone Flow was indistinguishable from an earlier step of the still-active run.
   The existing controller test's "historical" case was in fact a restart. Replaced the
   boolean with `SessionFlowOccurrence { current, earlier, past }`, computed from exact
   invocation id + boundary key (Task: `RunFlowStep::occurrence`; standalone: finished →
   past). No defaults: Rust enum, Swift enum/decoder, breadcrumb help text
   ("earlier step of the Flow's current run" / "a Flow run that has since finished or
   restarted"), three fixtures, and a new `past` fixture record inserted before the
   final `unavailable` record. Swift label: `· past run`.
2. **Closed compress gap — complete by boundary Run ID.** Extended the real-CLI
   `boundary_names_follow_run_ids_and_replacement_runs`: `lf session complete <replacement
   Run>` fails with the shared "not marked this ready" Ask message (it is not treated as
   an interactive Session), and after readiness the same command sets the Ask record to
   `completed` with its summary. Two test assumptions were corrected along the way
   (listed state is `closed` after the provider exits; Ask completion legitimately
   resolves its native history), neither a product change. Pre-compression, the Run ID
   resolved interactive-first; the source trace implies the old behavior would fail the
   first assertion. That old binary was not rebuilt to demonstrate it.
3. **Retained, not a defect.** A *completed* retained Ask's Run ID falls past
   `find_boundary_for_run` (Waiting only) to the interactive branch, while
   `boundary_run_ids` hides it from `list`. Unchanged from before compression.
4. **Minor.** `LF_HUMAN_SESSION_RUN_BIND` survives only in `engine/process.rs`'s
   env-scrub list; nothing produces it. Harmless; delete it when the next slice
   touches that list.

Compression verdict: no ownership or lifetime change beyond the intended one. The
Interactive target lost its eagerly-read `provider_session`; open/complete re-read it.
That keeps their "no provider history yet" refusal. A boundary's Run ID now selects the
boundary for open/complete, as it already did for rename. `open --replace/--try` on
that Run ID now refuses (the "interactive only" message) instead of replacing the
boundary's client as if it were a plain interactive Session. That is intended.

## Negative searches

Zero reachable hits in `rust/loopflow/{src,tests}` and `swift/{Loopflow,LoopflowMac,LoopflowTests}`
for `find_named_session`, `named_surface`, `fn is_current(&self`, `publish_run_binding`,
`require_flow_decision`, `decideFlow`, `resolveFlowSession`. `NamedSession` survives only
as a Swift test actor (`NamedSessionSource`); `current: bool` only in unrelated CLI flags.
One `find_session`, one `session_surface`, one `rename` writer.

## Commands and receipts

Environment: all `LF_*` Home/Run/Task/Flow/Session variables unset.

- `cargo test -p loopflow --lib -- ops::human_session::tests ops::flow_session::tests flow_sessions_are_named_through_their_run_and_keep_exact_membership --test-threads=1`
  → **21 passed** (`/tmp/loo291-c2-review-lib.log`).
- `cargo test -p loopflow --test session_cli_tests` → **7 passed** after the two test-assumption
  corrections (`/tmp/loo291-c2-review-cli{,2}.log` failures; `cli3.log` pass).
  `--test dto_fixtures` → 8 passed (`/tmp/loo291-c2-review-cli.log`).
- `swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter 'DTOFixtureTests|SessionRenameTests|WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal'`
  → **18 tests / 3 suites passed** (`/tmp/loo291-c2-review-swift.log`).
- `cargo clippy --all-targets -- -D warnings` pass. `cargo fmt --all` reformatted two test
  arrays after the lib/CLI runs; `--check` then passes. Formatting only; not rerun.
- `flow_tests bound_flows_keep_task_context…` was not rerun: its code paths are unchanged
  here and the parent's integration receipt covers it.

Final SHA-256 (these supersede the compress receipt's hashes for the two Session files):

```text
human_session.rs            f5ecf36e8daf4745911a0c85ed87f32b7eff080228779acfbb3d0e09cfe77007
flow_session.rs             13217f5634abf6d996a4f475863cb664338cb871b949c0fc40db2e8021a65c6b
run_record.rs               0bc9ff70ac4a0bfb379059969b2b4d99da7dead1b03a9fecbd1f0da41ebfe938
controller/task/mod.rs      8672969fab9757f134ec4509c49c5bd61f87833d383316c139d65edf4e508a63
tests/session_cli_tests.rs  9a4acd7fdc887f7c6ee5366630742e6e9fa3f2608befde175153d8c9866cd654
SessionRecord.swift         dd741a09ecee7f839306e661c4b85543160eece4e74c09f1c61d484c91330b35
WorkspaceBreadcrumbBar.swift f7b09d0d5d0aae6607ba1a178a99adec3b7c2c19275752e41a5433a288c10665
SessionFixture.swift        d84df04af025894a08a9726a329c78499537e19cafe51d5e6e652eac1c4df8a5
DTOFixtureTests.swift       f88aa2b622d790c814110b20628ecac3a8430e30c68fc941b3948dbe02939952
session.json                57ba827e0630f71dcf105f97711960bbc2aac2e51be5e953d2c317e0d428242f
sessions.json               07677582834de347832618f2ed278258558e9f6496dc2dbb39636c6576a6d257
session_memberships.json    de131e3d472afcbd7d3a6c41cf6388567f71cd4a6289933c6248955eee299608
engine/agent.rs             10df06751b1f975d264ef85638df4878935b746b17d46c8466aaadfd16b72a99
```

Hashes are post-`cargo fmt` and were rechecked after writing this receipt. The fmt-only
reflow came after the test runs, so those runs cover the same code in its pre-fmt layout.

## Review limits

Read the complete working-tree Rust delta (`git diff`) and the committed
`0a4731f9d` `agent.rs` patch. Earlier cycle-2 Swift and Rust changes were not
re-read line by line; this review relies on their receipts and inspected them only
where the focus questions touched them. No configured app, vendor provider, remote
Home, or hosted UI run. The Codex turn-start rejection defect remains out of scope.
Historical `*-archive.txt` files and their `.md` indexes were left untouched.

## Remaining full-design gaps

- Remote-Home title and rename: source only. The canonical remote name still needs a routed read.
- Rename keyboard focus and Escape are proven only through model and ViewInspector tests, not configured keys.
- Flow Session `detail` still equals the step skill.
- Frame A, Wave page, Task situations, the pinned Flow diagram with both Feature return
  edges and the queue → land tail, comments, New session beside the title. Two-loop visual
  composition awaits the human UI discussion.
- Configured demo, both performance measures, external trials and authorized edit.

## Next slice

Cycle 3 (see `cycle-03-preparation.md`): replace frame, Wave and Task presentation with
accepted A/C. Render the Task's pinned Flow from shared definition and cursor. Use per-occurrence
state, and the new `occurrence` field for Session membership chips. Read real comment
counts through the planning boundary, not description text. Use shared started-work evidence
for sidebar membership. New session goes through existing Task preparation without
starting the managed Flow. Hold the two-loop diagram's final visual treatment for the human
UI review; implement data plumbing first.
