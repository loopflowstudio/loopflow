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

- Execute and repair the driver-handoff proof. Add failed/interrupted command
  results, nested runtime scope and stale-driver write rejection evidence.
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
