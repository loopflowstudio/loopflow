# LOO-709 idle conversation demo — 2026-11-03

Decision: **advance to gate**. The headless caller delegated this decision to
Codex, standing in for Jack Heart. This applies Jack's accepted preservation and
return criteria; Jack did not observe this run or supply new feedback. No new
design change or concrete implementation defect emerged.

The [accepted design](loo-709-idle-sessions-end.md) preserves editable drafts,
the same pending request, active work and ordinary single-Session resume. The
preceding pass intended to repair queued-ending/output/viewer races and the
precedence of silent Working turns. Its focused regressions remain applicable.
This pass adds configured native startup and twenty complete returns, beyond the
previous three-return evidence. Gate still owns the affected suites and lint.

## Observed experience

Used this checkout's `target/debug/lf` at HEAD `74880b72e3`, native Claude Code
2.1.317, and disposable Loopflow stores. The binary reports
`0.15.1+39f57361d.dirty`; its SHA-256 is
`5f84c91024ceb5af2eabb6514da86b8dfcb54b24f9411ceda5091e64a97ec1d0`.
The configured-provider cohort retained the normal native Claude configuration
and service endpoint, with `claude:haiku`; only Loopflow storage and the detached
grace (4s) changed. No user input was sent during return other than terminal
protocol replies.

| Action | Expected | Observed |
| --- | --- | --- |
| Start twenty conversations, each asking for `LOO709_READY`, then detach after settlement | Owners end and save their screens | All twenty ended; twenty Ghostty screen files remained |
| Return to each with single-Session `lf resume` | Original reply and provider history, no replay | All twenty painted their original reply and retained their provider thread; native client startup appeared in each terminal capture |
| Detach each returned conversation | Settled owners end again | All twenty ended again; no recorded probe PID remained alive |
| Delay native resume by five seconds through a launch shim | Paint history before startup, then reach the native client | The saved reply appeared in the first three seconds alongside startup progress; later output showed the native client and history |
| Make the native resume shim exit 42 | Show the failure without starting another turn | Deliberate startup failure was visible; viewer exited 1; provider identity and native messages remained intact |

The twenty-conversation cohort was sequential. It completes twenty end/return/end
cycles, not a fresh measurement of simultaneous memory reclamation. The prior
twenty-owner/60-process, 9.9 GB teardown remains the concurrent evidence.

No-replay proof uses native transcript message UUIDs and message types, not the
probe's `provider_turn` counter: that column was empty for these native Sessions.
After all returns, every transcript still had exactly one submitted user prompt
and unchanged assistant messages. The baseline for the first two was collected
after their returns had begun; their single original prompt/reply pair and
unchanged subsequent history support the same conclusion. The remaining
eighteen had their message baseline captured before return.

The separate native interaction probe used real Claude with a credential-free
local Messages fixture. It typed `NATIVE_DRAFT_X`, detached beyond the grace,
removed the executable, returned to the same owner, edited `X` to `Y`, and
submitted `NATIVE_DRAFT_Y`. A pending `AskUserQuestion` survived both deadlines
and accepted “Keep” on the original `pending-native` tool-use ID. A separate plain
submission settled and its owner ended. Seven fixture requests; all three owners
and their recorded child PIDs were gone afterward. This confirms native editor
and request behavior, not a live-model choice of tool.

## Evidence and limits

- Twenty configured returns: `/private/tmp/loo709-demo-v_p9iigp/`, including
  `results.json`, `native-history-baseline.json`, `native-history-check.json`,
  twenty initial captures and twenty return captures. Runner:
  `.lf/tmp/demo/returns.py`.
- Slow/failed configured startup: `/private/tmp/loo709-startup-98hh22af/`,
  `results.json`, early/later terminal captures and two recorded launch attempts.
  Runner: `.lf/tmp/demo/startup.py`.
- Current native draft/request proof: `/private/tmp/native-input-kfw2u3lq/`.
  Runner: `.lf/tmp/demo/native-input.py`.
- The old fixture copy at `/private/tmp/loo709-demo-eljaik6j/` could not open:
  its recorded migration history differs from this branch. The original fixture
  was preserved. A fresh cohort replaced it; the old seventeen were not reopened.
- The first fresh attempt at `/private/tmp/loo709-demo-kq8xvzu0/` used the saved
  helper's obsolete JSON-line viewer protocol. Updating the helper to the current
  length-prefixed protocol resolved this harness failure. No production repair
  was needed. Both unsuccessful attempts were cleaned up.
- The installed app remains 0.15.1 and still advertises `resume --all`. This is
  configured-provider proof through the branch CLI, not proof that the shared
  installed app contains the change. Do not report installed acceptance yet.
- This headless run captured terminal bytes, not rendered UI screenshots or new
  visual feedback. No `scratch/demo.html` was produced; the interactive-only
  presentation requirement does not apply here.

Existing conservative input retention remains explicit: edits, paste, ambiguous
submissions and missing acknowledgement IDs can retain owners until native exit.
The successful draft edit does not imply those owners later become idle-eligible.
No workspace, visibility or scheduler change was selected.

Recommended next action: continue the authored Graph to gate, publication and
authorized delivery. Run affected checks, verify the migration frontier, and
verify the merged installed build at delivery. The demo adds no reason to return
to implement and does not itself land a PR or complete the Task. All probe
commands, the local fixture server and their recorded processes were stopped
before this decision.
