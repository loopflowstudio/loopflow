# Loopflow Architecture Report

91k lines across 3 languages. Ships a daemon, macOS app, iOS app, CLI, prompt engine, Python API, and wave scheduler. This report examines why the codebase is compact, compares deeply to peers, and places loopflow in the broader open source landscape.

---

## Product strategy context (public loopflow vs private Symphonia/lfdhub)

This architecture should be judged against the intended split:

- **loopflow (public OSS):** single-developer, local-first orchestration that can integrate into large-company workflows without requiring company-wide platform adoption.
- **Symphonia / lfdhub (private/paid):** team-scale control plane for cross-user coordination, hierarchical orchestration, governance, and hosted operations.

### Is this split a good strategy?

Yes — this is a proven open-source platform pattern **if** the boundary is explicit and enforced.

### Lessons from open-source platform companies

Patterns that work repeatedly (GitLab, HashiCorp/Terraform, Grafana, Sentry, Supabase style models):

1. **Keep OSS excellent as a standalone product.**  
   If OSS feels crippled, trust erodes and adoption stalls.
2. **Monetize control-plane concerns, not basic execution.**  
   Team identity, governance, policy, fleet management, analytics, compliance, and reliability SLOs are durable paid surfaces.
3. **Define the boundary in architecture, not only in pricing pages.**  
   Clear APIs/contracts prevent accidental feature leakage.
4. **Avoid split-brain engineering between repos.**  
   Shared protocol definitions and compatibility tests are mandatory.

### Strategic risks for this split

1. **Monetization boundary bleed**  
   If lfdhub-critical coordination features land in loopflow by default, paid differentiation weakens.
2. **Dual-repo drift**  
   Public/private repos can diverge in protocol and behavior, increasing integration cost.
3. **False simplicity in OSS**  
   If loopflow accumulates enterprise concerns, it loses the elegance/local-first advantage.

### Guardrails to keep now

1. **Decision rule:**  
   If a feature requires org identity, cross-user state, policy/compliance, or fleet operations, it belongs in lfdhub by default.
2. **Contract-first boundary:**  
   Treat lfd as an edge runtime with a versioned control protocol.
3. **Compatibility discipline:**  
   Add hub↔lfd contract tests and capability negotiation early.
4. **OSS simplicity bar:**  
   Loopflow changes should optimize for single-developer UX first.
5. **Public API-first for lfdhub:**  
   lfdhub must expose a clear public API.
6. **Subset rule for lfd API:**  
   Treat lfd API as a semantic subset of lfdhub API by default (same resources/verbs/error model where overlap exists).

---

## Case studies: open-source + paid platform splits

These cases are most relevant to loopflow/lfdhub.

| Company | Split pattern | What happened | Lesson for loopflow |
|---|---|---|---|
| GitLab | Open core with free + paid tiers, commercial features unlocked by license | GitLab explicitly documented CE/EE behavior and made upgrade paths low-friction (EE install can run free features; license key unlocks paid) | Keep upgrade from OSS runtime to paid control-plane simple and reversible; avoid migration pain as the default experience |
| Grafana Labs | Strong OSS core + Enterprise/Cloud add-ons | Grafana kept OSS central (including AGPL relicense) while monetizing enterprise plugins, governance, support, and Cloud operations | Monetize enterprise operational requirements without weakening OSS day-1 value |
| HashiCorp (Terraform) | OSS adoption funnel into Cloud/Enterprise, later BUSL switch | Licensing boundary shift triggered trust shock and a community fork | Boundary and license expectations are part of product contract; abrupt changes can permanently damage ecosystem trust |
| OpenTofu | Fork under neutral foundation governance | Fork moved quickly from manifesto to public repo to GA, emphasizing neutrality and predictable governance | If trust breaks, ecosystems can re-coordinate fast; governance credibility matters as much as code quality |
| Elastic → OpenSearch | Source-available shift to restrict direct hosted competition | Elastic protected commercialization path; ecosystem still produced a durable fork (OpenSearch) | Restrictive licensing can preserve monetization but may split community and mindshare |
| Supabase | OSS building blocks + managed platform value | Self-hosting is possible, but docs show multi-service ops burden and recommend managed path for many users | Team-scale hosted operations are a real paid value surface when OSS remains usable but operationally heavy |
| Sentry | OSS/self-host available with hosted SaaS advantages | Self-host exists, but hosted SaaS gets faster support and often earlier feature availability | Support, operational excellence, and release velocity are strong monetization boundaries |

### Sources

- GitLab tier/distribution model: https://handbook.gitlab.com/handbook/marketing/brand-and-product-marketing/product-and-solution-marketing/tiers/  
- GitLab CE/EE repo architecture notes: https://about.gitlab.com/blog/gitlab-tiers/  
- Grafana OSS vs Enterprise docs: https://grafana.com/docs/grafana/latest/  
- Grafana relicense announcement: https://grafana.com/blog/grafana-loki-tempo-relicensing-to-agplv3/  
- Grafana enterprise boundary philosophy: https://grafana.com/blog/2019/09/04/how-we-differentiate-grafana-enterprise-from-open-source-grafana/  
- HashiCorp BUSL update/FAQ: https://www.hashicorp.com/en/blog/hashicorp-updates-licensing-faq-based-on-community-questions  
- HashiCorp BUSL announcement mirror: https://www.hashicorp.com/ja/blog/hashicorp-adopts-business-source-license  
- OpenTofu manifesto and fork timeline: https://opentofu.org/manifesto/ and https://opentofu.org/blog/the-opentofu-fork-is-now-available/  
- Elastic license clarification: https://www.elastic.co/blog/license-change-clarification/  
- OpenSearch GA (fork outcome): https://opensearch.org/blog/opensearch-general-availability-announcement/  
- Supabase self-hosting complexity: https://supabase.com/docs/guides/hosting/docker  
- Sentry hosted vs self-hosted differences: https://sentry.zendesk.com/hc/en-us/articles/39647157386139-What-are-the-main-differences-between-SaaS-and-Self-Hosted-Sentry  

### What to copy vs avoid

**Copy**
1. GitLab/Grafana pattern: clear “enterprise value = governance/support/scale,” not “cripple OSS.”
2. Supabase/Sentry pattern: monetize operational burden reduction and support guarantees.
3. Explicit tier semantics and upgrade path documentation from day one.

**Avoid**
1. Boundary ambiguity that looks like retroactive rule changes (HashiCorp/Elastic trust shock pattern).
2. Tight coupling between OSS and paid logic without contract versioning.
3. Repo split without shared protocol tests and compatibility policy.

---

## The Numbers

```
Rust       45,951 lines   51%   daemon + engine + CLI + ops
Swift      20,360 lines   22%   Concerto (macOS) + LoopflowCore + tests
Markdown   15,587 lines   17%   prompts, docs, wave plans
Python      5,110 lines    6%   client library + scripts
Other       3,706 lines    4%   proto, SQL, YAML, shell, config
───────────────────────────────
Total      90,714 lines   527 tracked files   ~726k tokens
```

23% of code lines are tests. The whole repo fits in a 1M context window with 274k tokens to spare.

---

## Deeper branch-level findings (this review pass)

The original report is directionally right, but a deeper pass shows *where* architectural risk is concentrating:

- **Complexity is file-concentrated.** In `rust/loopflow/src`, the top 10 files hold **15,498 / 42,414 lines (36.5%)**. The top 20 hold **55.1%**.  
  Biggest hotspots: `lfd/executor/docker.rs` (2,839), `engine/prompt.rs` (2,561), `lfd/store/mod.rs` (1,846).
- **Store API surface is now mostly forwarding code.** `impl Store` is **406 lines**, including **39** `WaveStateStore` forwarders, **14** `ExecutionStore` forwarders, **2** `StoreAdmin` forwarders, plus **8** backend `match` blocks for sessions.
- **One prior recommendation is already shipped.** Session harnesses are already trait-based in `lfd/sessions/harness/mod.rs` (`SessionHarness` + provider constructors), so that item should move from “recommendation” to “done”.

Implication: the main risk is no longer “too many features,” it is **responsibility concentration inside a few coordinator files**.

---

## Extra-deep scan: broader-frame scorecard

This pass scans the codebase using clean-code frames (ownership, amplification, contracts, blast radius, operations), with concrete signals from current code.

| Frame | Evidence from code | Why it matters |
|---|---|---|
| Ownership boundaries | `lfd` is 27,295 LOC; top three subdomains are `executor` (5,689), `store` (5,491), `http` (4,777) = **58.5%** of `lfd` | Core behavior is concentrated in a few seams; these seams need strong contracts |
| API surface concentration | HTTP router defines **33** routes; `waves.rs` alone implements **16 handlers** (~48%) | One file carries a large share of product-facing API behavior |
| Data contract breadth | Store query catalog has **56** query variants | Good explicitness, but cross-dialect/query drift risk rises with catalog size |
| External boundary fragility | Rust code calls `Command::new(...)` **117** times across **32 files**, touching **12 binaries** (`git`, `gh`, `docker`, `claude`, `codex`, etc.) | Delegation stays thin, but behavior depends on many external CLI contracts |
| Runtime orchestration complexity | Scheduler starts **7** background loops (CI, ticker, watch, cron, queue reconcile, recovery, summary refresh) | Incident diagnosis spans multiple async loops and state transitions |
| Provider consistency | Engine execution supports `claude/codex/gemini/opencode`; sessions currently implement `claude/codex` and reject `gemini/opencode` | Capability drift between execution paths increases user surprise and maintenance cost |
| Client contract fragility | `LocalWaveService.swift` has **116** `json["..."]` key lookups (61 unique keys) | Manual JSON parsing increases break risk when server DTOs evolve |

### Change amplification signal (history sample)

From the last 200 commits, co-change clustering is strongest around store + HTTP:

- `store/mod.rs` ↔ `store/sqlite.rs` (**24** co-changes)
- `store/mod.rs` ↔ `store/postgres.rs` (**23**)
- `waves.rs` ↔ store files (**16–17**)
- `http/mod.rs` ↔ `http/routes/*` + store (**14–16**)

This suggests common feature work crosses API + persistence boundaries together, so reducing change amplification here has high leverage.

### Test-shape signal (where confidence is strongest vs weakest)

Test attribute density is strongest in `engine` (211 test attrs over 8,543 LOC), but thinner in high-risk lfd surfaces:

- `lfd/store`: 6 test attrs over 5,491 LOC
- `lfd/triggers`: 0 test attrs over 938 LOC
- `lfd/executor`: 30 test attrs over 5,689 LOC

Directionally: parsing/formatting logic is well-tested; persistent-state and background-loop invariants need more explicit coverage.

---

## Why It's Compact

### 1. Single daemon, multiple thin clients

lfd owns ALL state, persistence, and execution. Everything else is an HTTP wrapper.

```
openclaw:   650k TS + 82k Swift + 14k Kotlin  (3 full stacks)
loopflow:   46k Rust daemon + 7k shared Swift + 0.8k Python client
```

The daemon pattern means business logic lives in one place. Adding a new client costs hundreds of lines, not thousands. The Python client is 360 lines. LoopflowCore is 7k lines shared between macOS and iOS.

### 2. Delegates to agents instead of reimplementing

The single biggest architectural decision. Loopflow doesn't parse code, analyze ASTs, run linters, or interpret diffs. Agents do all of that.

```
ruff:       513k lines to parse/lint Python (builds its own AST + type checker)
codex:      348k lines including sandbox, tool dispatch, execution engine
loopflow:   0 lines of language parsing — agents read code, loopflow orchestrates
```

The engine module (8.6k lines) assembles context and hands it to a subprocess. It doesn't understand what the agent will do with that context.

### 3. File-based extension, not plugin architecture

Steps are markdown files. Flows are YAML files. Directions are markdown files. `build.rs` discovers files at compile time and embeds them as `include_str!()`. At runtime, user-defined steps override builtins.

### 4. Hand-written SQL, no ORM

13 migration files, ~500 lines total. Direct `rusqlite` and `tokio_postgres` calls. The store layer is ~1.8k lines with ~50 async methods. SQL is the contract.

### 5. One source of truth per concept

A Wave is a Rust struct. serde serializes it to JSON. Pydantic deserializes it in Python. Codable deserializes it in Swift. No schema file, no code generator, no IDL.

### 6. Subprocess over SDK

Loopflow doesn't import `anthropic`, `openai`, or `google.generativeai`. It spawns `claude`, `codex`, or `gemini` as subprocesses. No API version management, no retry logic, no streaming parser.

---

## Deep Comparisons

### vs opencode (198k lines, 2.2x larger)

opencode is a TypeScript/Bun monorepo with 17 packages. The headline "198k lines" is inflated — the actual coding agent core is ~53k LoC of TypeScript, and the TUI app adds ~50k more. The rest is a SaaS console, docs site, desktop Tauri shell, and asset files.

**Where opencode is bigger and why:**

| Area | opencode | loopflow | Why different |
|------|----------|----------|---------------|
| LLM integration | ~5k lines (Vercel AI SDK, 20+ providers) | ~500 lines (subprocess to CLI) | opencode imports SDK packages; loopflow spawns processes |
| Tool system | ~8k lines (44 files, Zod schemas, tool registry) | 0 (agents own their tools) | opencode dispatches tools itself; loopflow lets agents handle it |
| TUI | ~50k lines (SolidJS reactive TUI + web app) | 0 (no TUI, Concerto is native) | opencode builds a terminal UI framework; loopflow uses SwiftUI |
| Storage | ~3k lines (SQLite via Drizzle ORM) | ~5.5k lines (SQLite + Postgres, hand-written SQL) | loopflow is actually larger here — two backends |
| LSP client | ~2k lines (spawns language servers) | 0 | opencode talks to pyright etc. for hover/diagnostics |
| Cloud console | ~37k lines (SaaS dashboard, billing) | 0 | opencode has a hosted product |

**Key architectural difference:** opencode is a single-language (TypeScript) project that builds everything in one stack. SolidJS powers the TUI, web app, and desktop app identically. loopflow splits across three languages with a shared-nothing architecture connected by HTTP.

**Where loopflow is bigger:** storage (two SQL backends vs one ORM), native apps (17k Swift vs 0), and the prompt engine (2.5k lines of token-budgeted context assembly that opencode doesn't have an equivalent of — it relies on the LLM's native context window).

**The real comparison:** opencode's core agent (~53k TS) vs loopflow's Rust backend (~46k). Similar scale, different tradeoffs. opencode invests in tool dispatch and provider SDKs. loopflow invests in execution orchestration (flows, forks, waves, scheduling) and context assembly.

---

### vs convex (459k lines, 5x larger)

convex-backend is a Rust+TypeScript monorepo with 65+ Rust crates and 30+ npm packages. It builds its own database from scratch.

**What makes convex enormous:**

1. **Custom MVCC database engine** (~30k lines) — snapshot manager, OCC commit protocol, write log, transaction index. loopflow uses SQLite/Postgres as-is.

2. **Embedded V8 JavaScript runtime** (~35k lines) — Deno Core integration to run user-defined queries/mutations inside V8 isolates, with reimplemented Web APIs (fetch, crypto, streams, console). loopflow has no runtime — agents bring their own.

3. **Reactive subscription system** (~10k lines) — read-set tracking, interval-based overlap detection, write-log scanning, WebSocket state transitions. loopflow's closest equivalent is SSE event streaming (~500 lines).

4. **Full-text + vector search** (~20k lines) — Tantivy for text, Qdrant segments for vectors. loopflow has no search.

5. **Three persistence backends** (~15k lines) — SQLite, Postgres, MySQL all behind a `Persistence` trait. loopflow has two (SQLite + Postgres) in ~5.5k lines, but convex's persistence is an append-only log with custom indexing, not standard SQL.

6. **Dashboard + CLI + client SDK** (~80k TS) — Next.js admin dashboard, full CLI with deploy/codegen, React hooks (`useQuery`, `useMutation`). loopflow's equivalents total ~18k (Concerto + Python CLI + lfq).

7. **Custom value system** (~10k lines) — ConvexValue with its own types, sorting, serialization. loopflow uses serde JSON.

**Architectural insight:** convex is a database company. It builds everything below the application layer. loopflow is an orchestration layer that sits above everything. They're at opposite ends of the stack — convex goes deep (custom storage, custom runtime, custom reactivity), loopflow goes wide (many clients, many agents, many workflows).

If convex adopted loopflow's delegation philosophy, it wouldn't exist — you can't delegate "be a reactive database" to a subprocess. Conversely, if loopflow adopted convex's "build everything" philosophy, it would be 300k+ lines and would still need external coding agents.

---

### vs supabase (549k lines, 6x larger)

The supabase/supabase monorepo is misleading — it's primarily a **frontend/docs monorepo**. The actual backend services live in separate repos.

**What's in the 549k:**

```
apps/studio     13.4 MB   55% of all TypeScript — the dashboard (3,029 files)
apps/www         3.8 MB   marketing website
apps/docs        1.2 MB   documentation site
packages/*       3.2 MB   shared UI, types, utilities
everything else  2.4 MB   design system, examples, config
```

The Studio dashboard alone is ~335k lines of TypeScript. It's a massive Next.js app with 46 feature modules (Auth, Billing, Database, SQLEditor, Storage, Realtime, Observability...). This is where the LOC lives.

**The real supabase backend** is distributed across separate repos:

| Service | Repo | Language | What it does |
|---|---|---|---|
| Auth (GoTrue) | supabase/auth | Go | JWT auth, user management |
| Realtime | supabase/realtime | Elixir | WebSocket CDC from Postgres |
| Storage | supabase/storage | TypeScript | S3-compatible file storage |
| Edge Runtime | supabase/edge-runtime | Rust | Deno-based edge functions |
| pg_graphql | supabase/pg_graphql | Rust | GraphQL Postgres extension |
| Supavisor | supabase/supavisor | Elixir | Connection pooler |
| CLI | supabase/cli | Go | Local dev, migrations |

Self-hosted supabase is **15 Docker services**. The collective backend is probably 200-400k lines across Go, Elixir, Rust, and Haskell.

**Architectural comparison:**

supabase wraps Postgres with companion services. loopflow wraps coding agents with a daemon. Both follow the same pattern: don't reimplement the core (Postgres / LLMs), build orchestration around it.

The key difference is the dashboard. supabase's Studio at 335k lines is a full IDE for Postgres — SQL editor, table viewer, auth management, storage browser, real-time inspector. loopflow's Concerto at 17k lines is a wave manager. If loopflow built a full agent IDE (code editor, diff viewer, session debugger, prompt inspector), it would grow similarly.

**What loopflow can learn from supabase:** the `pg-meta` package introspects Postgres by running raw SQL against system catalogs — no ORM, no abstraction. Same philosophy as loopflow's hand-written SQL.

---

### Comparison framework for infra decisions

Use peer comparisons as a decision tool, not a LOC leaderboard.

#### Compare on these four axes

1. **Control-plane size**  
   Compare orchestration core to orchestration core (exclude dashboards/docs/marketing).
2. **Boundary fragility**  
   Measure external contract risk: provider CLIs, Docker behavior, git semantics.
3. **Blast radius**  
   Measure how many modules/systems a typical feature or fix touches.
4. **Operational complexity**  
   Measure how hard failures are to diagnose and recover in production.

#### Quick qualitative read (current state)

| Project | Control-plane size | Boundary fragility | Blast radius | Operational complexity |
|---|---|---|---|---|
| loopflow | Small | Medium-high (delegation-heavy) | Medium (hotspot files) | Medium |
| opencode | Medium | Medium | Medium-high (tool + provider + UI coupling) | Medium-high |
| convex | Large | Lower external CLI fragility, higher internal system complexity | High | High |
| supabase (ecosystem) | Large distributed surface | Medium | High (multi-service) | High |

#### Practical use

- Keep loopflow’s delegation model.
- Prioritize reducing fragility and change amplification in hotspot modules.
- Judge infra progress by contract hardening and blast-radius reduction, not only by LOC.

---

## What's NOT Built (By Design)

| Capability | How loopflow handles it | What building it costs others |
|---|---|---|
| Language parsing / AST | Agents read source code | ruff: 200k+ lines of parser |
| Sandbox / container runtime | Shells out to Docker | codex: custom sandbox ~50k lines |
| LLM API client | Subprocess to agent CLI | opencode: 5k lines of SDK wiring |
| Tool dispatch | Agents own their tools | opencode: 8k lines of tool registry |
| TUI | No TUI, native app instead | opencode: 50k lines of SolidJS TUI |
| Plugin system | Markdown/YAML files on disk | VS Code: extension host + marketplace |
| Database engine | Uses SQLite/Postgres | convex: 30k lines of custom MVCC |
| JS runtime | N/A | convex: 35k lines of V8 embedding |
| Reactive subscriptions | SSE event streaming | convex: 10k lines of read-set tracking |
| Admin dashboard (full) | Concerto (17k lines) | supabase Studio: 335k lines |
| Auth UI | Delegates to `gh` CLI | supabase: full auth dashboard |

---

## What IS Built (Densely)

### lfd daemon — 27k lines

```
lfd/executor    5,568   Spawn agents (Docker/local), manage worktrees, recover state
lfd/store       5,491   Persist waves, runs, sessions — SQLite + Postgres
lfd/http        4,785   REST API, WebSocket for sessions, SSE for events
lfd/sessions    3,397   Interactive agent sessions with streaming
lfd/config      1,140   Load lfd.yaml, resolve auth, GitHub tokens
lfd/service     1,011   Wave lifecycle: create, start, stop, status
lfd/types         967   Domain types: Wave, WaveRun, Agent, Session
lfd/triggers      938   Cron stimuli, GitHub webhooks, cross-wave listening
lfd/queue         804   State machine: pending → queued → running → done
lfd/auth          634   Token validation, provider dispatch
lfd/other       2,448   GitHub, security, redaction, scheduling, IDs, prompts
```

### engine — 8.6k lines

```
engine/prompt     2,500   Token-budgeted context assembly
engine/config       971   Frontmatter parsing, step/flow/direction discovery
engine/flow         834   YAML flow parsing, step dispatch, fork execution
engine/git          820   Merge-base, rebase, push, PR creation via subprocess
engine/stream     1,000   Real-time output parsing from agent stdout
engine/agent        500   Process spawning, environment setup
```

### Concerto + LoopflowCore — 17k lines

A dual-platform native app. SwiftUI views over `@Observable` state. LoopflowCore shared between macOS and iOS. No local persistence — all state comes from lfd.

---

## The Landscape

### Where loopflow sits

```
SIZE TIERS

Micro    <20k     Moya (14k), Vapor (39k), TCA (40k)
Small    20-100k  loopflow (91k), mlc-llm (92k), LiveKit (92k), Mastodon iOS (99k)
Medium   100-300k opencode (198k), jj (210k), Keras (261k), LangChain (282k)
Large    300-800k codex (348k), openclaw (764k), ruff (513k), convex (459k)
Massive  800k+    Transformers (1.6M), PyTorch (3.5M)
```

### Projects admired for elegance at similar or smaller scale

| Project | LOC | Language | What makes it elegant |
|---|---|---|---|
| antirez/kilo | 1k | C | Functional text editor in 1000 lines |
| zoxide | 1.7k | Rust | Replaces cd across every shell on every OS |
| charmbracelet/bubbletea | 3.7k | Go | Complete TUI framework, Elm architecture |
| age | 7.4k | Go | Modern encryption. Small size IS the security argument |
| solidjs/solid | 11k | TypeScript | React-level capability without virtual DOM |
| serde | 31k | Rust | Serialization for the entire Rust ecosystem |
| preact | 26k | JS | Full React API in 3kB gzipped |
| esbuild | 128k | Go | 100x faster than webpack, zero dependencies |
| Redis | 209k | C | Full database server that fits in your head |
| SQLite | 358k | C | Most deployed database in history. Test suite is 700x larger |

The **Charm ecosystem** (bubbletea 3.7k + lipgloss 8.4k + gum 4.1k + vhs 4.2k) deserves special mention. Each library does one thing, composes via Go interfaces, zero unnecessary abstraction. The most disciplined application of Unix philosophy in modern open source.

### Pattern: "smart routers" at ~90k lines

loopflow, mlc-llm (92k), and LiveKit Agents (92k) all cluster at the same size. All three are orchestration layers that delegate heavy lifting to external systems. This appears to be the natural size for a well-scoped coordinator: enough to handle scheduling, state, and protocol translation, but not enough to reimplement the capabilities being coordinated.

---

## Architectural Risks of Being Small

1. **Subprocess coupling** — CLI interface changes in Claude/Codex/Gemini break loopflow. No SDK means no version pinning.
2. **No offline mode** — everything requires the daemon.
3. **Single-process daemon** — no horizontal scaling. Fine for dev tool, limits SaaS.
4. **SQL without safety nets** — hand-written SQL means no compile-time query validation.
5. **Thin client limits** — Python client is useless without lfd.

These are acceptable tradeoffs for a developer tool. Most could be addressed incrementally.

---

## Summary

Three principles:

1. **Centralize state, distribute UI** — one daemon, many thin clients
2. **Delegate, don't reimplement** — agents parse code, Docker runs containers, git manages history
3. **Files over frameworks** — steps are markdown, flows are YAML, config is YAML

The result is 91k lines that ship more surface area than many 300k+ projects. The closest architectural peers are LiveKit Agents and mlc-llm — other "smart routers" that coordinate external capabilities rather than reimplementing them.

Compared to direct peers: opencode invests in tool dispatch and provider SDKs where loopflow invests in execution orchestration and context assembly. convex builds a database engine from scratch where loopflow uses SQLite. supabase builds a 335k-line admin dashboard where loopflow builds a 17k-line native app. Each made different tradeoffs. loopflow's bet is that agents get smarter and the orchestration layer stays thin.

---

## Goal-fit assessment for *this* repo

Against loopflow’s public-OSS goals (individual developer orchestration, elegance, local-first operation), the repo is directionally strong:

### Where this repo is doing well for its mission

1. **Local-first runtime shape is correct.**  
   Single daemon + thin clients keeps adoption friction low.
2. **Delegation keeps complexity bounded.**  
   Subprocess model avoids reimplementing provider stacks.
3. **Composable extension model works.**  
   Filesystem steps/flows/directions support experimentation without platform buy-in.

### Where more investment is needed to prepare for Symphonia/lfdhub

1. **Boundary hardening (edge runtime contract).**  
   lfd should expose a stable, explicit control-plane interface that lfdhub can rely on.
2. **Public API alignment (hub-first contract).**  
   lfdhub should have a clear public API, and lfd should consume that API directly where possible.
3. **Capability negotiation + versioning.**  
   Hub/edge compatibility should be explicit (not inferred from behavior).
4. **Subset exposure discipline.**  
   lfd API should be a documented semantic subset of lfdhub API unless explicitly marked local-only.
5. **Drift prevention across public/private repos.**  
   Shared protocol schemas + compatibility tests are needed early.
6. **Reduce hotspot concentration before adding team-scale hooks.**  
   Otherwise future hub integration will amplify change cost.

In short: the architecture is good for current goals, but contract discipline must increase now to make the two-layer future practical.

---

## Line-Level Efficiency Analysis

Beyond file and module structure, this pass focused on **concentration risk** and **boundary quality**.

### What's still excellent

- **`StoreRow` is high-leverage abstraction.** One mapping surface across SQLite/Postgres still avoids widespread row-decoding duplication.
- **SQL catalog remains a strong contract.** Query identifiers are explicit and centralized, which keeps SQL discoverable and diff-friendly.
- **Prompt budgeting model is extensible.** `ContextBreakdown` + source-tagged documents remain a good backbone for future prompt policy work.
- **Python client remains appropriately thin.** `client.py` at 360 lines is still the right shape for an HTTP wrapper.

### What is now the bigger issue

1. **Hotspot monoliths (new high-priority risk).**  
   `docker.rs` and `prompt.rs` are each >2.5k lines and carry mixed responsibilities (execution, workspace, recovery, path rewriting, budget policy, formatting). This raises regression blast radius.

2. **Store façade is mostly glue.**  
   `impl Store` now acts as a compatibility façade with a large forwarding surface (55 trait forwarders + 8 backend matches in 406 lines). This is maintainability tax.

3. **Provider wiring is still switch-based.**  
   `build_model_command()` is a central `match` with provider-specific branching. It works, but every provider feature grows core branching pressure.

4. **Some previous “future work” has already landed.**  
   Session harnesses are already trait-based. This should be marked shipped and removed from future roadmap debt lists.

### Better recommendations (reprioritized)

#### Now (highest leverage)

1. **Split Docker executor by lifecycle boundary.**  
   Break `docker.rs` into: image lifecycle, workspace lifecycle, recovery/reattach, and container IO. Keep `AgentExecutor` surface unchanged.

2. **Collapse store forwarding boilerplate.**  
   Either (a) expose trait methods directly to callers, or (b) generate forwarding via macro. Goal: shrink `impl Store` from façade-glue to a real boundary.

3. **Create a provider command registry.**  
   Replace central model-command `match` with provider structs implementing a shared builder trait. New providers should add files, not edit switch statements.

#### Next

4. **Refactor prompt pipeline into 3 passes.**  
   Separate document gather, budget/trim policy, and prompt formatting. This makes token strategy iteration safer.

5. **Add compile-time-ish SQL catalog validation in `build.rs`.**  
   Verify every `Query` has all required dialect definitions and placeholder sanity checks before runtime.

6. **Introduce invariants tests around recovery paths.**  
   Focus on fork cleanup, workspace branch resolution, and reattach behavior rather than command-flag micro-tests.

#### Later (strategic)

7. **Push-based stimuli alongside polling.**  
   Add webhook/file-watch triggers while retaining polling as safety net.

8. **Flow language enrichment.**  
   Conditional steps, richer fork fan-out, and composition are likely higher product leverage than more provider-specific tuning.

### Roadmap implication from this deeper pass (max 3 passes)

1. **Pass 1 — Core boundary cleanup**  
   Store boundary cleanup + Docker executor decomposition + provider command registry.
2. **Pass 2 — Contract hardening**  
   Prompt pipeline decomposition + SQL catalog validation + recovery invariants tests.
3. **Pass 3 — Orchestration expansion**  
   Push-based triggers + richer flow/fork composition.

---

## Clean-Code + Fragility Lens (merged)

### Executive snapshot

Risk is concentrating at boundary files and cross-layer seams:

- Top 10 files in `rust/loopflow/src` hold **36.5%** of LOC (top 20: **55.1%**).
- `lfd/http` exposes **33** routes; `waves.rs` holds **16** handlers (~48%).
- Store catalog defines **56** SQL queries across two dialects.
- `Command::new(...)` appears **117** times across **32** files, invoking **12** binaries.
- Scheduler runs **7** background loops; startup includes recovery + janitor passes.
- Engine provider path supports 4 backends; sessions currently implement 2.
- Swift client has **116** manual JSON key reads in `LocalWaveService.swift`.

### Frame 1 — Clear ownership boundaries

**Evidence**
- `lfd` is **27,295 LOC**.
- Largest `lfd` subdomains: executor 5,689, store 5,491, http 4,777.
- These three are **58.5%** of `lfd`.

**Interpretation**
- Ownership is mostly clear at module level, but too much policy still concentrates in coordinator files.

**Priority moves**
1. Split `lfd/executor/docker.rs` by lifecycle (image/workspace/recovery/IO).
2. Reduce `lfd/store/mod.rs` façade/dispatch surface.
3. Push shared route orchestration into smaller helpers.

### Frame 2 — Low change amplification

**Evidence (last 200 commits)**
- `store/mod.rs` ↔ `store/sqlite.rs` (24)
- `store/mod.rs` ↔ `store/postgres.rs` (23)
- `waves.rs` ↔ store files (16–17)
- `http/mod.rs` ↔ `http/routes/mod.rs` ↔ store files (14–16)

**Interpretation**
- Routine work still crosses HTTP + DTO + store backends in one pass.

**Priority moves**
1. Thinner service contracts between routes and store.
2. Replace provider switch wiring with registry/trait modules.
3. Track files-touched-per-infra-change as a quality metric.

### Frame 3 — Explicit contracts + invariants

**Strong**
- Query catalog + dialect rendering in one place.
- Shared `StoreRow` adapter for SQLite/Postgres.
- Fork contract constants centralized in `engine/fork.rs`.

**Gaps**
- Session providers are a subset of engine providers.
- Cross-client schema mapping still partly manual (especially Swift).

**Priority moves**
1. Build-time query coverage/placeholder checks.
2. Provider capability matrix checks (engine vs sessions).
3. Move Swift parsing toward stronger typed decoding where feasible.

### Frame 4 — Small blast radius

**Evidence**
- `waves.rs` carries ~48% of route handlers.
- `docker.rs`, `prompt.rs`, `store/mod.rs` remain high-concentration files.
- `bin/lfd.rs` startup path coordinates many responsibilities before steady state.

**Priority moves**
1. Decompose hotspots before adding feature surface.
2. Keep new features anchored behind one boundary module.
3. Use hotspot-touch count as infra PR guardrail.

### Frame 5 — Behavior-first tests

**Evidence**
- `engine`: 211 test attrs / 8,543 LOC (strong)
- `lfd/store`: 6 / 5,491 (thin)
- `lfd/triggers`: 0 / 938 (gap)
- `lfd/executor`: 30 / 5,689 (mixed)

**Priority moves**
1. Add trigger/recovery invariant tests.
2. Add SQLite/Postgres parity tests for critical behavior.
3. Shift some command-builder micro-tests toward behavior/invariant tests.

---

## Four-Angle Analysis (merged)

### Estimated production code allocation

Estimated LOC scanned (Rust `src/` + Swift app/core + Python client, excluding tests/scripts/docs): **59,474**

| Angle | Estimated LOC | Share |
|---|---:|---:|
| Modularity / flexibility / interoperability | 35,001 | 58.9% |
| Robustness / reliability / scalability | 17,746 | 29.8% |
| Performance / efficiency | 5,216 | 8.8% |
| Security | 1,511 | 2.5% |

### Security

**Doing well**
- Dedicated auth/security/redaction/token modules.
- Explicit path traversal/root-escape defenses and tests.
- Explicit auth modes with startup enforcement.

**Needs work**
- Security effort is small relative to orchestration surface.
- Security posture depends heavily on external tooling/deployment hygiene.

**Peer-relative**
- Better than thin DIY agent tools on explicit auth/path hardening.
- Behind convex/supabase-class systems on deep platform security depth — **mostly intentional for loopflow scope** (team-scale security belongs to lfdhub).

### Robustness / reliability / scalability

**Doing well**
- Strong investment in execution + persistence + recovery.
- Dual backend support (SQLite/Postgres) with shared abstractions.
- Background loops cover watch/cron/recovery/reconcile behavior.

**Needs work**
- Single-daemon architecture limits horizontal scale.
- Reliability logic still concentrated in hotspots.
- Trigger test density is low for risk profile.

**Peer-relative**
- Stronger than many CLI-only orchestrators on recovery/state handling.
- Less scalable than distributed backend peers — **by design for this repo’s mission**.

### Performance / efficiency

**Doing well**
- Token-budgeted prompt assembly + diff tiering bounds context costs.
- Thin daemon model avoids heavy framework overhead.
- Stream/subprocess paths are pragmatic and compact.

**Needs work**
- Subprocess/git/docker boundaries dominate latency in hot paths.
- End-to-end optimization control is limited by external tool boundaries.

**Peer-relative**
- More efficient per LOC than “build-everything” stacks.
- Less tunable than custom runtime/database systems.

### Modularity / flexibility / plays-well-with-others

**Doing well**
- Strongest architectural trait.
- File-based extension model is composable and low-friction.
- Multi-client architecture is real (Rust daemon + Swift + Python).
- Clear HTTP/WS/SSE surfaces for integrations.

**Needs work**
- Provider capability drift between engine and sessions.
- Manual schema parsing (notably Swift) increases contract fragility.
- High-surface route files can become integration chokepoints.

**Peer-relative**
- Ahead on lightweight extensibility.
- Behind typed contract ecosystems with generated/shared schemas.

---

## SQLite + Postgres in lfd: strategy fit analysis

This is a key decision for the public/private split.

### Current upside of dual support

- Postgres support made remote/containerized lfd scenarios easier to reason about.
- Shared abstractions (`StoreRow`, query catalog) reduce some duplication cost.

### Current cost of dual support

- Store code shows persistent co-change across `mod.rs`, `sqlite.rs`, `postgres.rs`.
- Every storage-facing feature has higher testing and review surface.
- Complexity lands in the OSS repo that is supposed to optimize for simplicity.

### Goal-relative framing

If loopflow’s core job is single-developer, local-first orchestration, **SQLite aligns better as the default center of gravity**.

If lfdhub’s job is multi-tenant/team-scale orchestration, **Postgres depth should primarily compound there**.

### Recommended posture

1. **SQLite-first for public lfd (default + primary path).**
2. **Postgres in lfd as compatibility mode, not growth surface** (short term).
3. **Move major Postgres-heavy investments to lfdhub** where multi-tenant/team requirements justify the complexity.
4. **If dual support remains, enforce parity gates** so divergence is caught early.

### Decision trigger to keep or retire Postgres-in-lfd

Keep dual support only if one of these is true:
- standalone self-hosted lfd with Postgres is a strategic product commitment for OSS users, or
- removing it would materially block adoption.

Otherwise, simplifying lfd storage toward SQLite-only (with migration guidance) likely improves elegance and velocity for the public layer.

---

## Unified Scorecard (track each infra pass)

1. **Change amplification index** — median files touched per infra task (target: down).
2. **Hotspot churn rate** — % of infra PRs touching hotspot files (target: down).
3. **Invariant coverage index** — recovery/workspace/provider/store parity tests (target: up).
4. **Contract fragility index** — manual schema and CLI contract points (target: down).
5. **Incident diagnosability index** — systems touched to explain common failures (target: down).

---

## Documentation + Prompt Quality Alignment (new pass)

### What quality forms are emphasized in documentation

Approximate lexical scan across core docs (`STYLE.md`, `PROMPT_STYLE.md`, `README.md`, `TESTING.md`, `VISUAL_DESIGN.md`, `AGENTS.md`, `docs/*.md`) shows strongest emphasis on:

1. **Correctness/testing**
2. **UX/accessibility**
3. **Security (moderate)**

Relatively weaker emphasis in docs:

- reliability/operations
- performance/efficiency

### What quality forms are emphasized in prompts

Approximate lexical scan across built-in prompt markdown (`engine/builtins/**/*.md`) shows strongest emphasis on:

1. **UX/workflow/design**
2. **Testing/verification** (especially code steps)
3. **Simplicity/clarity**

Relatively underrepresented in prompt corpus:

- security (present mostly in scan steps)
- performance
- reliability/operational resilience

Coverage signal: out of 47 builtin prompt files, only ~12–13 mention security/performance/reliability terms at all.

### Comparison with this report’s focus

This report focuses heavily on:

- boundary fragility
- operational reliability
- contract hardening
- API compatibility
- storage complexity tradeoffs

So there is a mismatch: prompt/doc guidance is strong on shipping quality and readability, but weaker on system qualities that matter for the public-vs-hub split (contracts, reliability, performance envelopes, API subset discipline).

### How prompts should evolve for software quality

1. **Add first-class quality directions** ✓ Shipped
   Replaced role-style directions with composable quality-focused groups (`infra/`, `ux/`, `values/`). Built-in directions now include `security`, `reliability`, `performance`, `observability`, `visibility`, `feedback`, `clarity`, `simplicity`, `craft`, and more. Group expansion (`-d infra`, `-d ux`) works in prompt context and fork execution.
2. **Quality-tagged step frontmatter**
   Steps should declare which quality axes they optimize (`quality: [correctness, security, reliability, ...]`), and outputs should reflect those checks.
3. **Gate template upgrade** — partially addressed
   `gate` and `review` steps now use quality-language from the direction taxonomy. Full per-axis gate checks remain open.
4. **API-boundary prompts**
   Add prompt patterns that enforce “lfd API is subset of lfdhub API” when relevant changes are detected.

### Automatic prompt evolution loop (internal + customer)

#### Internal loop (loopflow maintainers)

1. **Instrument outcomes by prompt version**  
   Track prompt hash/version, run outcome, retries, wall time, and post-merge regressions.
2. **Prompt eval corpus**  
   Build a stable set of representative tasks (bugfix, refactor, API change, migration) with expected quality outcomes.
3. **Variant testing**  
   Run baseline vs candidate prompts on the corpus; compare success, quality checks, and cost/latency.
4. **Auto-propose prompt PRs**  
   Nightly/weekly job proposes prompt deltas that outperform baseline, with eval evidence attached.
5. **Regression guards**  
   Keep golden prompt tests and add quality-eval tests so prompt edits can’t silently reduce quality.

#### Customer loop (so evolution is not maintainer-only)

1. **Versioned prompt bundles**  
   Ship stable/canary prompt bundle channels; customers can pin, preview, and roll forward safely.
2. **Local override compatibility**  
   Customer `.lf/steps` overrides should extend, not break, bundle contracts (schema + lint validation).
3. **Portable quality packs**  
   Publish reusable quality directions/steps (security/reliability/performance/api/ux) customers can adopt directly.
4. **Customer-visible quality telemetry**  
   Expose simple metrics (pass rate, retries, lint/test failures, median run time) tied to prompt bundle version.

This creates compounding prompt quality in OSS and makes that improvement model portable to lfdhub customers.
