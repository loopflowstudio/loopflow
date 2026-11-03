# Consistent settings and personal graph-step overrides — LOO-714

Jack Heart approved this direction and requested implementation on 2026-11-02
(“lg2m”, “lets build it”). Placement: Infrastructure. This is the authoritative
first-PR design, superseding storage-model.md, route-design.md and the selection
questions in design-questions.md. Preserve the separate recovery outcome below.

## Current pursue contract — approved 2026-11-03

Jack Heart approved applicable agent choices before all default_agent fallbacks,
then source > scope > target within each class. Preserve explicit Graph forcing,
Session-only selection, native metadata/history and existing non-agent semantics.
The five builtin recommendations now live in this repository's configuration.

Finish this revision and PR #1805's known CI repairs, then publish for review through
the authored Graph. Use Codex; keep Auto-Ship stopped. No merge or Task completion;
limit recovery stays PR2. Preserve the separate critique and portability review;
no per-skill accounts or settings synchronization is authorized here.

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

Agent selection revised by Jack Heart on 2026-11-03: an applicable explicit
`agent` choice beats every `default_agent` fallback, regardless of source or
target specificity. Only when no explicit choice applies are defaults considered.
Within either class, retain source order: explicit CLI > explicit environment >
lfdb > personal config > repo/Wave config > built-ins. Within a source, Wave >
repo > global; within one scope, graph-step > skill > general. Builtin skills
declare no agent recommendations. The application's general fallback is a default.

Thus personal implement `agent: codex` beats repository occurrence `agent: claude`,
but personal implement `default_agent: codex` loses even to repository general
`agent: claude`. This supersedes the proposed target-first alternative. Preserve
source, scope, target and choice/default identity until resolution. The resolver retains both authored keys through selection; inspection's effective
agent lists fallback candidates before explicit choices. Non-agent settings retain
their existing resolution until their default semantics are explicitly designed.

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

## Existing implementation

The shared resolver retains source layers through prompt preparation, config
inspection and Wave setup. InvocationSettings owns CLI intent; captured
SkillInvocation retains Graph origin. One settings_authority.sql migration removes
Task agent policy and converts legacy skill preference keys. History and account
routes remain. Released config compatibility retains the workflow alias (LOO-637).

## Acceptance boundaries

config-audit.md records Jack Heart's nonsecret settings; preserve native model
defaults, account order and unrelated contributions. No cost/quality optimization
is established. Resolver tests prove source/scope/target and collection semantics;
wave_setup_tests proves DB set/inspect/unset and defining-Graph target identity.
settings_launch_tests proves runtime/exported recipes, repeated composition,
routers/loops, Session/resume-to-Task isolation, false/empty and launch parity.
Native resume tests preserve provider history. Stand-ins need no paid provider.
Gate owns affected suites; installed acceptance and recovery remain distinct.

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
| agent, default_agent | harness[:model], choice before fallback | all precedence layers; general, skill, authored occurrence; prompt/Session launch |
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
- Removed `ProcessPromptInput::{agent,yolo_mode}`, the prompt-only `resolve_agent`
  wrapper and the unused Task/conversation override parameters. InvocationSettings
  and the shared resolver own launch selection; the existing provider-policy tests
  now exercise that path. Settings owns agent validation. Resume reads its winner
  once; CLI booleans share one conversion. No exclusive fixtures remain.
- No new settings authority or migration is required. Sync owns the captures in
  each generated bundle; content-addressed files are listed in its frontmatter,
  refreshed before recipe publication and pruned only while their contents match.
  A recipe captures a named skill reference plus occurrence metadata. It must load
  the skill in the launch repository: a global builtin recipe cannot freeze out a
  repository's skill override. Runtime Graph captures retain their exact bodies;
  both paths use `SkillInvocation` and the shared settings resolver.

## Remaining delivery — November 3

Choice-before-fallback is implemented, including resume: a fallback refreshes a
same-provider model without requesting a cross-provider import. The eight known
CI failures passed locally before compression; the saved evidence below retains
those commands. Linear connection keeps repository identity while personal
transport retains precedence. No migration was added.

Gate owns affected suites and Clippy; the authored Graph owns review-only
publication. Auto-Ship is stopped (`ship_approval` null, `ship_stop` present).
No merge or Task completion; recovery remains PR2. Preserve the separate
portability review and critique.

Historical design/review evidence is retained in local Git blob
`6ae574bd76b7b34afb61f96249f910492da8ef3d`; use `git show` with that hash.

Check: `cargo nextest run -p loopflow --lib -E 'test(config::settings::tests::) | test(prompt::process::tests::)'` — 37 passed; settings-launch/resume matrix — 18 passed; `cargo fmt`, `git diff --check`, `lf context` passed. One earlier nextest output-handle leak did not recur; no surviving child from this checkout was found. Gate owns Clippy/broader checks. Prior implementation commands/design: Git blob `a9f13d18bf8f32b2012fb346bcbb269f1978f13c`.
