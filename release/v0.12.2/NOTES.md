# v0.12.2

v0.12.1 built the Run and Steer control spine alongside the Session lifecycle it was meant to replace. v0.12.2 finishes the demolition: Session is gone, Run is the sole executor, and everything that used to be "the session's problem" — provider continuity, account routing, failure recovery, where the work physically runs — now hangs off durable Run and Launch records. On top of that spine sit two surfaces a person actually touches: `lf queue` and `lf work feedback`, the explicit human door, and `lf work place`, which names the machine a Work runs on. Net across 235 files: 21,337 lines added, 25,628 removed.

## Run is the only executor

The parallel Project and Task Session lifecycle is deleted — tables, status mirrors, process generations, write leases, CRUD, and recovery bridges. Stable Project and Task Work now runs through one Run-authorized `__work` entrypoint, Launch owns provider continuity, and Work status is derived from durable execution evidence rather than mirrored into a second store. Completion and replacement are fenced by Run lease, Basis, containment, and successful-boundary evidence, so a Work cannot be marked done by a body that no longer holds authority (#1099).

Recovery is the payoff. A retryable failure no longer ends the Run — it selects the next exact provider, model, and account route and keeps going under the same Run identity, with one auditable history.

- Transient provider failures (capacity, 429s, 502/503, dropped connections) retry on a 2s/5s/15s/30s ladder, carrying the provider session token forward so Claude and Codex resume the interrupted task instead of restarting it (#1080).
- Subscription exhaustion takes the other path: the account is recorded as limited with its reported reset time, work fails over to the next account in the grant, and the agent is re-prompted to pick up where it left off. No alternate account means the original result is returned, not swallowed.
- Route selection weighs credentials, capacity, cooldowns, routing policy, grants, and account strain; it exhausts eligible accounts on the primary provider before falling back to the configured backup agent, and excludes routes already tried in the current chain (#1098, #1100).
- Harnesses are pinned to the account recorded on their Launch and reject route drift. Containment is stopped before a Launch is replaced — if it cannot be proven stopped, the Run is fenced rather than duplicated. Incompatible provider sessions are cleared when the account or provider changes.
- When every permitted route is unavailable, the Run records a typed `provider_route` capability wait instead of failing opaquely.

## A door humans can actually open

The durable noun formerly called Review is now **Feedback**, everywhere — store, DTOs, CLI, docs, and Swift. More consequential than the rename: the trigger moved from a skill's `interactive:` frontmatter to an explicit `feedback: true` on the flow step. A skill no longer silently makes every flow that references it block; the flow author declares where the human door is (#1093).

- `lf queue` lists Work holding User attention, oldest first.
- `lf work feedback <kind> <id>` opens the recorded Launch. With `--continue-on-exit`, a detached guard process (own session, flock'd per `LaunchId`) owns continuation for the lifetime of the pipe — a crashed or signalled client still advances the flow exactly once instead of stranding the Work.
- `lf work continue` advances past the current Feedback; `lf work escalate` hands a child's Feedback up to the User rather than answering it.
- `lf task attach` is removed. Attaching is `lf work feedback` against the Launch — same surface, durable fencing.

## Work runs somewhere specific

A durable Work now names one authoritative execution Home. A single `Placement { work, home_id, placed_at }` records where Work runs; a Home carries stable identity plus a mutable observed SSH route, so a machine can change address without moving the Work. Run reservation resolves placement itself and refuses on every Home except the placed one — a caller can no longer claim that remote Work ran locally (#1075).

```bash
lf home observe <home-id> ssh://jack@mini.local
lf work place wave <wave-id> <home-id>
lf start shipper intelligence     # one keeper per Home, one listener per Wave
lf stop shipper                   # leaves the keeper and intelligence running
```

New Waves begin on the local Home; Projects and Tasks inherit their parent's Home once, then own an explicit placement. A live Run fences placement changes. Every addressed remote invocation carries `LF_EXPECTED_HOME_ID`, and the remote process proves its durable identity before acting. Durable remote lifecycle uses credentials installed on the remote Home — only foreground `lf ssh` still borrows a bounded credential lease.

## Accounts addressed by who they are

Managed provider accounts are addressed by login email — full address or unambiguous prefix — instead of a hand-picked account ID. The path-safe internal ID is now a private storage key derived from the login, which kills the class of confusion where an account had two names and a connect could bind a login that didn't match the record it was stored under (#1095).

```bash
lf auth connect claude primary@example.com --chrome-profile primary@example.com
lf auth set claude jack@ --paid-through 2026-08-14
lf --account claude=jack@ --account codex=loopflow-eng@ implement
```

Authority also stops leaking and stops going stale. `lf auth accounts --verify` checks credentials live rather than trusting cache; a revoked token is recorded as missing, the exact reconnect command is printed, and work routes to the next healthy account instead of stranding (#1107). Terminal and SSH launch surfaces now carry the same routing as headless runs — managed Claude and Codex sessions honor repository/default account selection and account health, OpenCode Zen uses its stored provider credential, and remote-native boundaries strip provider API keys they don't own (#1109). Docs record the route order, the 95%-usage demotion, and which launch surfaces support what.

## Integration is one owned operation

Publishing a PR now commits, pushes, and updates the review surface **without rebasing** — a PR may honestly sit behind base until an explicit integration command. Submit and land clear `scratch/`, collapse checkpoint history into one tree-identical authored commit, replay it once, and push only after the owned rebase passes its postconditions, with `git ls-remote` as the authoritative remote-head proof (#1096).

- Operation records live beside the worktree's absolute Git dir, not in the shared database — sequencer state and advisory locks are already worktree-private.
- A conflicting rebase keeps its first sequencer alive; one scoped recovery child runs `lf rebase --continue` or `--abort` while the parent still verifies and pushes.
- The fetched target SHA is pinned once, so a concurrent fetch cannot move the proof.
- `rerere` is enabled per Loopflow command with auto-stage disabled, so an identical conflict reuses the resolution you already reviewed.
- `--adopt` is limited to stale or explicitly acknowledged raw rebases. Loopflow will not steal a live foreign operation, and there is no general lock over arbitrary raw Git.

## Failures you can read six weeks later

OpenCode failures carry structured evidence in `ConversationEvent::Error`, persisted to `conversation.jsonl` and visible through `lf runs --launch <launch-id>`. SSE disconnects, hollow turns, decode gaps, and upstream session errors are now diagnosable from the durable receipt instead of by correlating transient logs against a raw provider stream. Model, provider, endpoint class, timing, and the last accepted SSE event are recorded; authorization headers, bearer tokens, and token query parameters are redacted first (#1102).

Capture reconciliation stopped lying in the other direction. Terminal loss is now distinguished from absence: `pruned` is preserved for known-absent conversations, and explicit `interrupted` and `lost` states were added. Dead launches and running turns close atomically, while fresh partial capture loss stays red rather than being tidied away. Doctor validates retained artifacts and keeps the 48-hour race guard. On a copied Home, the first pass removed 777 orphan directories, marked 82 intact captures interrupted, and recorded 10 ENOSPC partials as lost; the second pass changed nothing (#1104).

## Operational notes

**Codex is the implicit agent.** With no `agent:` set in config, work runs on Codex. Explicit skill and config choices still win, and `agent:` remains the override in both global and repo config. Claude stays explicit for kickoff, review-design, demo, code-review, and prompt (#1097).

**Removed commands.** `lf task attach` is gone — use `lf work feedback`. The Session-era lifecycle storage is deleted outright; there is no compatibility shim, because the records no longer exist.

**Migrations are now scoped to the package release that introduced them.** A release cut concatenates its dependency-ordered drafts into one atomic `<major>.<minor>.<patch>.001_release.sql` batch, retaining `-- draft: <name>` provenance markers. Migration identity, parsing, and ordering gained an optional patch component, so a skipped patch release stays pending rather than being silently absorbed. Second batches for the same release, new legacy-format migrations, missing provenance, and forged provenance markers are all rejected (#1111).

The same work closes the gap that let a post-0.12 binary publish an unreadable 0.11 migration under the package-only 0.12.1 identity: canonical migrations behind the active package namespace are rejected, materialization requires explicit release or test authority, and every persisted JSON DTO gets semantic post-migration validation before commit. Post-tag builds now report a package-plus-revision version in `lf --version` and install output (#1108).

This release's batch, `0.12.2.001_release.sql`, retires obsolete pending Linear reopen writebacks left behind by the Session era — stable Tasks no longer retry that operation. Store promotion still advances `~/.lf/loopflow.db` only through an official release install under the drained-body promotion lock. Upgrade through the release; never mid-turn. Running process generations are pinned to immutable `lf` executable bytes during promotion (#1107).

## Small changes

- `lf land` clears `scratch/`.
- The unused generic skill-frontmatter `sh -c` fast path is removed; typed ops still try mechanics first and launch judgment only for a structured conflict.
- Revoked-token recovery is exercised in nightly release acceptance coverage.
- A visible macOS Terminal handoff appears when Claude authorization cannot complete automatically.
- ruff 0.15.21 → 0.15.22 (#1066).
