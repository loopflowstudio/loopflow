# One settings catalogue

Draft for Jack Heart · LOO-714 / infrastructure · 2026-11-03

Build one executable definition of every LF-owned setting, so its accepted values,
supported locations, editing path and explanation agree across CLI, files, DB and
Desktop. This develops the existing settings-portability-review.md audit. It is a
design for review, separate from the running choice-before-fallback implementation.
No catalogue implementation or synchronization is approved by this draft.

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
| Set/unset | Generic preferences plus separate account/machine parsing and dispatch | Resolve one catalogue entry, then call its storage/domain adapter. Preserve domain effects. |
| Inspection | Runtime and `config show` share resolution; candidates include source, scope, target, value and authored key | Add catalogue discovery and structured explanations; show contributions, clearing, fallbacks and unavailable sources. |
| Special settings | Theme, stream, portfolio and logs have other readers/writers; account/machine choices have domain stores | Include them in discovery through adapters, with explicit capabilities. Do not force them into `Config` or the DB. |
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
| Persistence binding | Files or DB preferences according to the requested source | Same |
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

Read each source as present, absent or unavailable. The existing fallback on an
unreadable DB can remain usable, but inspection reports incomplete evidence.
Bootstrap readers request only sources available at that stage; listing settings,
loading a theme and reporting a store-version problem must not require a working DB.

## Domain settings remain domain settings

Account routes expose their ordered preference and existing read/write operation,
not credentials. Machine/session selection exposes its existing operation,
including movement/copy semantics; generic writes must not bypass those effects.
Theme keeps its draft/Apply/Cancel behavior and file writer. Installation settings
retain bootstrap access. Portfolio remains a managed collection.

The catalogue advertises whether an entry supports generic assignment or needs
a domain action. Unsupported source/target combinations include the reason and
the supported action. Inclusion does not automatically add every field to every
surface. Native provider configuration is identified as delegated, not reimplemented.
Unknown LF keys are diagnosed after dispatching all known owners; native skill
frontmatter and recognized non-setting structure are not treated as unknown LF keys.

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
LF-owned settings in discovery, with correct adapter capabilities. No new storage
layer or database migration is required for the catalogue itself.

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
- Applicable file/DB/CLI/env declarations produce the same values and explanations
  through real launch preparation; restrictions agree across those surfaces.
- Choice/default precedence, list append/clear and unset explanations match execution.
- Catalogue and bootstrap reads work with no DB; unavailable sources remain visible.
- Special-setting adapters preserve existing domain behavior and do not expose secrets.
- Exported DTO fixtures agree across Rust/Swift if the Desktop boundary changes.

Gate: `uv run python scripts/test.py --reuse-passing --rust-filter 'test(config::) | test(settings_launch_tests::) | test(wave_setup_tests::)'`, with inherited LF
variables cleared as required by scripts/testing.md; add affected adapter suites
identified by the final diff. Formatting/Clippy and relevant DTO checks are part of
that gate. No display, live provider or paid launch is required.

## Review decisions

1. Approve the keystone boundary: one authoritative catalogue plus discovery and
   explanations, preserving existing storage owners and applicability?
2. Is catalogue visibility with an explicit domain-action binding sufficient for
   theme, portfolio, account routes and machine movement initially, rather than
   requiring every operation to become a plain assignment?

Check: source audit only; no catalogue code or new tests executed. The active
choice-before-fallback pursue continues independently; this draft is not its directive.
