# Consistent settings and personal graph-step overrides — LOO-714

Accepted by Jack Heart on November 2, revised November 3, 2026. Infrastructure.
This is PR1's authoritative design; earlier storage/route/tier proposals are
superseded. Delivery is review-only PR #1805 with Auto-Ship stopped. No merge or
whole-Task completion: automatic limit recovery remains PR2. Catalogue, portability
and critique drafts are separate contributions, not scope expansion.

## Accepted resolution

Every applicable `agent` choice beats every `default_agent` fallback. Within
either class: CLI > explicit environment > lfdb > personal files > repository/Wave
files > builtins; within a source, Wave > repository > global; within a scope,
graph-step > skill > general. Resolve each setting independently. Thus personal
implement choice beats repository occurrence choice, while personal implement
fallback yields even to repository general choice. Builtin skills recommend no
harness; five recommendations moved to this repository's configuration. Bare
harnesses retain native model defaults; `harness:model` selects a model.

```yaml
# Personal config
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

```sh
lf set --global skills.implement.agent codex
lf set --global graphs.release.steps.write-code.agent claude
lf unset --global graphs.release.steps.write-code.agent
lf config show --graph release --step write-code --json
```

Occurrence targeting uses the defining Graph's canonical name and authored local
ID, never offsets or flattened/process IDs. Composition, repeated children and
loop iterations retain that origin. Other Graphs' same-named IDs do not match;
duplicates within one definition fail. Anonymous steps use skill/general values.
Preferences for absent Graphs remain valid and can appear unmatched in inspection.

Session/resume `-a` selects only that conversation; neither history nor a resolved
default persists Task policy or escapes into unrelated agent-created Tasks.
Explicit Graph/Task execution selection reaches all steps, composition and routers.
CLI beats environment within its class; explicit user environment follows children.
Internal LF identity plumbing is not a preference. Resume's implemented assumption:
a fallback may refresh a same-provider model but cannot request cross-provider
import; native history retains its provider. This is recorded in questions.md,
not a further decision attributed to Jack.

## Full coverage and semantics

Jack explicitly selected everything settable anywhere, including non-agent
preferences. Consistent names/types apply wherever meaningful; restricted or
separately owned settings must be explained, not silently ignored.

| Preference family | Accepted scope, semantics and owner |
| --- | --- |
| agent, default_agent | All layers; general/skill/occurrence; choice before fallback. LF-owned metadata only; native Claude agent names remain native. |
| yolo, chrome, isolate, diff, diff_files, paste | Launch targets; explicit false replaces. Native resume preserves account-home identity. |
| docs, context, exclude | Launch targets; additive lists, explicit [] clears. Keyed maps merge; omission/unset reveals lower layers. |
| summaries, summary_tokens | Existing parsing/merging retained; no runtime summarization consumer was found. Activation requires separately specified behavior. |
| context_budgets, supported_harnesses, automation.retries, graph | General preferences; positive keyed budgets, additive harness list, unsigned retries, replace Graph name. Task takes up its Graph once. |
| planning, release, land, merge_method, pr | General repository operations, no Wave/skill/occurrence targets. Typed planning unit replaces; repository team identity survives personal transport selection. |
| session.terminal, session.end_detached_after, user.name, repo_root | Personal general; terminal, duration/off, name (empty uses Git), absolute/home-relative path. Display name never grants ownership. |
| theme, stream, logs.linear_requests, portfolio | Personal file/dedicated owners: Appearance Apply/Cancel, bootstrap installation, pre-store Machine diagnostics, registered navigation collection. No DB/step parity invented. |
| machine.tasks, machine.session | Dedicated set/unset and Machine APIs, repository/Wave DB scopes; movement/registration remain effects. |
| account.claude, account.codex | Ordered registered routes replace lists; dedicated DB/API plus explicit CLI/env restriction. Preserve ordering and recorded-account resume; no per-skill accounts. |
| invocation controls, LF_HOME/LF_BIN, provider homes/credentials | Execution/bootstrap ownership, not inherited scalar preferences. No credential output. |
| skill action_style/native metadata, Graph topology/interactive/returns, Desktop view state | Authored behavior or local UI owners, not generic launch preferences. |

The shared resolver preserves source/scope/target and authored choice/default key
through preparation and inspection. CLI/Desktop reads expose effective values and
candidate provenance, including list contributions. Parsing, DB set/unset and
execution share typed validation; false, empty, omitted and unset remain distinct.
One resolver replaces old paths. No permanent Codex-only policy, account sharing,
file-only preference model, tiers or settings synchronization was accepted.

## Implementation and evidence — November 3

InvocationSettings owns CLI intent; SkillInvocation retains Graph origin. Runtime
captures retain exact bodies; exported recipes retain occurrence metadata but load
the named skill in the launch repository, preserving repository overrides. Sync
owns generated captures and prunes only unchanged generated contents. Authored
settings no longer become CLI overrides. TaskProcessOptions.agent, prompt-only
selection and duplicate scope/merge/inspection paths were removed.

The single settings_authority.sql migration removes Task agent policy and converts
legacy skill keys while preserving history/routes. The released workflow alias
remains for LOO-637. No additional migration accompanied the November 3 revision.

Choice/fallback resolution, captures and provenance are implemented. The eight
original CI regressions passed locally: personal-only name scope, Linear transport
versus repository team identity, help text, duplicate review IDs and four empty
config expectations. Config inspection's misleading agent warning is repaired.
The November 3 reconciliation inspected resolver, resume and stand-in assertions;
no bounded implementation mismatch was found.

Refresh integrated main's viewer wire (#1800) alongside earlier owner/snapshot
changes. Transport improvements establish no limit recovery. PR #1805 is open at
`04bdec77e3`, auto-merge off; hosted Rust and aggregate checks are skipped, so this
head has no hosted test pass. Prior Task projection confirmed Auto-Ship stopped;
this reconciliation changes no shipping control. The earlier demo remains dated
evidence, not acceptance of the revision.

Acceptance still requires stand-in proof of source/scope/target precedence,
composition/repetition/routers/loops, Session and resume isolation from Tasks,
forced Graph execution, native history, non-agent values, false/empty/unset/merge,
and inspection/execution/export parity. Resolver, wave_setup_tests,
settings_launch_tests and session_launch_contract cover these boundaries;
passing syntax alone is insufficient. config-audit.md records actual builtins,
Loopflow and manabot selections, native defaults and ordered routes. No comparative
cost/quality conclusion is established.

Remaining: gate's affected suites and Clippy, final review-only publication and
review/demo acceptance. Catalogue/portability proposals remain undecided and are
not blockers for this revision. No code test rerun is justified by this prose pass.

## Concept review — November 3

Outcome: no design change and no unresolved product choice for PR1. This reviews
Jack Heart's accepted revision against the branch, not installed behavior or demo
acceptance. The catalogue and portability drafts remain proposals outside this run.

The concrete interaction is to set `skills.implement.agent=codex`, inspect a named
Graph occurrence, then launch it. A repository occurrence's Claude choice loses
to that personal choice. Changing the personal key to `default_agent` deliberately
lets the repository choice win. Removing the DB preference reveals the file
preference; clearing a list instead deliberately suppresses earlier contributions.
These are useful distinctions, not interchangeable spellings of “default.”

The model needs declarations with source/scope/target and choice strength, an
authored occurrence identity, and a derived resolution. `set`/`unset` own preference
writes; inspection and preparation share resolution; Graph execution carries only
its explicit invocation choices to its steps. Session history records execution
and preserves provider/account continuity. It does not need a second Task agent
policy or a persisted copy of the effective preference. No additional state or
lifecycle is required by this review.

Failure and recovery remain distinct: an invalid targeted value is corrected or
unset before launch; it must not silently become a different valid preference.
A resumed conversation keeps its native provider/account unless an explicit
supported switch is requested. Exhaustion during a turn remains PR2's failure:
changing a fallback is neither an account retry nor automatic recovery, and must
not rewrite the original thread's attribution.

Preserve these counterexamples in the next proof: a high-source fallback cannot
defeat a lower-source choice; a parent Graph's same-named ID cannot target its
child; an exported occurrence cannot promote authored settings to CLI authority;
a Session's `-a` cannot select a Task worker; explicit `false`, `[]` and unset
cannot collapse together. Existing usage docs express the accepted interaction;
no new skill instruction is needed. The refreshed `concept-review.html` removes
the stale “resolver has not yet been changed” claim. Gate and demo retain the
remaining proof already listed above; this read-only code review reruns no tests.

## PR2 remains unimplemented

Both outcomes remain: interactive Sessions recover without individual manual
switches; headless Graph steps continue on eligible accounts/models, or truthfully
wait for reset and resume automatically when none can continue. Preserve work,
Session continuity and original provider-thread/account attribution. Routing and
retry must share one decision; failed preparation cannot bind an old thread to an
account that never ran it. Exact fallback/wait/continuity mechanisms need PR2's
design. limit-failures.md and weekly-limit-2026-11-02.md retain the reproduction;
no retry-invariant deletion or transcript symlinking is accepted.

Historical full inventory/deletion rationale: `04bdec77e3:scratch/settings-scope.md`;
earlier design/review blob `6ae574bd76b7b34afb61f96249f910492da8ef3d`.

Check: `git diff --check` and `lf context` — passed; reused prior `cargo nextest run -p loopflow --lib -E 'test(config::settings::tests::) | test(prompt::process::tests::)'` (37 passed) and launch/resume matrix (18 passed); gate owns broader validation. Exact earlier commands: Git blob `a9f13d18bf8f32b2012fb346bcbb269f1978f13c`. One earlier nextest output-handle leak did not recur; cause unknown.

Publication check, November 3: affected nextest set ran 909 tests (908 passed); isolated the remaining Linear connection fixture from live personal preferences, then `cargo nextest run -p loopflow --lib -E 'test(ops::linear::tests::connect)'` passed both regressions. `scripts/test.py` passed architecture, website (72), headless Swift (664 plus isolated lifecycle), inventory and formatting; its initial Rust filter incorrectly named an integration module as a binary, corrected in the separate nextest run. `scripts/check_migrations.py`, `cargo fmt`, `cargo clippy --all-targets --jobs 4 -- -D warnings` and `git diff --check` passed. Logs: `/tmp/loo-714-publish-{checks,rust,repair,clippy}.log`. Hosted proof remains deferred while scratch is retained; configured-provider/demo acceptance and PR2 recovery remain open. Auto-Ship is stopped.
