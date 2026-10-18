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
case with `--scenario NAME` from the table below.

| Scenario | What it proves |
|---|---|
| `retry` | One model request fails; only explicit later input makes the next request, retaining exact history. |
| `empty` | Blank answers fail both initial and resumed headless execution. |
| `overflow` | Real Pi compaction and internal recovery refresh changed scratch before the next model request. |
| `threshold` | Reported usage crosses Pi's compaction threshold; the next explicit turn retains the conversation and refreshed scratch. |
| `native` | Native non-TTY launch and explicit continuation retain one conversation file. |
| `pty` | Native terminal accepts an edited Unicode draft across resize, exits, and resumes the same history. |
| `manual` | Native `/compact` writes a compaction entry; the next request sees changed scratch and exact resume still works. |
| `steer` | A public Session steer gets an independent emitted receipt while the model is held, reaches the next request, and retains one initiating turn/Process. |
| `rate-limit`, `server-error`, `invalid-model` | Real Pi receives HTTP 429, 503 or model-not-found 404; Loopflow fails without synthetic input, then explicit continuation recovers. |
| `crash` | Killing real Pi during its bash tool must fail the turn, retain paid usage and leave no running tool children. Currently fails child cleanup; the fixture kills its surviving children. |

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

October 18 gate: all eleven non-crash scenarios passed, including threshold
compaction (5 model requests), manual compaction (4), and each HTTP failure (2).
The `crash` scenario reproduced a release blocker: Pi's detached bash group and
its `sleep` child survived Pi's SIGKILL and Loopflow exit. The turn failed and paid
usage survived; the fixture removed both children. `--scenario all` intentionally
ends with this failing acceptance check until child lifetime ownership is repaired.
Shared group cleanup cannot reconstruct detached-child ownership after reparenting.

Active viewer handoff and repeated partial-usage recovery remain uncovered by the
real-Pi probes. Demo owns rendered judgment and the selected personal account route.

## Shared skill discovery

Run `lf sync-skills --yes`, then `/reload` in an open Pi session. Generated native
names use lowercase letters, digits and hyphens within 64 characters. Long names
and collisions retain distinct exports; original `lf` commands and resource paths
remain available. Third-party bundles are preserved.

On October 18, Pi 1.3.0's loader checked the configured shared catalog before and
after candidate sync: 85 skills / 59 diagnostics became 101 skills / zero diagnostics.
Both synced PDF variants survived, all 27 third-party SKILL.md files stayed
byte-identical, and repeat sync made no writes or prunes. Candidate help resolved
both slash and dashed Loopflow commands. This checks actual discovery and CLI
resolution; it does not record a rendered reload review.
