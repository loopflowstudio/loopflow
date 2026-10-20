# v0.14.8

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.8 makes returning to work more predictable: structured IDs resolve regardless of casing, Codex reconnects retain the saved conversation without automatically replaying uncertain input, and stacked Tasks can sync after a parent's checkout is removed. Codex relay failures now appear in Session history, giving operators evidence when a connection fails. Startup recovery remains unresolved; this release improves continuity and diagnosis without establishing a fix for the original fresh-launch reset.

## Find work with the ID you have

Commands now accept structured IDs in any casing and store and print one canonical spelling. Task and Session selection distinguish Loopflow identities from provider-owned values, so convenience in lookup preserves the exact identity of native conversations. (#1602)

- Use `LOO-506` or `loo-506`, mixed-case Task and Session IDs, or mixed-case Linear UUIDs to find the same record.
- Structured UUIDs and prefixed IDs use lowercase; Linear ticket labels use uppercase. Desktop decodes the same canonical identities across the CLI transport.
- Provider conversation IDs, tokens, hashes, names and paths retain their exact bytes. UUIDs with and without hyphens remain distinct.

## Return to Codex with better continuity and evidence

Reconnects use the Session's saved Codex thread, including when its identity arrives after the terminal disconnects. Initial-input submission evidence distinguishes explicit rejection from uncertain delivery, reducing the risk of duplicate submissions while keeping connection failures inspectable. (#1598, #1599)

- Explicitly rejected initial input remains retryable. Dispatched input without an acknowledgment is never replayed automatically.
- Client failures leave the reconnect listener available; stale attachments cannot overwrite the current attachment's submission evidence.
- Run `lf history show SESSION` to inspect retained relay failures and their original AgentProcess. Add `--events` for the failed operation, connection direction and error payload, including failures before a native turn starts.
- Diagnostics live in private Session events, replacing the separate `codex-relay.log`. Retention is capped at the first 16 failures per captured input and 2048 UTF-8 bytes per error. Explicit client closes stay quiet; diagnostic writes are best-effort and do not settle turns or interrupt the native terminal.

## Keep automated work moving

Flow routing can now read answers captured when transport delegates execution to another process. The lookup follows that delegation while keeping captures from ordinary agent-issued nested commands separate. (#1600)

- Routing steps find their captured Session turn through `lf transport serve` delegation.
- This repair requires no schema, command or wire-format change.

## Operational notes

- **Identifier migration:** existing structured IDs and declared JSON references convert in one transaction. A collision after case normalization aborts the migration; subsequent non-canonical writes are rejected with `canonical identity required: <table>`. Existing fence, receipt and trace paths are retained without renaming files, and metric payloads and hashes remain unchanged. (#1602)
- **Live work delays migration:** the upgrade defers without changes when dispatch is busy, an affected owner is live, or a receipt is unreadable or malformed. An older client holding a mixed-case live claim must settle before the upgrade can proceed. (#1602)
- **Codex recovery limits:** disposable probes verified retained startup diagnostics and preserved edited Unicode drafts across reconnects. Injected faults do not establish the original installed reset's cause. An after-thread reset with Codex 0.163.0 displayed “Reconnect failed” and timed out; startup recovery remains unresolved. (#1598, #1599)
- **Verification limits:** the supplied branch records leave Linux and the full hosted matrix to CI. Compiled acceptance and Clippy for the transport-routing repair were deferred for insufficient disk space; its recorded query simulation does not replace compiled tests. (#1599, #1600)

## Small changes

- `lf sync` in a stacked child Task targets the default branch after its parent lands, even when the parent's worktree has been removed. PR observation falls back to the Task's repository instead of failing with a missing-directory error. (#1597)