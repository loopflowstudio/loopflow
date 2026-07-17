# Open questions / assumptions — W2-281

## Implementation status (design approved cf3870f0)

Shipped in PR #1079 (rust-lint green; rust-test in flight):
- Move 1 — `CallerAuthority` + `resolve_caller_authority` (fail-closed;
  Project→`route.current`); `command_source`/`project_command_source` rewired.
- Move 2 — fast bar before `Resume` creation; `reject_persisted_child_command`
  store primitive; terminalize-on-launch-error for a just-created `Resume`.
- Move 3 — audience-correct `open_pr_bar`.
- Tests: funnel matrix (adds stray-marker fail-closed, all-absent Operator);
  `project_caller_is_validated_against_the_live_route`; store-primitive
  persisted→failed + claimed-refused; `a_supervisor_resume_of_an_open_pr_
  refuses_before_creating_a_command` (steady-Open, ledger empty).

**Known test gap — the injected phase-flip end-to-end.** The terminalize-on-race
path is only reachable when the fast bar passes (PR `Working`) and `launch` then
bars (PR flipped to `Open`) — a genuine TOCTOU. The two PR reads hit the same
store row inside one `queue_command` call with no test-controllable interpose
point, and `launch` spawns a real tmux process (the existing lifecycle harness
deliberately never reaches it). So the flip is not cleanly unit-testable without
a process-spawn / PR-observation seam the harness lacks. What *is* proven
deterministically: the `reject_persisted_child_command` predicate + its
`fail_child_command` sabotage (the reviewer's persisted-state assertion), and
the fast-bar steady-Open refusal. The remaining ~12 lines (launch `Err` →
reject + `Failed` event, `Resume`-only) are composed from the tested primitive.
Note the wiring fires on *any* launch error for a `Resume`, not only the bar — a
strict superset that also clears a spawn-failure orphan. If review wants the
injected flip asserted, it needs a harness seam (follow-up).

## Assumptions (proceeding on these)

- **Trust model is local, single-operator.** A caller who scrubs *all* identity
  env can present as an operator; that is inherent and acceptable. The task's
  target (Done-when #4) is the *one-variable* strip (`env -u LF_WAVE_ID`), which
  the fail-closed marker set closes. Not building unforgeable authority tokens.
- **`open_pr_bar` only ever reaches a supervisor.** Verified via
  `launch_intent`: `(Resume, Human)` → `ExplicitResume` (abandon-only bar), so
  an operator resume never hits the open-PR bar. If a future change routed an
  operator resume through the supervisor bar, Move 3's message would need an
  audience branch. Pinned by a test.
- **Managed-marker set is `{LF_WAVE_ID, LF_PROJECT_SESSION_ID,
  LF_TASK_SESSION_ID, LF_CHANNEL}`.** Chosen because each body type carries at
  least two of these, so a single-var strip always leaves one. `LF_RUN_ID`/
  `LF_PROCESS_ID` are excluded — the journal sets them on *every* `lf` process
  including the human CLI, so they cannot distinguish a body from a shell.

## Resolved in review (v1 → design revision)

- **Project authority validates against the live route, not historical
  provenance.** `CallerAuthority::Project` compares the incoming
  `LF_PROJECT_SESSION_ID` to `resolve_task_project_route(...).current`
  (`ops/project.rs:1254`), so a live successor Project Session (W2-243) can
  supervise and a terminal predecessor cannot. Regression added: historical-live,
  terminal-historical-with-successor, and terminal-no-successor.
- **Move 2 fixed twice.** v1's pre-persist bar was a TOCTOU. The v2 revision
  ("persist-after-launch, contingent on reservation") was itself wrong: `launch`
  is not one atomic cut (it reads `active_task_pr`, then reserves and starts tmux
  in separate calls), so persist-after-launch could start a generation whose
  `Resume` insert later fails. v3 uses the truthful shape: keep persist-before-
  launch; a **fast bar before creation** makes the steady refusal write nothing;
  a **post-creation phase flip** that bars `launch` terminalizes the just-created
  `Resume` from `Persisted` to `Failed`, so the generation is unchanged and no
  `Persisted`/`Claimed`/`Uncertain` orphan remains. **The existing
  `fail_child_command` cannot do this** — its UPDATE
  (`store/sqlite/child_sessions.rs:1884`) matches only
  `state IN ('claimed','delivering')` and returns `InvalidData("already
  resolved")` on a `Persisted` row, so v3's claim it was capable was wrong. v4
  adds a distinct pre-delivery primitive `reject_persisted_child_command(id,
  error)` gated on `state='persisted'`, records the bar text as the error, and
  appends a `Failed` `CommandChanged` event before the CLI returns the bar. Two
  deterministic regressions: steady-`Open` (count unchanged) and injected
  phase-flip (receipt `Failed` with bar error, generation unchanged, no orphan);
  sabotage swaps the new primitive back to `fail_child_command` so the raced
  `Resume` stays `Persisted`. Not an atomicity claim — no single-transaction
  primitive exists, and inventing one is out of proportion to the incident.
- **Environment's role is documented, not denied.** Authority resolves at the
  invocation boundary; env is transport of a body's stamped identity +
  consistency evidence. Explicit `--wave` is a distinct deliberate assertion.
  Done-when reworded accordingly.

## Genuinely open (resolve in review or a follow-up)

- **Headless review handoff with no operator present.** W2-319's persisted
  review directive (`cc_03a…`) needs an operator resume to reach the Task. In a
  fully headless fleet with no human, a submitted Task whose review is complete
  has no supported actor to deliver the directive without violating W2-129
  (supervisor must not restart a submitted Task). This design refuses cleanly
  and names the owner; it does **not** invent a headless delivery path. If the
  fleet needs one, it is a separate typed verb (e.g. an operator-authority
  "answer-review" delivery gated on a *completed* review), filed as follow-up.
  Assumption: out of scope for W2-281, which is about *authority classification*
  and *not stranding inert commands*, not about closing the headless-review gap.

- **Should `CallerAuthority` also gate steer/follow-up/interrupt, or only
  resume?** This design threads the typed authority through every control verb
  (they all call `queue_command`), but only *reorders the bar* for `Resume`.
  Steer/interrupt against a live body are unaffected. If review wants the same
  pre-persist discipline for supervisor-issued steers that would strand, that is
  a natural extension — but no live incident shows it, so kept out of scope.

- **`--wave` as operator's explicit wave assertion — keep or restrict?**
  Assumed kept: it resolves to a registered row, so it is a deliberate typed
  assertion, not ambient inheritance, and it is the intended surface for a human
  acting as a wave. Flagging in case review wants operator↔supervisor kept
  strictly disjoint at the human CLI.
