# Loopflow critical review and report card

Prepared for Jack Heart · 2 November 2026 · Research in the LOO-714 checkout

**Overall: B today, with an A-level product thesis.** Loopflow makes a persuasive case for organizing agent work around durable purpose and deliberate participation. Its largest remaining burden is making that machinery feel dependable and easy to understand when ordinary things go wrong.

## An instrument still learning to stay in tune

Loopflow’s most attractive promise is that you might stop being the memory of your own tools.

Anyone working across several coding conversations eventually becomes their unpaid archivist: remembering which window contains the useful decision, which branch contains the actual work, which agent finished, and which merely sounded finished. Loopflow takes that exhaustion seriously. A Task gives the work an address. Its checkout, conversation, workflow and pull request belong together. A Wave gives an area of responsibility a life beyond the current ticket. There is something humane in this ambition: the computer should remember enough that a person can leave and come back. [The product model](../README.md)

The orchestra language on the website is grand, but beneath it sits an unusually specific concern about attention. More simultaneous activity is easy to display. Choosing the moments when someone should participate is harder. Loopflow’s interactive workflow steps put that choice into the work itself. In the Mac app, the small phrase “At demo · yours” expresses the idea beautifully. It locates the work and offers a place to enter. The New Task form is similarly assured: “What do you want?”, a destination, a workflow, and the points where someone takes part. That is the product at its clearest. [Public positioning](https://loopflow.studio/)

I like that the visual design gives this idea some warmth. Burgundy, cream, restrained borders, a serif heading in the task form: it feels like a place intended for sustained work. Yet the larger workspace is still dominated by a terminal transcript. The elegant frame surrounds a long conversation whose consequential facts can be buried several screens back. Sidebar descriptions truncate into fragments; activity, ownership and delivery compete for a narrow strip of attention. The room is inviting. Finding the thing that matters can still require rummaging.

The underlying workflow system deserves real credit. Skills are Markdown; Graphs are YAML. These are materials a developer can read, change, version and argue about. I created a disposable task, gave it a three-step workflow, watched it stop before interactive research, continued it through a command, and saw it stop at demo. A final continuation completed the task while retaining its PR-less checkout. The mechanism behaved coherently. This is a useful amount of structure, and it does not require buying into every other concept at once. [Built-in code workflow](../rust/loopflow/src/builtins/task/graph/code.yaml)

The commitment to working locally also has substance. I created tasks with no tracker account and no remote. Adding a local bare Git remote let another clone acquire the shared plan without a hosted Loopflow service. I particularly like that the shared plan and private conversations have different boundaries. That makes collaboration possible without treating every conversation as organizational property. It also leaves more responsibility with the product to explain when the local view has caught up. My second clone initially showed an empty list; a later synchronization brought in the task. The eventual behavior worked, but the route to a fresh answer was less obvious than the promise. [Planning behavior](../docs/work.md)

Then there is the code’s almost stubborn insistence on evidence. Missing usage is not quietly converted to zero. A process exit does not establish that the work succeeded. A provider’s claim does not acquire authority to complete a task. Native process identity and durable identity are treated separately. Those distinctions are valuable in software that delegates work to systems capable of confidently describing events that did not happen. Loopflow has the temperament of a bookkeeper who enjoys music but still wants the receipts. [Telemetry](../docs/telemetry.md) · [Graph transitions](../rust/loopflow/src/graph/transitions.rs)

That temperament is supported by substantial engineering. I read provider-trace tests, shared Rust/Swift data fixtures, migration guidance, and a two-person planning scenario. The architecture inventory check passed, mapping all 77 discovered SQLite owners/mirrors within its declared scope. None of that proves the whole product works. It does establish that this is a serious system with explicit boundaries and an effort to test them. The development culture also asks agents to simplify, reconsider concepts, demonstrate outcomes and verify what actually shipped. Those are good instincts to encode. [Test boundaries](../rust/loopflow/tests/README.md) · [Two-person scenario](../rust/loopflow/tests/two_people.rs)

But the precision of the machinery is escaping into the experience.

At one point my test task reported “Planning: unstarted,” a task state of “active,” and a “Completed” Graph, while its workflow waited at demo. Each statement refers to a different fact. Together they make a person solve a small ontology problem before answering “What happens next?” Task, Graph, Process, Session, provider thread, ownership, availability and shipping approval all earn a place somewhere in the implementation. They do not all earn equal prominence in the first explanation. I want the leading sentence to say: “Ready for your demo; nothing is running.” The machinery can remain available underneath it.

This is where Loopflow sometimes feels like a workshop whose operating manual has become another machine. Even its concise commands carry distinctions to learn: `ship` runs the remaining task workflow; `land` requests merge; `research` resolves to a Graph while `skill research` selects the Skill. The help explains these distinctions, which is good. The number of explanations still accumulates. The full `code` Graph I inspected expanded to twelve entries, including two decision steps. There is craft in that arrangement. There is also a risk of paying for ceremony on work that needed a lighter touch. I would want comparative evidence of better outcomes per unit of time and cost before assuming the longer workflow always earns its keep. The inspected evaluation documentation carefully limits its own claims to proposed actions. [Evaluation limits](../scripts/evals/README.md)

My strongest criticism concerns continuity. It is central enough to the product that it deserves a higher standard than merely recovering files.

The current account-failover code clears the provider resume token and supplies a new prompt asking the replacement to reread the workspace. Transient retries can retain the provider conversation; this account-switch path starts differently. Pi and OpenCode also return without this automatic retry path. Meanwhile, conversation text remains in provider-native history, and the telemetry documentation describes provider retention as a limit on what can later be recovered. Durable Loopflow identity and durable access to the entire conversation are different promises. The README’s reassuring language makes the seam easy to overlook. [Failover implementation](../rust/loopflow/src/agent/mod.rs) · [History retention](../docs/telemetry.md)

The app made the consequences tangible. The workspace I opened contained repeated limit messages followed by a manual login and retry sequence. Its sidebar also showed shipping retries alongside broad “Working” labels. This was observed history and displayed state, not a controlled reproduction of those failures. Still, it showed exactly the kind of attention tax the product exists to reduce. The LOO-714 recovery drafts recognize the problem; they remain proposals. A critic should grade the experience that exists, while giving the diagnosis its due.

Documentation has a related problem: it is plentiful, thoughtful, and unevenly synchronized. The work guide correctly says a bare task run returns once the Graph starts; the architecture page says it returns when the Graph ends. The current data guide still lists Linear as the shared-planning owner even though Git planning worked in my experiment. These are consequential discrepancies in a product whose complexity makes documentation unusually important. More prose will not resolve this; fewer conflicting accounts will. [Work guide](../docs/work.md) · [Architecture guide](../docs/architecture.md) · [Data guide](../docs/architecture/data.md)

Performance is mixed rather than uniformly slow. My small CLI interactions mostly finished in tens or hundreds of milliseconds. Historical timing records for six app launches labeled 0.15.1 showed a median recorded first frame of about 679 ms, but a maximum of 23.4 seconds. That is a small, uncontrolled sample, not a benchmark or a diagnosis. It is enough to temper the promise that the tool disappears. A product devoted to preserving concentration is judged partly by its worst interruption. [Measurement definitions](../scripts/benchmarks/desktop-performance/README.md)

I also appreciate the security documentation’s plainness. It explicitly says that a worktree is not a security sandbox and explains the more permissive policies used for unattended work. The practical consequence is that Loopflow’s autonomy relies on the execution environment and credentials entrusted to it. That tradeoff belongs in any serious assessment of the product; workflow review and process accounting do not replace containment. [Security model](../docs/security.md)

What I like most is the conviction that agency should come with memory, inspectability and places for judgment. What I like least is how frequently understanding the system still seems to require thinking like its implementer. The next substantial improvement would be felt as relief: ordinary failures preserve the thread, the next useful action is unmistakable, and a person can return after lunch without reconstructing the orchestra’s administrative history. I want to close the laptop and trust the rehearsal will still make sense tomorrow.

## Report card

Grades are editorial judgments about the inspected product, not measured reliability percentages. A means unusually strong; B means useful with material friction; C means recurring limitations in an important promise.

| Area | Grade | Assessment |
| --- | --- | --- |
| Product thesis | A | Durable work and deliberate participation address a recognizable burden of using multiple agents. |
| Workflow composition | A− | Plain files, inspectable expansion, meaningful interactive stops; the disposable lifecycle worked. |
| Local control and portability | A− | Local tasks worked without a tracker; Git planning reached a separate clone. Freshness needs clearer presentation. |
| Architecture and evidence discipline | B+ | Strong ownership distinctions and substantive tests; maintaining the growing surface is expensive. |
| CLI experience | B | Fast small operations, useful help and context preview; overlapping names and status concepts require study. |
| Desktop design and orientation | B− | Warm visual character and excellent entry form; transcript density and competing status cues weaken orientation. |
| Failure recovery and continuity | C+ | Work survives, but account failover can replace the native conversation and provider behavior differs. |
| Documentation consistency | C+ | Excellent examples and useful depth, undermined by concrete contradictions about lifecycle and storage. |
| Performance | B−, provisional | Small CLI probes felt quick; six historical app launches showed a substantial startup outlier. |
| Security transparency | B+ | Clear description of authority and containment limits; this is not a security audit. |
| Workflow cost effectiveness | Ungraded | No controlled comparison here establishes that additional review loops pay for themselves. |
| Overall | B | A compelling tool for engaged practitioners; recovery and comprehensibility still limit effortless trust. |

## System map

```text
Request → Task → isolated checkout + durable work identity
             │
             └→ saved Graph → commands and agent steps
                                  │
                         stop for participation
                                  │
                         continue → delivery / end

SQLite: identity, history references, observations, authority
Repository files: Skills, Graphs, goals, working notes
Provider: native conversation text and model execution
Git ref / tracker: shared plan       GitHub: PR and merge facts
CLI / Desktop: views and actions over these owners
```

## Evidence and limits

- Source inspected at commit `4e9ade2d04`; installed CLI reported `lf 0.15.1`. Source findings are not proof that the installed executable has identical bytes.
- Read the README, getting-started/work/config/security/session-history documentation, architecture and storage material, Graph definitions and selected Skills, retry and session-record code, graph transitions, Desktop status types, tests, CI guidance, benchmark documentation, release notes, and existing recovery drafts. Also inspected the public homepage and repository page. This was a broad review, not an exhaustive audit of every module.
- Inspected the running Mac workspace and opened/cancelled its New Task form. No real task was submitted or altered through the UI. Displayed failure history does not establish a current provider outage.
- Used a disposable `LF_HOME`, repository, task checkout, bare remote and second clone. No paid model run, remote rental, production tracker write, release or PR merge was performed. The toy Graph used only interactive stops and a mechanical context command; it does not prove autonomous implementation quality.
- Verified task creation without a remote, checkout creation, Graph expansion, malformed definition diagnostics, context preview, task pauses and continuation, command/process history, completion and checkout retention.
- The peer initially returned an empty task list. `wave status --sync` subsequently acquired the shared plan but exited 1 because no Wave was selected; the next task list contained the shared task. This establishes transport and import in this fixture, with an awkward standalone refresh interaction.
- `uv run python scripts/check_architecture.py --json` passed: 22 public API families, 4 process boundaries, 77 SQLite owners/mirrors, 10 read projections, 8 provider edges, 35 subprocess edges and 1 compatibility seam mapped. This inventory check is not a behavioral test suite.
- Timing data came from the existing local journal, not newly controlled trials. For six 0.15.1 launches: first-frame median 678.5 ms, maximum 23,387.1 ms; fresh-data median 3,191.9 ms, maximum 24,368.4 ms. No usable endpoint samples were recorded, so no time-to-usability claim is made. No p95 is justified by six samples.
- Original unrelated scratch files were preserved. Research artifacts do not approve the LOO-714 design or change product behavior.

## Questions that would change this assessment

1. Can an interrupted task preserve its actual conversation across account exhaustion with one understandable recovery action?
2. Can the default status explain present activity and the next useful action before exposing the underlying state distinctions?
3. Do the default review loops improve accepted outcomes enough to justify their additional latency and model usage?
