# Engineering craft across Paseo, Loopflow, T3, Orca and Paradev

Research snapshot: November 8, 2026. Commit statistics cover August 10 through November 7 UTC; code inspection includes the November 8 snapshots. No Task. Sources: five Git histories, 150 recent merged PR records, selected production code and tests. Companion evidence and detailed audits are linked below.

**T3 and Paseo show the strongest craft in the sampled code. T3 leads on explicit correctness and recovery mechanisms; Paseo on direct, product-focused engineering. Paradev is a capable commercial product codebase. Loopflow combines serious systems craft with unstable integration during foundational changes. Orca has the most concerning overall structure and strongest signs of output outrunning architectural control.**

For overall sampled code quality, the defensible grouping is **T3 / Paseo first; Paradev / Loopflow in a mixed middle; Orca last**. This is a qualitative judgment about the inspected code, not a measured defect-rate ranking. Within the middle, Paradev is more locally straightforward; Loopflow has stronger examples of domain modeling and structural deletion, but more troubling evidence about core operational behavior. None is uniformly excellent or uniformly junk.

## Distribution of work

These counts distinguish contribution breadth from actual engineering headcount. An agent can commit as a person; a shared account can represent several people. Obvious bot-labelled identities are excluded from concentration percentages, but remain in total main-commit counts. Exact name/email aliases are normalized.

| Repository | Main commits in 90 days | Largest nonbot author share | Top three share | Author identities accounting for 80% |
|---|---:|---:|---:|---:|
| T3 | 7,297 | 34% | 52% | 32 |
| Orca | 11,630 | 41% | 72% | 4 |
| Paradev | 2,010 | 44% | 88% | 2 |
| Paseo | 1,574 | 84% | 86% | 1 |
| Loopflow | 601 | 100% | 100% | 1 |

T3 is decisively the most distributed contributor network. Orca has enormous activity concentrated in a small core. Paradev looks like a compact core with recurring areas of ownership. Paseo is much closer to an auteur-led project than its contributor count suggests: Mohamed Boudra accounts for 84% of nonbot-attributed commits, while two bot identities contribute another 523. Loopflow's nonbot commits all resolve to one author identity. These are contribution patterns, not geographic conclusions. Email-only counting changes T3's 80% threshold from 32 to 33, leaving the ordering intact. [Recorded measurements](agent-platform-craft-evidence-2026-11-08.json)

**Integration is more concentrated than contribution.** In the latest 30 merged main PRs before the cutoff, T3 has 25 merged by someone other than their author, but 26 of the 30 merges come from one account, Yash-Singh1. Paseo has three non-self-merges, Paradev one, Orca and Loopflow zero. T3 has five PRs with recorded nonauthor, nonbot-account formal review before merge; the other samples have zero. This cannot see local agent review, ordinary comments, pairing or offline review. Orca's sample spans only 103 minutes, Loopflow's four days, so this is a recent integration snapshot, not a quarterly review-rate estimate.

## T3: strongest explicit correctness machinery

T3's good abstractions do real work. Its event sink groups receipts, events, projections and queued effects transactionally, orders publication, and handles cancellation after commit. Its effect outbox distinguishes work safe to replay from work tied to a dead process, records worker ownership and preserves ordering within a thread. These mechanisms address recognizable failure modes rather than merely decorating the code with types. [EventSink](https://github.com/pingdotgg/t3code/blob/af65a55dc/apps/server/src/orchestration-v2/EventSink.ts#L285), [EffectOutbox](https://github.com/pingdotgg/t3code/blob/af65a55dc/apps/server/src/orchestration-v2/EffectOutbox.ts)

The tests provide unusually concrete support. One command-replay regression seeds **25,000 SQLite events**, checks the returned events, and checks that the query uses the command index. A usage-limit recovery test exercises the production persistence/reconciliation service and repeats the retry to check that the visible continuation is not duplicated. This is better evidence than a large passing-test count. [Replay regression](https://github.com/pingdotgg/t3code/commit/9fba2093e50f208c52d9041c70a793296e311477), [Recovery test and implementation](https://github.com/pingdotgg/t3code/commit/240446e2d9625600b0c841faf6bddf367bd26967)

The cost is heavy coordination. The production `Orchestrator.ts` is **14,607 lines**, with tests elsewhere; `ChatView.tsx` is 4,075. Typed services clarify many contracts while leaving a very large central decision-maker. The V2 replacement merged roughly 1,900 files and was followed by foundational restart and replay fixes. It was developed over May–October with 823 branch commits, so its size is evidence of integration risk, not an overnight generation binge. [Orchestrator](https://github.com/pingdotgg/t3code/blob/af65a55dc/apps/server/src/orchestration-v2/Orchestrator.ts), [V2 replacement](https://github.com/pingdotgg/t3code/pull/2829), [Restart follow-up](https://github.com/pingdotgg/t3code/commit/5108c978b1d9a8bd029e5ea96781d8cbc90b3877)

**Team inference:** a broad contributor network under a much smaller architectural and integration center. The history combines many narrowly scoped compatibility fixes with major coordinated architecture. Best candidate for the strongest formal engineering; not the simplest codebase.

## Paseo: strongest everyday product craft

Paseo's history repeatedly attends to things a person notices while using the tool: lost history after reload, chat movement, keyboard clearance, narrow layouts, foreground reconnects and terminal replay. That alone could be superficial polish. The implementation samples show stronger substance.

The new diff-stream assembler is a small owner with explicit begin/body/end semantics: it rejects inconsistent revisions, ignores stale frames, commits a complete snapshot atomically and reuses unchanged objects. Tests exercise supersession and inheritance of unchanged data; daemon journeys use actual Git repositories and patch contents. The associated PR even records a slower cold-computation sample alongside its wire-size improvement. This is a convincing combination of useful behavior, comprehensible ownership and honest tradeoffs. [Diff stream implementation and tests](https://github.com/getpaseo/paseo/blob/fcde2072abf5378db17db7b805c840df1e68bdde/packages/client/src/diff/stream.ts), [Change and reported measurements](https://github.com/getpaseo/paseo/pull/8907)

Paseo also deletes what it replaces: its terminal-renderer cleanup removes the old renderer, settings and drift-check machinery, about 1,800 lines. That matters more than adding another abstraction beside the old one. [Terminal cleanup](https://github.com/getpaseo/paseo/commit/6376f2bf409405b9857c03262381655433a6d0ee)

It still has significant lifecycle debt. A foreground heartbeat change was followed by a documented 40-second dead-connection detection case; whole-turn history changes required compatibility and presentation repairs shortly after landing. Current client and agent-manager production modules are about **8,000 and 6,000 lines**, respectively. The small, good components sit inside large coordinators. [Foreground repair](https://github.com/getpaseo/paseo/commit/f539ccf1c00f17c2f4b8925fcd1d11d40118583e), [History repair](https://github.com/getpaseo/paseo/commit/ef58ace424089c220b6d42bd618234a2159e3db3), [Client](https://github.com/getpaseo/paseo/blob/27872a8e/packages/client/src/daemon-client.ts)

**Team inference:** an exacting central product maintainer, assisted by automation and outside contributors, shaping a multi-device tool through repeated use. Best candidate for product-focused craft and relatively direct local code. Its polish does not establish superior runtime reliability.

## Paradev: a commercial product team with competent internals

Paradev's change mix is visibly commercial: cloud workspaces, onboarding, pricing, billing, organization integrations, migration paths and marketing ship alongside infrastructure. Its cadence is steadier than the recent acceleration in T3 and Orca. Recurring author lanes suggest a small team dividing product, chat/host and cloud/business work; they do not prove organizational titles.

The sampled internals are generally understandable. Interrupted-chat handling separates a pure reducer for unfinished events, transactional journal writes, and host-level recovery decisions. Browser-extension work separates browsing contexts and checks CRX signatures. A real HTTP/CRX fixture test corrupts a download and verifies that nothing is installed. [Interrupted-event reducer](https://github.com/ParadevAI/Paradev/blob/b77bc046ee98bbc3ac05c7ae792e630d7e7b73d0/packages/chat-runtime/src/journal/strandedEvents/strandedEvents.ts), [Browser-extension implementation](https://github.com/ParadevAI/Paradev/commit/4b972edd), [Download tests](https://github.com/ParadevAI/Paradev/blob/b77bc046ee98bbc3ac05c7ae792e630d7e7b73d0/apps/desktop/src/main/lib/browser/extension-host/web-store.test.ts)

Quality is uneven within those same areas. Other security tests assert that patch source contains certain strings; they do not establish runtime denial of an unauthorized request. The 1,242-line `LiveSession` accumulates interacting queue, steering, replay, cancellation and background-work state. It is locally clearer than Orca's runtime, but carries familiar state-machine pressure. [LiveSession](https://github.com/ParadevAI/Paradev/blob/b77bc046ee98bbc3ac05c7ae792e630d7e7b73d0/packages/chat-runtime/src/sessions/liveSession/liveSession.ts)

Browser-extension follow-ups were substantial, but landed before the next version bump. Counting each as a shipped regression would be wrong. One separate revert explicitly says three security PRs merged without sign-off: a concrete process lapse, not evidence that all their work is careless. [Extension follow-ups](https://github.com/ParadevAI/Paradev/commit/4cccb5b3), [Next version bump](https://github.com/ParadevAI/Paradev/commit/e34fb4e2), [Sign-off revert](https://github.com/ParadevAI/Paradev/commit/c61f8f1a)

**Team inference:** a compact, pragmatic startup team optimizing a business and a product together. More disciplined local code than a “feature factory” label would imply, with integration debt and uneven tests.

## Loopflow: strong systems thinking, costly foundational churn

Loopflow has concrete craft beyond its style guide. It gives agent processes a distinct ID type so the compiler can catch ownership mistakes. It captures a Task's Graph as a definition independent of the run cursor. Its migration checks use real Git histories and SQLite schemas. It removes obsolete mechanisms across code, commands, release artifacts and documentation. [Process IDs](https://github.com/loopflowstudio/loopflow/pull/1840), [Migration checks](https://github.com/loopflowstudio/loopflow/pull/1827)

The best recent example removes the Codex relay from the conversation's byte path and records through a side observer. Store loss now affects recording rather than transport. It deletes more code than it adds, and probes use real Codex against a local synthetic model endpoint. The reader-floor removal likewise deletes update vetoes, catalog machinery and hidden commands together. [Direct Codex connection](https://github.com/loopflowstudio/loopflow/pull/1831), [Reader-floor deletion](https://github.com/loopflowstudio/loopflow/pull/1830)

The preceding failures matter. The store-loss repair explicitly records that a November 4 installation advanced the store under live sessions and the relay lost conversations. Automatic Desktop conversations then needed a follow-up after 14 of 23 were stuck at trust dialogs and board loading exceeded the intended bound. The follow-up still records providers accumulating after pagination and about 6.96 GB used in its 23-provider run. Those are the PR's reported observations, not measurements repeated in this study. [Lost-conversation repair](https://github.com/loopflowstudio/loopflow/pull/1833), [Automatic-conversation follow-up](https://github.com/loopflowstudio/loopflow/pull/1842)

Tests can be ambitious without being affordable: recently introduced world-level Swift suites were removed after rebuilding expensive worlds pushed CI beyond its time limit. That is an honest correction, and also an initial test-design failure. [World-test removal](https://github.com/loopflowstudio/loopflow/pull/1645)

**Team inference:** one operator using automation to build an increasingly complete operating system for their own work. The feedback loop turns operational pain into models, code, tests and rules. It also lets changes to storage, installation and session ownership outpace integrated acceptance. Loopflow's engineering ambition and its achieved stability should be graded separately. This assessment uses repository evidence, not the extra private incident history available in this checkout.

## Orca: the strongest warning about uncontrolled complexity

Orca's central concern is structural, not its commit rate. Mechanically following superclass imports from `OrcaRuntimeWithAgentTransitions` reaches the root through **202 classes/modules and 201 inheritance edges**. These files total 37,684 lines including imports and comments. Terminal lifecycle, browser routing, mobile tabs, workspaces and orchestration all contribute to one inherited runtime. Short files do not make these independent components. [Runtime entry](https://github.com/stablyai/orca/blob/5fc3b26766a9fc3ba95d8bc72600325f036f0db4/src/main/runtime/orca-runtime.ts), [Chain start](https://github.com/stablyai/orca/blob/5fc3b26766a9fc3ba95d8bc72600325f036f0db4/src/main/runtime/construction/orca-runtime-agent-transitions.ts), [Chain root](https://github.com/stablyai/orca/blob/5fc3b26766a9fc3ba95d8bc72600325f036f0db4/src/main/runtime/construction/runtime-construction-services.ts), [Recorded chain](agent-platform-craft-orca-chain-2026-11-08.json)

That is stronger evidence of poor maintainability than thousands of quick commits. File-splitting and reorganization campaigns may improve navigation while leaving the connected mutable system intact. Orca has the strongest mismatch here between local tidiness and global simplicity.

Its massive test-pruning campaign is also revealing. Actual deleted examples include source-string checks and mock-return plumbing; a sampled purge removes over 21,000 lines. Deleting those tests is a correction worth crediting. Their accumulated volume is evidence that the development process previously rewarded low-information output. It is not proof that every removed test was useless or that agents authored them. [Test purge](https://github.com/stablyai/orca/commit/5fc3b2676), [Test policy](https://github.com/stablyai/orca/blob/5fc3b26766a9fc3ba95d8bc72600325f036f0db4/docs/agents-md-refs/test-antipatterns.md)

There is real craft inside Orca. A duplicate-send fix puts the claim at the shared queue owner and tests remount behavior. Terminal transport work bounds recovery snapshots and tests a fast terminal on a slow link. Failure receipts retain relevant output and redact secrets. These are useful systems improvements, not empty churn. [Duplicate-send fix](https://github.com/stablyai/orca/commit/fca1d7b13), [Backpressure simulation](https://github.com/stablyai/orca/commit/4e66fb551), [Failure receipts](https://github.com/stablyai/orca/commit/14492c79c)

**Team inference:** a small dominant core operating parallel feature, repair and optimization campaigns over a very broad platform matrix. Of these five, Orca is closest to the requested “slop factory” category at the whole-codebase level: output and maintenance volume have outrun structural clarity. That judgment is about the resulting code and process, not the competence or motives of individual engineers. Its current cleanup work is a countertrend.

## How autonomous the development flows appear

**Paseo has the clearest publicly visible bot-operated maintenance loop. Loopflow has the most explicit end-to-end workflow machinery. T3 and Paradev look like supervised agent teams with substantial delegated follow-through. Orca shows coordinated high-volume campaigns, but the evidence does not establish that its high output means more independent decision-making.** Autonomy over implementation is much easier to demonstrate than autonomy over scope, risk or release decisions.

| Project | What the evidence supports | What still appears directed or is unknown |
|---|---|---|
| Paseo | Bot-authored fixes, its own two-pass review bot, status updates and bot merges; agent release babysitting after go-ahead | Private authorization of each bot run is unknown; release scope and publication are explicitly person-directed unless standing authorization exists |
| Loopflow | Graphs cover implementation, review, repair, gate and landing; CI-fix retries and release scheduling are built in; actual PR commits follow those stages | Repository Wave defaults include human demo/verification steps; no evidence that agents independently choose product direction |
| T3 | Agents are told to watch PR events and repair valid findings; automated reviewer infrastructure and nightly releases | Developer request required before opening PRs; integration concentrated in maintainer accounts; stable release promotion separate |
| Orca | Numbered fleet follow-up sweeps, design variants, queued repair campaigns and hourly preview builds | Examples include a person choosing the design; stable release is maintainer-managed; configured agent-review service does not establish current use |
| Paradev | Automatic read-only issue triage, Task-to-worker commands, cloud-workspace agent review and main-triggered production deployment | Instructions reserve push/PR/reply decisions for authorization; desktop releases default to drafts, with explicit headless completion options |

Paseo provides a particularly strong observed example. An external draft-loss report appeared November 8 at 03:59Z; a bot opened a fix at 04:27, posted its own review at 04:29, communicated status back to the report, then merged at 06:10. No human conversation appeared in the inspected PR comments. This establishes a bot-operated chain, though not whether a private human instruction started or approved it. Separately, the release playbook has the agent create a heartbeat every ten minutes and own publication/retries/verification after a release go-ahead. [Issue #9072](https://github.com/getpaseo/paseo/issues/9072), [PR #9079](https://github.com/getpaseo/paseo/pull/9079), [Release playbook](https://github.com/getpaseo/paseo/blob/27872a8e/docs/release.md#L440)

Loopflow's autonomy is unusually legible in both specification and execution. `auto` composes pursuit and delivery; delivery runs gate then `land --wait-and-fix`. The CI-fix agent is instructed to inspect the exact failed head, repair, verify and re-arm the same PR. PR1831's branch history visibly proceeds through concept review, compression, realignment, demo, gate and repeated CI fixes. All those commits use the same person's Git identity. The repository's `code` and `feature` defaults retain interactive demo/verification, so the existence of `auto` does not mean every Task runs unattended. A scheduled release graph and daily bootstrap exist; repeated release-repair commits establish that this machinery has been exercised, not that its present scheduler state was checked here. [Auto graph](https://github.com/loopflowstudio/loopflow/blob/1c757e9fe2028c0073414dc6826395931a5aa743/rust/loopflow/src/builtins/task/graph/auto.yaml), [Delivery graph](https://github.com/loopflowstudio/loopflow/blob/1c757e9fe2028c0073414dc6826395931a5aa743/rust/loopflow/src/builtins/task/graph/deliver.yaml), [Observed PR stages](https://github.com/loopflowstudio/loopflow/pull/1831/commits)

T3 explicitly expects development through T3 and prescribes event-driven PR babysitting: watch, wake on checks/comments/conflicts, validate findings, repair and continue. The same instructions require a developer request before PR creation. Nightly publication checks run twice hourly and publish new work after a six-hour interval; stable promotion is separate. Its ambitious contribution-triage procedure must not be mistaken for proven deployment: CONTRIBUTING explicitly distinguishes the intended rollout from operating automation. [Agent instructions](https://github.com/pingdotgg/t3code/blob/af65a55dc/AGENTS.md#L147), [Release workflow](https://github.com/pingdotgg/t3code/blob/af65a55dc/.github/workflows/release.yml), [Contribution policy](https://github.com/pingdotgg/t3code/blob/af65a55dc/CONTRIBUTING.md)

Paradev demonstrates why account labels are insufficient. Its create-PR command assigns `superset-home` to launch a cloud review workspace and publish a review Page/comment. On PR9432 that account reported a specific resume-mode/model bug, tests and browser checks; later changes carry those fields. This is agent review through an ordinary-looking account, outside the formal review statistic above. Automatic issue triage is read-only; it is not an agent independently deciding to implement every reported issue. Successful main-push deployments were observed, but production-environment approval settings were not inspected. [Create-PR procedure](https://github.com/ParadevAI/Paradev/blob/b77bc046ee98bbc3ac05c7ae792e630d7e7b73d0/.agents/commands/create-pr.md), [Review example](https://github.com/ParadevAI/Paradev/pull/9432), [Issue triage](https://github.com/ParadevAI/Paradev/blob/b77bc046ee98bbc3ac05c7ae792e630d7e7b73d0/.github/workflows/triage-issue.yml)

Orca PR39470 identifies numbered items in a fleet follow-up sweep; PR39379 refers to a person's choice among design variants. That supports parallel delegated work with centralized product judgment. Hourly Mac preview release runs were observed under an ordinary account; stable releases remain manually dispatched. Its Pullfrog workflow exists, but recent sampled runs were older/cancelled, so it cannot establish a currently active autonomous review loop. [Fleet sweep](https://github.com/stablyai/orca/pull/39470), [Design selection](https://github.com/stablyai/orca/pull/39379), [Hourly build workflow and observed run](https://github.com/stablyai/orca/actions/runs/42474268910), [Release cut](https://github.com/stablyai/orca/blob/5fc3b26766a9fc3ba95d8bc72600325f036f0db4/.github/workflows/release-cut.yml), [Pullfrog workflow](https://github.com/stablyai/orca/blob/5fc3b26766a9fc3ba95d8bc72600325f036f0db4/.github/workflows/pullfrog.yml)

**The shared pattern is autonomous execution under concentrated taste and authority.** No inspected evidence establishes autonomous product direction across an entire project. The useful question is how far a delegated outcome travels before another human decision is required, and whether the loop can recover without quietly lowering its quality bar.

## What dogfooding helps explain

Assuming each platform is used to build itself, the history suggests different feedback loops:

```text
Paseo     repeated interaction → notice friction → refine interaction
T3        broad contributions → integration center → formal recovery contracts
Paradev   adoption and business needs → product slice → integration follow-ups
Loopflow  operator failure → new system model → repair and simplify the machinery
Orca      parallel expansion → compatibility repairs → cleanup campaigns
```

These are interpretations, not proof of internal working practices. Dogfooding makes frequently used paths highly visible. It can leave first-run setup, older installations, less-used operating systems and recovery after unusual failures underexercised. The builder's environment can be both an excellent test bed and a narrow one.

## Better comparisons than commit volume

1. **Does a change reduce what a maintainer must understand?** Deleting a relay or an obsolete renderer can eliminate a failure mode. Splitting one runtime across 200 inherited pieces changes file size without giving it independent ownership.
2. **What does a test prove?** Prefer interrupted recovery, duplicate suppression, real query plans and tampered-download behavior over snapshots of source strings or mocked return values.
3. **How much repair follows a feature, and when does it ship?** A feature-to-repair sequence is informative; a raw fix percentage confuses polish, compatibility work and pre-release integration with escaped defects.
4. **Who can contribute versus who can integrate?** T3 is broad in authorship yet concentrated at merge time. Paseo's contributor tail and bot identities obscure strong central authorship. Local agent review remains invisible in GitHub review counts.
5. **What is the unit of change?** In 100 evenly spaced commits per repository, median changed paths are T3 2, Paseo 4, Paradev 7, Orca 7.5 and Loopflow 9.5. These include docs/generated files and are not code-size or quality scores. They help explain why raw commit counts exaggerate throughput differences.

The consecutive 30-day counts are Paseo 481 / 458 / 639; T3 1,357 / 1,293 / 4,647; Orca 2,308 / 2,601 / 6,721; Paradev 751 / 613 / 646; Loopflow 60 / 228 / 313. These count nonmerge commits reachable from main by committer date. Almost all are mainline squash commits; Paseo has a small merge-history difference. They describe acceleration, not productivity.

## Scope and sources

Approximately 23,000 mainline commits were analyzed mechanically. Qualitative inspection deliberately selected positive and negative examples around persistence, reconnects, lifecycle ownership, performance, browser isolation and code removal; it is not a random defect census. No full applications or test suites were executed, so this study does not rank observed uptime, security, visual polish or customer satisfaction. Test mechanisms were read, not merely inferred from test names. Reported run measurements remain attributed to their PRs.

Snapshots: Paseo `27872a8e86a48a31ee1ed8ec202b4715ecd4155e`; T3 `af65a55dc` (full SHA in evidence); Orca `5fc3b26766a9fc3ba95d8bc72600325f036f0db4`; Paradev `b77bc046ee98bbc3ac05c7ae792e630d7e7b73d0`; Loopflow `1c757e9fe2028c0073414dc6826395931a5aa743`.

The [measurement record](agent-platform-craft-evidence-2026-11-08.json) retains frozen heads, attribution counts, date windows, PR sample numbers, actors and formal-review observations. The [Orca superclass listing](agent-platform-craft-orca-chain-2026-11-08.json) retains each observed class, parent and path. Source links above pin the qualitative examples.

Questions a deeper comparison could resolve:

1. Which system best survives the same end-to-end failure exercise: restart mid-turn, reconnect, upgrade, resume and ship?
2. Which refactors reduce cross-module ownership and state complexity six weeks later?
3. Does contributor breadth translate into independent subsystem stewardship, or mostly contributions through a few integrators?
