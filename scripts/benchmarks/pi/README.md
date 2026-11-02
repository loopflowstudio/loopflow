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
| `handoff` | Killing the native viewer during a model call and attaching a replacement preserves the provider, attachment, conversation and initiating turn; a public steer still reaches that turn. |
| `rate-limit`, `server-error`, `invalid-model` | Real Pi receives HTTP 429, 503 or model-not-found 404; Loopflow fails without synthetic input, then explicit continuation recovers. |
| `crash`, `crash-native` | Killing real Pi during bash fails the turn, retains paid usage and leaves no running tool children; two explicit resumptions preserve identity and earlier usage. |
| `tools` | Native bash output, shell prefixes, nonzero exits and timeouts remain intact. |

The overflow fixture keeps one token of recent history so its short conversation
has something to summarize. It edits scratch in the Session's recorded checkout;
Loopflow may promote the initial fixture repository during launch.

The JavaScript regression exercises manual and automatic compaction boundaries,
retained native system/tool declarations, and an unbound child. Rust's focused
`pi_` tests cover direct RPC steering rejection/uncertainty, frozen turn ownership,
large Unicode skills, native partial-usage checkpoints and duplicate observations,
failed-start reopening, and managed Task/Graph routing with a nested `lf` command.

Evidence dated October 17, 2026: the real Pi scenarios passed against a local model
(retry 2 requests; empty 2; overflow 5; native 2; PTY 3; steering 2). This establishes
protocol and terminal input behavior. It does not establish rendered appearance,
account entitlement or authenticated Loopflow execution. Pi owns login and model
settings; no subscription credentials were inspected or copied.

October 18 gate: all eleven non-crash scenarios passed, including threshold
compaction (5 model requests), manual compaction (4), and each HTTP failure (2).
The original `crash` scenario found detached bash/sleep children surviving Pi's
SIGKILL. The repair gives each built-in bash command a FIFO writer held by Pi and
admits Loopflow's existing watchdog before the command. RPC/native crash checks
now pass, including two native recoveries preserving earlier usage. The `tools`
check passes for output, shell prefixes, failed exits and timeouts. Custom extension
tools keep their own ownership; no Pi process registry is introduced. The repaired
13-scenario sweep passed; added native-crash and extended RPC recovery checks also
passed independently.

The gate continuation's `handoff` probe passed against real Pi: two model requests,
one initiating turn/Process, unchanged provider PID and attachment, and an emitted
steer receipt across viewer replacement. The replacement exits through native
terminal input. Demo owns rendered judgment and the selected personal account route.

The broad Rust gate stopped under sustained macOS host-security
pressure (syspolicyd above 1,300% CPU; load above 130). It recorded 929 passes,
three assertion failures and four interrupted tests; 1,760 tests did not run.
The unresolved failures were
`pm_read_linear_oauth_sqlite_contention_has_bounded_failure_and_recovers`,
`native_runner_is_quiet_until_its_turn_ends_claude_foreground`, and
`planning_export_late_readback_preserves_newer_edits_and_inventory_identity`.
Their test sources are unchanged from main. This is not a passing broad result;
hosted CI must resolve it before merge. The repository's required merge queue
runs the full matrix on the scratch-free candidate. Desktop's 603 headless tests
and all four macOS clone checks passed locally.

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
