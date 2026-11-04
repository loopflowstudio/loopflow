# File-backed settings, one catalogue, and remote preference authority

LOO-714 · Accepted for implementation by Jack Heart · November 3, 2026

Jack's “ok. lets build.” authorizes this settings extension in the existing Task
and checkout. This is the current implementation plan. It supersedes the DB-backed
storage and catalogue/portability exclusions in settings-scope.md; preserve that
file's accepted choice/default, targeting and invocation semantics and its evidence.
The preceding research was checkpointed before consolidation in Git. Research notes
remain evidence, not competing designs.

Delivery: continue PR #1805 for review with Auto-Ship stopped. Automatic limit
recovery remains the later PR; neither merge nor whole-Task completion is authorized.

## Documentation-first direction — November 3

Jack Heart requested rewriting configuration documentation first, then making code
match it. docs/config.md is the user contract and docs/architecture/settings.md is
the internal ownership contract. These replace historical DB-backed explanations;
the checked architecture inventory remains current-source evidence until cutover.
The user explicitly emphasized conceptual cleanup, not just behavior parity.

Separate ConfigSource (Builtin, Repository, Personal, Environment, CommandLine),
SettingScope (Global, Repository, Wave) and work target. Remove the old ConfigLayer
Global/Personal file-versus-DB split and Repo/Wave source split, including DTOs,
serializer labels, rank tables, compatibility aliases and inspector vocabulary.
Use one catalogue, resolver, writer and explanation across interfaces. Demonstrate
that the documented examples agree with real launch preparation before publishing.

## Repository scope — resolved November 3

Jack Heart explicitly chose private preferences per independent clone, including
account order, with linked Task worktrees sharing one file. This is an intentional
scope change from legacy origin-wide DB account routes. No origin-wide runtime
preference file or DB fallback is required. docs/config.md and
docs/architecture/settings.md now specify this behavior.

Migration materializes the legacy policy into explicitly identified existing clones
with collision handling; these become independent policies. Preserve absent or
ambiguous destinations as recovery exports until explicitly imported. Future clones
do not automatically acquire old private settings. An arbitrary registered checkout
cannot own every clone's policy, and registration cannot gate normal config access.
The old requirement to preserve origin-wide routing scope is superseded by this
explicit decision; preserve values, ordering and recovery evidence instead.

The superseded prototype archive and research are documented at
`a7a64269c6:scratch/settings-catalog.md`; current source implements the accepted
clone scope and is authoritative over that unfinished prototype.

## Prior implementation evidence

The file-backed catalogue, independent source/scope/target model, file migration,
configuration observation and destination repair stand. Detailed reconciliation
and earlier check references are preserved in Git blob
`d02b6b6595a90788f21de8c7e81b0b6ed1427c91`. The conflict-free sync merged main
`ac8e088f5f` in `ff8eb9a7f6`; preserve LOO-720's native/headless distinction.
Earlier passes are evidence for their named trees, not acceptance of this repair.

## Outcome and scope

Every LF-owned preference has one typed catalogue definition, consistent meaningful
access through CLI/files/Desktop, one authoritative file writer and an explanation
of its effective value and source. Close coverage gaps rather than merely documenting
them. Remove DB preference overrides, including machine/account preference declarations.
Origin preferences govern remote execution and can be retained for later direct use.

Existing operational state remains in its domain owner: Tasks, Sessions, actual
saved layouts, local machine identity, account observations, usage, cooldowns and
execution history. Layout templates and Wave layout defaults are configuration;
applying one changes live state. This is a classification requirement for existing
surfaces, not authorization to build a new layout editor or template product.

## Files and editing

- Personal global: ~/.lf/config.yaml.
- Personal repository: ./lf.config.local.yaml, at the repository root. This exact
  filename was selected by Jack; it is not .lf/config.local.yaml.
- Committed team: .lf/config.yaml and existing Wave configuration files.
- Skill/Graph metadata remains an authored source where the setting applies.

Private repository settings are shared across linked Task worktrees. Implementation
choice: discover the main checkout through Git's worktree/common-directory APIs and
resolve one root lf.config.local.yaml there; taskless non-Git use resolves its own
project root. Do not copy diverging per-Task preference files. Handle relocated/main
worktrees through Git metadata rather than guessing paths or branch names.

Implementation choice: on first LF write, add /lf.config.local.yaml to the common
Git info/exclude if needed. Keep existing rules and avoid changing global Git ignores
or the committed .gitignore. Do not silently untrack an already tracked private file;
report that condition. Reads should diagnose an existing unignored private file.

CLI and Desktop edits target these same files, with explicit write destinations.
Ordinary unqualified repo-scoped setters use the private repository file; global
setters use the personal global file; shared writes require an explicit destination.
Preserve comments/unrelated keys, validate before writing, use atomic writes and
protect against concurrent changes. Unset removes a declaration, not its lower-layer
value. Hand edits and programmatic edits must be visible through the same invalidation
and inspection path. Preserve existing appearance Apply/Cancel semantics.

Migrate existing personal repos: entries to private repository files deliberately;
no second writable personal repository authority remains after cutover. Wave-scoped
personal declarations live in the private repository file. Use one canonical syntax
for general/skill/graph-occurrence declarations wherever they apply, retaining the
existing accepted selectors instead of inventing parallel key names.

## Resolution

For agent selection, all applicable agent choices beat all default_agent fallbacks.
Within a class: CLI > explicit environment > personal files > repository/Wave files
> builtins. Within personal ownership, narrower scope wins: Wave > repo > global;
within a scope, occurrence > skill > general. Removing the DB removes a source tier,
not these distinctions. Other settings retain their typed replace/merge/append/reset
semantics. No per-skill accounts and no built-in per-skill agent recommendations.

Preserve examples already implemented:

- Personal skill agent Codex beats repository occurrence agent Claude.
- Personal skill default_agent Codex yields to repository general agent Claude.
- Session/resume -a selects only that conversation; unrelated Tasks and agent-issued
  child commands do not inherit an internally exported preference.
- Explicit Graph/Task execution -a reaches all its steps, composition and routers.
- Occurrence targets use defining Graph canonical name plus authored local step ID.
- False, empty, omitted and unset remain distinct.

## One executable catalogue

Rust is authoritative. Co-locate preference types/defaults with descriptors or derive
policy from one declarative source; do not maintain parallel Swift/help/schema lists.
A definition contains typed codec/validation, default, applicability over source ×
scope × target, combination policy, persistence/lifecycle, portability and description.
Dynamic keyed settings use typed selector patterns. Planning remains a tagged unit.
Internal bookkeeping and native provider configuration are not generic LF settings.

Keep the existing resolver, making it consume catalogue policy. Definitions own
eligibility and validation; declarations carry address/value/source locator; resolution
returns values, candidate contributions and reasons. CLI flags bind to definitions.
Generic setters use the common writer; domain actions such as login/provisioning or
moving a Session remain explicit operations. Their preference fields still get normal
CLI/file/UI access where meaningful.

Catalogue discovery explains where each setting can be set. Inspection explains why
it wins, contributes, clears, falls back or is inapplicable, with actual file path and
scope/target. Inspection and launch preparation must agree. Unknown LF keys are
diagnosed after dispatch to known owners; native skill frontmatter is not misclassified.
Bootstrap reads and config inspection work without a compatible DB. Resource availability
may depend on runtime facts, but this is distinct from preference availability.

Inventory all existing fields, including theme, stream, logs, portfolio, launch/context,
planning/release and machine/account fields. Dormant settings must be identified as
inactive; invent no runtime behavior to make a parser field appear supported.

## Machine and account declarations

Files own ordered allowed accounts, preferred machines, declared aliases, identity
references, connection declarations and authored limits. Domain owners retain secrets,
observed identity/auth state, availability and usage. Audit mixed records field by field.
Credentials keep the existing secret mechanism and never enter shareable configuration.

A machine alias, durable referenced identity, connection address and this installation's
own local identity are distinct. Only the last is nonportable runtime identity. Resolve
placement at origin to a durable ID. Selecting the current ID executes locally; assigned
work does not select placement again. Keep host routes/bindings that cannot travel in
host-local configuration or existing SSH configuration, described by catalogue policy.

## Remote authority — accepted

The initiating LF instance supplies the personal profile; the executor supplies its
environment. Forwarded work uses the origin's portable personal declarations instead
of implicitly merging origin and destination personal profiles.

| Launch entry | Personal preferences |
| --- | --- |
| Laptop launches on remote | Laptop profile |
| Phone goes through laptop server | Laptop profile |
| Shell or phone launches directly on remote | Remote's saved profile, including retained preferences |

Keep original source/scope/target/choice strength. Shipping declarations never promotes
them to CLI overrides. Host-local fields and credentials come from the destination;
repository/Wave configuration remains its authored source, not copied personal intent.
CLI and explicit environment retain invocation semantics; no persistence of those values.
Existing account/thread continuity rules remain enforced even when a new preference
cannot be applied to a resumed provider conversation.

Use and retention are distinct. A remote request can use the carried profile even when
saving it would overwrite an independent remote edit. Later direct remote commands use
the actual saved profile. Inspection must explain the difference and the source.

## Remote retention — conservative implementation choices

The desired behavior is accepted; the following mechanics resolve the remaining routine
choices for implementation. No multi-writer auto-merge, universal Home or background
synchronization service is introduced.

- Retain portable authored declarations in ordinary personal config files, preserving
  machine-local fields. Initial absent/equal destinations can be adopted automatically.
  Existing differing personal preferences are preserved and exposed for explicit adoption.
- Record last-applied portable content/revision, origin and destination identity as
  operational metadata. It can live in the DB; it is not another preference source.
- Refresh on remote request boundaries only while retained content still matches the
  last-applied content, or already equals the incoming content. Remote deletions count
  as changes. Compare coherent documents/projections, not auto-merged leaf values.
- Serialize compare-and-write; reject stale deliveries from the same origin. Another
  client cannot silently take ownership of an independently retained profile. Repository
  identity selects the correct private file across differing filesystem paths.
- On divergence retain remote edits, expose an inspectable diff and explicit keep/replace
  or import action. Forwarded work can continue with its carried profile; report that
  persistence was not applied. Repeated identical divergence must not repeatedly prompt.
- Offline direct work uses last saved preferences. It cannot observe undelivered edits.
  Existing running steps are unaffected by delivery. No automatic reverse propagation.

Catalogue portability must distinguish authored paths/connection bindings from portable
references. Account ordering is atomic, not a union. A snapshot includes removals within
its declared coverage; partial delivery must not look like deletion. Delivery receipts
and metadata never become a resolved-settings DB override or a second writable mirror.

## Migration and deletion cut

Reuse this Task's single unreleased settings_authority.sql migration; check its release
status before editing. Database upgrades and file writes need a recoverable cutover:
preserve authored values, scopes/targets/strength and account order, not merely an
effective snapshot. Inventory collisions with existing files before retiring the only
copy. Preserve rollback/recovery evidence, including values displaced by explicit adoption.

Audit the effect of collapsing DB and file authority: an old DB-global override could
previously beat a more specific file declaration. Do not silently change that behavior
or pretend a same-address export preserves it. The migration must make the affected
choices reviewable and preserve intended effective behavior or require explicit resolution.
No permanent compatibility reader or dual-writer DB preference layer remains afterward.
Keep released and branch-readable shared config compatible until supported cutover, per
AGENTS.md; any necessary temporary compatibility has a named removal outcome.

### Delete — do not maintain

The completed independent capture reduction removed `graph::StoredSkill` and the duplicate
`Skill.settings["docs"]` representation. `Skill.docs: Option<Vec<String>>` keeps
omission, empty-list reset and authored paths on one surviving field, including
retained JSON captures. Export ownership checks live in `skills::writable_target`;
the caller no longer repeats them. The separate file migration now implements the
accepted independent-clone scope.

The prior deletion cut removed DB preference readers/writers, parallel key lists,
serialized-default schema inference, duplicate parsers/writers and client schemas,
inspector winner guesses, numeric target ranks and the duplicate inactive flag.
Domain readers now select typed Config fields directly; account SQL retains runtime
columns only. Migration and retention share the authored-document overlay. Domain
setters delegate to `files::edit`, including private-file protection. Literal map
keys survive editing/resolution; dotted labels are display text. Config-file
subscriptions replace tests of retired provider_routes revisions. Session aliases
come from configuration without obsolete machine-table joins.

The complete preceding reconciliation, including removed symbols/test names, is
preserved in Git blob `e36ba458f227ee3b04f15e567588328d464c107c`.

## Configuration observation

CLI/FFI/Desktop subscriptions share watch::Reader's one-second content observation
of config inputs, missing paths and definition directories. Bounded Task workers
inherit capture. Hand edits, replacement/removal and invalid-YAML repair retain
coalescing and unchanged-frame suppression; planning refresh is separate. The
five-second fixture at `a7a64269c6` is not a latency guarantee or configured Desktop
acceptance. Full prior evidence: `d02b6b6595a90788f21de8c7e81b0b6ed1427c91`.

## Destination repair and acceptance boundaries

### Destination ownership — preserved

The destination repair at `aa4f3c75ee` binds CLI/Desktop edit and explanation to
one file/source/scope/prefix. Its focused Rust, CLI, FFI, Swift and lint results
remain evidence for that unchanged path. Detailed repair and review evidence,
including the uncommitted reconciliation, is preserved in Git blob
`d02b6b6595a90788f21de8c7e81b0b6ed1427c91` (this plan).

Remaining: gate's affected suites,
review's configured demonstration, then review-only publication. Limit recovery
remains PR2; neither merge nor Task completion is authorized.

Headless acceptance covers:

- Real file/set/unset/CLI/Desktop paths resolve identically, including false/empty values,
  targeted defaults, explanations and invalid settings.
- Linked worktrees share the exact root private file; it is ignored; concurrent writes
  and hand edits retain unrelated content.
- Bootstrap/config discovery survives DB absence/version skew; runtime unavailability is
  reported separately. DTO fixtures agree across Rust and Swift.
- Migration fixtures cover collisions, scope/strength/order, interrupted file cutover and
  removals, plus Wave rename/relocation and old-name reuse. No obsolete DB preference
  can influence later launches.
- Stand-in remote launches prove origin wins, direct remote uses retained settings,
  repository precedence remains, local-ID placement executes locally, and nested Graph
  execution does not leak invocation selection into unrelated work.
- Retention proves initial absent/equal/different cases, unchanged updates, local edits
  and deletions, stale/concurrent deliveries, restart, host-only fields, no credential or
  CLI/env persistence, and nonblocking forwarded execution with clear persistence status.

Check: with inherited LF_* unset, `rustup run stable cargo nextest run -p loopflow --lib -E 'test(config::remote::) | test(config::files::) | test(config::settings::) | test(skills::catalog::) | test(graph::tests::)'` — 94 passed, one nextest leak diagnostic on the scoped-thread file-write test (no remaining child); `rustup run stable cargo fmt --check` — passed. Gate owns affected-suite acceptance and leak diagnosis; lint owns current-tree Clippy; review owns configured behavior.

## Concept review — one personal profile across declaration locations

November 3, source reviewed at `aa4f3c75ee`. **Accepted ownership implemented;
focused tests passed. No open product choice.** Jack Heart's accepted origin-owned
profile includes portable LF settings wherever authored. The earlier destination
repair stands. This finding concerns definition metadata, not another edit scope.

The review reproduced a carried general `agent: codex` losing to executor
personal skill `agent: claude`: config replacement left definition metadata
outside profile ownership. Direct executor work correctly chose Claude; a
repository choice correctly beat an origin fallback. Reproduction details and
binary digest remain in Git blob `d02b6b6595a90788f21de8c7e81b0b6ed1427c91`.

Ownership requirements:

- Select the personal profile before collecting portable preference candidates,
  regardless of whether their locator is config YAML, LF skill metadata or a
  personal Graph occurrence. Include applicable origin-authored definition
  declarations with their source, scope, target, locator and choice/fallback key.
  Exclude competing executor portable metadata through that same ownership rule.
  Remove the config-document-only assumption; do not add a new precedence tier.
- Keep definition instructions/topology and preference authority distinct. Preserve
  repository definition selection, native provider-owned fields, host-local settings,
  defining Graph/step identity and captured instructions. This does not authorize
  synchronizing whole skill bundles or changing how skill bodies are selected.
- Use the same declaration ownership in inspection and real preparation, including
  captured/composed/repeated occurrences. Preserve existing retention conflicts,
  deletions, stale-delivery protection and use-versus-save reporting when retaining
  these authored preferences. Receipts remain operational evidence, never policy.

Proof: forwarded general choice plus executor personal skill choice selects the
origin; origin personal skill metadata itself survives transport; an origin fallback
still yields to a repository choice. Repeat for personal Graph metadata and a
non-agent portable setting, with inspection agreeing with preparation. Direct
executor work keeps its own policy. On a retention conflict, forwarded work still
uses the origin, the executor file remains untouched, and later direct work uses
that file; explicit adoption must not promote a fallback to a choice. Missing
referenced assets retain their existing actionable failure rather than silently
substituting executor policy.

These counterexamples reject both a blanket removal of definition metadata and
promotion of the carried profile to CLI authority. Gate, configured acceptance
and review-only publication remain; recovery is PR2.

### Current deletion cut

Definition candidate construction and profile selection now belong to
`config::definitions`; the resolver/transport mutation handoff is removed.
Metadata normalization, occurrence address lookup and recursive YAML patching
have one implementation each. Retention matches definition identities through
one keyed lookup. Destination ownership and the profile/retention contract stand.

Removed config-only profile collection and unconditional personal definition
candidate insertion. One resolver and one retention compare/write policy remain;
project definition preferences separately from instruction bodies/topology.
Preference retention edits existing definitions, never installs a skill bundle.
Graph metadata keeps defining graph, authored ID (or local position for unnamed
metadata), locator and choice/fallback key through capture and composition.

The profile now carries portable personal definition declarations with their
locators. The resolver excludes executor portable metadata before ranking;
captured work uses current executor host-local metadata. Skill captures record
source authority; Graph captures retain defining name, authored address and path.
Repository Graph selection stays first; personal Graph discovery supplies the
previously absent fallback before builtins. Retention shares compare/write and
receipt policy across config and definition projections, editing only preferences
through the existing comment-preserving writer. No body or topology is copied.

Focused boundary: config::remote, config::files, config::settings, graph::tests;
direct consumers also include prompt::process, skills::catalog/invocation,
definition, config::api and settings_launch_tests. Gate owns their full acceptance.
