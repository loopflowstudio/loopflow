# v0.11.2

v0.11.0 collapsed the runtime to waves, projects, and tasks; v0.11.1 made the surfaces on top of it honest. v0.11.2 builds the operating machine around that core. A Wave now runs its work across many provider accounts and hosts, owns its own Linear team, and reports all of its intent and attention on one machine-wide plane. A Task can hand off to a human — an OAuth login, a manual gate — and resume exactly where it parked, or hand its next body to a different agent when a provider runs out of quota, without losing durable work. And because many builds now touch one shared production store, this release hardens migration and store authority so a branch checkout can no longer rewrite the database everything depends on.

## Run work across accounts and hosts

A single preferred account and a local-only session were the ceiling. This release replaces both with **profiles** — a routing identity bound to a Chrome profile on the host — and a Wave **Home**, a user-owned execution address that every surface probes, starts, and routes through one shared API. Provider accounts (Claude and Codex) attach to profiles, keep independent auth under `~/.lf/accounts/`, and rotate on rate limits and cooldowns while a running body stays pinned to the account it started on. Credentials never leave the host: only a locally refreshed access-token lease is forwarded over SSH stdin, never a refresh token or an auth file.

- **Profiles route each repo** — `lf profile route set --default … --backup …` gives a repository a default account plus ordered backups; a provider child stays pinned to its selected profile and account for its lifetime, and shared accounts are tried once behind a single cooldown (#936).
- **Managed OAuth, real browser, no device codes** — `lf auth connect claude` drives the OAuth handoff through Claude in Chrome when the extension is connected, falling back to a hidden terminal prompt otherwise; `lf auth connect codex` uses Codex's native local callback. Neither path prints URLs, state, or codes (#921, #943, #948, #926).
- **Adopt existing logins** — `lf auth import` adopts an active Claude or ChatGPT credential into an isolated managed home without re-authenticating, and binds an email-less Claude Code login by the selected Chrome email while keeping strict mismatch rejection (#959, #936).
- **A Wave has a Home** — `WaveHome { owner, location }` (`jack@local`, `ssh://jack@host`) is resolved, probed, and started idempotently through one control path: `start_home` returns a running resident instead of launching twice, and a reachable-but-stopped Home is started on the Home itself. Reachability is operational evidence, never enforced at parse time (#895).
- **A maintained cron host** — `lf cron sync --wave <wave>` reconciles a Mac mini's launchd jobs to the schedules declared in the Wave's `GOAL.md` frontmatter — installing, replacing, and pruning per declaration, and skipping non-daily expressions with a reason rather than mismapping them. `lf ssh` now bounds its probes, so an unhealthy host fails fast with a named error instead of hanging (#896, #893).

## Every Wave owns its Linear team

Every Wave shared one machine-global Linear team, so every Task was `W2-*` regardless of which Wave produced it. Now each Wave binds to its own team and its work gets wave-scoped identifiers (`PRD-*`, `INF-*`, `INT-*`). Reads follow Initiative → Project → Issue and stay team-agnostic, so binding a team steers only *creation* and never disturbs existing issues. Migration is explicit, conservative, and safe because Linear keeps the stable issue UUID across a team change — Session, PR, and comment links survive the renumber.

- **Bind and initialize** — `lf pm init --team-key <KEY>` adopts or creates a Wave's team and diagnoses a key or name already owned elsewhere instead of guessing; `pm.linear_team` records the stable team id in `GOAL.md` (#899).
- **Migrate the backlog** — `lf pm reteam` defaults to a dry run that names every issue it would move, defer, or leave, and only mutates with `--apply`. It moves Projects before their open Issues, defers any Task with a live Session, leaves completed `W2-*` issues historical, and comments each moved issue's prior identifier (#902, #930).
- **Human edits stream in** — verified Linear webhooks replace polling: an issue or comment edit becomes durable Task direction transactionally, exactly once, and wakes the owning Task Session without duplicate incorporation (#944).
- **`lf pm doctor`** flags a Wave with no team binding, two Waves sharing a team, and a Project stranded on a foreign team (#899, #907).

## One machine-wide view of work

`lf status` reports a single Wave. This release adds the plane above it: `lf roadmap` joins every Wave's durable local plan to whatever live execution evidence exists and buckets it into `now`, `needs_attention`, `available`, and `later`. The attention decision is made once in Rust — projecting PM, Session, process, PR, and local-progress evidence into green/red/black/unknown with one reason and a set of legal controls — and stamped on the wire, so the CLI, Mac, and iOS bucket identically without re-deriving the rule or re-running Git.

- **`lf roadmap`** prints planned work across every Wave (or one, with `--wave`), reusing the leaf snapshots `lf status` already emits (#900).
- **Shared attention, one reason** — the eight-state attention table lives in Rust; Swift consumes the contract and its constituent evidence directly rather than maintaining a second lifecycle state machine, and missing evidence stays visible as *unknown* instead of defaulting to clean (#933).
- **The Mac gets a NOW lens and roadmap controls** — a NOW ⇄ ROADMAP lens over one `lf roadmap` read shows all current and available work across every Wave; rows expose start, attach, resume, and interrupt, and a stopped Wave keeps its durable plan visible (#911, #904).
- **Control → Active Sessions** — a new Mac Control destination renders a machine-wide census of every live Wave, Project, Task, direct-execution body, and interactive handoff, grouped by Wave. A waiting handoff reddens its Task, Project, and Wave while its body is alive; only handoffs expose Open, everything else is view-only (#954).
- **Home on the Wave row** — each Mac Wave card shows its Home address, a probed liveness chip, and one contextual action (Open the running resident, or Start on the configured Home) (#918).

## Work survives the body running it

A durable Session and the provider process running it are now cleanly separated: the process plus its transcript is a replaceable "body generation," while the Session — identity, directive, worktree, supervision state, PR history — persists across handoffs. That separation is what lets work outlive a crashed process, a rate limit, or a wrong provider.

- **Lease a body to another agent** — `lf task resume INF-123 --model codex --reason "…"` keeps the Session, directive, worktree, and serial PR chain but hands the next generation to a different provider; plain `resume` continues the same transcript. It refuses while another body is still writing (#901).
- **One writer at a time** — a per-generation write lease (`ChildLeaseToken` + generation number, handed to the body via env) fences every store write that mutates a child Session, so two bodies can't both advance one Task; the token stays private and never surfaces in status or events (#903).
- **Observe the body, not the intent** — `BodyObservation` (Working, Stalled, Recovering, NeedsInput, Stopped, Failed, Terminal, Unobservable) records what the *current* body is doing, separate from durable Session state (#898).
- **Successors after a terminal pursuit** — an abandoned Project Session stays queryable and spawns exactly one pinned successor, linked by `predecessor_session_id` / `successor_session_id` (#894).
- **Runs are skill invocations** — `lf runs` reports agent-backed skill calls with context and token totals (the grain that owns cost and outcome); raw process orchestration moved to `lf execs`, and `lf trace <exec-id>` reconstructs a process tree (#906).
- **Dev builds can't touch production** — a build carries embedded provenance; a development build resolves a per-worktree store under `~/.lf-dev/` and refuses `~/.lf/loopflow.db` outright, so a branch checkout can no longer migrate the shared production store (#908, #964).

## Interactive handoffs and the Task review lifecycle

Some steps only a human can do — an OAuth login, a manual gate, an attended review. This release makes that rendezvous durable. A Task gets one lifecycle (kickoff once, then iterate and gate), interactive steps become durable review records, and a body that opens a handoff parks its parent on a human and resumes exactly once — replaying to the same rendezvous across process death, app restart, and host restart because every body re-derives its obligation from durable records at birth.

- **One Task lifecycle** — kickoff, iterate, and gate flows are pinned per Task; a gate change returns the same Session, transcript, worktree, and PR history to another iterate cycle, and existing Tasks migrate straight into Iterate so upgrades don't replay kickoff (#945).
- **Park on a human, resume once** — when a body opens an interactive handoff the parent blocks until the handoff is terminal; `lf handoff complete` advances the step, and `lf handoff hand-back --summary "…"` resumes the same step with the human's summary as the next message (#953, #956).
- **Durable review conversation** — an `InteractionReview` bound to an exact phase epoch and step cursor carries a FIFO parent↔Task dialogue and an authorized disposition; a later gate epoch opens a fresh review (#951, #957).
- **Headless keeps the exercise** — a headless Task routes its interactive steps to the owning Project instead of skipping them; interactive builtin skills now describe both attended and parent-reviewer modes (#952, #955).
- **Attach from the CLI** — `lf handoff present <id>` execs into the interactive terminal session and records first-attach evidence, closing the gap where `attach` returned a descriptor but never connected the human (#961).
- **Explicit interaction policy** — every resolved interactive step is either `WaitInteractive` or `DeferInteractive`; non-interactive skills run under defer, and no twin headless flow is needed (#941, #942).
- **Wave catch-up** — `lf reviews catch-up --wave <wave>` gathers every parent-reviewed review in a Wave with its Task/PR/disposition evidence and launches one attended human exercise; `--plan` prints the evidence packet without launching an agent. Catch-up can't rewrite source dispositions — findings become follow-up Tasks (#958).

## PR and worktree mechanics

The PR path learned the difference between publishing work and presenting it, and serial Task delivery stopped losing its worktree.

- **Publish vs present** — `lf pr publish` pushes and creates-or-refreshes headlessly and prints state + URL; `lf pr open` publishes *then* opens the browser for a human. A failed review-surface launch now fails only `lf pr open`, leaving the published PR untouched — headless runs no longer spawn browser windows or read a published PR as failed (#949).
- **One worktree across serial PRs** — a Task keeps its worktree as it rotates through serial PRs; `lf pr land --next <slug>` names the following branch, and the status snapshot shows ordered `prs`, the `active_pr`, and the next branch (#886).
- **Reconcile out-of-band merges** — `lf pr next` repairs a Task stranded by a GitHub auto-merge, preferring the merged PR over noisier siblings, rotating to the next serial PR, and carrying uncommitted edits forward (#913).
- **Stacked Tasks** — `lf task run CHILD --stack-on PARENT` forks a child worktree from the parent's active published PR, rebases child-only work when the parent moves, and refuses to land before the parent merges; `lf pr stack` is removed (#914).
- **Task PR parity** — publication gates on `M == B` (merge-base vs the recorded base commit) before any GitHub side effect, guaranteeing a Task PR carries only that Task's work (#924).
- **`lf wt list` is side-effect free** — bare inspection no longer fetches or fast-forwards main; use `--sync` to opt in (#912).
- **CI-derived next move** — for a Task with an open PR, `lf status` reads the required checks and reports CI as the owner while they're pending or failing, then hands back to Review once they're green (#916).
- **One ambient-wave resolver** — every `lf` command that acts on "the Wave I'm in" resolves it the same way from `LF_WAVE_ID`, so `lf pm` no longer ignores the environment; explicit `--wave` still wins (#915).

## Wave Chat

Chat keeps getting faster and more legible without changing what it is.

- **Delta-granular streaming** — the live wire opened by re-broadcasting the whole accumulated turn on every token (O(prose²) bytes). It now opens a turn with a whole `turn` frame and rides each content increment on a small `turn-delta`, so speech renders token-by-token and a lagging client can `resync` (#897).
- **History paints first** — the Mac paints bounded durable history before live endpoint discovery, then reconciles the live replay by stable turn id while keeping honest missing/partial/unavailable states (#934).
- **Typed references** — inline links with popovers for typed references in the thread (#947).

## Operational notes

**Bare `lf` now opens the desktop app.** `lf` opens or focuses Loopflow.app (aliased as `lf desktop`); it no longer launches an implicit vendor session. Agent work requires a named skill, flow, or inline prompt (#931).

**`lf top` is the health surface.** `lf top` shows a one-hour output-token throughput graph and the live `lf` and provider processes with their worktree names, reading the release ledger through a query-only connection so source builds can observe telemetry without migration authority. Built-in operating instructions point agents at it when work feels slow (#938, #946).

**Migration authority now belongs to `main`.** A branch-local migration reordering (`0.11.009`–`0.11.012`) reached the live store before `main` established its canonical sequence. The fix landed in three parts and is why patch releases now carry migration-history machinery:

- **Convergence** — a store carrying a *known* permuted prefix is repaired in place: Loopflow verifies the complete schema, publishes an unmodified pre-migration backup, canonicalizes the ledger, and continues the chain. Genuine schema drift and unknown histories still fail closed. The live database retained all 82 Task Sessions and passed integrity checks through `0.11.015` (#940, #960).
- **Authority** — only clean `main`/tag installs and official packages advance the release-owned database; an unpublished branch build validates but never advances it. Applied migrations record SQL checksums, parent history, source/revision, provenance, and package version, and CI rejects any ordinal, name, or content divergence from `main`. `lf doctor` surfaces whether a binary has publishing or validation-only authority (#962).
- **No escape hatch** — the short-lived `LF_ALLOW_PRODUCTION_DB_FROM_DEV` break-glass override is removed entirely; a development build can never open the release store, even through aliases or `LF_DB_PATH` (#908, #964).

**Local release gate.** `scripts/test.py --all` now runs every phase under a printed wall-clock budget — an overrun is killed, process group and all, and reported as `TIMEOUT`. The real hosted `LoopflowUITests` run is split into a separately named required `--ui-host` gate rather than looking like it ran when it only compiled (#905).

## Small changes

- The Mac trajectory-and-evidence section (#922) was reverted in full — that interaction was not the desired conduct model (#925).
- The Mac preserves the last successful Wave detail when a refresh fails, using one cancellation-aware refresh task so a stale read can't replace a newer one (#932).
- Context Lab makes the text shaping a Wave's sessions inspectable — separating initial-prompt load, lifetime input, and peak request pressure against a ranked Sources worklist (#923).
- Curated `MEMORY.md` facts and KR proofs can bind to evidence `Receipt`s — stable pointers to the raw record (chat turn, worker report, PR) that justifies a claim, surviving Markdown edits, branch deletion, and migration (#919).
- `lf pull-local-bin` no longer dies under macOS bash 3.2 when run with no flags (#929).
- Docs drop retired commands from active infrastructure memory (#892).
