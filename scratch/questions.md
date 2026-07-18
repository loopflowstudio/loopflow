# Open questions — PRD-5 PR 2 (durable failure evidence)

## 1. `backup_agent` is not configured for the product wave (decided: operator step)

`grep -rn backup_agent wave/` → empty. `wave/product/GOAL.md` frontmatter has no
`backup_agent`. PR #1020 built the recovery routing (`classify_disconnect_recovery`
→ `HandoffToBackup`/`AllowRetry`/`Stop`), but with no backup configured, every
disconnect-class failure routes to `AllowRetry` (replay-safe) or `Stop` (not
replay-safe) — never `HandoffToBackup`.

The directive says "route the next generation through the configured backup
provider" — but there is no backup configured. Configuring
`backup_agent: claude:opus` in `wave/product/GOAL.md` is a **wave-config
decision** (it changes which provider spends money on recovery), not a code
change. PR 2 flags it in the root-cause writeup and `Done when`; it is not
shipped in code.

**Assumption taken (headless, proceeding):** PR 2 does not configure
`backup_agent`. The operator decides. If the operator wants the directive's
"backup handoff" path to be live for the 10-body validation run, they add
`backup_agent: claude:opus` to `wave/product/GOAL.md` frontmatter before the run.

## 2. The operator 10-body run is an operator step, not a code step (decided)

The directive's "finish with ten real GLM Product bodies plus forced disconnect
recovery evidence" is an operator validation run. PR 2 ships the observability
(structured `FailureEvidence`) that makes the run checkable — but does not run
the 10 bodies. A code PR cannot run 10 real GLM bodies; that requires live
provider access and is the operator's proof, not the implementation's.

**Assumption taken (headless, proceeding):** PR 2's code `Done when` is the
evidence fields + tests + replay writeup. The 10-body run is named separately as
operator. If the gate expects the 10-body run to be part of THIS PR's code, the
directive needs a different reading — but the directive itself says "Publish in
reviewable slices and finish with ten real GLM Product bodies," which separates
the publish (code) from the finish (operator).

## 3. `lf runs` evidence rendering (decided: Debug derive is enough for this PR)

`runs.rs:865` prints `event.event_type()` + `Debug` of the event. The `Debug`
derive on `ConversationEvent` (via `FailureEvidence`'s derive) will show the
evidence struct in `lf runs` output. A prettier human-readable format (e.g.
`error  disconnected  model=opencode/glm-5.2  endpoint=harness_event_stream
last_event=session.status  terminal=stream_eof`) is a follow-up, not this PR.

**Assumption taken:** the `Debug` output is sufficient proof that the evidence is
durable and queryable. Formatting is polish, not substance.
