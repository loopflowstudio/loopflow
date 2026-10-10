# LOO-447 — headless takeover and stop

Jack Heart requested provider-independent takeover and stop on 2026-10-09.
The outcome is accepted; the transport design below remains a draft (2026-10-10).
Dependencies #1519/#1520 are integrated at `be4a2b2af`, satisfying Jack Heart's
steer `28e0c5cc`. LOO-443's remaining item 3 was read at
`4f6ed76b2^:scratch/introduce-agentprocess-record-the-provider.md`.

## Outcome and preservation

The current attachment can stop its headless AgentProcess; superseded attachments
cannot. Takeover preserves AgentProcess identity, native history and outstanding
turns in both death orders: launcher first and attacher first. Foreground terminal
providers remain outside headless group control. Tests signal only their own
throwaway children; the lifeline shell stays dash-compatible (`kill -s TERM -- -pgid`).
Jack accepted the SIGKILL timing demonstration, three historical orphans and
configured-provider runs as out of scope. No installed-store writes or delivery
are authorized by this reconciliation. Stop-only work is not independently
shippable or Task completion.

## Implemented boundary

Runtime settlement, Claude stop/interrupt and OpenCode stop share exact-identity
group close under the attachment fence, recording death before releasing it. Unknown identity
and duplicate ownership refuse; Codex also refuses unrelated conversations.
Identity is rechecked after Codex I/O. Leader death alone grants neither group
settlement nor authority to signal surviving helpers. Both harnesses drop their
child handles only after common close succeeds.
OpenCode prompt, abort and stop share history's frozen attachment; config changes
cannot refresh control authority. Abort uses the bounded fenced HTTP writer without retries. Failed startup
retains the admitted child for common cleanup; cleanup refusal remains in the error.
Dropping OpenCode no longer signals its child. Its existing lifeline retains
lifetime custody until lf exit; the shared named-custody replacement remains open.

Fixtures cover current/stale settlement and foreground exclusion for all providers,
Claude harness stop fencing, and helpers surviving leader exit. They prove neither
public takeover nor Linux acceptance. Invalid trace IDs and foreground defaults
in Claude fixtures were repaired; production schema and installed data are unchanged.

## Remaining implementation

1. **Replace launcher-owned-only Claude transport.** Its anonymous stdin/stdout/
   stderr and pending correlation die with the launcher; a named watchdog FIFO
   cannot preserve communication. Give the existing stream reader a per-AgentProcess
   transport process owning all three pipes and native-history correlation, rather
   than adding another answer parser. Drain output without a client. Publish a
   private Unix endpoint before takeover; validate frozen attachments at this owner
   before writes. Lost responses or transport death retain uncertain inputs, never
   replay them. This proposal is unimplemented, not an accepted relay design.
2. **Replace OpenCode's unconditional server spawn on reconnect.** Reuse its saved
   server URL and native Session. Public attachment and harness takeover must use
   the same connection/claim path, with fenced writes and no raw endpoint bypass.
   `ops/human_session.rs::open` currently dispatches live connection only for
   Codex; other providers fall through to native resume. Generalizing harness
   startup alone cannot satisfy the public demo. Preserve client-only settlement
   when replacing `connect_live_codex`, rather than closing the surviving provider.
   Reusing the URL alone loses pending-turn correlation: `opencode_history::History`
   starts with an empty request map, saves origin only on observed native output,
   and emits completion only for locally submitted requests. Recover original
   request IDs/origins and pending permissions across launcher death, including
   death after HTTP acceptance but before the first observation. Native receipts
   must settle the original input without resubmission; SSE is only a wake edge.
   Keep ownership frozen for recovered prompts, permission replies, abort and stop.
3. **Delete anonymous lifelines and `HELD_LIFELINES`.** One per-AgentProcess named
   lifeline replaces both providers' anonymous paths and optional `open_lifeline`.
   Acquire custody before committing transfer; failed claims release only their
   own holder. Current `connect_live_codex` claims first and holds the FIFO later;
   it is not a safe ordering template. Cover launcher exit between these two
   operations for Codex as well as the new providers. A scoped holder observes group death and releases then or on its own
   exit, instead of accumulating dead providers in a process-global vector.
   Keep pre-exec recording and closed-stdio descriptor safety.
   The launcher may remain a non-writing standby after takeover: immediate release
   would kill its provider if the attacher dies first. Custody never restores stale
   write authority. Prove both death orders and failed claims before deleting the
   old mechanisms and their exclusive fixtures.
4. **Prove the public path.** Both death orders, AgentProcess identity and pending
   turns, A → B → A stale-token rejection, current stop, foreground exclusion,
   missing identity and no retained dead-provider lifelines. Include takeover before
   first native output and while a permission is pending. Linux CI owns platform
   acceptance. The demo attaches a second lf, SIGKILLs the launcher, checks `lf top`,
   then stops from the current attachment, for Claude and OpenCode.

Neither current transport gap proves takeover impossible or authorizes a refusal.
These are implementation choices, not missing human input or waived acceptance.
Removed already: Codex-only signaling, runtime live-close dispatch and Claude's
direct-child stop, OpenCode's direct-child/group shutdown and retrying abort.
The shared close and Codex unrelated-thread inspection survive.

## Related work and review

LOO-450's local `5ce7cd5e4` routes headless Claude Flow steps through ClaudeHarness
and saves native Session identity under the fence. It is not in this branch or
local main. Reuse that history path when integrated; preserve schema, skill-input
and correction turns. Its Flow fixture proves no launcher-independent transport.

Source reconciliation (2026-10-10): local main remains `be4a2b2af`; LOO-450
is still outside this branch. `b5b089978` completes the earlier OpenCode
stop/abort/drop step using frozen history ownership, not mutable launch config.
Release is the only immediate child Wave; its complete goal and memory were read.
Its entry-point lesson applies: harness close tests cannot establish public handoff.
Source inspection also exposes OpenCode's launcher-local request map; connection
reuse must recover correlation, not only retain the server. The transport and
custody changes remain substantial implementation work, not completed behavior.
Earlier review details: `5ea5cf5d6:scratch/stop-and-take-over-claude.md`.

The OpenCode regression enters real harness startup with a throwaway stand-in,
cancels after admission, transfers the attachment, rejects stale stop/abort even
after config changes, drops the old harness without killing the provider, and closes from the current
attachment without a local child handle. This is not a public takeover proof.

## Checks

`cargo test -p loopflow --lib --no-run`, network-isolated `harness::opencode::tests::stop_and_drop` (1 passed), `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `git diff --check`: pass (prior implementation); realign source inspection and `git diff --check`: pass, no executable changes; prior shared-close checks remain at `6c5a4505a:scratch/stop-and-take-over-claude.md`; public takeover and Linux acceptance remain implementation/CI work.
