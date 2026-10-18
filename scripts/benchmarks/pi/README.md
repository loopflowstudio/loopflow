# Pi runtime proofs

```bash
cargo build -p loopflow --bin lf
uv run python scripts/benchmarks/pi/probe.py --pi /path/to/pi
node scripts/benchmarks/skill-invocation/test_pi_context.mjs
```

Use an installed Pi 1.3.0 executable. Each scenario creates a disposable Git
repository, Loopflow Machine, Pi settings directory and local HTTP model. It
inherits no account credentials and never changes personal Pi settings. JSON
receipts go to stdout; failed assertions include fixture diagnostics. Select one
case with `--scenario retry|empty|overflow|native|pty|steer`.

| Scenario | What it proves |
|---|---|
| `retry` | One model request fails; only explicit later input makes the next request, retaining exact history. |
| `empty` | Blank answers fail both initial and resumed headless execution. |
| `overflow` | Real Pi compaction and internal recovery refresh changed scratch before the next model request. |
| `native` | Native non-TTY launch and explicit continuation retain one conversation file. |
| `pty` | Native terminal accepts an edited Unicode draft across resize, exits, and resumes the same history. |
| `steer` | A public Session steer gets an independent emitted receipt while the model is held, reaches the next request, and retains one initiating turn/Process. |

The overflow fixture keeps one token of recent history so its short conversation
has something to summarize. It edits scratch in the Session's recorded checkout;
Loopflow may promote the initial fixture repository during launch.

The JavaScript regression exercises manual and automatic compaction boundaries,
retained native system/tool declarations, and an unbound child. Rust's focused
`pi_` tests cover direct RPC steering rejection/uncertainty, frozen turn ownership,
large Unicode skills, native partial-usage checkpoints and duplicate observations,
failed-start reopening, and managed Task/Flow routing with a nested `lf` command.

Evidence dated October 17, 2026: the real Pi scenarios passed against a local model
(retry 2 requests; empty 2; overflow 5; native 2; PTY 3; steering 2). This establishes
protocol and terminal input behavior. It does not establish rendered appearance,
account entitlement or authenticated Loopflow execution. Pi owns login and model
settings; no subscription credentials were inspected or copied.

Gate still owns the affected suites and the remaining compatibility matrix:
real threshold/manual compaction, active viewer handoff, provider death during
real tools, repeated partial-usage recovery and child-process cleanup, and real
429/5xx/invalid-model cases. Demo owns rendered judgment and the selected personal
account route.
