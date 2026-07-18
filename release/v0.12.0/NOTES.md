# v0.12.0

v0.12.0 answers four questions that v0.11.3 left ambient: *who* runs the work, *as whom* it spends, *on which machine* it lives, and *how it recovers when no one is watching*. Provider Accounts become the thing routes select and bill against; a Wave gains an addressable Home you can probe, start, and forward authority to over SSH; a Task now repairs its own red CI and recovers its own stranded bodies without a human; and every Wave owns its own Linear team instead of sharing one namespace. Underneath, one hard rule now governs upgrades: only an official release install may advance the production store, and the release cut is the single moment migrations are canonicalized.

## Accounts and Homes become routable

The biggest shift is that "which paid account an agent spends against" and "which machine hosts a Wave" are no longer implicit. Accounts are the routing primitive (#1053): routes order provider accounts directly with a store-wide default, and Chrome profiles demote to mere login venues rather than run-time identity. A Wave's execution site becomes a user-owned address you can inspect and act on.

- Give a Wave an execution address (`jack@local`, `ssh://jack@host`) and probe or idempotently start it through one API — `lf home probe|start` — surfaced as a per-card liveness chip with a contextual Open/Start action in the Mac app (#895, #918).
- Connect managed Claude and Codex accounts via native OAuth with a browser/Chrome-profile handoff — no device-code paste — and import or adopt existing logins without re-auth (#921, #926, #943, #948, #959, #1023, #970).
- Run any invocation as a chosen account with `lf --account <selector>` / `LF_ACCOUNT`, prefix-matched and live-verified; repeatable `--account claude=…` / `--only-account` grants flow over a lease broker (#1029, #1071).
- Route each repo through an ordered default+backup account list, resolving and refreshing each credential once (#936, #1053), and auto-demote a strained account (limit window ≥75% used and not yet reset) below healthy ones before the provider refuses (#1061).
- Forward account authority to a remote Home as a short-lived opaque lease over an owner-only reverse-forwarded socket — no credential files ever land on the remote, detached forms are rejected, and it fails closed (`docs/security.md`) (#1071).
- `lfd`, a machine-level Home daemon, receives signed Linear webhooks into a durable, deduplicated delivery inbox; installable via launchd/systemd with bounded logs (#1012, #1049).

Not yet live: `lf home`'s remote cold-start and attach transport shipped built and unit-tested but not verified against a live host (#895).

## Tasks repair and recover themselves

A Task now owns its full repair-and-recovery loop. When required checks go red on an open PR, the supervisor wakes a bounded ci-fix body, resumes that repair across a crash, settles it against the authoritative pushed head, and parks. When a body stalls, dies, or pins a lease, a live Project runner recovers it unattended and replay-safe. Completion is gated on real evidence, and every surface offers only actions the lifecycle can actually execute.

**Autonomous ci-fix repair**
- Persist a per-head CI reading so a waiting Task gets a CI-derived next move instead of sleeping through a failure, and keep an open PR CI-owned until fresh checks prove green before review handoff (#916, #1054, #1084).
- Wake one bounded ci-fix generation on a fresh current-head failure, idempotent per (head, failing set), seeding the failing leaves rather than the required roll-up — never opening a PR or rotating the branch (#967).
- Route wakes through a durable command ledger so a crash between "body armed" and "incident stamped" is recoverable, and settle exactly the wake the body was born for at turn exit, against a fresh GitHub read of the repaired head (#1024, #1026, #1018, #1063, #1070).
- Refuse to arm on land-time preconditions only `lf pr land` can green, classify ci-fix infrastructure failures as Blocked, and count autonomy only when a serviced ci-fix command owned the repair (#1062, #1001, #1042, #1045).

**Unattended recovery**
- Recover abandoned work into one linked successor that adopts the worktree, directive, and PR history, carrying external Linear direction across succession and refusing unsafe worktree shapes before any ownership moves (#980, #1007, #994, #995, #1014).
- Revoke dead leases on explicit resume for Tasks and Projects, release a lease stuck at `revoked` only on a fail-closed "provably gone" probe, recover stalled bodies replay-safely, redispatch stranded bodyless Sessions bounded by attempts, and self-heal worktree disk (#968, #973, #1055, #996, #1016, #1043).

**Completion and PR authority**
- Gate completion on settled PRs plus approved reviews, scoped to active epochs, withholding on committed follow-up past the merged tip while still settling over a proven-empty successor (#1015, #1060, #1050, #1056).
- Prove Task PR range M==B parity end-to-end and refuse contamination before any push; reconcile out-of-band merges and reopened PRs (#924, #977, #989, #913, #928, #1032, #1046, #1059).
- Derive one truthful legal-action model from total evidence, recommending only executable actions, and resolve caller authority as an explicit input (#1011, #1041, #1079).

## Every Wave owns its Linear team

The shared `W2-*` namespace is dissolved: each Wave binds its own Linear team and identifier prefix (ENG-, SCI-, PRD-). The release ships the whole arc — binding, migrating existing issues, linking PRs back to their issue, streaming human edits inbound, and resolving completion from the issue's own owning team.

- Bind a stable `pm.linear_team` in GOAL.md; `lf pm init --team-key` adopts or creates the team and steers only creation, leaving existing issues undisturbed (#899). The three real Waves' bindings landed (#1072).
- `lf pm reteam` migrates older issues into the Wave team — dry-run by default, renumbering via Linear's stable issue UUID so Session/PR/comment links survive, moving Projects before their Issues and legacy issues before narrowing a Project (#902, #930, #1078). Ownership tightened to exactly one team, refusing to mutate while any Task body can still write the old identifier; `lf pm doctor` flags a Project stranded on a foreign team (#992, #907).
- Opening/submitting/landing a Task PR writes an idempotent Linear attachment and managed comment without ever failing publication; degraded links surface in `lf task status` (#1010).
- Verified Linear webhooks replace polling — human issue and comment edits become durable Task direction exactly once and wake the owning Session (#944).
- `lf pm task done` resolves workflow state from the issue's own team (a Project can span teams), and `lf task reconcile` lets a Wave or Operator attest that an applied-but-unincorporated final directive was incorporated, settling a merged, Linear-complete Task with no new PR or provider turn (#1085, #1087, #1090).

## One roadmap across CLI and Mac

A machine-wide intent plane now spans CLI, Mac, and iOS from a single read. `lf roadmap --json` joins every Wave's durable plan to live execution evidence, buckets it in Rust, and stamps the sections on the wire so every surface projects identical Now / Available / Later views without re-deriving the rule.

- `lf roadmap` ships the cross-Wave intent view and a shared Task-reference contract for multi-Task skills; Mac consumes it for all-Wave controls and a Now ⇄ Roadmap lens carrying PM completion, Session status, and liveness as three orthogonal facts (#900, #904, #911).
- Task attention is projected once in Rust (one green/red/black/unknown signal with reason and legal controls) and consumed by Swift instead of a second state machine; the shared Wave/Project/Task lens folds attention upward and preserves last-good detail across refresh failures (#933, #963, #932).
- Active Sessions lands as a new Control destination — a machine-wide census over `lf roadmap`, `lf runs`, and `lf handoff list --json` (#954).

## Durable Sessions, disposable bodies

A Session's durable identity — directive, worktree, PR history — is now cleanly separated from the disposable body that runs it, with an observation state model, fencing leases, successors, and honest interactive handoff.

- `BodyObservation` is a derived projection (Working, Stalled, NeedsInput, Terminal…) over intent, liveness, and progress, wired onto the runtime snapshot with a real event-log progress signal (#898, #985).
- Bodies are replaceable generations: `resume --model` hands off across providers keeping the Session, a monotonic write lease fences a single writer, and resumed bodies resolve the current Home lf rather than pinning the boot binary (#901, #903, #986).
- Interactive handoff gains a durable shared-store contract, explicit wait-vs-defer flow policy, same-step resume on hand-back, a CLI exec adapter, remembered-surface resolution made honest so only exact-attach surfaces claim `.attach`, and IDE attach for known Claude sessions (#935, #941, #956, #961, #969, #978, #998).
- `opencode serve` process trees spawn in their own group and are group-killed on stop, drop, signal, and at resident boot, ending orphan leaks (#1000).

## Spend you can actually read

Usage collapsed onto one grain: per-boundary deltas that readers sum instead of diff. `lf usage` now answers the subscription question — how much of each account's plan remains — with plans and window headers read from the provider, not hand-entered.

- One spend grain: `run_events` become per-boundary deltas written at drain; migration `0.11.025` converts history (Claude totals preserved) and de-cumulates Codex turns that had inflated input to ~9.6B tokens (#1022).
- Subscription-first `lf usage` over a new `provider_account_limits` table fed by harness rate-limit snapshots and on-demand Claude/Codex polling; revoked credentials surface `lf auth connect`; PLAN, SESSION USED, WEEKLY USED, and % TOKENS all read as provider truth (#1022, #1025).
- `lf runs` / `lf status` report agent-backed skill runs with context, tokens, and cost; the raw process ledger moves to `lf execs` with `lf trace <exec-id>` (#906).
- `lf top` graphs one-hour output-token throughput alongside live lf and provider processes via query-only SQLite (#938, #946).
- opencode reports provider-measured tokens only when usage was actually reported (ending the zero-clobber), makes SSE hollow-body disconnects observable and recoverable, and a CI incident ledger with `lf ci` measures failed-PR recovery latency (#1036, #1020, #1021).
- Ops telemetry routes to git-ignored `.lf/tmp/metrics/ops.jsonl` behind a path-contract guard, so read-only ops stop dirtying the checkout dispatch depends on (#982, #999).

## Wave chat and review authority

The live wave stream went delta-granular, and wave chat learned to paint durable history first, then reconcile live. Review authority was re-grounded in inherited session identity rather than self-minted run ids.

- Delta-granular wire: full `turn` frames re-baseline, small `turn-delta` frames carry each new item, and `resync` heals lag — O(fragment) instead of O(prose²) (#897).
- Wave chat paints bounded durable history before endpoint discovery, reconciling live replay by stable turn id, with inline typed references and popovers, curated supervisor narration routed to conclusion + steps, and atomic journal delivery that truncates to a checkpoint on failure (#934, #947, #950, #990). Context Lab lets you inspect and edit the text shaping a Wave's sessions and spawn a real Project Task to refine it (#923).
- Review authority is decided from inherited session markers (`LF_TASK/PROJECT_SESSION_ID`, `LF_WAVE_ID`), not a self-minted `LF_RUN_ID`, so humans can dispose reviews; a durable parent-dialogue protocol and attended `lf task review` gates ride on top (#1033, #1034, #951, #957).
- Phase budgets are bounded with a split-off required `--ui-host` gate, and pre-land budget evidence persists under the git common dir with `--history` verdicts (#905, #917, #1057).
- The docs and website were retold agent-first: README as landing page, reference material moved into `docs/`, and the operating contract shipped as a skill (#1069).

## Operational notes

**The promotion boundary.** v0.12.0 makes a hard rule out of a lesson learned twice: advancing `~/.lf/loopflow.db` past the installed binary's frontier is no longer a side effect of any ordinary command. Only `lf install promote` writes to the shared store, under a machine-global exclusive promotion lock that fences live Task and Project bodies; every other open is read/validate-only (#1086, #1077). Dev builds carry embedded provenance and resolve a per-worktree store under `~/.lf-dev/…` — they can no longer touch the production DB, and the `LF_ALLOW_PRODUCTION_DB_FROM_DEV` break-glass was removed entirely (#908, #964).

- Gate every promotion on `lf install preflight [--json]`, a read-only verdict (`Promote` / `PromoteAndMigrate` / `Reject`) that fails closed on unreadable evidence and names every blocker — any live lease blocks replacement, leaving all targets untouched (#1074). `lf install promote` content-addresses the binary into `~/.lf/bin`, atomically repoints the CLI symlink, retains immutable rollback bytes, and activates before migrating; `install.py` now only stages artifacts and every global mutation (CLI swap, app bundle, helper, skills, rollback) routes through the Rust boundary (#1077, #1082, #1083).
- **Migration authoring changed.** Author drafts ordinal-free with `new_migration.py <name>` and declare order via `--depends-on` — no git fetch, no rebase, no hand-assigned ordinals. Drafts are files named `<name>__<token>.sql` that touch neither the Rust registry nor git. Canonicalization happens once, at `lf release <version>`, which assigns contiguous ordinals and deterministic ids inside the release worktree so the generated files land in the release PR under real CI (#1076, #1081).
- This release adds migrations `0.11.019`–`0.11.029`: task PR GitHub observation and Linear linkage, provider deliveries, Task Session successors, pruned capture state, CI incidents (and repaired-head), usage deltas, lineage boundary, and accounts-first. It also converges previously permuted live histories and can initialize an existing schema-less database (#940, #960, #962, #1035). A no-op migration open no longer takes an exclusive write lock that was killing five-second lease heartbeats (#1030).
- **Upgrade path:** install v0.12.0 through an official release; only that install advances the store, and it does so under the drained-body promotion lock — never mid-turn.

## Small changes

- `lf` opens the desktop app by default (#931).
- Bare `opencode` resolves to Loopflow's `opencode/glm-5.2` default, and Codex launches are forced to standard `service_tier=default` at the shared boundary (#965, #976).
- `lf status` treats terminal Sessions for an absent Project as history, not a fatal error (#1002).
- `lf wt list` is side-effect free (#912), and `lf wt ci --logs` resolves Actions run/job ids (#997).
- `lf doctor` reports the running build's revision against merged main, plus provenance and the resolved DB path (#1065, #908).
- `pull-local-bin` no longer breaks on empty-args expansion under macOS bash 3.2 (#929).
- CI stopped caching Xcode DerivedData in `loopflow-ui-test` (#1037).

The Mac Trajectory/evidence memory section (#922) was reverted in full (#925); its conduct model was rejected before further work built on it.
