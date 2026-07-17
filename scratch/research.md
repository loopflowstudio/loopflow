# Loopflow architecture research

Research snapshot: 2026-07-16. This reads the current tree, git history, Product and Infrastructure Wave goals and memories, their local Linear roadmaps, tests, and user-facing documentation. It describes what exists and what the project has already learned; it does not yet choose the replacement architecture.

## Executive finding

Loopflow has already converged on a strong product center:

> A Wave conducts durable goal-authored work. It delegates through Project and Task Sessions, while humans and agents use the same local control contract to inspect and steer the work.

The implementation has not converged to an equally small set of representations. Most live debt sits at seams where one real concept has two or more models:

- durable intent is represented by a Session status, a process lease, and a derived body observation;
- direction is represented by chat, radio, child commands, directives, and Linear edits;
- execution evidence is represented by run events, agent turns, traces, journals, and domain events;
- provider identity and routing are split across accounts and browser profiles;
- Wave identity is both a durable UUID and a globally unique bare name;
- Task and Project share child-control mechanics but duplicate lifecycle and persistence paths;
- CLI JSON is becoming the agent API, but much of its shape still exposes implementation accidents.

The existing roadmaps already identify these as architecture problems. The next architecture document should not invent another generic runtime first. It should make authority, identity, state ownership, and the three domain loops explicit, then use the open roadmap failures as adversarial examples against that model.

## How the product changed

The git history shows repeated compression and re-expansion rather than a straight line.

| Period | Product center | Architectural move |
| --- | --- | --- |
| January 2026, v0.4-v0.6 | Reusable prompts, pipelines, and worktrees for a human operator | CLI-first execution toolkit |
| Late January, v0.7 | An “agent orchestra” where an agent combined area, goal, flow, and stimulus | Long-lived daemon loops appear |
| February-March, v0.8-v0.9 | Waves coordinate multiple agents, attention, PM, and native UI | Shared engine, HTTP daemon, Mac surfaces, provider abstractions |
| June, v0.9.11 | Vendor-neutral orchestration layer, self-hosted, skills-based | Native conversation/provider layers and iOS are deleted; Wave is briefly renamed Loop |
| July 5, v0.10 | One persistent goal agent with Wave, Run, and Session | Postgres, queue, Docker daemon, conversations, and provider layers are aggressively removed |
| July 7-13 | Generic flow loop, then explicit Wave/Project/Task planning | Generic drivers are tried, then domain lifecycle reappears; Project and Task Sessions become durable |
| July 14 onward, v0.11 | One `lf` control surface, local store, native Linear, leased replaceable bodies | Recovery, directives, observations, PR succession, CI wakes, accounts, traces, and fleet operations expand quickly |

Two lessons survive those reversals:

1. **Composition and lifecycle are different.** A Flow composes skills and operations. Wave, Project, and Task each repeat for different domain reasons. Attempts to make a generic run or loop own all repetition were later removed.
2. **The durable object is not the provider process.** Provider bodies fail, stall, upgrade, and get replaced. Goal, Project, Task, direction, and evidence have to survive them.

The current website still mostly tells the March story: teams of coding agents and a manual user product. It does not yet explain personal dogfooding, `lf --json` as an agent-facing control API, replaceable Session bodies, fleet monitoring, or the local decentralized topology.

## Current system understanding

### Product concepts

The repo’s planning doctrine is intentionally small:

- **Wave** — durable operating context. Owns goal, memory, cadence, budget, chat, and judgment about which Projects matter.
- **Project** — measured bet inside exactly one Wave. Owns definition, KRs, and closure criteria in Linear.
- **Task** — concrete implementation, investigation, document, or shipped change under a Project.

Runtime concepts are also necessary, but are not yet presented as one stable public ontology:

- **Session** — durable execution intent and history for a Project or Task.
- **Body** — one disposable provider process/generation working a Session.
- **Flow** — ordered composition of skills and operations inside one invocation.
- **Command** — durable request to control a child Session, with delivery state.
- **Directive** — authoritative current direction plus evidence that a body incorporated it.
- **Observation** — derived statement about body category, owner, progress, and legal actions.
- **Run / trace / event / receipt** — overlapping evidence about execution and lineage.
- **Home** — user-owned execution location such as local or SSH host.
- **Account** and **profile** — provider spending identity and login venue, currently in transition.

The three planning nouns are sound, but they cannot carry the whole architecture. Hiding the runtime nouns makes the API less clear rather than simpler.

### Deployment topology

Loopflow has no central Loopflow server.

```text
                        Linear                    GitHub
                    plan + edits              PR + CI evidence
                         |                           |
                         v                           v
+----------------------------- one user-owned Home -----------------------------+
|                                                                               |
|  lf CLI / --json / skills / Mac app / SSH                                     |
|                 |                                                             |
|                 +--------------------+----------------------+                  |
|                 v                    v                      v                  |
|          local SQLite store     Wave listener         optional lfd            |
|          control + evidence     journal + HTTP        webhook inbox only       |
|                 ^                    |                                         |
|                 |                    v                                         |
|                 +-------------- Wave resident                                  |
|                                      |                                         |
|                                      v                                         |
|                               Project Session                                  |
|                                      |                                         |
|                                      v                                         |
|                                Task Session                                    |
|                                      |                                         |
|                                      v                                         |
|                    leased Claude/Codex/OpenCode body                            |
|                                      |                                         |
|                                      v                                         |
|                             stable Git worktree                                |
|                                                                               |
+-------------------------------------------------------------------------------+
```

The SQLite registry is opened directly by local commands and runtimes. A Wave listener owns its local HTTP endpoint and journal; the Mac app connects to it directly. Remote operation uses SSH to another Home. `lfd` is a machine-level durable webhook ingress and liveness process, explicitly not the read/write API.

This is decentralized in the useful product sense: no Loopflow control-plane service owns every user or company. It is not peer-to-peer or independent of central SaaS: Linear and GitHub remain systems of record for planning and delivery.

### Authority and data ownership

The intended ownership appears to be:

| Truth | Intended owner | Projection / consumer |
| --- | --- | --- |
| Wave goal and durable memory | authored files under `wave/<name>/` | Wave resident and surfaces |
| Project definition and KRs | Linear Project | local PM snapshot, Project Session |
| Task definition and state | Linear issue | local snapshot, Task Session |
| Child control intent | SQLite commands and directives | Project/Task runners |
| Process ownership | leased body generation in SQLite | observation and recovery |
| Implementation changes | Task worktree and Git | GitHub PR |
| Review and CI evidence | GitHub | Task PR evidence and reconciliation |
| Wave conversation | journal JSONL | listener, resident, Mac/CLI |
| Cross-runtime notification | SQLite bus / observation outbox | listener and parent Sessions |
| Execution evidence | run ledger, agent turns, traces, domain events | `lf runs`, `top`, `usage`, `trace`, status |

The last row is not actually settled. Open Infrastructure work explicitly calls for one spend store and one usage parser. The same ownership question also appears in lineage, status projection, and event retention.

### Steering data flow

There are several paths because they cross different boundaries:

1. A human steers a Wave through its listener. If the provider supports live steer, the resident delivers it to the current body; otherwise it queues or interrupts for the next turn.
2. A Wave or Project controls a child by persisting a typed child command.
3. A runner claims the command for a specific body generation, chooses a live effect or next-turn replacement, records delivery, and updates the authoritative directive.
4. The next body must explicitly acknowledge incorporating that directive.
5. Child observations travel back through a durable outbox/bus and are folded into the parent’s view.
6. Linear edits can enter through `lf` polling or the durable `lfd` webhook inbox, then become domain direction rather than a parallel worker queue.

The child command state machine correctly admits an unavoidable distributed-systems fact: after delivery begins, a crash may leave the result `Uncertain`. The architecture needs to make this ambiguity a first-class user/API outcome, not bury it in runner behavior.

The weak seam is caller authority. W2-281 records that removing `LF_WAVE_ID` changed the same Task command from a restricted Wave caller into a Human/operator caller. Environment currently helps decide identity; the stated doctrine says environment may configure a process but may not decide what it is.

### Provider control and native delegation

The current `Harness::Capabilities { supports_steer }` describes Loopflow's adapters, not stable provider capability. `rust/loopflow/src/harness/conformance_tests.rs` pins Codex true and Claude/OpenCode false, but the underlying surfaces are more nuanced:

| Surface | Current control behavior |
| --- | --- |
| [Codex app-server](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) | `turn/steer` injects into a named active regular Turn and requires `expectedTurnId`; review and compaction Turns can reject steering. `turn/interrupt` ends the active Turn. Threads can be resumed, forked, and spawned as persistent subagent children. |
| [Claude Code Agent SDK](https://code.claude.com/docs/en/agent-sdk/streaming-vs-single-mode) | Streaming input holds a long-lived query, queues messages, and supports interruption. The SDK still spawns a bundled `claude` executable, using persistent `stream-json` input/output instead of Loopflow's one-shot `claude -p`. Anthropic's documented third-party integration path requires API-key or cloud-provider auth; richer SDK control is therefore informative, not the supported Max-account substrate Loopflow currently targets. |
| [OpenCode server](https://opencode.ai/docs/server/) | A persistent local server exposes session message, asynchronous prompt, abort, child-session, and event APIs. Loopflow currently prevents concurrent `send_input` and deletes the server-local session when the harness stops. |

Thus same-turn injection is a per-Turn operation, not a provider-wide trait. Provider acceptance is also weaker than incorporation: a returned Codex Turn id, queued Claude input, or OpenCode `204` does not prove that the agent reasoned from the direction.

The Claude distinction is protocol and authentication policy, not two independent agent implementations. Both SDK and `claude -p` run Claude Code's agent loop. The SDK package adds a persistent bidirectional control wrapper around the CLI, but its setup guide still directs product developers to an API key and forbids offering Claude.ai login without approval. Loopflow uses isolated `CLAUDE_CONFIG_DIR` homes so the creator can route among personal Max accounts; that keeps the one-shot CLI adapter as the governing portability case unless Anthropic sanctions the SDK subscription path for this use.

All three providers can create their own nested agents:

- [Codex subagents](https://developers.openai.com/codex/concepts/subagents) are child threads that the root can follow up, wait for, interrupt, and close.
- [Claude SDK subagents](https://code.claude.com/docs/en/agent-sdk/subagents) are isolated Agent-tool conversations whose results return to the parent; custom children expose ids that can be resumed inside the same session.
- [OpenCode subagents](https://opencode.ai/docs/agents) are Task-tool or `@agent` child sessions with separate agent configuration and permissions.

Their common concepts are isolated context, a parent-child edge, targeted follow-up, stop/wait, and a result returning to the parent. Their durability, direct human control, and nesting rules differ. They are informative execution substrates but do not map 1:1 to Loopflow Project or Task: they inherit one root execution's workspace and authority and have no independently authored planning identity.

The architecture consequence is recorded in `scratch/architecture.md`: provider-native children remain nested Handle/Turn evidence under one Run. Durable, independently steerable delegation remains Loopflow child Work.

### Looping data flow

There are three real loops, not one loop at three sizes.

| Loop | Why it repeats | Boundary decision |
| --- | --- | --- |
| Wave | Maintain a durable goal-oriented mind on cadence and incoming stimuli | schedule another root invocation, interrupt, or stop after repeated runtime failure |
| Project | Reassess KRs, choose/supervise Tasks, and detect whether the bet advanced | complete if KRs hold; wait on active work; block on an unchanged full iteration; otherwise repeat |
| Task | Move one concrete change through kickoff, iteration, review, CI, and landing | wait, block, enter gate, repair CI once, rotate PR evidence, complete, or repeat |

Flow composition sits inside those loops. It should not decide domain completion. The engine still contains older loop/composite node shapes, while current plans mostly use Skill, Op, and Xor. This is an area to reduce after the domain state machines are explicit.

## What has already been flagged

### Product / Loopflow API Wave

The Product roadmap has already compressed to three Projects: Mac surface UX, Loopflow API, and auditability. The Loopflow API Project is the main architecture contract: Waves, Projects, Tasks, agents, runs, chat, delegation, termination, recovery, and evidence should be coherent through the same surfaces.

Its open work identifies these contract gaps:

| Item | Flagged gap | Architectural signal |
| --- | --- | --- |
| PRD-5 | OpenCode/GLM disconnects must be observable and recoverable | Provider failure must map into the shared body model, not provider-specific silence |
| PRD-6 | Wave control-plane reconciliation must be self-healing | Status, counts, attention, and legal controls need one projection over durable intent and fresh observations |
| PRD-7 | Cadenza/Linear team ownership | Planning identity and team authority are not fully repository/Wave scoped |
| PRD-8 | Failed CI should wake one bounded repair body | A special wake must remain a typed command and explicit lifecycle mode, not leak into generic gate repetition |
| PRD-9 | Recover an abandoned Task | Durable Task intent, attempts, bodies, and linked successors need precise identities and history |
| PRD-10 | Session bodies must be leased, progress-aware, and recoverable | Liveness, progress, durable intent, process generation, and ownership must remain separate but form one public state model |

PRD-6 is the broadest simplification statement: build one reconciliation projection, do not add a second supervisor or store, keep reconciliation bounded and mechanical, and expose exact evidence and freshness.

PRD-8 and its Infrastructure splits ENG-19/ENG-20 are equally revealing. The desired result is not merely “CI fix works.” It is one command model, one body for one failure set, one explicit repair lifecycle, and no direct Project-runner bypass.

PRD-9 and PRD-10 still contain terminology tension worth resolving. At different points they describe Wave/Project/Task, Session, or successor Session as the durable intent. Before implementation expands further, the architecture needs one answer for:

- whether Project and Task are durable authored identities while Sessions are execution attempts;
- whether a Task Session itself is durable across all bodies and PRs;
- when a successor is a new attempt versus continuity of the same intent;
- whether Wave uses the same Session abstraction or deliberately has a different runtime.

### Infrastructure Wave memory

The Infrastructure memory names unfinished reductions from the July “minds” review:

- factor duplicated inbox/interrupt branches;
- lift duplicated lease-renewal logic;
- merge `interrupt_child` and `interrupt_harness` behind one interruption operation;
- finish endpoint resolver consolidation;
- inline the remaining loop-flow requirement wrapper.

The current tree still contains separate `interrupt_child` and `interrupt_harness` paths in `flowloop/wave.rs`, plus another child-control interruption implementation. The reduction remains live.

The memory also preserves architectural constraints that should survive cleanup:

- Wave IDs have durable identity and separate directory/branch projections; do not derive identity from path strings.
- One writer per worktree is dispatch discipline, not a filesystem lock abstraction.
- SQLite is the local bus.
- Do not build a generic platform ahead of a demonstrated product need.
- “Heartbeat idle” is real scheduling input, not dead code to remove for cosmetic simplicity.

### Infrastructure roadmap

The current Developer Efficiency Project is effectively the architecture stress-test queue. Its most relevant open findings group into six themes.

#### 1. One concept, multiple stores or parsers

- **W2-280:** retire parallel spend recorders. `run_events` and `agent_turns` both record usage at different grains and coverage. The chosen end state is `agent_turns` as the spend source, with `run_events` retaining process/lineage only. Net code should shrink.
- **W2-289:** retire parallel usage parser stacks. `StreamEvent::Usage` accumulates while `ConversationEvent::TurnUsage` replaces; OpenCode could overwrite real usage with zero. One parse path should own usage end to end, and “unreported” must differ from zero.
- **ENG-18:** execution lineage crosses trace and retention boundaries without typed edges, so valid external/pruned parents look corrupt. Lineage needs an explicit boundary model.

These are not telemetry polish. They expose an unsettled execution-evidence ontology.

#### 2. Durable intent, body, status, and owner disagree

- **ENG-3:** an interrupt against an already-dead body parks the Session in Waiting with an instruction no human will read. The state says Human owns the next action even though the command did not stop anything.
- **ENG-4:** a failed reap leaves a nonexistent process in a permanently `revoked` lease, blocking every future generation. Durable status blames manual cleanup while the process is already gone.
- **ENG-5:** Task recovery is parent-driven, so a Task with a dead Project parent has no observer. Adding a Wave-level recovery path risks creating a second dispatcher.
- **PRD-10:** body observation and leases are intended to separate intent, liveness, and progress, but these failures show the transition rules are not yet one state machine.

The core clarification is ownership: for every observable state, exactly one actor must be named as able and responsible to make progress. A status reason containing an imperative is not an owner.

#### 3. Typed authority and identity are missing at API boundaries

- **W2-281:** Task command authority is inferred from ambient environment. Caller identity must be explicit and typed across Wave, Project, and operator surfaces.
- **W2-283:** accounts and browser profiles point in the wrong ownership direction. An account is the spending credential; a profile is only a login venue whose actual identity must be verified. Routing should select ordered accounts per provider, while each account lists verified access profiles only for authentication ceremonies.
- **ENG-21:** Waves are globally keyed by bare name, so two repositories cannot both own an Infrastructure Wave safely. Canonical identity needs repository plus slug while preserving durable UUID and history.
- **W2-292:** hostname is not a stable machine identity. It is being removed from current credential ownership, but future multi-host work needs a persisted machine ID before hostname becomes load-bearing again.
- **W2-295:** `project_route_succeeded: false` describes the healthy JSON path because the flag really means “rerouted to successor.” A typed route state should replace a misleading implementation boolean.

W2-295 matters beyond naming: the human CLI hid the healthy false value, while agents consuming JSON interpreted it as a fleet incident. The agent API has a higher obligation to encode meaning directly.

#### 4. Model mirrors drift

- **W2-298:** the Task runner’s interaction policy was correct in memory, but a handwritten SQL update dropped it. In-memory tests passed; persistence round-trip behavior failed.
- **W2-288:** the PM JSON carried five KRs while a Project seed carried four. The loop acted on a prompt mirror that silently omitted authored truth.
- DTO fixture tests protect Rust/Swift/JSON wire fields, but internal row mapping, prompts, status calculations, and command registration remain hand-maintained mirrors.

The store is a major concentration of this risk: `store/mod.rs` and `store/sqlite/child_sessions.rs` are each several thousand lines, with paired Project/Task operations and lease variants. `ops/task.rs` alone is more than eight thousand lines. The architecture should first eliminate redundant representations; splitting files without changing ownership would only distribute the drift.

#### 5. Domain lifecycle leaks through generic mechanics

- **ENG-19:** a CI-fix wake can fall through into ordinary iterate/gate behavior and launch a second unrelated body. Repair mode must survive settlement, not only initial Flow selection.
- **ENG-20:** failed-head recovery must enter through the durable command ledger, not a direct runner call or parallel queue.
- **W2-297:** changes-requested review can become permanently terminal and trigger empty serial PR loops. A review belongs to a specific gate cycle; headless work must park instead of spin when a human owns the next action.
- **W2-296 / W2-300:** repeated closed-PR reconciliation and committed follow-up races need idempotent boundary decisions.

These support keeping explicit Task lifecycle state. They argue against a generic loop swallowing the distinctions, not against domain state machines.

#### 6. Local decentralization has operational edges

- **ENG-7:** roughly fifty concurrent local processes can hit `SQLITE_BUSY`, lose receipts, and kill live Session bodies. Direct SQLite access needs one bounded contention policy and deterministic exactly-once evidence behavior.
- **ENG-14:** a Task provider received canonical main as an additional directory and wrote outside its worktree. “Task owns mutations” must be enforced at the provider boundary, not only stated in prompts.
- **ENG-15:** a historical Project absent from the current PM snapshot can make status/roadmap fail. Historical execution identity and current plan projection need separate lookup rules.
- **W2-278:** completion selected a workflow state from the Wave’s current team instead of the issue’s owning team. External authority must be resolved from the object being mutated.

The no-server topology is worth preserving, but “direct local calls” cannot mean every process invents its own retry, identity, projection, and mutation policy.

## Complexity and quality observations

### Momentum

- The three planning nouns now match the dogfooded workflow and Linear model.
- The replaceable-body model is a real operational advance over treating provider sessions as durable.
- Commands and directives distinguish delivery from authoritative instruction and incorporation proof.
- Task worktree ownership, serial PR evidence, and successor records preserve history instead of rewriting it.
- DTO fixtures, extensive Rust tests, and typed JSON surfaces create useful pressure against silent cross-language drift.
- The July deletions show willingness to remove entire architectures when the product model changes.

### Drift

- The public story, architecture doc, Wave memories, and current runtime describe different generations of the product.
- `lf --json` is the de facto agent API, but fields and command grammar are still designed unevenly for humans, agents, or internal runners.
- There are several append-only histories without a stated hierarchy: Wave journal, domain events, run ledger, agent turns, trace files, observation outbox, command receipts.
- “Only Waves are minds” is useful doctrine, but Project and Task Sessions retain private provider history and make repeated judgments. The difference between a mind and a hand needs an operational definition, not a slogan.
- Project and Task share child-control mechanics while retaining large parallel runner/store APIs. Some duplication is domain truth; some is uncollapsed substrate.
- Wave uses listener/resident/playhead mechanics unlike child Sessions. That may be correct, but the public word “Session” cannot imply uniformity if the runtime deliberately is not uniform.

### Highest-leverage reduction test

For each proposed abstraction, ask:

> Which existing roadmap failure becomes impossible to represent incorrectly?

Examples:

- A typed caller authority makes W2-281 impossible.
- A typed Project route state makes W2-295 impossible.
- One usage producer/store makes W2-280 and W2-289 impossible.
- One persisted row mapping or generated mapping makes W2-298 harder to create.
- One authoritative reconciliation projection prevents PRD-6’s divergent status/count/action surfaces.
- An explicit Task transition table makes ENG-19’s CI-fix fallthrough and W2-297’s gate spin visible at design time.

If a refactor cannot name the invalid state it removes, it is probably file organization rather than architecture reduction.

## Questions the architecture document must hold open

1. What is the durable identity at each layer: authored Wave/Project/Task, execution Session, body generation, Flow invocation, run, and trace?
2. Does every Project and Task have exactly one long-lived Session, or can Sessions be attempts/successors under one authored object?
3. Is Wave intentionally a different runtime kind from child Session? If so, which controls and observations are shared at the API rather than implementation layer?
4. Which record is authoritative for current child state: mutable Session status, command/directive ledger, lease, domain evidence, or a deterministic projection over them?
5. What is the single execution-evidence spine, and which journals/traces are narrative or indexed projections of it?
6. What semantics distinguish follow-up, steer, replacement, interrupt, resume, decision, authored Linear revision, and CI wake? Which are commands, directives, stimuli, or evidence?
7. How does a caller prove authority without ambient environment or an implicit process role?
8. What is the repository-scoped identity model for Waves, Projects, Tasks, Homes, machines, accounts, and remote forwarding?
9. What freshness guarantee does each read API provide, and which reads may reconcile external truth?
10. How many concurrent bodies should one Home support before direct SQLite coordination needs a single local write broker—or can bounded WAL/retry discipline carry the target fleet?
11. Which concepts belong in the public agent API, and which should remain internal evidence used to derive a smaller status and legal-action surface?
12. What does “decentralized in a large company” promise when planning and PR truth still live in Linear and GitHub?

## Recommendations for the top-down architecture document

Write the next document around contracts, not modules.

1. **Concept inventory.** Define authored identity, durable execution identity, disposable body, direction, observation, evidence, and location. Give each one owner and stable ID.
2. **Authority matrix.** For Human, Wave, Project, system webhook, and provider body, state which commands each may issue and how identity is carried explicitly.
3. **Truth hierarchy.** Name the authority for plan, code, process ownership, direction, CI/review, conversation, usage, and lineage. Mark every other representation as cache, projection, index, or narrative.
4. **Three lifecycle state machines.** Specify Wave scheduling, Project KR supervision, and Task delivery separately. Put Flow composition inside them. Include owner and legal controls on every state.
5. **One steering protocol.** Show command persistence, generation claim, delivery ambiguity, directive replacement, incorporation acknowledgement, child observation, and restart behavior end to end.
6. **Agent API contract.** Treat `lf --json` as a local control API with typed query, command, event, and error shapes. Human text, Mac, skills, and SSH should adapt the same semantics.
7. **Decentralized topology and failure model.** State what is per Home, per repository, per Wave, external, replicated, or never replicated. Include SQLite contention, SSH loss, webhook retry, provider death, and stale external snapshots.
8. **Reduction ledger.** For every retained duplicate, explain the distinct truth it represents. Use W2-280, W2-289, W2-298, W2-281, W2-295, ENG-19, and PRD-6 as acceptance cases.

Do not start by designing a universal actor, event, or workflow framework. The history already tested that move. First make the existing domain concepts map 1:1 to data and API contracts; the common substrate will be visible after the differences are explicit.

## Likely implementation sequence after ratification

This is not a committed plan, but the dependency order exposed by the research is:

1. Ratify identity, authority, and truth ownership without moving code.
2. Pin current failures as contract tests and persistence round-trip tests.
3. Consolidate read projection and legal-action derivation.
4. Consolidate child command/interruption/lease substrate while leaving Project and Task transition policies separate.
5. Remove duplicate execution evidence stores and parsers.
6. Reshape JSON/CLI surfaces only after the internal state and names are stable.
7. Rewrite the website and user docs from the ratified product contract, not from current command inventory.

That order preserves the dogfooding system while turning each simplification into a falsifiable reduction rather than another model layered beside the old one.
