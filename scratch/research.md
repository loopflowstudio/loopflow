# Research: testing

## System understanding

Loopflow has a large, mostly implementation-local automated suite and a much
smaller set of real product proofs.

### Architecture

- Rust owns the binary, local ledger, agent harnesses, Waves/Projects/Tasks,
  Git and GitHub operations, and builtin skills. `cargo nextest run --all`
  discovers 1,941 tests across 41 binaries.
- Swift owns the shared models and Mac app. Its 236 package tests exercise
  models, parsing, launchers, and presentation state. CI separately compiles
  the app and UI-test runners.
- Python tests cover repository and release tooling. The 120-test suite runs in
  16.8 seconds locally.
- Website Playwright and accessibility tests exercise rendered pages. A deploy
  job also probes the production homepage and rolls back on failure.
- Shell E2E tests exercise CLI/worktree/store flows against isolated homes.
- `scripts/test.py` maps changed paths to suites, bounds every phase, and runs
  local suites serially. CI runs its jobs in parallel.

### Data flow

Agent launches write start/end metadata to the Loopflow ledger and normalized
conversation events under `~/.lf/traces`. Command items retain their command,
start/completion timestamps, exit status, and (when the vendor supplies it)
duration. Older captures retain raw provider streams, from which the same
command intervals can be recovered for Codex and Claude.

The trace layer can therefore measure actual test/check command time by skill
and worktree without emitting prompt bodies, tool output, or command text.
This is a better observation boundary than timing only `scripts/test.py`.

### Key abstractions

- **Focused proof**: one test or target that proves the behavior currently
  being changed.
- **Affected-suite proof**: the repository runner's changed-aware plan.
- **Full-matrix proof**: all CI suites, owned by CI and release rather than
  repeated in every local lifecycle phase.
- **Product proof**: the real CLI, app, provider, deployed service, log, or
  metric that demonstrates the user outcome.
- **Fresh evidence**: a passing result bound to the exact source tree and the
  command/phase it ran.

## Tensions

- Guidance says to test behavior, but most coverage is close to parsers,
  command construction, store internals, view models, and duplicated prose.
  The released binary, live provider boundary, and hosted Mac interaction have
  much thinner proof.
- Local lifecycle skills all say some version of “run tests.” `implement`,
  `compress`, `rebase`, and `gate` therefore repeat verification without clear
  ownership. `lint` says not to, but agents still ran tests there.
- `gate` requires a full local matrix even though GitHub CI is the parallel,
  required full-matrix authority. `task-gate` can invoke `gate` twice.
- The changed-aware runner is useful, but its 30-day full-gate budget verdict
  measures a rare ritual rather than the commands agents actually spend time
  running.
- Demo guidance strongly prefers direct experience, but demo evidence is not
  a named release/monitoring signal. Hosted UI proof remains blocked on host
  permission; nightly packages only prove `lf --version`.

## Observations

### Rebase verification is broad by policy, not retry-driven

The trace ledger contains 82 completed or partial `rebase` launches across
`manabot`, `managym`, `etude`, loopflow, and cadenza during the observed week.
Fifty launches performed real conflict resolution (`git rebase --continue`).

- Conflict-free rebases used 8.4 non-overlapping minutes on 14 test commands.
  Twenty-seven of 32 correctly ran no tests.
- Conflicted rebases used 156.9 non-overlapping minutes on 149 test commands:
  111 focused and 38 full-suite invocations.
- Seven launches ran focused proofs and then escalated to a full suite.
- Only five exact test commands repeated within a launch. The waste is breadth
  and repeated compilation across distinct targets, not retrying flaky tests.
- Individual conflict-resolution launches fanned out to as many as 12 test
  commands. That deserves a separate task: correlate conflicted paths with the
  proof targets selected, compilation invalidation, and the next gate/CI run.

Quick win: `rebase` now skips tests after a conflict-free rebase and asks for
one smallest relevant behavioral proof after conflict resolution. Gate and CI
own suite-wide verification.

### Cost

Seven days of Loopflow trace evidence for this repository contain 968 completed
agent launches and 41.0 hours of captured wall time. A privacy-preserving
command classifier finds approximately 6.64 hours of tests/builds/checks
(16.2% of wall time) across 68 launches:

| Skill | Test/check command time |
|---|---:|
| `rebase` | 141.9m |
| `gate` | 89.1m |
| `compress` | 81.5m |
| `implement` | 63.3m |
| `lint` | 22.4m |

Focused/selected commands account for 3.87 hours; broad/full commands account
for 1.91 hours. Selection is usually narrow. Repetition across lifecycle
phases, and repeated Rust compilation after edits, are the larger costs.

`loopflow.architecture` is the clearest example. Six completed launches cover
3.74 hours. Raw provider events attribute 60.4 minutes to 39 Rust test commands
and 9.3 minutes to builds/static checks. The `compress` launch ran 20 focused
Rust commands for 41.3 minutes; `lint` ran nine focused Rust commands for 8.7
minutes despite its “lint only” contract. Thirty-seven of the 39 Rust command
shapes were unique, so this was not a retry loop; it was repeated compilation
around many small proof targets. Thirteen additional launches are abandoned
`capturing/running` shells with no usable completion evidence.

A warm full Rust measurement took 135.35 seconds wall time: 26.57 seconds to
rebuild/query and 74.317 seconds to execute 1,939 tests (two skipped). The
assertions are not the dominant cost in the long architecture session;
recompiling after successive edits is.

Python is not a meaningful runtime problem: 120 tests pass in 16.79 seconds.
Five seconds of that is two deliberate process-timeout tests.

Recent green CI has a 4.4-minute median critical path (`rust-test`). CI is
parallel and is not the primary developer-flow cost. In the last 100 CI runs,
55 were red, but 28 of the most recent 35 red runs failed `scratch-clear`;
only four sampled SHAs had both red and green runs. Flakiness is not the main
signal.

### Quality

Strong proof surfaces:

- Store, recovery, authority, migration, Git/worktree, and CLI integration
  tests use isolated real SQLite stores and Git repositories and describe
  observable invariants.
- Shell E2E tests exercise real CLI processes against isolated homes.
- Website browser/accessibility tests exercise rendered behavior, and deploy
  smoke rolls production back on failure.
- DTO fixtures catch cross-language wire drift explicitly.

Weak or overfit surfaces:

- `python/tests/test_release_automation.py` mixes executable behavior with
  exact workflow strings and duplicated documentation wording.
- `python/tests/test_loopflow_skill_alignment.py` maintains duplicated doctrine
  through substring anchors instead of one generated source.
- Prompt, CLI-argv, config, and presentation suites contain many useful
  boundary contracts but also lock implementation spellings and combinatorial
  variants. They should be reduced opportunistically when touched, not deleted
  wholesale.
- `scripts/test.py`, its budget/history tests, and two gate documents total
  roughly 1,900 lines before `TESTING.md`. The durable budget verdict has only
  three full-run records, all failed, while agent traces already contain the
  more relevant timing evidence.

### Gaps

- Nightly package smoke proves only that the extracted binary prints a version.
- Live Claude/Codex skill sync exists as an opt-in script, not a maintained
  signal.
- CI compiles the Mac app and test runners. The required hosted UI run is
  blocked on Automation/Accessibility permission and has never reached 5/5.
- The Wave-surface screenshot check proves seeded states are distinct, not that
  a real local ledger drives the intended production experience.
- Before this audit, no developer-tool report exposed test/check time from
  Loopflow traces, even though the data existed. The new reader intentionally
  remains a script rather than a product CLI surface.

## Open questions

- Which maintained Mac should own the hosted UI gate, and who will grant its
  interactive TCC permissions?
- Should a scheduled live provider canary spend real subscription capacity, or
  remain an attended release demo?
- Which prompt/argv/config test clusters are painful enough in practice to
  justify focused compression after this workflow fix?

## Recommendations

### Give each lifecycle phase one verification scope

**Observation**: verification is repeated across `implement`, `compress`,
`lint`, `rebase`, and `gate`.
**Cost**: guidance edits and a small amount of runner support.
**Benefit**: removes broad tests from phases that do not own them.
**Verdict**: do now. Inner loops run focused proof; `lint` runs no tests;
`compress` verifies only behavior it changes; `rebase` verifies conflict
resolution; `gate` runs the affected-suite plan once; CI/release own full.

### Reuse exact-tree passing evidence

**Observation**: a second gate may see byte-identical source.
**Cost**: fingerprint dirty work plus persist and match the selected phases.
**Benefit**: makes “do not rerun unchanged checks” enforceable.
**Verdict**: do now, explicitly and transparently; never reuse for `--all` or
required-host gates.

### Replace the full-gate budget verdict with trace-derived test time

**Observation**: the current budget history measures rare full gates, not agent
behavior, and duplicates phase timeout data.
**Cost**: remove history/reporting code and add a developer-only
`scripts/test_time.py` reader.
**Benefit**: the metric answers where agent time actually goes, by skill and
worktree, without exposing conversation content.
**Verdict**: do now.

### Make product proof a peer of automated tests

**Observation**: the strongest current production proof is website smoke;
package/UI/provider surfaces are shallow or manual.
**Cost**: strengthen package smoke now; keep hosted UI and live-provider work
as named follow-ups that require real host/account authority.
**Benefit**: catches failures simulations cannot and makes demos operational
evidence.
**Verdict**: strengthen the safe package path now and update demo/gate guidance.

### Delete maintenance-only assertions when touched

**Observation**: exact prose/string assertions detect edits, not failures a
user experiences.
**Cost**: decide whether each invariant needs generation, schema validation, or
no test.
**Benefit**: less brittle maintenance with no runtime regression.
**Verdict**: remove the clearest documentation wording assertions in this pass;
leave larger source-of-truth collapses for dedicated work.
