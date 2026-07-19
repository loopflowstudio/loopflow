# v0.12.2

v0.12.1 built the Run and Steer control spine but left the old Session lifecycle running underneath it, mirrored rather than replaced. v0.12.2 finishes the cut: Session is deleted, Run is the sole executor, and the space that frees up is spent on the thing a single executor makes possible — recovery. A run whose provider rate-limits, drops its stream, or has its credential revoked now re-routes to another exact account or provider inside the same durable Run instead of dying with its partial work. The release also gives the interactive pause a real door (`lf queue`, `lf work feedback`), makes execution placement authoritative across machines, and puts integration under explicit worktree ownership. Net across 232 files: about 20,800 lines added, 25,400 removed.

## One executor

Project and Task ran through a Session lifecycle that duplicated everything the Run spine already owned: status mirrors, process generations, write leases, recovery bridges. All of it is gone. Stable Project and Task Work now runs through one Run-authorized `__work` entrypoint, Launch owns provider continuity, and Work status is derived from durable execution evidence rather than stored twice and reconciled (#1099).

- Session tables, status mirrors, process generations, child write leases, and recovery bridges are dropped; a one-way migration rekeys surviving records before the storage goes.
- Completion and replacement are fenced by Run lease, Basis, containment, and successful-boundary evidence — not by a lifecycle flag.
- Roadmap, CLI, and Swift surfaces read normalized Work status from the same place.
- Codex is now the implicit agent. Unset config resolves through one compiled default; explicit skill and repo config still win, and Claude stays explicit for kickoff, review-design, demo, code-review, and prompt (#1097).

## Runs that survive their provider

With one executor there is one place to put failure policy, and this is where most of the release went. A retryable failure no longer ends the Run — it selects the next exact provider, model, and account route and continues.

Transient provider trouble (capacity, 429s, 502/503, dropped connections) retries on a 2s/5s/15s/30s ladder, carrying the provider session token so Claude and Codex resume the interrupted task rather than restarting it (#1080). Subscription exhaustion takes the other path: the account is recorded as limited with its reported reset time, and work fails over immediately instead of backing off against a limit that will not clear. Recovery exhausts eligible accounts on the primary provider before falling back to the configured backup agent, and excludes routes already tried in the current chain so a Run cannot loop against the same dead route (#1098, #1100).

- Harnesses are pinned to the account recorded on each Launch; route drift is rejected, and incompatible provider sessions are cleared when the account or provider changes.
- Provider containment is stopped before a Launch is replaced; if containment cannot be proven stopped, the Run fences instead of double-launching.
- When every permitted route is unavailable, the Run records a typed `provider_route` capability wait rather than a generic failure.
- Revoked credentials are now distinguished from cached state. `lf auth accounts --verify` checks live, a revoked token is recorded as missing with the exact reconnect command printed, and agent work routes to the next healthy account instead of stranding (#1107).
- Accounts are addressed by login email — full address or unambiguous prefix — everywhere: `auth connect|import|set|reset|disconnect`, `auth access`, and `--account`/`--only-account`. The path-safe internal ID is now a private storage key derived from the login, so an account can no longer have two names that disagree (#1095).
- Terminal and SSH launch surfaces carry the same authority as headless ones: managed Claude and Codex sessions honor repo/default routing and account health, OpenCode Zen uses its stored provider credential, and remote-native paths strip provider API keys they do not own (#1109).

## The human door

The interactive pause is now a first-class, attachable boundary. The durable noun formerly called Review is Feedback across store, DTOs, CLI, docs, and Swift, and — more consequentially — the trigger moved from a skill's `interactive:` frontmatter to an explicit `feedback: true` on the flow step. A skill no longer silently makes every flow that references it block; the flow author declares where the door is (#1093).

```bash
lf queue                                   # what needs you, oldest first
lf work feedback task task_... --continue-on-exit
lf work continue task task_...             # advance past the current Feedback
lf work escalate task task_...             # hand a child's Feedback up to the User
```

`--continue-on-exit` spawns a detached guard process, flock'd per `LaunchId`, that owns continuation for the lifetime of the pipe — a crashed or signalled client still advances the flow exactly once rather than stranding the Work. `lf work escalate` lets a parent Run hand its child's Feedback to the User instead of answering it.

## Where work runs

Every durable Work now has one authoritative execution Home, recorded as a single `Placement { work, home_id, placed_at }`. A Home carries stable identity plus a mutable observed route, so an SSH address can change without moving the Work. Run reservation resolves placement itself and refuses on every Home except the placed one — a caller cannot claim that remote Work ran locally (#1075).

```bash
lf home observe <home-id> ssh://jack@mini.local
lf work place wave <wave-id> <home-id>
lf start shipper intelligence
lf stop shipper
```

New Waves begin on the local Home; Projects and Tasks inherit their parent's placement once, then own an explicit one. One shared Home resident owns many Wave listeners, so `lf stop shipper` leaves the keeper and `intelligence` running. Every addressed remote invocation carries `LF_EXPECTED_HOME_ID` and the remote process proves its durable identity before acting. Durable remote lifecycle uses credentials installed on the remote Home; foreground `lf ssh` may still borrow a bounded lease.

## Integration you own

Integration is one explicit, owned operation whose Git result is proved before Loopflow reports success (#1096). `lf pr publish` commits, pushes, and updates the review surface without rebasing — a PR may honestly stay behind base. `submit` and `land` clear `scratch/`, collapse checkpoint history into one tree-identical authored commit, replay it once, and push only after the owned rebase passes its postconditions.

- The operation record lives beside the worktree's absolute Git dir, not in the shared database, so sequencer state and advisory locks stay private per linked worktree.
- A conflicting sequencer is kept alive; one scoped recovery child uses `lf rebase --continue|--abort` while the parent still verifies and pushes.
- The fetched target SHA is pinned once so a concurrent fetch cannot move the proof, and `git ls-remote` is the authoritative remote-head check after a requested push.
- rerere is enabled per Loopflow command with auto-stage disabled, so identical conflicts reuse a reviewed resolution.
- `--adopt` is limited to stale or explicitly acknowledged raw rebases; it will not steal a live foreign operation.

## Failures you can read afterward

OpenCode failures now carry structured evidence in `ConversationEvent::Error`, persisted to `conversation.jsonl` and visible through `lf runs --launch <id>`. SSE disconnects, hollow idle turns, decode gaps, transport read errors, and upstream session errors are diagnosable from the durable receipt rather than by correlating transient logs against a raw provider stream. Model, provider, endpoint class, timing, and the last accepted SSE event are recorded; authorization headers, bearer tokens, and token query parameters are redacted first. Provider output-token counts are preserved where they prove a mapping gap rather than an empty model response (#1102).

Capture reconciliation got the same treatment from the other side. Known-absent conversations stay `pruned`, dead launches and running turns close atomically under explicit `interrupted` and `lost` states, and fresh partial capture loss stays red instead of being swept into a terminal state. `lf doctor` validates retained artifacts, and the 48-hour race guard is preserved. On a copied Home, the first pass removed 777 orphan directories, marked 82 intact captures interrupted and 10 ENOSPC partials lost, and pruned nothing; the second pass changed nothing (#1104).

## Operational notes

**Migrations and build identity.** A post-0.12 binary was able to publish an unreadable 0.11-namespace migration under the package-only 0.12.1 version string. That gap is closed (#1108). Newly introduced canonical migrations behind the active package namespace are now rejected, materialization requires explicit release or test authority, and every persisted JSON DTO gets semantic post-migration validation before commit. Post-tag builds now report a package-plus-revision version in `lf --version` and install output, so a running binary can be told apart from the release it claims.

This cut canonicalizes one draft as `0.12.001_retire_obsolete_pm_reopen_writebacks`, which retires Session-era pending Linear reopen writebacks that stable Tasks no longer perform. It also publishes `0.11.036_delete_sessions` and `0.11.037_capture_terminal_states`, landed during the cycle. As established in v0.12.0, only an official release install advances `~/.lf/loopflow.db`, under the drained-body promotion lock — upgrade through the release, never mid-turn. Running process generations are pinned to immutable `lf` executable bytes during promotion (#1107).

**Removed and changed commands.**

- `lf task attach` is gone. Attaching is `lf work feedback` against the Launch — the same surface with durable fencing.
- Skill-level `interactive:` frontmatter no longer triggers a flow pause. Declare `feedback: true` on the flow step that owns the door.
- The generic skill-frontmatter `sh -c` fast path is removed with no compatibility plumbing. Typed ops still attempt mechanics first and launch judgment only for a structured conflict.
- Account selectors take a login email, not an account ID. Update any scripted `--account`/`--only-account` arguments.

**Known gaps.** `refs/loopflow/recovery/*` is not pruned automatically. There is no general worktree lock over arbitrary raw Git commands — raw Git remains an explicit escape hatch, not a second supported ownership path. The operator-level ten-body GLM validation for OpenCode backup routing has not been run.

## Small changes

- `wt list` matches merged PRs by current branch head.
- Stale cooldown state is cleared when an account reconnects.
- A visible macOS Terminal handoff appears when Claude authorization cannot complete automatically.
- Revoked-token recovery is exercised in nightly release acceptance coverage.
- Troubleshooting docs explain the retry ladder and when to resume on another provider; `docs/config.md`, `docs/lf.md`, and `docs/security.md` follow the email-selector syntax.
- ruff 0.15.21 → 0.15.22 (#1066).
