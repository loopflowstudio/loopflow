# The docs read in the new voice

Design for LOO-309 · 2026-09-26 · Reviewed implementation direction

Task: The docs read in the new voice, from the overview to the reference.
Base: `102522bc25adc146d4b8cc3f343677ed8170f636` (PR #1297).
Accepted audience and writing decisions: [growth memory](../wave/growth/MEMORY.md).
Jack completed the design review with the full Task as the implementation scope
and unbounded PR size. This does not record approval of new copy.

## Problem

Scott and Kim have enough programming knowledge to understand variables, loops,
and files. The docs assume they also know why engineers use branches, reviews,
CI, separate accounts, and remote machines. Changing the first paragraph leaves
that gap throughout the rest of each page.

Jack requested friendly, literal documentation that can carry the same reader
from a first task to his level of use. The Mac app is the primary experience;
commands belong beside the relevant app explanation. The content must still
explain exact behavior, including advanced behavior. It must never become a
second homepage or a separate beginner track.

The growth objective is that readers can understand Loopflow and reach a first
finished task. This Task supplies the explanations after the front door. It
does not fix installation or invent a first-task path that does not work.

## The demo

Open `/docs/conducting` on the local website. A reader can find how to inspect
running work, distinguish a Task from a Run and a Session, and understand why
finishing a review returns feedback rather than approving a merge; each concept
has a working glossary link and the corresponding command beside the explanation.
The same explanations are available at `/docs/conducting.md`.

At Task completion, repeat that reading check across the public documentation
and fetch `/llms.txt`: its first summary defines Loopflow in the accepted words,
then points to the same pages without a second technical introduction.

## Approach

Rewrite the full in-scope documentation in place. Jack confirmed that PR size
is unbounded; page boundaries do not limit delivery scope. Extend the one
glossary alongside the pages. Preserve URLs, reference tables,
flags, and valid technical examples. Keep advanced explanations on their current
pages; explain the term and consequence where the reader encounters them.

Every revised page follows this editorial contract:

1. Open with the situation in which the reader needs it and what they can do.
2. Establish the app context first where that capability exists. For file or
   command-only work, say plainly that it is done in a file or terminal. Do not
   invent an app editor, setting, or button to satisfy a template.
3. Define each unfamiliar term at first use and link the glossary. A reader
   arriving directly at a reference page must not need to read all earlier pages.
4. Put the command adjacent to its explanation. Explain placeholders before a
   reader might paste them, especially `<wave>`, task identifiers, and Home IDs.
5. Explain the practical consequence of defaults: what happens without an
   override, why that is useful, and when to choose differently. Do not invent
   historical reasons for an implementation choice.
6. Preserve the distinctions that change behavior: review versus merge, stopped
   versus completed, missing usage versus zero usage, Task versus Run, and local
   versus remote records. Plain language must make those distinctions clearer.

### Existing owners and integration

| Surface | Authority and change |
| --- | --- |
| `docs/*.md` | Canonical authored public pages; edit these directly. |
| `docs/glossary.md` | One vocabulary source; extend its existing tables. |
| `website/main.py` `DOCS_AREAS` | Public page inventory, titles, descriptions, and navigation; revise descriptions where needed, keep slugs. |
| `website/main.py` `generate_llms_txt()` | Curated machine-facing summary; replace its introductory prose. |
| `generate_llms_full_txt()` | Already assembles canonical page content in navigation order; retain this mechanism. |
| `website/dev.py` `sync_docs()` | Copies canonical docs for serving/deployment; never hand-edit `website/docs/`. |
| `README.md` and `docs/index.md` | Shared accepted opening, covered by `test_readme_index_sync.py`; preserve it. Audit remaining overview prose for terminology only. |
| `rust/loopflow/src/lf/mod.rs`, operations, and Swift views | Evidence for behavior and existing app capabilities, not a substitute for walking the app. No runtime change in this Task. |
| `/architecture` and `docs/architecture*` | Existing developer path; leave it separate and unchanged. |

No new content store, docs generator, vocabulary schema, or parallel edition.
Delete obsolete explanations in their owning pages rather than retaining both
old and current instructions. Preserve existing heading anchors when possible;
if a heading must change, update all referring links in the same change.

### Page plan

| Page | Reader's situation and required work | Details that must survive |
| --- | --- | --- |
| Conducting | Work is running; find what needs attention. Keep the accepted opening, connect app surfaces to nearby commands, explain status/history/usage and review completion. | Home-local reads; Task conditions versus execution versus Sessions; missing telemetry; replay creates another Run; steer receipt does not prove compliance; exact Session completion. |
| Waves | Keep working toward an objective across several Tasks. Move existing app context before the first commands; explain Wave → Task and a chapter as its current plan. | One internal Project per chapter; started work retains identity; untouched backlog is canceled; unknown evidence does not authorize cancellation; retry behavior, metrics, chat, placement, and memory. |
| Authoring | Change what an AI step does or which steps run. Start with files behind the app's work; keep the working skill example, then flows and goals. | Source resolution; scratch between steps; `human: true`; Complete returns feedback; a deciding step owns backward edges; captured definitions on resume; explicit delivery authority. |
| Agent API | Ask an AI coding tool to operate Loopflow. Relate it to the commands behind the app, then teach the caller's authority and examples. | External versus internal callers; Work versus Run; delegation boundaries; Linear steering; JSON observations; publish/submit/arm/land distinctions. Remove stale Project-operator and miscellaneous `.lf/` learning guidance. |
| `lf` reference | Look up an exact command or flag used alongside the app. Replace the three-audiences opening with a reader action; explain the advanced prose around the existing tables. | All valid command/flag tables, examples, defaults, and section anchors. Explain code identifiers in prose; do not rename CLI tokens for tone. |
| Configuration | Change behavior for one run, a repository, or all work. Introduce that scope choice before the long name-resolution discussion; retain name behavior in its own section. | Precedence, additive lists, personal-only settings, launch modes, account/model selection, and Task-specific sandbox exceptions. Preserve reference tables. |
| Subscriptions | Choose which existing coding-tool account work uses. Explain account, route, profile, and fallback before the detailed machinery. | Connect does not create an account; account versus browser profile; local versus forwarded credentials; `--account` versus `--only-account`; session identity. No invented account-management screen. |
| Security | Understand what files, accounts, and machines an agent can reach when started from the app or terminal. Explain boundaries before operational detail. | A worktree is not OS containment; permission prompts differ from isolation; general launch policy versus durable Task write scope; secret forwarding and persistence guarantees. No stronger safety claim from simpler wording. |
| Troubleshooting | The app shows stopped, waiting, or failed work. Explain which state to read before choosing recovery; command examples remain concrete. | Do not restart an active worker; ready does not mean running; Run-history limits; account exhaustion behavior; safe worktree cleanup. Remove the nonexistent `project run` recovery path. |
| Glossary | Look up the words used by the pages above. Extend definitions as each page lands. | One meaning per concept; distinguish overloaded words explicitly; no implementation identifier dump. |
| Overview and agent index | Check the existing front door and make `/llms.txt` agree with it. | README/index opening equality; existing Markdown negotiation, routes, and public/developer corpus split. |

### Vocabulary coverage

"No page uses a term the glossary lacks" means every Loopflow concept and every
borrowed technical term requiring knowledge beyond the stated CS 101 baseline.
It does not mean ordinary English, filenames, every flag spelling, or variable
names need dictionary entries. Exact API fields still need a nearby explanation
when the reader must interpret them.

Start the inventory with the gaps already present in the source:

- Conducting: context, Work, roadmap, telemetry, process, JSON, SSH, TUI,
  manifest, receipt, replay, and append-only. Prefer plain replacements for
  "projection," "semantic condition," and "provider-neutral" where possible.
- Waves and Authoring: metric, target, freshness, cadence, frontmatter, cron,
  daemon, Initiative, issue, interactive/headless, routing/XOR, backward edge,
  invocation, and checkpoint. Define measurement "instrument" separately from
  the product's metaphor if it remains necessary.
- Agent API and reference: API, SDK, argument/flag, standard input/output,
  environment variable, glob, diff, remote, credential, authentication,
  authorization, subscription, account route, access profile, sandbox,
  container, VM, OS, PID, OAuth, HTTP, and MCP when retained.

This is a seed inventory, not a claim of exhaustive coverage. For each page,
read every heading, paragraph, table description, and example comment; either
replace an unnecessary term with ordinary language or add its definition and
first-use explanation. Check existing glossary definitions against behavior too.

The glossary currently uses table rows, which have no per-term heading anchors.
Use links to `glossary.md#loopflows-words` or
`glossary.md#borrowed-from-software-engineering`, with the term as link text.
Do not emit nonexistent `#worktree` or `#session` anchors. Keep the existing
two-section layout; per-term navigation is not required for this rewrite.

### Defaults and explanation

Keep actual defaults and their exceptions unchanged. Explain their use without
claiming undocumented intent. Examples for the rewrite:

- Per-run options let the reader try a change without changing later work;
  repository configuration applies to work in that repository. Explain additive
  lists separately from scalar overrides.
- Default context includes the operating guidance and working notes, while
  extra files are opt-in; explain the information/token tradeoff and how to add
  the specific source needed for the task.
- Interactive runs support conversation; unattended work cannot stop for typed
  answers. Explain how authored review steps bring a person back in.
- Home-local observations describe one computer. To inspect another computer,
  address it explicitly; do not imply that the app aggregates all machines.
- Readiness records feedback; explicit completion releases the waiting caller.
  Review completion and the later flow decision remain different actions.
- Account fallback can keep work going when one account is unavailable;
  restricting accounts changes that behavior and which subscription can be used.

## De-risking

Observations below come from this checkout and read-only help probes on
2026-09-26. No app walkthrough, reader study, provider launch, or live account
exercise was performed.

| Question | Finding | Impact on design |
| --- | --- | --- |
| Is the first-pass PR available? | `git log -1` identifies HEAD as “Clarify Loopflow’s first steps and use an installed coding agent (#1297)”; glossary and accepted introductions are present. | Continue the first pass. No cherry-pick, rebase, or dependency on its old open-PR status. |
| Does an earlier design constrain this work? | `scratch/` contained only `.gitkeep`; supplied Task and growth memory hold the accepted direction. | This is the first design artifact; preserve those accepted decisions without treating new copy as approved. |
| Can app-first copy be written as a new walkthrough here? | The run supplies no rendering environment. Swift contains existing Wave, Session, and Task-flow views, but inspecting those does not prove the click path. | Retain established app orientation and state capabilities; do not author new click-by-click paths from code. LOO-306 owns the first-task and terminal walkthrough. |
| Are review controls current? | `docs/conducting.md` says Approve/Iterate. `session_actions()` in `rust/loopflow/src/ops/human_session.rs` exposes Complete; Flow help says it returns feedback to the next step. `SessionsView.swift` renders that action. `lf session --help` lists open/complete/ready. | Replace the obsolete review paragraph; explain readiness, completion, and subsequent decision separately. Code evidence verifies semantics, not an observed app interaction. |
| Is Project a user-operated tier? | `docs/agent-api.md` describes Project operation; Troubleshooting recommends `project run`. `Commands` in `rust/loopflow/src/lf/mod.rs` has no Project command, and `ops/project.rs` resolves the internal chapter plan. `lf project --help` prints generic help and exits 0. | Exit success is not command-existence proof. Use Wave → Task; remove obsolete Project-operation directions and preserve Project only as the internal chapter plan. |
| Is the Agent API's learning contract accurate? | Its last contract bullet directs learnings into `.lf/`. Builtin `LOOPFLOW.md` assigns conventions, configuration, skill instructions, and Wave memory to their respective owners and forbids miscellaneous handoff notes. | Correct the summary in place; do not copy the entire operating contract. |
| Is a generic sandbox explanation sufficient? | Configuration already documents stronger write restrictions for durable Task turns; Security's opening policy does not explain that exception. | Reconcile descriptions against `engine/agent.rs` before rewriting them. Preserve scope differences; do not promise isolation from all tools or credentials. |
| Can a new glossary link point at any term? | Terms are bold table cells; renderer enables Markdown `toc` on headings. | Link the existing sections, then verify rendered fragments. No custom anchor system needed. |
| Which pages are public? | `DOCS_AREAS` contains 12 slugs; developer architecture has a separate inventory and routes. `llms-full.txt` reads `DOCS_NAV`. | Scope comes from the existing inventory, not every file under `docs/`. Keep architecture excluded. |
| Can testing read a stale docs copy? | `doc_path()` prefers `website/docs/` over root `docs/`; `dev.py sync-docs` refreshes it. | Sync before HTTP or rendering checks. Generated copies are not authored changes. |
| Will voice edits break existing tests? | Docs tests assert literal Security and Subscriptions phrases as well as endpoint behavior. | Keep meaningful semantic checks. If a phrase changes, update that assertion to the same guarantee; never weaken or delete it just to pass prose. |
| Are there chapter targets to satisfy? | The supplied snapshot has no metric targets; no KR evidence is supplied. | Use the Task's reading and retrieval outcomes. Do not invent a numeric comprehension claim or query unrelated planning state. |

The same-day Wave memory also says the Mac app lacks a flow model. This checkout
has `TaskFlowView.swift` mounted by `WorkSurfaceView.swift`. That is evidence
that the old observation needs revisiting, not proof of the requested people-only
view. This Task makes no new claim about LOO-317's completion.

## Alternatives considered

| Approach | Tradeoff | Why not |
| --- | --- | --- |
| Rewrite introductions only, with a glossary link | Small diff and little risk to references. | The first unexplained internal concept after the opening recreates the original problem. Does not meet the Task's acceptance. |
| Add a beginner guide and keep technical pages as-is | Leaves technical detail untouched. | Requires a second reader track Jack rejected; users arriving at reference pages still face unexplained terms. |
| Rewrite each existing page through its full explanatory prose, keeping exact reference material | More editorial work and behavioral checking; each page is usable on its own. | Chosen. One vocabulary, one set of URLs, and the same reader can reach the full capability. |

## Key decisions

- Jack's review correction on 2026-09-26: “Unbounded size per PR.” The current
  implementation scope is the full Task. Do not stop after Conducting or split
  delivery by page count. The page plan organizes the work within that scope.
- Reference tables and flag spelling stay intact. Correct verified stale
  instructions in surrounding prose/examples; if a table itself is wrong,
  record the exact discrepancy and source before making a targeted correction.
- The literal accepted definition leads `/llms.txt`: “A software instrument.
  It doesn't make the software for you. You make the software through it.” Keep
  H1 and blockquote structure. Follow it with concise retrieval/use context,
  replacing the current Work/Home/authority preamble. Do not copy the marketing
  problem paragraph into reference pages.
- At most two musical/product metaphor uses of “instrument” per page. This
  cannot ban exact metric identifiers or necessary measurement terminology;
  prefer “metric” in explanatory prose and define the technical meaning once.
- Wild success: Scott arrives at a deep reference link, understands why its
  default exists, changes it deliberately, and can explain the consequence.
  The page serves him again as he learns more; no second edition is needed.
- Wild failure: friendly openings hide dense internals, removed detail sends
  Kim to source code, or invented app buttons stop her. Full-page term review,
  retained technical distinctions, and the ban on unobserved walkthroughs address
  those concrete failure modes.

## Scope

- In scope: Waves, Conducting, Authoring, Agent API, all six public Reference
  entries including the glossary; overview terminology audit; agent-index copy
  and page descriptions; necessary source-backed accuracy corrections and links.
- Out of scope: rebuilding Get started or adding the terminal page (LOO-306),
  Jack's blog post (LOO-311), installer/onboarding fixes (LOO-312/316), the
  people-only flow view (LOO-317), homepage positioning, architecture pages,
  runtime behavior, live Wave controls, and planning-account setup.
- Get started's existing terms are included in the final public-corpus glossary
  audit; this does not authorize rewriting its walkthrough. Preserve the README
  and index introduction unless a separately accepted change requires both.

## Done when

For every in-scope page:

1. Its opening states when it is useful; existing app context comes before the
   corresponding commands. Command-only capabilities are identified plainly.
2. A full-page editorial pass records retained unfamiliar terms and their
   glossary coverage in this design's ledger. Terms are explained on arrival,
   defaults have practical reasons, and no sentence sells or compares virtues.
3. Original technical behavior is retained or a correction has a concrete source.
   Table and flag changes have an explicit accuracy justification.
4. Existing local links and new glossary fragments resolve in rendered HTML;
   the canonical HTML and Markdown endpoints serve the revised page after sync.
5. The page remains useful when opened directly; the glossary is support, not
   required reading before understanding every sentence.

Focused implementation proof for docs rendering and retrieval:

```bash
cd website
uv run python dev.py sync-docs
uv run --extra test pytest tests/e2e/test_docs.py tests/e2e/test_agent_readers.py -q
```

Use the existing server fixture for route/retrieval checks. Extend a behavioral
link check only if needed to cover actual glossary-link resolution; do not add
snapshot tests pinning the new prose or a heuristic that claims to detect jargon.
The browser-backed command above requires a rendering-capable environment. In
this headless run, source review and help probes are evidence only; do not report
the browser check as passed or silently replace it with app-use proof.

At delivery gate, run the affected suite once and the repository-required check:

```bash
uv run python scripts/check_architecture.py
cd website
uv run python dev.py test
```

For the complete Task, verify every public slug in `DOCS_AREAS`, README/index
opening equality, unchanged routes and Markdown negotiation, glossary coverage
across the corpus, and the accepted definition at the start of `/llms.txt`.
Keep `/llms-full.txt` generated from the revised pages, with architecture excluded.

A read-aloud with Scott, Kim, or another non-engineer is the growth program's
user proof. Ask the reader what they would do and why, and record their exact
words with permission and attribution. It has not occurred here. Technical and
editorial checks cannot establish comprehension; record that limitation rather
than claiming the reader test passed. No new click-by-click app path ships without
an observed running-app walkthrough.

## Forbidden outcomes

- Calling LOO-309 complete after rewriting one opening or one page.
- A second beginner/professional track, renamed URLs, or a new public Project tier.
- A plain-language summary that removes operational exceptions or puts the
  indispensable explanation only in developer architecture.
- Invented UI controls, a terminal command presented as AI chat input, or a
  first-task walkthrough inferred solely from Swift source.
- Completion described as merge approval, readiness described as completion,
  an obsolete `project run` recipe, or Work attribution described as authority.
- A glossary that claims coverage while unexplained terms persist in table
  descriptions and example comments; links to nonexistent term anchors.
- Hand-edited generated docs, duplicated page authorities, or static agent
  summaries that diverge from the public docs.
- Passing wording assertions reported as proof of non-engineer comprehension.

## Implementation coverage

These are parts of the full implementation, not separate PR boundaries.

1. **Conducting + its glossary coverage.** Review current work, read history and
   usage, steer, and complete a review with accurate semantics.
2. **Waves + vocabulary.** Objective, current chapter, Tasks, memory, metrics,
   chat, and Homes in the same reader's language.
3. **Authoring + vocabulary.** Skills, flows, goals, and reviews; keep working
   examples and explain the decisions those files encode.
4. **Agent API + vocabulary.** Agent operation in the same model; correct the
   stale Project and learning-contract descriptions.
5. **Reference pages.** `lf`, Configuration, Subscriptions, Security, and
   Troubleshooting, each with explanations and glossary coverage.
6. **Corpus closure.** Overview/remaining public-term audit, page descriptions,
   `/llms.txt`, README parity, links, and retrieval checks. Reconcile drift from
   neighboring docs Tasks without taking over their walkthroughs.

## Current implementation scope

Implement the full page plan: Conducting, Waves, Authoring, Agent API, reference
pages, glossary coverage, overview terminology, page descriptions, and the
`/llms.txt` opening. PR size is unbounded. Conducting is a useful reading
scenario, not a stopping point or a separate delivery requirement.

Edit canonical `docs/*.md` and the relevant descriptions and summary in
`website/main.py`. Modify existing website tests where needed for meaningful
link, retrieval, or semantic assertions. Preserve the scope exclusions above,
including new app walkthroughs and runtime changes. Verify the complete public
corpus against the Done when criteria.

This review updates the design only. It does not execute later Flow steps,
publish, or mark the Task finished.

## Evidence and review ledger

- 2026-09-26 — Kickoff at `102522bc2`. Clean initial workspace; read repository
  guide, growth goal/memory, public inventory, guide sources, reference openings,
  relevant behavior implementation, and website test setup. No existing design.
- 2026-09-26 — Read-only probes: `lf --help`, `lf session --help`,
  `lf wave --help`, `lf project --help`. Last probe returned generic help with
  exit 0; command inventory and source show no Project command. Do not use exit
  code alone to validate examples.
- 2026-09-26 — Review findings incorporated: preserve reference material while
  fixing actual stale prose; do not infer app paths from source; avoid phantom
  glossary anchors; distinguish review completion from flow navigation.
- 2026-09-26 — Jack rejected the page-sized delivery interpretation:
  “No. This is totally wrong. Unbounded size per PR.” Removed the
  Conducting-only implementation boundary and page-by-page PR plan. The full
  Task is the implementation scope. This confirms delivery scope, not approval
  of new copy or every other design detail. See [review notes](docs-voice-review.md).
- Implementation, website tests, app demonstration, and reader study remain
  unperformed. No quantitative chapter target or comprehension baseline exists.
- 2026-09-26 — After the agent reported no other blocking design questions,
  Jack requested “ok Complete”. The corrected design is ready to drive
  implementation; remaining editorial choices follow the recorded audience
  and writing rules. No Flow navigation decision is recorded by this review.
