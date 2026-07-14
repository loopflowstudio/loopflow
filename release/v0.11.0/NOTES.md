# v0.11.0

This is the release where loopflow stops being a distributed system. The `lfd` daemon, the `lfq` remote-exec door, the HTTP route surface, the queue reconciler, and a SQLite catalog carrying sixty-odd accreted migrations are all gone. What's left is one `lf` binary: a resident wave server you chat with, a store rebuilt from an honest baseline, and three planning nouns — **wave**, **project**, **task** — that the runtime now models directly instead of approximating with five overlapping ones. The `lf op` drawer is gone too; the mechanical operations you use every day are top-level commands. Upgrade for the smaller thing that does more: this release is a net deletion of tens of thousands of lines while adding a trustworthy run ledger, durable prompt traces, Linear-native planning, and a Mac app that finally shares the framework's name.

## The runtime is waves, projects, and tasks

The style guide has committed to three planning nouns for a while; the code hadn't. It does now. A wave is a durable operating context that owns memory, cadence, chat, and project selection. A project is a measured bet with KRs, pursued by a durable child session. A task is concrete work under a project, run by the same primitives. Everything that existed to support the older, more speculative model was deleted rather than ported — the daemon, the worktree fork machinery, the provider/session token plumbing, and the three-tier phase ladder that used to sit between a wave and its work.

- **`lfd` and `lfq` are gone** — no daemon, no HTTP service, no remote-exec door. `lf wave <name>` boots a resident server (listener, thread, playhead) in-process; `lf stop <name>` stands it down gracefully and is idempotent against a stale endpoint (#872, #866).
- **One session layer, shared** — wave, project, and task each run one policy skill (`*_clarify` / `*_mutate` / `*_pursue`) over a common child-session layer, instead of each tier growing its own executor. Launch and control state on a child is explicit; interrupting a stopped child is a no-op (#872).
- **The prompt unit is a *skill*** — `.lf/steps/` → `.lf/skills/`, `Step` → `Skill`, `--skill`, and every DTO field renamed across Rust, Python, and Swift. The persistent wave "mind" became the **flowloop**: a looping flow where the agent sets the termination bit and the runner checks it, replacing ~2.5k lines of tiered scheduler (#845).
- **Inline-first execution** — the current process and worktree are the default surface, not a dispatch desk. A wave resolves its own blocking move inline and spins off a child loop only for a strict subset that earns its own lifecycle or real parallelism. `LOOPFLOW.md` and the builtin skills were rewritten to teach that (#864).
- **`GOAL.md` is the durable charter** — Objective, Measures, Cron, Process. Routing judgment lives in Process; `primary_flow` is gone from the wave shape entirely (#843).

## One binary, one command grammar

`lf op` was a drawer you had to know about. Its contents are now first-class verbs, and the same grammar is used by humans, builtin flows, docs, prompts, and smoke tests.

```bash
lf commit -m "message" -p     # gate + commit + push
lf pr open / submit / land    # visible mid-stream / human merges / hands-off merge
lf wt create next-thing       # sibling worktree; --child to stack
lf pm show --wave systems
lf release run minor
```

- **Ops promoted to top level** — `lf pr`, `lf wt`, `lf rebase`, `lf commit`, `lf pm`, `lf release`, `lf cron`. Flow steps use the same payloads (`- op: pr land --create-pr`) (#853).
- **Retired spellings say what to use instead** — `lf op land` no longer reports "skill or flow not found: op". It errors with the replacement (`lf pr land`), and `lf op next`, which has none, says so. This matters for agents running an installed `lf` that predates the collapse (#874).
- **Flags cross to the subcommand that owns them** — the arg pre-parser derives flag ownership recursively from the clap definition, so a flag written after its subcommand normalizes to the canonical global-first order rather than failing (#864).
- **`lf chat` is the sole human thread surface** — `lf wavechat` is folded into `lf chat --follow`, one pane that watches the wave's turns and speaks into its thread. Machine speech has its own wire: `lf radio pub` / `lf radio sub`. The old bare `lf sub` fails with a pointer (#867, #870, #856).

## Planning lives in Linear, natively

The wave/project/task model now maps onto Linear one-for-one: a wave is an Initiative, a project is a Project, a task is an Issue. Linear owns project definitions and KRs in Project content — `wave/<wave>/projects/` becomes a generated offline cache and a migration seed, not a second editable source of truth.

- **Waves map to Initiatives** — `pm.linear_project` becomes `pm.linear_initiative` in `GOAL.md`; `lf pm init` creates the Initiative and migrates seeded project files into native Linear Projects (#865).
- **OAuth stops dropping** — tokens refresh automatically before expiry using a recorded PKCE client id, so a long-running wave keeps its connection (#865).
- **Task operations in wave-project terms** — create, move, close, filter, rename, and diagnose Linear tasks against the local wave project tree; `lf pm sync --plan` shows Linear/cache drift before writing anything (#852, #870).
- **Linear driving fixes found in headless runs** — `issueUpdate` no longer wipes an issue title when only the description changed; workflow-state queries type `$teamId` as `ID!` (what Linear's schema wants) rather than `String!`, so status transitions resolve; `lf pm show --json` emits parseable JSON with no progress chatter (#873, #834).
- **KRs read as proof** — the Wave Chat project's KRs were rewritten with explicit windows, pass conditions, and named failure events instead of aspirations (#863).

## Memory a wave can actually carry

`lf memory add` used to append a bullet onto `MEMORY.md`, so the compiled file accreted forever. The add stream and the compiled checkpoint are now separate things.

- **`add` publishes; `update` curates** — `add` writes a replayable fact to a memory stream without touching the checkpoint. `update` is the sole writer of `MEMORY.md` and clears the accumulated delta, since the checkpoint becomes the new seed (#823).
- **`lf memory log` reads the delta** — facts added since the last curation, through the live server's replay buffer when one is running, falling back to the journal fold otherwise. Those recent facts are also layered above the compiled base in a wave's prompt, so it sees them before the next fold (#833).
- **Scheduled curation without a live server** — `lf cron add --wave <w> --flow export-memory --schedule daily` installs a launchd job; the `export-memory` skill compiles `MEMORY.md` from base plus stream and commits it, preferring the live server and falling back to writing the file directly (#835). The compile organizes memory into typed blocks (Decisions, Constraints, Glossary, How To) — a suggested vocabulary the agent owns and can reshape, not an enforced schema (#837).

## See what your agents actually did

Two independent records, both local, both auditable after the fact.

- **A trustworthy run ledger** — canonical process identity, cumulative usage boundaries that reconcile exactly, codebase-weight history, and trace trees. `lf runs`, `lf usage`, `lf tokens`, and `lf doctor` all read the same evidence, and the Mac app's Telemetry page renders it: tokens by skill and model, cache reuse, and a zoomable code-weight icicle (#857).
- **Durable prompt and conversation capture** — every provider invocation writes the prompt and the normalized conversation exchange under `~/.lf/traces`. `lf trace <run-id>` prints paths and metadata; `--events` reads recorded bodies. `lf context` surfaces turn, asset, and inclusion-decision rows, keeping initial assembled context separate from follow-up input and provider-reported history (#871).
- **The journal survives concurrency** — appends to `events.jsonl` take an exclusive `flock` and write each event as one buffered call, so parallel writers stop interleaving partial lines into unparseable JSONL (#840).

## Loopflow on the Mac

The desktop app was called Concerto. It is now Loopflow, matching the framework — a shared `Loopflow` Swift library with `LoopflowMac` and `LoopflowiOS` app targets (#848). The wave viewer shows a wave's Objective beside its live projects, reading the wave directory directly rather than adding a new wire shape (#846). Wave Chat gained a truthful stop/retry story, and CI now compiles the Mac UI test runners **signed** (ad-hoc identity) rather than with signing disabled — it refuses to build an unsigned DMG (#866).

## Worktrees, stacks, and sandboxes

- **`WaveId` is the worktree identity** — one type, two deliberately non-derivable projections: a flat `dir_component` for the local path and an author-scoped `branch` for the remote. Creation is two relative-to-here verbs — **sibling** (the default, roots from main) and **child** (stacks under its parent) — with `lf wt up`/`down` stack navigation and a tree-view `list`. Landing no longer rotates the worktree; a merged worker's tree is pruned when its branch is deleted (#818).
- **Stacked children re-parent when their parent lands** — the old lazy rebase replayed a merged parent's commits against `origin/main` and blocked the queue with a spurious conflict. Landing is now detected content-independently (so a *reworked* parent is handled too) and the child rebases with `--onto <default> <parent-tip>`, dropping the parent's history exactly (#836).
- **Agents respect your sandbox config** — loopflow treats its permissions as a floor: it reads the effective Codex `config.toml` and Claude `settings.json`, supplies its conservative default only when yours is unset or weaker, and leaves permissive configs alone. Sessions launched from a worktree add the main repo as a writable directory, so Git worktree metadata stays writable without loosening the sandbox (#851).
- **Claude keeps its prompt in a worktree** — `--add-dir` is variadic and was swallowing the positional skill seed, opening a blank session (#880).

## Release engineering

- **Migrations are namespaced by release** — a migration's identity is `<major>.<minor>.<ordinal>` (`0.10.001_initial`), ordered by the numeric tuple so `0.9.001` correctly precedes `0.10.001`. `lf release check` now fails if a shipped migration was edited: rewriting one doesn't fix the databases already in the wild, it just makes their ledger lie. Databases carrying the old flat `001_initial` stamp are adopted byte-identically, so nothing runs and no data moves (#876).
- **The central `DECISIONS.md` ledger is retired** — release notes are synthesized from merged PRs and their descriptions; per-decision context lives in each wave's `MEMORY.md` (#839).
- **Release PR copy is deterministic** — the title reuses the release commit message and the body is the generated `RELEASE_NOTES.md` verbatim, so no LLM sits in the loop for a PR whose contents are already known (#854). Release worktrees use flat hyphenated names (`release-default-v0-11-0`), matching the worktree identity contract (#879).
- **Notes still generate on a bare runner** — when the host has no Claude/Codex/OpenCode CLI, the release falls back to deterministic notes instead of failing (#819).

## Small changes

- **`lf ssh` forwards local credentials** — GitHub, agent, and Linear tokens plus `--secret NAME` Doppler values are resolved *locally* and piped into a remote command's environment, so a stateless remote host can run authenticated `lf` while storing nothing. Values never touch argv, `ps`, or logs (#831).
- **`sync-skills` compiles into home only** (`~/.claude/skills`, `~/.agents/skills`). It never writes into a working repo, so skills resolve identically from any repo; `--global` is gone (#830, #850).
- **`lf doctor` drives off a declared `SYSTEM_DEPS` list**, adds `uv` and `doppler` as required deps, and generates the repo-root `Brewfile` (#827).
- **Git identity fallback** on headless hosts with no `user.name`/`user.email`; a configured identity is never overridden (#820).
- **Provider OAuth credentials fall back to Doppler** when the env vars are unset (#805).
- **Dependabot groups minor/patch bumps** into one PR per ecosystem; majors still open individually (#868). Bumps: `bytes` 1.12.1, `regex` 1.13.0, `ignore` 0.4.28, `rand` 0.10.2, `ruff` 0.15.21, `actions/cache` v6 (#858–#862, #869).
- **Dead code swept** — unreferenced store tables and pre-rename serde shims, the wave server's six hand-copied SSE stream closures, and the retired `LocalWaveService` (~1500 lines) (#821, #838, #846).

## Operational notes

- **The daemon is gone.** Anything that talked to `lfd` over HTTP, or dispatched through `lfq`, has no replacement endpoint. Use `lf` directly: `lf wave <name>` boots a wave, `lf stop <name>` stands it down.
- **`lf op <verb>` is retired.** Use `lf pr`, `lf wt`, `lf rebase`, `lf commit`, `lf pm`, `lf release`. Retired spellings error with their replacement rather than executing anything — update `.lf/` adaptations and prompts on other machines.
- **Reconnect Linear once** (`lf auth linear`) to record the PKCE client id that automatic refresh needs. Rename `pm.linear_project` to `pm.linear_initiative` in each wave's `GOAL.md`, then run `lf pm init --wave <w>` to create the Initiative and migrate seeded projects into Linear.
- **Rename `.lf/steps/` to `.lf/skills/`.** The prompt unit is a skill everywhere.
- **`lf wavechat` and bare `lf sub` are gone** — use `lf chat --follow` and `lf radio sub`.
- **New migrations use the release-namespaced form.** Create them with `uv run python scripts/new_migration.py <name>` and check with `uv run python scripts/check_migrations.py` (what CI and the release run). Editing a shipped migration is now a build failure.
