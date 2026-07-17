## Try it!

```bash
cargo test -p loopflow task_runner_records_reported_turn_usage --lib
```

The test drives `run_task_session_inner`, emits input/output usage, and reads
the persisted `agent_turns` row. Removing the runner's
`capture.record_conversation(event.clone())` call makes the input-token
assertion fail with `None` instead of `Some(321)`.

## Intent

Task Sessions run provider harnesses in-process, so no child `lf` records their
spend. Route their conversation events through the existing trace capture path
so their turn usage becomes durable.

## Assumptions

- Trace capture remains best-effort and must not fail the Task body.
- Prepared harness-turn metadata remains the capture provider/model source.
- The surviving `agent_turns` store is the destination for turn-grain spend.

## Key decisions

- Reuse the existing harness-construction function seam from Wave bodies.
- Prove the runner call site with a scripted harness and real runner fixtures,
  not with a lower-level `TraceCapture` test.
- Close capture on every terminal body path so usage is flushed.

## Not included

Project Session runner capture, capture metadata changes, parser unification,
reader cutover, and spend-column removal remain outside this PR.
