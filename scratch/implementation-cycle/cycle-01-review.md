# Cycle 1 — review-slice: shared Session naming

Disposition: **the naming slice advances the full design after two bounded
fixes.** Run-owned naming stays valid: the Session → Run link is required and
the one replacement path can carry the name forward. No design revision needed.
The native breadcrumb/rename UI, Flow membership and everything else in
`main-view-task.md` remain open. No publication, commit, PM or user-Session change.

## Findings fixed

1. **The agent instruction failed inside Ask/Flow Sessions.** `lf session rename
   "$LF_RUN_ID" … --suggest` resolved the boundary's Run as an *interactive*
   Session and bailed “has no provider history yet” before history existed;
   after history it would return a record of the wrong kind/id (and one `list`
   hides). Reproduced: `before-run-id.log`. `rename` now resolves a Run ID to
   the waiting Ask/Flow boundary that links it (`find_boundary_for_run`), and
   interactive naming no longer requires provider history.
2. **A replacement Run dropped the human name, so a later suggestion won.**
   `open_boundary` replaces a consumed Run that produced no history; the name
   lived only under the old Run. Reproduced: the in-Session `--suggest`
   returned “Better guess” (`before-replacement.log`). `carry_session_name`
   copies name + provenance to the new Run *before* the boundary publishes it.
3. **Rename could race that replacement** (write to the Run being replaced).
   Boundary renames now take the existing per-Session launch lock and
   re-resolve the current Run. The lock is released during provider waits;
   the new test renames while its provider runs, proving no deadlock.
4. **Remote Flow Sessions** failed with an opaque missing-manifest error. They
   now fail precisely with the routed command (`lf ssh [--repo …] <home>
   session rename '<id>' <name>`) derived from the Session's own open argv.
   Nothing is written locally; remote names are never seeded locally.

`ask_records()` now backs both `boundary_run_ids` and Run-ID resolution (one
reader of the Ask directory). No new registry, title store or writer.

## Evidence

| Claim | Proof | Result |
|---|---|---|
| Raw/skill Sessions named before first reply; reads never write | `raw_sessions_are_named_by_a_stable_word_pair`, prepared-Ask seed | pass |
| List and open agree; human beats suggestion incl. 9-thread race | `session_names_are_shared_and_human_names_win`, `racing_suggestions…` | pass |
| `$LF_RUN_ID` reaches the Ask boundary before and after history, while provider is live | `boundary_names_follow_run_ids_and_replacement_runs` (real CLI, stand-in provider, isolated Home) | pass after fix |
| Human name survives replacement Run; suggestion can't override | same test: new run_id ≠ old, title “Launch notes”, source human; one listed Session | pass after fix |
| Flow Run-ID resolution / replacement carry-over | same code path as Ask; no Flow CLI fixture | **source only** |
| Remote Flow rename refused precisely | source inspection | **source only** |
| Blank/missing/invalid names keep old name, explicit error | existing CLI + run_record tests | pass |
| Swift requires `title_source`, no defaults | implement receipt (34 tests); Swift unchanged here | prior pass |
| Generator attribution | MAGICAL (34) and MUSICAL (26) identical to `309575f8e^`; FNV-1a determinism is new; no animal list existed | accurate |

Commands (logs in `cycle-01-review-evidence/`):
- `cargo test -p loopflow --test session_cli_tests` — 6 pass (`after-cli.log`).
- `cargo test -p loopflow --lib -- human_session racing_suggestions word_pairs` — 15 pass.
- `cargo clippy --all-targets -- -D warnings` — pass (first run caught a large
  enum variant; boxed). `cargo fmt --all`, `git diff --check` pass.
- `correction.patch` isolates this review's edits; `sources.sha256` pins source.

Read the full per-file diffs of `human_session.rs`, `run_record.rs`,
`session.rs`, `naming.rs`, `lf/mod.rs`, CLI tests, LOOPFLOW.md; the aggregate
`lf task diff` was not used as completeness evidence. Swift/fixture changes
were read via the implement receipt, not re-reviewed line by line.

## Remaining gaps (this slice)

- Flow-specific behavioral proof of Run-ID resolution, carry-over and the
  remote refusal (needs a CLI Flow fixture with placement).
- `LOOPFLOW.md` guidance is injected into headless Runs too; there `rename`
  errors “not an interactive Session”. Harmless but noisy; scope the paragraph
  to human Sessions or accept.
- New Flow nodes/iterations are new Sessions and correctly start with fresh
  seed names; no carry-over there by design.
- Flow `title` == `detail` (both the step skill); Task title shows only via `work_path`.

## Full-design obligations still open

Native frame A/Wave/Task composition, Flow graph with occurrences and real
controls, comments read, exact Flow membership in `SessionRecord`, named
Session drill-down + in-place rename UI, New session beside Task title,
configured native continuity, measurements, external trials/edit.

## Next bounded slice

Native Session breadcrumb + in-place rename via `lf session rename --json`
with authoritative readback into Podium's reading and rejected-edit retention;
add Flow membership (exact execution/step link or explicit unknown) to the
shared `SessionRecord` with Rust/Swift fixtures; add a Flow CLI fixture
covering Run-ID rename and remote refusal.
