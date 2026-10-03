# Flow decision schema rejection

LOO-367 unblock · 2026-10-02 · failed Session event 30887

## Observation and cause

The supplied failure records Codex rejecting `loop-decide` before producing a
decision: HTTP 400, `invalid_json_schema`, root `maxProperties` unsupported in
`codex_output_schema`. Retrying that same schema cannot resolve this rejection.
This event provides no Advance or Iterate verdict and no evidence against the
Task design.

Commit `fac48dd22ba8f98dc45174549cb1e281ce340db8` (#1401), already present in this
checkout, removes that keyword and the root union from
`rust/loopflow/src/engine/flow_output.rs`. The schema requires `decision`,
`summary` and `reason`, with nullable explanation fields. Local decoding still
requires exactly one nonempty explanation appropriate to the decision and
accepts historical two-field receipts. `AgentConfig::output_schema` constructs
this schema from the selected boundary; `harness/codex.rs` sends it as
`turn/start.outputSchema`.

## Resolution and remaining uncertainty

Reuse the existing repair; no additional production edit or product decision
is needed. Jack Heart's accepted Task requirements remain unchanged. This Ask
received no new product feedback. Existing dirty implementation and memory edits
were preserved.

The installed CLI reports `lf 0.12.29`; the source test build reports `0.12.30`.
Those version labels do not identify the executable responsible for event 30887
or prove that the waiting driver contains the repair. No installation was
changed, worker restarted, Session completed, or Flow edge chosen here.

After Jack completes this Ask, the caller should reassess using a runtime that
contains `fac48dd22`. Verify the runtime before retrying the failed boundary;
retrying through an older live driver may resend the rejected schema. Success
means Codex accepts the request and a fresh selected completion yields a valid
decision receipt. Local schema tests alone do not prove provider acceptance.

The remaining Task implementation stays in
`scratch/jack-heart/start-and-finish-tasks-without.md`: optional placement and
admission are next. This infrastructure failure does not establish completion
of that work.

Check: `cargo test -p loopflow engine::flow_output::tests --lib` — 4 passed;
provider acceptance and the waiting driver's executable remain unverified.
