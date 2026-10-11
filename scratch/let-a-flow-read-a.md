# LOO-450 — Claude Flow answers

Jack Heart requested one Claude turn-recording path and native JSON schema delivery
on October 9, 2026; October 10’s deletion steer extends that cut.
The engine/driver vocabulary strike (#1520) is integrated; the current PR base
is `5f44fd9e71`, including #1518's terminal context hooks.
The former headless print path bypassed ClaudeHarness; it is removed.

## Accepted deletion — implemented

Jack Heart’s October 10 steer removes `run_batch`, `run_streaming`, their
exclusive stream parsers/renderers, obsolete Codex/OpenCode batch command branches
and their fixtures, the unknown-provider Claude
fallback, and `session_record::final_answer` / `FinalAnswer.exact`. Preserve
terminal launches, replay/run display types, native correlated answer history,
captures and all three headless harnesses. Removed the exclusive capture stream
writer too; usage fixtures now exercise harness conversation checkpoints. Shared
`StreamFormat` stays for run/replay; captured event schemas remain readable.
No migration or capture rewrite. The terminal-only Codex command builder also
loses its unused ProcessConfig argument and duplicate context-file fixture;
the surviving native-instructions test covers the retained behavior.

## Implementation

Headless Claude now uses the existing harness, preserving native skill
selection, context, resume and attachment ownership. The Flow output schema travels
through the ordinary CLI launch, capture/replay and correction turns to Claude. The existing
native answer reader is the only answer source. The `FinalAnswer` type is deleted;
the reader returns `Option<String>`. Missing native output returns absence, even
when capture prose exists.
Unknown providers fail in both terminal and headless launch modes.

## Review findings

Clap's inferred JSON Value parser treated a schema as a JSON string; the launch
fixture exposed it. Explicit JSON parsing preserves the object. Claude publishes
its native Session ID after start; the result now captures that final identity.
The correction fixture also exposed
that Claude kept its native identity only in memory/capture, not the Session's
selected thread. Selection now records under the current attachment when Claude
announces it; stale attachments cannot replace it. This is required to resume the
same deciding conversation, not a new continuation policy.
Native skill commands retain declarations and non-query context on the surviving
harness. Non-query context and the correlated query share one write/error path;
a failed context write now tears down the process too. No store migration or
third answer reader is needed.
Deletion review found capture usage fixtures still calling the retired writer;
they now use conversation checkpoints and retain cumulative/retry/finality proofs.
Historical capture schemas and payloads remain intact; only answer inference is removed.
Post-sync review removed the unused Codex builder argument rather than suppressing
the warning. Main’s terminal context hooks and harness-only headless execution remain.

## Remaining acceptance

- Demo: the configured `lf -a claude task run <task> pursue` reaches `pr-publish`
  without a hand-run step. The fake-provider test runs a taskless authored Flow
  ending in `publish-proof`, not the Task workflow or a real publication.
- Installed acceptance remains unproved; no installed store or live Session was
  changed by these source fixtures. Publication and landing belong to the caller.

The focused Flow fixture covers a valid first answer and an invalid first answer
corrected on native resume, with `started`, `output` and `completed` counts for
both. Terminal behavior and native/translated skill declarations remain required;
removing the print branch does not waive them. No new product decision is needed.
Release's entry-point lesson applies: Flow success is not Task-to-publication proof.
October 10 reconciliation inspected Release's top-level goal and memory; that
lesson applies here without transferring its release or schedule work to LOO-450.

Checks: network-isolated `scripts/test.py --reuse-passing --base 5f44fd9e7139e0e98874df36ce7b699bf6d0fa58` passed architecture/fmt/Clippy but timed out in stale EOF-waiting Rust fixtures; focused `cargo nextest run` launch/Flow/context/replay/repair selections passed across repairs (56 affected tests plus result-mapping checks); `cargo clippy --all-targets -- -D warnings` passed; `cd website && uv run python dev.py test` passed 76 tests after dependency preparation outside isolation; full materialized Rust matrix remains CI-owned.

## Delivery decision — October 10

Jack Heart approved landing after reviewing the harness path and accepting quieter
headless output: assistant text remains, but the old tool summaries and cost/time
footer are no longer rendered. This authorizes `ship` and completion after verified
merge; the configured live demo remains unproved, not a claimed pass. The replay/launch checks pass after the gate repairs below; CI owns the full matrix.


## Gate findings — October 10

The replay fixture now reaches Claude with a non-null saved schema and a disposable
recorded account; it retains the request and inherited capture identity without
requiring planning Work. Old subprocess fixtures waited for stdin EOF, emitted no
native final answer or read skill context from the retired channel. They now speak
the native protocol and preserve launch, correction, reconnect and task-input proofs.
Codex stand-ins publish a completed native message, not just deltas; correction
fixtures retain the schema while checking the same Session is resumed.

Gate also exposed a production regression: Claude's failed `result` supplied no
harness error event, so deleting raw-output parsing lost transient retry diagnostics.
The mapper now forwards failure text (or the native errors array), and the captured
launch test proves a transient failure resumes successfully in the same Session.
No print-mode path or answer fallback was restored. The two EOF fixture hangs used
up the broad gate budget; their repaired large-input and saved-workspace checks pass.
