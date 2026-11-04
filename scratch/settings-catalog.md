# One settings catalogue

Draft for Jack Heart · LOO-714 / infrastructure · 2026-11-03

Build one executable definition of every LF-owned setting, so its accepted values,
supported locations, editing path and explanation agree across CLI, files and
Desktop. This develops the existing settings-portability-review.md audit. It is a
design for review, separate from the running choice-before-fallback implementation.
No catalogue implementation or synchronization is approved by this draft.

Jack Heart clarified the design goal on November 3: **close the coverage gaps**,
not merely catalogue today's exceptions. For every supported setting, implement
consistent access through appropriate surfaces or document a real domain/lifecycle
constraint. Existing missing support is not its own justification.

Accepted design direction, confirmed by Jack Heart on November 3: remove the DB configuration
layer, including machine/account preferences. CLI and Desktop edit the same files
that hand edits change, with explicit write destinations and source inspection.
The earlier tentative DB-only meaning of `lf set` is superseded. Machine aliases,
identity references and connection declarations live in files; each installation's
own identity remains local durable state. Placement resolves to an ID at origin;
matching the current ID means local execution. Distribution is still open.
This accepts the design direction; implementation/migration planning remains separate
from the active pursue. Paseo research in `paseo-settings-reference.md` and Git inform
the editing contract; this does not propose using Git's config parser or storage
format for LF.

Git's `config set` writes a selected file, local settings are private to a clone,
and conditional includes select personal policies by path or remote URL. LF still
needs private repo storage that behaves correctly across Task worktrees and
machines, and a distinct committed team layer. Git's local config is not equivalent
to LF's committed `.lf/config.yaml`.
Primary reference: https://git-scm.com/docs/git-config (FILES, OPTIONS and
Conditional includes), inspected November 3, 2026.

Jack's request: “designing a single settings catalog and explaining and clarifying
what exists today versus what would need to change.” Preserve the accepted agent
rule: choice before fallback, then source > scope > target. No built-in skill
recommendations or per-skill accounts.

## Current system → proposed system

“Current” means the working branch, including the active revision, not the installed
release. The revision now carries `default_agent` and authored candidate keys;
its full validation is ongoing. These observations do not claim shipped behavior.

| Concern | Exists today | Change |
| --- | --- | --- |
| Types and defaults | `Config`, nested Rust types, serde annotations and `Default` implementations | Attach setting policy to these definitions; derive catalogue/schema from the same declarations. |
| Eligible locations | `LAUNCH_KEYS`, `REPO_ONLY_KEYS`, `validate_scope`, special path handling | One applicability policy per definition, used by every input surface. |
| Value validation | CLI/DB share `parse_value`; files/metadata use related validation, partly by serializing `Config::default()` | Parse through the definition's existing typed codec and semantic validator, without using a default value as a schema. |
| Combining values | One resolver; additive-key list and agent-specific strength handling | Keep the resolver; select merge/resolution behavior from definitions. |
| CLI convenience flags | `Cli::setting_values` translates flags into preference values | Keep flags; register their bindings to the same definitions, including multi-key flags such as `--diff`. |
| Set/unset | Generic DB preferences plus separate account/machine parsing and dispatch | Validate through the catalogue and edit the selected file. Keep registration, login and movement as explicit domain operations. |
| Inspection | Runtime and `config show` share resolution; candidates include source, scope, target, value and authored key | Add catalogue discovery and structured explanations; show contributions, clearing, fallbacks and unavailable sources. |
| Special settings | Theme, stream, portfolio and logs have other readers/writers; account/machine choices have domain stores | Put declared preferences behind the common file writer, including account order and machine selection. Keep runtime facts and domain operations separately owned. |
| Portability | No common setting-level classification | Describe portable declarations and machine dependencies; implementing sync remains later work. |

Evidence: [Config](../rust/loopflow/src/config.rs),
[resolver and validation](../rust/loopflow/src/config/settings.rs),
[CLI bindings](../rust/loopflow/src/lf/mod.rs),
[writers](../rust/loopflow/src/lf/commands/setting.rs),
[inspector](../rust/loopflow/src/lf/commands/config.rs),
[Wave projection](../rust/loopflow/src/work/wave/setup.rs),
[appearance owner](../swift/LoopflowMac/Design/AppearanceStore.swift).

## What one definition says

A definition describes a setting, not a configured value. Sources continue to
hold declarations; resolution continues to produce values for a particular context.

| Definition field | Example: `docs` | Example: `agent` |
| --- | --- | --- |
| Value and validation | List of path/glob strings | Harness/model string with existing validation |
| Default | Empty list | No authored choice; application fallback is separately identified |
| Applicability | Launch settings at general/skill/occurrence targets | Same targets; LF-owned metadata only |
| Combination | Append; explicit empty list clears prior contributions | Choice/fallback resolution group with `default_agent` |
| Persistence binding | Selected config file; invocation values are not persisted | Same |
| Lifecycle | New context assembly | New launch; resume has an explicit continuity operation |
| Portability/dependencies | Relative repo paths portable; absolute paths need local mapping | Declaration portable; selected harness/model must be available |
| Description | Additional documents included in context | Agent to use for applicable work |

Applicability is a rule over **source × scope × target**, not three independent
checkbox lists whose cross-product accidentally permits invalid combinations.
For example, session presentation is personal-global; account routes allow
personal global/repo/Wave scope but no skill target.

Dynamic paths are declared patterns with typed selectors: `release.targets.<name>`
and its supported fields, or a validated context-budget category. `skills` and
`graphs` are target selectors around a setting, not independent setting definitions.
Planning remains a tagged unit; a generic editor cannot splice together fields
from different planning providers.

## Ownership and data flow

Use Rust as the authority. For core Config fields, a small declarative field list
can emit the typed fields, defaults and descriptors together. Retain existing
domain types/codecs; derive value schemas from them rather than hand-writing a
parallel string/bool/list schema. Settings owned elsewhere register descriptors
bound to their existing typed domain operations. Export the catalogue as a typed
DTO for Desktop/help/editor consumers. No separately maintained Swift key list
or external schema file becomes another authority.

The exact macro spelling is implementation detail. The requirement is that adding
a Config preference also requires its policy definition, while internal fields
such as retained source layers never become public settings. Type validators and
default constructors remain single-owned even when reused by domain code.

Four concepts are sufficient:

- **Definition:** key/pattern, typed codec/default, applicability, resolution policy,
  adapter, lifecycle, portability and description.
- **Address:** setting plus scope and optional skill/graph-occurrence target.
- **Declaration:** address, authored value and source locator. Choice versus fallback
  comes from the declaration key's definition, not a UI guess.
- **Resolution:** effective value, each declaration's contribution/status, comparison
  reason and source diagnostics. This extends current `EffectiveSetting`/candidates.

Core operations, names illustrative:

```text
describe_settings(filter) -> definitions
parse_setting(address, input, source) -> validated declaration
write_setting(declaration) / unset_setting(address, source) -> adapter result
resolve_settings(context, declarations) -> values + explanation
```

The resolver owns both the decision and its explanation. Reasons identify the
actual comparison: explicit choice, higher source, narrower scope, more specific
target, additive contribution or clearing. A reset can clear earlier items while
later declarations append; there need not be one winning list declaration.

Read each source as present, absent or unavailable. Preference resolution no longer
requires the DB. Account/machine availability may still require runtime facts;
inspection distinguishes an unresolved resource from a missing preference.
Bootstrap readers request only sources available at that stage; listing settings,
loading a theme and reporting a store-version problem must not require a working DB.

## Domain settings remain domain settings

The boundary is declaration versus operational state, not account/machine versus
ordinary preference. Ordered allowed accounts, preferred task/session machines,
declared machine endpoints/aliases and user-authored capacity limits belong in
files. Observed account identity, authentication state, usage, cooldowns, machine
registration/health and execution history retain their runtime owners. Credentials
retain the existing secret mechanism and are never copied into shareable config.
Audit each mixed record field by field; do not move whole tables by their names.

Setting a preference changes future resolution. Login, provisioning, moving an
existing Session and distributing files are explicit domain operations. Inventory
today's setter side effects and migrate their command paths deliberately. Theme
keeps its draft/Apply/Cancel behavior. Installation settings retain bootstrap access.

The catalogue advertises whether an entry supports generic assignment or needs
a domain action. Close missing appropriate CLI/file/UI access; an adapter that
only labels a current omission does not fulfill Jack's direction. Unsupported
source/target combinations need an actual domain or lifecycle reason and the
supported action. No second DB override or bidirectional config mirror is introduced.
Native provider configuration is identified as delegated, not reimplemented.
Unknown LF keys are diagnosed after dispatching all known owners; native skill
frontmatter and recognized non-setting structure are not treated as unknown LF keys.

## Layouts: templates are intent, saved arrangements are state

Jack Heart clarified and confirmed this distinction on November 3. A saved Task
layout is persisted live state; a layout template is declared intent. Saving state
or deliberately changing it does not turn it into configuration. This fits the
file/DB boundary rather than requiring an exception.

- Config owns reusable layout templates and which template a Wave defaults to.
- The DB owns a Task's actual arrangement, including panes and Session bindings.
- Applying a template creates or changes live state. Rearranging a Task updates
  that state without editing its template.

Template selection participates in normal configuration precedence. Changing a
default should not silently rearrange existing Tasks; applying a template to
existing work is a separate action. Shared templates must be portable rather than
depending on private DB-only Session identities. This records the storage model;
layout syntax and implementation remain separate design work.

## Personal repository file location — accepted

Jack Heart selected `./lf.config.local.yaml` on November 3: a private file at the
repository root. This supersedes the proposed `.lf/config.local.yaml`, private Git
metadata path and external repository-keyed storage alternatives. Personal global
settings remain in `~/.lf/config.yaml`; committed team settings remain in
`.lf/config.yaml`.

The private repository file must be ignored by Git and available across Task
worktrees. Implementation must define ignore setup and resolve one shared file
across those worktrees rather than create diverging per-Task copies. Migration
must account for existing personal `repos:` declarations without retaining a
second writable authority for the same preferences.

Forward and retain its authored repository-scoped personal declarations for remote
work. Physical file placement does not itself implement cross-machine propagation;
remote editing and conflict handling remain open.

Prior art informing the choice: Claude Code's private project settings and mise's
local config files. Git conditional includes informed the external alternative.

Sources checked November 3:
[Claude settings](https://code.claude.com/docs/en/settings),
[mise configuration](https://mise.jdx.dev/configuration.html),
[Git conditional includes](https://git-scm.com/docs/git-config#_conditional_includes).

## Remote execution and originating preferences

Accepted by Jack Heart on November 3: the initiating LF instance supplies personal
preferences; the destination supplies its execution environment. For laptop-origin
work, the laptop's personal profile wins over the destination's personal profile.
A phone using the laptop server uses that server's preferences. A shell or phone
starting work directly on the remote uses the remote's saved preferences, including
previously retained declarations. Different entry points may therefore produce
different results. Do not implicitly combine two personal profiles as extra layers.

Repository settings, choice-before-fallback and invocation boundaries retain their
existing semantics. Credentials, installed tools and local machine identity come
from the executor; they are not competing personal preferences. Selecting a profile
for a launch does not authorize overwriting independently edited destination files.
Using the carried profile and retaining it for future commands have separate results.

November 3 research follow-up: Jack Heart requested simpler alternatives before
accepting a conflict rule. See [Config distribution research](config-distribution-research.md).
The recommendation is forwarding plus retention with automatic refresh only while
the retained portable config is unchanged, preserving remote edits for explicit
reconciliation. The per-launch authority rule above is now accepted; the exact
retention/conflict mechanism remains proposed. Automatic multi-writer merging is
not accepted or required by this decision.

Jack clarified the requirement: personal config authored on the laptop should be
respected for remote work, including work initiated through a mobile client. Merely
forwarding settings with commands launched on the laptop is insufficient. The mobile
client or remote executor needs access to the same personal declarations, preserving
scope, target, strength and provenance. A setting must not become a stronger CLI
override simply because it traveled across machines.

Jack is open to Rex's personal-home model if it solves this requirement, but explicitly
requires a demonstrated problem before introducing that concept. Home is therefore
an evaluated distribution mechanism, not an accepted prerequisite or assumed owner
of all preferences. Alternatives include distributing versioned personal config to
execution machines, or consulting one chosen installation with a cache. File editing,
identity and placement semantics remain accepted independently of that choice.

Direct remote/mobile work can use saved preferences while the laptop is unreachable;
it cannot see updates that have not arrived. Define retention freshness and conflict
handling without adding an unexplained DB override or silently
let stale copies become competing authorities. Explicit exclusion remains Rex-style
shared-account servers and multi-user login servers; personal remote authentication
is still required. Being a settings source does not make a host the default executor.

Further direction from Jack: the initiating LF instance (including a laptop server
proxying mobile requests) can push personal config with work. The remote should
retain it so follow-up commands launched there, through mobile or otherwise, use
the last received preferences without requiring the laptop. Develop this as the
distribution candidate before introducing home. Persist only portable authored
config, not command-line or environment overrides. Track origin/version separately
from setting precedence. Remote editing, conflict handling and update timing remain
open; do not silently overwrite independent destination declarations.

Machine identity/naming evidence and open decisions:
[Machine configuration prior art](machine-config-prior-art.md). Herdr's separate
name/target/permanent identity is the closest file-backed precedent. Rex's shared
home demonstrates distribution but also declaration overrides; Omnara uses a
central organization registry. Proposed LF config holds aliases, identity references
and connection declarations; each installation's own identity remains nonportable
local state. Resolve aliases at origin and carry the selected ID, so destination
names cannot silently change placement. File distribution never copies local identity.

Jack clarified that local preferences should generally affect remotely executed
work, but machine selection must not cause the destination to forward work to
itself. Proposed contract:

- A request resolves placement at its origin. Once assigned to a destination,
  remote execution honors that assignment rather than selecting a machine again.
  Any machine selection resolving to the current machine executes locally, using
  stable machine identity rather than hostname/string equality.
- Carry the originating personal declarations with their source/scope/target and
  agent/default_agent strength, retaining the portable profile for future remote
  commands. Do not flatten resolved values into CLI overrides or persist invocation
  flags as preferences. Define migration of any preexisting destination personal
  declarations explicitly. CLI, explicit environment and repository settings retain
  their semantics.
- The destination supplies host facts and local bindings: installed harnesses,
  credential access, filesystem paths and capacity. A portable account/machine
  preference refers to those bindings; transporting a preference neither copies
  credentials nor establishes that the requested resource exists there.
- A later command started directly on the remote uses the retained personal profile,
  including when the laptop is unavailable. Avoid an unexplained precedence layer
  between that profile and separately edited host preferences; writer/conflict policy
  remains a design decision.
- Creating genuinely new work can make a new placement decision. An existing
  Graph's steps execute on its assigned machine. Do not equate new work creation
  with re-routing an already assigned request.

File distribution and per-request propagation are separate. Inspection should
show the originating declaration and its resolved destination, plus any missing
host binding. Remaining design work: define the carried declaration boundary and
repository identity mapping, and prove same-machine, remote and nested-launch
behavior. This is proposed follow-on design, not a change to the active pursue.

## Paseo reference: lessons for the writer contract

Paseo's daemon CLI and settings patches write the same `config.json`; its project
API writes `paseo.json`. Its schema supports validation and editor-schema generation,
but lifecycle/mutable-field lists remain separate. Client-local remembered model
choices are another launch input, not daemon configuration. Evidence and limits:
[Paseo settings reference](paseo-settings-reference.md).

For LF, a write result should identify the saved declaration and report whether
it applies now, to the next launch, requires an explicit lifecycle action, or is
shadowed by another declaration. Persist only requested intent; never serialize
a resolved view back into a preference source. Include UI-carried choices in launch
provenance rather than letting them become an invisible precedence tier.

This supports the proposed file-editor contract. “Close gaps” requires common
semantic access through appropriate interfaces. CLI and Desktop write authored
declarations to the selected file; inspection explains the same resolution.

## The demo

Command spelling is provisional; these are additions to the PR's inspector:

```text
lf config list
  agent          launch preference   general / skill / occurrence
  docs           launch preference   append; [] clears
  theme          personal file       appearance editor / Apply
  account.codex  account route        global / repo / Wave; no skill target

lf config show --skill implement --key agent --explain
  agent = claude
  selected: repo .lf/config.yaml · agent: claude
  fallback: ~/.lf/config.yaml · skills.implement.default_agent: codex
  reason: an explicit agent beats a default, regardless of source or specificity
```

The same query and structured explanation serve CLI and Desktop. With a personal
implement `agent: codex`, the result changes to Codex because personal source wins
among explicit choices. With `docs`, the explanation instead lists contributions
and resets. A “where can I set this?” view comes directly from applicability policy.

## Delete — do not maintain

- `LAUNCH_KEYS`, `REPO_ONLY_KEYS`, `ADDITIVE_KEYS` and hand-maintained eligibility
  branches, once their consumers read descriptor policy in the same change.
- Schema discovery by serializing `Config::default()` in `preference_path` and
  validation; retain the actual typed parsers and semantic validators.
- Separate account/machine *key discovery* in `parse_key`; keep their domain APIs,
  storage, validation and effects behind catalogue bindings.
- The inspector's assumption that the last candidate alone is selected and all
  others are merely earlier. Render resolver-provided dispositions instead.
- Any new duplicated catalogue in Swift, docs or JSON. Generate those projections.

Preserve existing behavioral tests for precedence, identity, native metadata,
unset, false/empty values, propagation and launched-provider agreement. Adjust
tests tied solely to deleted discovery helpers; do not retain obsolete helpers
just to keep their tests green. Dormant summary preferences must be identified as
inactive rather than advertised as working; removing them needs a consumer audit.

## Delivery shape and acceptance

**Additive series. Keystone:** replace scattered catalogue policy and wire discovery,
validation, writers and explanation to it in one coherent PR. Include all existing
LF-owned settings and close appropriate access gaps, with correct adapter
capabilities. The implementation inventory must identify each gap, its new entry
points or justified restriction, and its behavioral proof. The proposed removal
of DB preferences requires an explicit migration: preserve scope, target, strength,
account order and effective precedence; detect collisions with existing file
declarations; remove old readers/writers after cutover. Do not simply export a
resolved snapshot, silently discard overrides, or keep two writable authorities.

Internal sequence: (1) co-locate definitions and cut over validation/eligibility,
removing the old lists; (2) bind special owners and CLI adapters; (3) expose catalogue
and resolver explanations through shared DTOs and inspection. These are internal
slices, not separately maintained old/new implementations.

**Later outcomes:** generated config-editor schema/completion; historical launch
snapshots; cross-machine comparison and profile distribution. Catalogue metadata
prepares them but does not implement synchronization, credential distribution or
new history persistence. Preserve source and strength when exporting declarations;
a resolved snapshot is not a portable preference profile.

Headless acceptance proves:

- Every LF-owned persisted preference is discoverable, or explicitly classified as
  delegated/inactive; private Config fields and operational commands are excluded.
- Applicable file/CLI/env declarations produce the same values and explanations
  through real launch preparation; restrictions agree across those surfaces.
- Choice/default precedence, list append/clear and unset explanations match execution.
- Catalogue and bootstrap reads work with no DB; unavailable sources remain visible.
- Special-setting adapters preserve existing domain behavior and do not expose secrets.
- Migrated DB declarations retain their intended scope and effect; old DB preference
  paths no longer influence resolution. Account registration/history remain intact.
- Exported DTO fixtures agree across Rust/Swift if the Desktop boundary changes.

Gate: `uv run python scripts/test.py --reuse-passing --rust-filter 'test(config::) | test(settings_launch_tests::) | test(wave_setup_tests::)'`, with inherited LF
variables cleared as required by scripts/testing.md; add affected adapter suites
identified by the final diff. Formatting/Clippy and relevant DTO checks are part of
that gate. No display, live provider or paid launch is required.

## Review decisions

1. File-backed settings and private repository storage are accepted:
   `./lf.config.local.yaml`, with personal global and committed team files distinct.
   Implement ignore setup and sharing across Task worktrees. Preserve choice before
   fallback and personal scope ordering while removing the DB source tier.
2. Specify profile distribution: Git-style file editing does not itself synchronize
   files or provision credentials. Define portable account/machine references and
   per-machine bindings without copying runtime history as configuration.
3. Complete the field-by-field migration and setting-by-setting gap inventory. Closing
   appropriate gaps is accepted direction; retaining today's omissions is not a
   viable implementation boundary. Domain operations need not become plain writes.

Check: source audit only; no catalogue code or new tests executed. The active
choice-before-fallback pursue continues independently; this draft is not its directive.
