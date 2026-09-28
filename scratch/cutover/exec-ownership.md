# Exec admission and native-engine provenance — in progress

2026-09-28 · LOO-298 · Continues the accepted implementation; no cutover claim.

## Observations

- `tests/e2e/codex_connect.py` uses actual Codex 0.157.1 with a private
  `CODEX_HOME`, allowlisted environment and credential-free localhost Responses.
  The provider executes real `exec_command` tool calls. Per-thread
  `shell_environment_policy.set` carries distinct caller/generation values.
  A held turn survives the first client's disconnect, completes through the
  second, and retains its own values after a sibling conversation executes.
  Receipt: `.lf/tmp/execution-model/connect-probe-2/`. This is native transport
  evidence, not Loopflow driver-transfer or configured-model acceptance.
- The new actual-CLI inspection fixture failed before production edits:
  `every parsed lf command has an Exec row: no such table: execs` (4.08 s).
  An initial fixture compile failed because `open_ephemeral` is private; the
  fixture now uses the existing public `open_ephemeral_store` entry.
  Receipt: `.lf/tmp/cut-i/exec-observation-before-fixed.log`.
- After the initial journal/entry-point change, the same fixture passed
  (6.05 s): `lf session list --all --json` records one completed Exec and
  reserves no Run. Receipt: `.lf/tmp/cut-i/exec-observation-after.log`.
  Later edits have not yet reused this result as their proof.
- `.lf/tmp/cut-i/exec-release-proof.log`: four actual-CLI cases passed in
  36.77 s: success without work reservation; failed command with exit 1;
  actual SIGINT with the CLI's chosen exit 130 and unknown signal field;
  actual Codex child commands across driver release, handoff and original
  driver exit. A replaced-provider child retains the historical original Exec.
  Provider replacement in this proof is a store generation change, not a
  public restart operation or a stopped/relaunched engine.
- `.lf/tmp/cut-i/exec-runtime-proof.log`: the nested runtime regression passed
  in 3.12 s. Inner success preserves the outer Exec, whose subsequent failure
  records the sole terminal event. These are focused results, not an affected gate.
- Formatting, Ruff and all-target Clippy passed; Clippy receipt is
  `.lf/tmp/cut-i/exec-clippy.log`. Migration inventory preserves all 54 shipped
  files. Architecture initially rejected the missing Exec owner; the actual
  owner map now includes it, and all seven bounded coverage checks pass.

Measured against starting HEAD `2e7571019`, touched Rust production sections
are **+350 / −47** lines, including moved dispatch lines. New SQL adds 44 lines;
tests and notes are excluded from the Rust count. `human_session.rs` is unchanged
at **2,081** lines before its test-module attribute. Nothing on the old Run
deletion list is newly removed by this ownership proof. These additions are
explicit unfinished work toward the complete cutover, not a reduction claim.

## Current executable edits

The existing journal transaction now inserts an indexed Exec and its command
outcome alongside the event. Historical rows come only from process-labelled
journal events, never from old Run rows. The ordinary parsed CLI dispatch owns
one outer runtime; in-process runtime wrappers no longer finish that Exec early.
The interrupt callback retains the exact context and shared event sequence;
the signal name stays unknown because the current ctrlc callback does not expose it.

Session driver and provider generation are separate columns on the existing
Session owner. Compare-and-set transfers the driver without changing the live
provider generation. Child admission resolves that provider generation to the
current driver. Replaced-provider commands retain the originating Exec, never
the replacement. Consuming agent caller provenance removes it from this lf
process's environment so its direct children inherit a direct Exec edge.
Provider launches must install fresh provenance; the production launch migration
is still pending.

The actual-engine ownership fixture uses production Session reservation and
driver-transfer APIs plus actual lf processes and provider-executed commands.
It does not exercise a public connect implementation, provider replacement
mechanics, Flow settlement or headless Session creation. Those remain required.

## Remaining work in this slice

- Wire the proven transport into public connect and the ordinary harness; the
  native fence currently has an actual-engine fixture caller only. Session-row
  actions still need the same driver fence. A provider-native name rejection
  does not prove stale SQLite Session mutation is excluded.
- Wire real agent admission to the existing Session owner and explicit caller
  environment; migrate the reconnectable harness and public connect operation.
  Prove A → B direct, A's agent → C, C's agent → D, with driver exit/handoff.
- Main still dispatches installation and screenshots ahead of ordinary capture,
  and parsing/help exits happen before it. Installation preflight must retain
  its exact-store boundary without a logging-induced schema open. Resolve these
  entry cases and report bootstrap/unavailable-store limitations explicitly.
- Historical Run-level `escalated` events now keep unknown completion; no
  historical signal contract was established. New interruption comes from the
  explicit CLI termination hook. Provider identity survives a released driver;
  the release operation increments only its driver fence.
- Complete the accepted lifecycle, import, consumers, Chapters and deletion.
  Exec foundations and native transport success do not satisfy code-complete review.

No branch binary accessed the installed Home. No publication, promotion,
installation or Task/Flow disposition was requested by these proofs.

## Retained native client: counterexample and transport repair

Jack required the original native client to remain connected across transfer.
The direct native probe in `.lf/tmp/execution-model/retained-native-client/`
confirmed that a second client's resume does **not** revoke the first client's
ability to start another turn. The original and sibling both completed. This
is actual Codex 0.157.1 with synthetic upstream, not a configured model result.

The implementation choice is a conversation-scoped native connection in
`harness/codex_connection.rs`. It relays to the existing Unix WebSocket engine,
never launches or kills it, and has no separate durable owner. Passive
subscription strips resume overrides. Every other RPC, including approval
replies, compares the exact Session driver while holding the same SQLite write
transaction that serializes transfer, through the bounded socket send. The
comparison happens at dispatch rather than ahead of a queue. A send timeout
remains an unknown outcome; there is no automatic retry. This deliberately
holds the database writer during a local socket send (at most two seconds);
measure contention before calling the final path fast.

Focused actual-engine receipts:

- `exec-native-fence.log`: retained old client rejected on start, steer,
  interrupt and native name mutation; it still receives the active turn's
  completion. A passive client gets no claim; the new driver steers and starts
  another turn; the active sibling survives. One test passed in 13.99 seconds.
- `exec-native-parent.log`: the same scenario issues real nested `lf` commands.
  Initial command parent is the original Exec; subsequent commands from the
  continuing provider name the replacement Exec. One test passed in 24.85
  seconds. Sibling commands carry no original-conversation caller identity.
- `exec-native-transfer.log`: final proof also allows the original client's
  steer before transfer, then rejects its writes afterward. One test passed
  in 25.19 seconds. `exec-native-clippy.log` records all-target Clippy passing
  in 15.75 seconds; formatting, Ruff and working-diff whitespace pass.

These use the production relay and store writer, with fixture-owned endpoints
and real command processes. They do not exercise public connect, normal harness
admission, native TUI startup, pending approvals, Session-row writes, or actual
provider restart. The older driver-release/replaced-provider proof remains
separate. The full lifecycle and code-complete review remain unfinished.

This pass adds 166 production Rust lines and removes none against `03330279f`;
the relay is 144 lines and the existing Session writer adds 20. The two module
declaration lines are included. No Run owner or reader is newly deleted;
`human_session.rs` remains at 2,081 non-test lines. The relay is a retained
transport implementation for the next lifecycle cut, not an owner-only stage
offered as completion.

## Public headless baseline — 2026-09-28

The actual CLI / installed Codex / synthetic localhost Responses fixture now
checks headless launch, default visibility, explicit headless discovery and rename.
`uv run tests/e2e/codex_connect.py --codex /Users/jack/.local/bin/codex --lf target/debug/lf --launch --output .lf/tmp/execution-model/public-headless-before-2`
failed at the new-contract assertion: launch exited 0, the default inventory was
empty, and `session list --all --interactive false --json` exited 2 with
`unexpected argument 'false' found`. Copied CLI SHA-256:
`011b4cdb6eab252f08bc1af28f645b0805351a2bf5da9fa148886c4838186b93`.
No lifecycle production edit preceded this baseline. It does not prove discovery,
rename, continuation or handoff through public commands.

The first receipt (`public-headless-before`) failed in fixture setup: the copied
binary path collided with its private LF_HOME. That observation remains retained;
it is not a product launch failure. The fixture now copies to private `bin/lf`
and gives its exact child CLI graceful termination before forced timeout cleanup.
