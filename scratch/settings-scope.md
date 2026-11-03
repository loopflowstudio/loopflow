# Consistent settings and personal graph-step overrides — LOO-714

Jack Heart approved this direction and requested implementation on 2026-11-02
(“lg2m”, “lets build it”). Placement: Infrastructure. This is the authoritative
first-PR design, superseding storage-model.md, route-design.md and the selection
questions in design-questions.md. Preserve the separate recovery outcome below.

## Accepted outcome

One settings model across CLI, environment, lfdb, personal config, repository/Wave
config, skill files and graph-step files, with personal overrides for particular
Graph occurrences and explanations of effective values. Jack explicitly means
**everything settable anywhere**, not only agent strings.

This PR is the coherent settings/resolution cut. Recovery is the second PR.
No tiers, permanent Codex-only policy or account transcript-sharing scheme was
accepted. Publish for Jack's review; do not ship or complete the whole Task.

## The demo

```yaml
# ~/.lf/config.yaml
agent: claude
skills:
  implement:
    agent: codex
graphs:
  release:
    steps:
      write-code:
        agent: claude
```

```yaml
# Shared release graph
- step:
    name: implement
    id: write-code
```

Ordinary launches prefer Claude; implement prefers Codex; write-code in release
prefers Claude. Personal preferences change no shared graph file. Two occurrences
of implement can differ. Eligible settings use the same names/types in each block,
not separate skill_agents/skill_docs/skill_yolo maps. Agent remains harness[:model];
a bare harness preserves its native model default. Resolve each setting separately.

Implemented matching CLI:

```sh
lf set --global skills.implement.agent codex
lf set --global graphs.release.steps.write-code.agent claude
lf unset --global graphs.release.steps.write-code.agent
```

Retain repository/Wave DB scopes too. Unset exposes the next applicable layer.
`lf config show --skill implement --json` and `lf config show --graph release
--step write-code --json` expose effective values and candidate provenance through
the shared CLI/Desktop read operation. Agreement with provider launches remains
part of acceptance. Diagnostics contain no credential values.

## Resolution and propagation

Highest first: explicit CLI > explicit environment > lfdb > personal config >
repo/Wave config > built-ins. Source precedes specificity: even broad DB agent
beats a specific personal-file graph-step agent. Within a source, Wave > repo >
global. Within one scope, graph-step > skill > general. An authored graph occurrence
beats its skill default in the same source. Preserve source identity until resolving;
do not merge generic/skill settings independently and accidentally reverse authority.

Graph-step selectors name the defining graph and its authored local step ID.
Composition retains origin: release/write-code matches when release is composed
into another graph. Reusing that graph applies its preference to each instance;
loop iterations retain the same target. Other graphs using write-code do not match.
Use no list offsets or process IDs. Anonymous steps still run using skill/general
preferences; authored IDs enable occurrence targeting. Duplicate IDs within a
single definition cannot silently select an occurrence. A global preference for a
graph absent from the current repo remains valid; inspection can show it unmatched.
No arbitrary cross-graph selector language is needed.

Session `-a claude`, including lf resume, selects that conversation only. Its Task
work still honors implement=Codex. Neither the Session selection nor a resolved
default becomes an inherited override or a saved Task agent preference.
Explicit `lf -a codex graph ...` (and explicit Task execution) applies to all its
agent steps, including composition, routers and loops. Carry this as Graph execution
intent, not an ambient policy escaping into unrelated Tasks created by an agent.
Explicit CLI beats env. Document propagation of user-supplied environment settings;
internal LF_* identity plumbing is not another preference layer.

## Coverage and implementation

The inventory below covers Config fields, personal-only readers, CLI, public
environment, lf set, machine/account preferences, Desktop/API, skill metadata and
Graph metadata. Semantic restrictions remain explicit: UI theme/terminal are
personal; planning and release describe repository work; graph topology is authored
structure; lifecycle commands, registrations, credentials and usage observations
are not scalar settings. An inventory entry alone does not establish launch proof.

Each preference has a canonical key/type, valid sources/scopes/targets, default,
merge/reset rule, propagation boundary and reader/writer. Share typed semantics
between parsing, set/unset, resolution and inspection. Reuse Config/Skill/Graph
structures where possible; preserve provenance. Core operations collect applicable
candidates, resolve a setting with explanation, and apply it to the existing consumer.
Do not add a new resolver while retaining old execution paths.

Preserve intentional additive docs/context/exclude/summaries and keyed-map merging;
ordered account routes replace as ordered lists. Make false, empty, omitted and
unset explicit. File and DB values must share types/validation. Generic typed CLI
overrides can cover eligible nested settings instead of creating a flag or env name
for every leaf; existing flags remain syntax over the same semantics. Audit boolean
negation and collection replacement as well as scalar precedence.

Native provider metadata remains native: Claude agent: Explore is not an lf
harness. Only Loopflow-owned metadata participates.

## Implemented authority cut — reconciled 2026-11-02

`config/settings.rs` now retains source layers through resolution and produces
effective configuration plus candidate provenance. Prompt preparation, configuration
inspection and Wave setup consume that resolver. CLI and Desktop select
`InvocationSettings`; the duplicate `CALL_AGENT` / `AgentSelection` store and the
`layer_setting` / `graph_setting` wrappers are removed. CLI outranks environment;
resolved Session defaults are no longer exported as future policy.

The single `settings_authority.sql` migration drops `tasks.agent` and converts DB
`skill_agents.*` keys to `skills.<name>.agent`. Task API/DTO mirrors no longer carry
an agent preference. Execution/provider history and `provider_routes` remain.
Builtin metadata uses `agent`; retained captured Graphs deserialize old
`default_agent` without restoring a separate authority tier. Runtime Graph children
receive their captured `SkillInvocation`, including occurrence source and settings.
Native recipe export now uses the same invocation reader and settings resolver,
retaining authored occurrence identity and source authority.

Shared configuration must remain readable by released lf until installation;
the existing `workflow` alias is marked for LOO-637. No second Task migration or
parallel settings authority is selected.

## Internal slices and proof

Audit Jack's actual configuration (config-audit.md) and preserve intended values.
Do not rewrite account order, native model defaults or unrelated repo changes.
Original model-cost optimization remains evidence-driven: show actual selections
and known costs/usage where available; do not invent benchmark-backed assignments.

Stand-in providers must prove:

- Conflicting adjacent source layers, including CLI vs env and DB global vs repo Wave.
- General/skill/graph-step targeting through YAML and DB for agent and non-agent
  settings; other eligible settings use the same tested mechanism.
- Session Claude -> Task implement Codex, including resume and newly created Tasks.
- Explicit Graph Codex across nested steps/routers/loops; child Task creation does
  not persist it as future policy.
- Repeated skills, stable IDs and composed graph origin; false/empty/unset and
  intentional collection merging; inspected effective values match actual launches.
- Existing routing, remote launch, native metadata, history and migrations survive.

The existing checks cover distinct boundaries: resolver units prove precedence and
collection semantics; `wave_setup_tests::setting_targets_inspect_and_unset_through_the_cli`
proves persisted targeting and inspection. `settings_launch_tests` now covers the
previously missing exported-command parity and Session/resume-to-Task cases,
detailed below. Native launch/resume fixtures cover fresh settings with preserved
provider history. Together these establish focused source behavior, not broader
gate, installed acceptance or recovery. No paid-provider test is required.

## Second PR: recovery

Retain both original outcomes: interactive Sessions avoid separate manual limit
repairs; headless steps continue on eligible accounts/models with work and history,
and wait/report reset when none can continue. See limit-failures.md and
weekly-limit-2026-11-02.md. Retry and routing share one decision; never bind an old
provider thread to an account that did not run it. Exact fallback/wait/continuity
mechanisms need the second design. Do not delete retry invariants or symlink native
transcripts based on the superseded Route draft. This PR leaves recovery open.

## Implementation inventory — 2026-11-02

| Canonical preference | Type / merge | Sources and targets / owning consumer |
| --- | --- | --- |
| agent | harness[:model], replace | all precedence layers; general, skill, authored occurrence; prompt/Session launch |
| yolo, chrome, diff, diff_files, paste | bool, explicit false replaces | all layers and launch targets; permission/browser/context preparation |
| docs, context, exclude | string list, additive; explicit [] clears | all layers and launch targets; document preparation |
| summaries; summary_tokens | summary records; unsigned integer | Existing schema has no runtime consumer outside config tests. Preserve parsing/merging; activation would require a separately specified summarization operation, not an implicit paid launch. |
| context_budgets | BudgetKey → positive integer, merge by key | all general layers; authored memory/scratch targets, not per-turn instructions; context accounting; existing defaults retained |
| isolate | bool, replace | launch preference; native resume retains recorded home identity |
| graph | graph name, replace | general global/repository/Wave preferences; Task takes its graph up once, never rewrites an existing graph |
| supported_harnesses | string list, additive | general; selection UI catalog; not a per-step instruction |
| automation.retries | unsigned integer | general; retry operation policy; not an instruction to a skill |
| planning | tagged provider config; replace as a unit | repository transport/identity; no skill/occurrence target; team association remains repository-owned |
| release, land, merge_method, pr | typed release map, strategy, enum, bool | repository delivery operations; no skill/occurrence target |
| session.terminal, session.end_detached_after | terminal name, duration/off | personal general; terminal presentation and detached lifetime |
| user.name | string; empty uses Git | personal general and LF_USER_NAME; participant display, never ownership |
| repo_root | absolute or home-relative path | personal general; local repository lookup |
| theme | Ghostty theme selection string | personal file and Appearance UI; AppearanceStore synchronously loads, previews and watches these files independently of Work/store availability; Apply/Cancel owns persistence, so no database or agent-step target |
| stream | stable/preview/dev | installation command and personal file; selects installation before the selected store can be opened |
| portfolio | repository path selection | portfolio add/remove/reset operations and personal file; a registered navigation collection, not a launch preference |
| logs.linear_requests | bool, default false | Machine config; diagnostic opt-in must work before store/schema access |
| machine.tasks, machine.session | registered Machine/capacity reference | existing set/unset and Machine APIs, repository/Wave DB scopes; registration and Session movement are effects, not scalar writes |
| account.claude, account.codex | ordered registered account IDs, replace | existing set/unset route API + explicit CLI/env selection; preserves provider_routes, validation, ordering, and recorded-account resume |
| verbose, interactive/batch, account restriction, cwd/repo/task/wave, forward-agent | invocation flags/selectors | invocation/control inputs; not inherited work preferences |
| LF_HOME, LF_BIN, provider homes and credentials | execution/bootstrap context | Machine/provider ownership; not scalar settings; diagnostics never dump the environment |
| skill action_style, native metadata; Graph topology, interactive, returns | authored behavior | definition files only; personal overrides select execution preferences without rewriting instructions/topology |
| Desktop autosave/history filter/render diagnostics | local view preferences | UserDefaults; no repository, Wave, or agent-step meaning |

## Delete — do not maintain

- Removed `config.rs::validate_wave_keys` and the separate repository presentation
  key check. The shared resolver validates file and database scopes; Wave files
  still reject unknown keys and accept the released `workflow` alias. Diagnostics
  retain the source path. Effective Config values now derive once from the final
  provenance map, avoiding parallel merging state. Skill loading parses `docs`
  once and preserves explicit empty-list resets.
- Removed `commands/config.rs::inspect`'s compiled-local-ID filter. Targeted reads
  now match the loaded definition's canonical name and `OccurrenceSettings.id`;
  whole-Graph traversal still includes repeated children and routers. The CLI
  composition regression preserves coverage on this single inspection path.
- Removed `graph/instructions.rs::headless_command`'s authored-agent-to-`-a`
  promotion, direct-skill export and separate command-collision resolver. Captured
  `SkillInvocation` commands explicitly select `skill --`; conversational steps,
  native controls and exact shell-safe names remain.
- Removed the unused `TaskProcessOptions.agent` and its construction sites; Task
  placement carries no execution preference. Graph execution retains the CLI.
- Removed unused `Cli::{chrome_setting,diff_files_setting,diff_setting}` readers.
  `setting_values` and the shared resolver own these preferences. No exclusive
  tests or fixtures remain for these removed paths; no deletion target is open.
- No new settings authority or migration is required. Sync owns the captures in
  each generated bundle; content-addressed files are listed in its frontmatter,
  refreshed before recipe publication and pruned only while their contents match.
  A recipe captures a named skill reference plus occurrence metadata. It must load
  the skill in the launch repository: a global builtin recipe cannot freeze out a
  repository's skill override. Runtime Graph captures retain their exact bodies;
  both paths use `SkillInvocation` and the shared settings resolver.

## Remaining first-PR work — 2026-11-02

The supplied iteration feedback predates the current export and stand-in launch
changes. Source inspection at `d7561a4512` confirms those implementation gaps are
closed: `headless_command` uses `--skill-input` without promoting authored agents
to CLI authority; the captures retain occurrence values, source, Graph and ID.
The launch tests exercise the requested precedence, non-agent and Session/Task
cases. No further product decision or bounded code repair emerged in reconciliation.

The subsequent concept review below found one bounded inspection correction,
now implemented with regression coverage for canonical aliases, repeated
composition, conflicting parent/child preferences and child-only IDs.
Graph review and gate own the affected architecture, Rust, website and Swift
suites, followed by the concrete settings demo and publication for Jack's review.
Auto-Ship remains stopped; no merge or Task completion is authorized. Recovery
remains the second PR, with its exact mechanisms still unselected.

## Concept review — 2026-11-02

Outcome: **design clarified; inspection correction implemented**. Jack Heart's accepted
defining-Graph/local-ID target must mean the same thing in inspection and writes.
No new product choice is needed for this first-PR correction.

The normal interaction remains: set a personal preference for `outer/work`,
inspect that target, launch it, and unset the preference to expose the next
layer. A Session's explicit agent remains conversation-local; an explicit Graph
agent reaches that execution's steps. Source precedes specificity, collections
retain their declared merging rules, and history never becomes Task policy.

Counterexample: `outer` composes `inner` twice and declares its own `work` step;
`inner` also declares `work`. Personal settings select `claude:inner` for
`inner/work` and `codex:outer` for `outer/work`. The branch binary's
`config show --graph outer --step work --json` returns all three occurrences,
although `graphs.outer.steps.work.agent` changes only the outer occurrence.
`commands/config.rs::inspect` filters the flattened steps by local ID alone.
The resolver already retains and matches both parts of `OccurrenceSettings`.
This creates two meanings for the target precisely when composition is reused.

Proposed usage clarification for `docs/config.md`:

```sh
lf config show --graph outer --json              # every composed agent step
lf config show --graph outer --step work --json  # only authored outer/work
lf config show --graph inner --step work --json  # authored inner/work
```

With `--step`, inspection must select by the loaded definition's canonical name
and the original authored ID in `OccurrenceSettings`; a child's same-named ID
must not satisfy a missing parent target. Preserve whole-Graph inspection of
composed steps and routers when `--step` is absent. Do not add an instance selector,
new persisted identity, or alternate resolver. CLI and Desktop's existing
`Read::Settings` share this operation, so the correction belongs there. Existing
skill guidance needs no change.

Failure and recovery: a mismatched target must report no matching step; inspect
the defining child Graph to find its value. Unset continues to reveal the next
source without changing shared definitions. Preserve occurrence origin through
capture, repeated composition and loops, and preserve Session/provider history.
Automatic limit fallback and reset waiting remain the separate second PR; this
inspection correction adds no retry, account transfer or Task-completion policy.

The `wave_setup_tests` regression writes distinct parent/child preferences through
`lf set`, compares targeted reads with their whole-Graph occurrences, checks both
canonical and alias Graph names, and rejects a child-only ID on the parent.
Repeated interactive steps exercise original IDs after navigation renaming; a
composed XOR path preserves router and child inspection. `docs/config.md` explains
the target and missing-parent behavior. Existing launch parity tests continue to
own provider selection; gate and demo remain pending.

## Export and launch proof

Native recipes now invoke `lf -b --skill-input <capture> skill -- <name>`.
Each capture retains a named skill reference and its authored occurrence. Loading
in the launch repository preserves repository overrides of global recipes and
current skill defaults. Already resolved runtime captures retain their original
body and arguments. Command messages become the named recipe invocation's
arguments, while runtime correction direction remains separate.

Sync atomically writes content-addressed captures before their recipe, lists them
in generated frontmatter, repairs missing captures and interrupted first exports,
and prunes replaced/removed captures only when their contents still match. Foreign
assets and replaced captures remain untouched. Conversational steps stay in their
current Session and retain native controls. No second settings resolver was added.

`settings_launch_tests` now correlates inspection with stand-in provider arguments
and delivered documents for repeated/composed occurrences, same-ID unrelated
occurrences, changed skill defaults, personal YAML, global/repository/Wave DB
settings, unset, CLI/environment precedence, explicit false, empty collections and
additive document merging. Both runtime Graphs and executed exported commands
participate. An explicit Graph agent reaches routers and a repeated loop pass.
A native Claude Session and its model-switched resume each run an existing local
Task and create/run another Task: all four implement workers select Codex.

Claude's documented headless permission floor remains independent of `yolo`;
`yolo: false` removes explicit bypass preference, not that floor. Terminal yolo
negation remains covered by the existing native launch/resume checks. The launch
matrix proves headless false/empty behavior with chrome and document settings.

Implementation decisions (2026-11-02): repeated composed interactive stops get
unique captured navigation names (`review#1`, `review#2`); preference targeting
retains the defining Graph and original ID. Retained production Task graphs
normalize old `default_agent` to `agent` on deserialization, without preserving
a second selection tier in new captures. Context budgets remain general authoring
targets; they do not select a turn's content. Planning replaces as a tagged unit
to avoid carrying one provider's fields into another provider. File and database
settings share scope validation: Machine presentation stays global, and repository
planning/delivery settings cannot be scoped to a Wave.

Compression carries candidate blocks in `SettingCandidate` through sorting and
leaf resolution rather than a parallel six-field tuple. Source layers remain
intact. Prompt preparation consumes one resolved configuration;
Wave setup loads its layers once and resolves each distinct skill once. Occurrence
capture copies only preference values, avoiding serialization of skill bodies and
source frontmatter. Already captured occurrences do not reread their source files.
Bare references keep loaded skill defaults at their own source; they never become
authored occurrence overrides simply because the Graph used shorthand.
Native terminal commands carry resolved isolation through bootstrap and client
routing; resolving it again in an account worker loses thread-local CLI settings.
Native exit settlement now also carries the launch's isolation choice when
pinning the account home; the upstream merge had left that caller incomplete.

The selection/child-launch proof moved from the library fixture module to the
`settings_launch_tests` integration module, using Cargo’s fixture binary.
The replaced assertion was removed; the remaining library fixtures retain their coverage.

Upstream reconciliation: fetched `origin/main` remains `ece53eec1b`, already merged
in `d6d7057d97`. The Session screen/mode restoration, owner/viewer reduction and
installed-entry resume changes are present. The native exit caller repair above
completes their integration with the settings authority change.
Infrastructure and its immediate Release child's goal/memory were read. Release's
entry-point lesson applies directly: runtime Graph proof cannot establish exported
recipe behavior. Broader live/installed recovery is not established by either.

Check result: with inherited `LF_*` unset and null stdin, `cargo test -p loopflow --lib config::` (45), `cargo test -p loopflow --lib skills::catalog::` (8), `cargo test -p loopflow --test integration settings_launch_tests::runtime_and_exported_occurrences_resolve_live_preferences_at_the_same_authority` (1), and `cargo test -p loopflow --test integration wave_setup_tests::setting_targets_inspect` (2) PASS; `cargo fmt`, `git diff --check`, and `lf context` PASS. The test linker warns about its oversized unwind table. Prior launch/Session evidence remains applicable outside these reductions. Gate owns broader suites and Clippy; demo and review-only publication remain pending. Auto-Ship remains stopped per the prior `lf desktop task LOO-714 --json` read.
