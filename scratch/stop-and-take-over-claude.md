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
are authorized by this implementation step. Stop-only work is not independently
shippable or Task completion.

## Implemented boundary

Runtime settlement and Claude stop/interrupt share exact-identity group close
under the attachment fence, recording death before releasing it. Unknown identity
and duplicate ownership refuse; Codex also refuses unrelated conversations.
Identity is rechecked after Codex I/O. Leader death alone grants neither group
settlement nor authority to signal surviving helpers. Claude drops its child
handle only after common close succeeds.

Fixtures cover current/stale settlement and foreground exclusion for all providers,
Claude harness stop fencing, and helpers surviving leader exit. They prove neither
public takeover nor Linux acceptance. Invalid trace IDs and foreground defaults
in Claude fixtures were repaired; production schema and installed data are unchanged.

## Remaining implementation — Delete, do not maintain

1. **Replace OpenCode's unfenced stop/abort/drop paths.** Put stop, HTTP abort and
   failed-start cleanup under the saved attachment. Remove `kill_on_drop(true)`
   only with replacement lifetime ownership, so dropping a stale harness cannot
   kill the current attachment's provider.
2. **Replace launcher-owned-only Claude transport.** Its anonymous stdin/stdout/
   stderr and pending correlation die with the launcher; a named watchdog FIFO
   cannot preserve communication. Give the existing stream reader a per-AgentProcess
   transport process owning all three pipes and native-history correlation, rather
   than adding another answer parser. Drain output without a client. Publish a
   private Unix endpoint before takeover; validate frozen attachments at this owner
   before writes. Lost responses or transport death retain uncertain inputs, never
   replay them. This proposal is unimplemented, not an accepted relay design.
3. **Replace OpenCode's unconditional server spawn on reconnect.** Reuse its saved
   server URL and native Session. Public attachment and harness takeover must use
   the same connection/claim path, with fenced writes and no raw endpoint bypass.
4. **Delete anonymous lifelines and `HELD_LIFELINES`.** One per-AgentProcess named
   lifeline replaces both providers' anonymous paths and optional `open_lifeline`.
   Acquire custody before committing transfer; failed claims release only their
   own holder. A scoped holder observes group death and releases then or on its own
   exit, instead of accumulating dead providers in a process-global vector.
   Keep pre-exec recording and closed-stdio descriptor safety.
   The launcher may remain a non-writing standby after takeover: immediate release
   would kill its provider if the attacher dies first. Custody never restores stale
   write authority. Prove both death orders and failed claims before deleting the
   old mechanisms and their exclusive fixtures.
5. **Prove the public path.** Both death orders, AgentProcess identity and pending
   turns, A → B → A stale-token rejection, current stop, foreground exclusion,
   missing identity and no retained dead-provider lifelines. Linux CI owns platform
   acceptance. The demo attaches a second lf, SIGKILLs the launcher, checks `lf top`,
   then stops from the current attachment, for Claude and OpenCode.

Neither current transport gap proves takeover impossible or authorizes a refusal.
These are implementation choices, not missing human input or waived acceptance.
Removed already: Codex-only signaling, runtime live-close dispatch and Claude's
direct-child stop. The shared close and Codex unrelated-thread inspection survive.

## Related work and review

LOO-450's local `5ce7cd5e4` routes headless Claude Flow steps through ClaudeHarness
and saves native Session identity under the fence. It is not in this branch or
local main. Reuse that history path when integrated; preserve schema, skill-input
and correction turns. Its Flow fixture proves no launcher-independent transport.

Release's entry-point lesson applies: runtime close tests cannot establish public
handoff. Architecture docs name the remaining unfenced OpenCode paths. Compression
keeps the low-level close private, removes the inspector's tuple argument and
checks Claude's saved identity at launch rather than carrying optional evidence
through shutdown. Earlier review details: `5ea5cf5d6:scratch/stop-and-take-over-claude.md`.

## Checks

`cargo test -p loopflow --lib --no-run` plus network-isolated test-binary filters `harness::claude::tests::`, `harness::agent_process::close_tests`, `session_record::runtime::tests` (serial): 15 passed, 2 configured-provider tests intentionally ignored; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`: pass; public takeover and Linux acceptance remain with implementation/CI.
