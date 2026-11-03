# Settings surfaces, inspection and portability

LOO-714 · review for Jack Heart · 2026-11-03

Jack requested this audit while the approved choice-before-fallback pursue runs.
This is a separate design discussion, not authorization to expand the running
implementation into settings synchronization or per-skill accounts. Source paths
below describe the branch at inspection; `default_agent` support is in progress.

The catalogue portion is now developed into the draft `settings-catalog.md`, with
`settings-catalog.html` as its review page. This audit remains the supporting
inventory and portability evidence; it is not a second implementation plan.

## What can be set where

The shared resolver already covers much of the matrix. “All sources” below means
CLI typed settings, explicit environment settings, DB preferences, personal files,
repo/Wave files where eligible, then application defaults. It does not mean every
setting can sensibly apply to every target or that every consumer has been proved.

| Settings | Supported places / targets | Current qualification |
| --- | --- | --- |
| `agent`; accepted `default_agent` | All sources; general, named skill, named graph occurrence | Agent works; distinct default strength is the active revision. LF-owned frontmatter only. Builtin skill recommendations removed locally. |
| `yolo`, `chrome`, `isolate`, `diff`, `diff_files`, `paste` | All sources and launch targets | Explicit false matters. Account-home continuity on resume is a separate recorded constraint. |
| `docs`, `context`, `exclude` | All sources and launch targets | Additive lists; explicit empty clears. Inspection must show contributions, not just one winner. |
| `summaries`, `summary_tokens` | Accepted by launch configuration and targets | Prior implementation audit found no runtime summarization consumer; an accepted key alone does not prove useful behavior. |
| `graph`, `supported_harnesses`, `automation.retries`, `context_budgets` | General configuration through shared sources; global/repo/Wave scopes | Not skill/occurrence preferences. A Task takes up its graph once. Budgets name authored context categories. |
| `planning`, `release`, `land`, `merge_method`, `pr` | General config and typed CLI/env/DB; personal/repo layers | No Wave or skill/occurrence target. Planning replaces a tagged unit. Some operations deliberately read repository-owned setup. |
| `user.name`, `repo_root`, `session.terminal`, `session.end_detached_after` | Personal global settings; typed shared interfaces where applicable | No repo/Wave/skill target. Local paths and terminals differ between machines. |
| `theme`, `stream`, `logs.linear_requests`, `portfolio` | Personal file plus dedicated UI/commands/readers | Outside the generic Config schema. Theme has Apply/Cancel and file dependencies; startup diagnostics/install settings may be needed before the DB opens. |
| `machine.tasks`, `machine.session` | Dedicated `lf set`/`unset`, repo/Wave DB scope | No global machine target. Machine choices copy to peers; capacity choices remain local. Session movement is an operation, not merely config. |
| `account.claude`, `account.codex` | Dedicated route set/unset in global/repo/Wave DB scope; invocation account flags/env | Separate route parser/storage and registered identities. No per-skill accounts. Preference order is distinct from credentials and observed eligibility. |
| Provider-native model/options, plugins, personal skill and graph files | Provider config and definition files | Some behavior intentionally belongs to the provider. LF inspection should identify delegation instead of pretending to own every native option. |

Repository `.lf/config.yaml`, Wave `.lf/wave/<name>/config.yaml`, and personal
`~/.lf/config.yaml` are distinct sources. Personal files support repo sections and
nested Wave sections; this branch recognizes canonical repository identity as
well as local paths (path entry takes precedence if both exist).

Evidence: [Config fields](../rust/loopflow/src/config.rs),
[eligibility and resolution](../rust/loopflow/src/config/settings.rs),
[set/unset special routes](../rust/loopflow/src/lf/commands/setting.rs),
[Wave setup projection](../rust/loopflow/src/work/wave/setup.rs),
[theme ownership](../swift/LoopflowMac/Design/AppearanceStore.swift).
Operational flags such as interactive/batch, navigation and publishing actions
are not automatically persistent preferences. Their absence from the matrix
should be explicit, rather than encoded as an unexplained exception.

## What would make the architecture more consistent

Proposed: one typed settings catalogue used by all surfaces. This should replace
scattered definitions, not become another parallel schema. Keep Rust's typed
values, adding the metadata the UI, writer and inspector otherwise guess:

- Key, value type, default and description.
- Applicable sources, scopes and targets, with a reason for restrictions.
- Resolution and merge behavior: replace, additive/clear, keyed merge or fallback.
- Persistence owner: shared config, personal file, DB preference or domain API.
- Portability: personal portable, project portable, or machine-bound; referenced assets.
- Lifecycle: bootstrap, new launch, live preference or captured execution state.
- Consumer/delegation and safe display policy; credentials are not ordinary settings.

Use that catalogue to drive CLI/file/DB validation, editor completion, documentation,
discovery, inspection and portable export. Dedicated commands can remain convenient
adapters to the same typed rules. Account connection and Session movement retain
their domain operations; uniform discovery does not require identical storage.

Current duplication is concrete: `Config` supplies types; `LAUNCH_KEYS` supplies
target eligibility; `REPO_ONLY_KEYS` and `validate_scope` supply scope rules;
`ADDITIVE_KEYS` supplies merging; set/unset separately recognizes accounts/machines;
theme and installation have other readers. The new default strength must be visible
to all these consumers without each reimplementing resolution.

A small number of behavioral contract tests should prove equivalent settings
through applicable entry points reach the same execution values. Include false,
empty, unset, fallback versus explicit choice, and contributor provenance. Do not
test only schema acceptance or duplicate resolver code inside an inspector.

## Inspection should answer three different questions

1. **What may I set?** A complete catalogue, showing valid locations and target
   scopes. Include dedicated settings, and explain unavailable combinations.
2. **What would apply here?** Resolve for this repo, Wave, skill or graph occurrence.
   Show value, declaration key/strength, exact file or DB scope, and why competing
   values lost. Additive settings show each contribution and where a clear reset it.
3. **What did this execution use?** A nonsecret snapshot of the actual launch
   selection and provenance. Do not reconstruct history from today's config.

The PR already provides `lf config show`, including `--skill`, `--graph`, `--step`
and JSON. It shares the runtime resolver and reports candidate source/scope/target.
The installed lf here does not yet have that command. Its current text rendering
calls the last candidate “selected” and others “earlier”; that is insufficient for
additive settings and does not explain strength/source/scope comparisons.
See [inspection implementation](../rust/loopflow/src/lf/commands/config.rs).

Proposed explanation, illustrative rather than current output:

```text
release / write-code (implement): agent = codex
  selected: personal file, skills.implement.agent = codex
    personal source beats repository source among explicit choices
  shadowed: repository graph, write-code.agent = claude
  fallback: personal file, default_agent = codex
    not needed because an explicit agent applies
```

Useful additions: filter to configured values; inspect one key; preview removing
one override; compare two targets or Machines; distinguish source absent from
source unreadable. Today an unavailable settings DB logs a warning and falls back;
an inspector should show that uncertainty rather than implying the DB had no values.
File path plus YAML key is the minimum useful location; line links can follow.

## Does database sync plus a shared repository cover a complex setup?

Only if every relevant preference and dependency is covered. There are three
distinct distribution paths, and the same resolved value is not proof of the
same stored policy:

| Material | How it can travel | What remains |
| --- | --- | --- |
| Committed repo/Wave config, skills, graphs and referenced repo docs | Git | Receiving machine needs the intended revision; uncommitted edits do not travel. |
| DB preferences | Logical preference synchronization | Preserve global/repo/Wave target identity, ordering, deletions and concurrent-edit semantics. Current general `lf set` does not do this. |
| Personal `~/.lf/config.yaml` and personal definitions/assets | Personal config distribution, e.g. a dotfiles repo, or an LF-managed mechanism | Git for the project does not carry these. Keep personal-file authority when copying; importing values as DB overrides would change precedence. |
| Paths, terminal setup, installed harnesses and credentials | Machine setup/provisioning | Resolve portable references locally; identical preferences do not install tools or connect accounts. |

Current evidence: shared `lf set` writes only the local `scope_settings` table.
Machine selection has a separate SSH copy-to-peers implementation, with unreachable
peers retaining their old choice. History export copies Sessions/Processes/events
as history, explicitly separate from local execution authority; it does not export
settings tables. SSH carries explicit invocation settings but does not establish
ongoing personal-file or DB synchronization.
See [settings writers](../rust/loopflow/src/lf/commands/setting.rs),
[history copy contract](../rust/loopflow/src/machine_records.rs),
[SSH invocation transport](../rust/loopflow/src/lf/commands/ssh.rs).

Proposed portability contract: synchronize personal preference records, share repo
content through Git, and distribute personal config/definitions as personal files.
Keep machine-bound values local or explicitly bound to their Machine. Prefer
canonical repository identity over absolute paths in portable configuration.
Use stable account references with locally provisioned credentials; native model
configuration and personal skill/theme assets are dependencies too.

Avoid flattening everything into an “effective config” and syncing that: it erases
which values were defaults, promotes lower-source choices to overrides, and loses
future fallback behavior. Export declarations with source, target and strength.
Separate a portable profile from a diagnostic resolved snapshot.

The DB also contains Machine identity, local processes, history and credentials.
Synchronizing selected preference records is a smaller contract than cloning the
whole runtime database; existing history copying deliberately preserves that
boundary. No database replication design is accepted here.

## Decisions to review

1. Make one catalogue own setting semantics and applicability, including special
   settings through adapters, while retaining appropriate storage owners?
2. Treat a personal portable setup as declarations plus referenced assets, with
   explicit machine-local settings, rather than a flattened effective config?
3. Prioritize catalogue/why-this-value inspection now, then cross-machine comparison
   and profile distribution as a separate outcome?

The approved pursue is running as Graph 6559210a-be79-409c-8dca-1c469db45ba3 on
Codex, with Auto-Ship stopped. This review neither changes its scope nor approves
landing. PR #1805 remains the settings review destination.
