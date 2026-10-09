# Flows in chat, portable skill names

Realigned — 2026-10-08. Jack Heart requested both changes and accepted the naming rule and unprefixed Flow exports below. Both are implemented; automated acceptance and native-provider demo/review remain. Sync timing remains the reversible assumption in `scratch/questions.md`.

## What to build

Export every available Flow as a native chat skill that follows its compiled sequence in the current conversation, with one portable dashed name for each definition and hierarchical source organization.

Jack Heart's intent: “automatically got skills in claude code (or codex, etc) for every flow that just instructed to invoke the skills in order, basically. (make sure to handle loops)”. Naming direction: “we accept / where we can but do - everywhere too”. After raising “i do like hierarchy but i also like one canonical way HMMM”, Jack Heart accepted hierarchical sources with dashed public names and slash input normalization (“ok i guess”).

Placement: unresolved; no Wave was supplied, so no Wave memory was selected or curated.

## Implemented experience and pending demo

```text
Author:  .lf/skills/wave/session.md
Name:    wave-session
CLI:     lf skill wave-session       # wave/session also resolves
Claude:  /wave-session
Codex:   wave-session in the skill picker

Claude:  /pursue
Codex:   pursue in the skill picker
```

The `pursue` recipe expands to implement → compress → sync → realign → loop-or-next → pr-publish. Iterate returns to implement and repeats the intervening steps. Advance reaches publication. Blocked explains the missing input in this conversation and waits for new direction. No arbitrary iteration limit.

Demonstrate with a harmless fixture whose first pass needs repair, second passes, and final step writes a marker. The transcript must show the repeated range, exactly one final marker, and the same conversation throughout. A second fixture pauses for a person's review and continues only after their response.

## Naming and resolution

**Accepted naming direction — Jack Heart, 2026-10-08:** hierarchy belongs in source folders; dashed names are canonical in help, listings, CLI examples, exports, and newly captured definitions. Slash spellings remain accepted input. No duplicate exported slash aliases or maintained alias table.

One `definition_key(name)` replaces `/` with `-`. It does not reverse-split hyphens, lowercase arbitrary native names, or rewrite original asset paths. `.lf/skills/wave/session.md` and `.lf/skills/wave-session.md` therefore declare the same key. Discovery groups by `(kind, key, precedence)` before selecting a source:

- Preserve existing repository/personal/builtin and provider-root precedence. A repository dashed source overrides the slash-named builtin, and the reverse spelling works identically.
- Two different sources at the same precedence with the same key are an ambiguity naming both paths. The spelling typed cannot change the winner. Deduplicate repeated discovery of the same physical source.
- Skills and Flows retain separate kinds. Untyped lookup remains Flow-first across equivalent spellings; explicit `skill` or `flow` selects its kind. A malformed Flow never falls back to a same-key skill.
- Existing unique bare-name shortcuts and command precedence remain; normalized keys must not turn every hyphen suffix into a shortcut. Workflow-only names still produce the Workflow diagnostic, never become exported Flows.
- Apply the key to recursive-composition detection, loop target skill-name matching, customization, discovery lists, and help's same-name notices. Occurrence IDs are authored labels and remain exact; resolve them before skill names.

Preserve source paths, resource bases, native provider invocation names, and recorded history separately from the comparison key. Historical captures stay readable without renaming stored graphs. No database migration.

## Flow skills

**Accepted correction — Jack Heart, 2026-10-08:** “no flow prefix” and “just dont write flows that collide with skills (ours OR theirs)”. Generate `<definition-key>/SKILL.md` for each authored Flow whose name does not collide with a skill. Compare canonical keys across both Loopflow and third-party skills, including destination-native skills; skip the Flow export on collision and preserve the skill. This is a reported skip, not a fatal sync error. Never invent a prefix or suffix to escape a collision. LF's existing typed and untyped resolution remains unchanged; only native Flow export yields to skills. Export one flat directory per generated native name and report invalid portable names without silently truncating them.

Each generated wrapper says:

```text
Carry out this Flow in the current conversation using the current request.
Run `lf flow show pursue --instructions` to read its resolved instructions.
Follow that plan here, including its loop decisions and review boundaries.
Do not launch `lf run` or new Sessions to carry out its skill steps.
```

`--instructions` is a read-only view on `flow show`, mutually exclusive with process inspection and JSON. Resolve in the current checkout using `load_authored_flow` and `compile_flow`; emit a complete numbered plan and each distinct resolved skill body once, with original resource base and relevant declarations. This avoids stale repository overrides and dependence on another skill being discoverable in the provider menu. Loading/applying these bodies is the in-chat skill invocation; merely typing another slash command in assistant prose is insufficient.

The wrapper is small; the compiler owns topology. Expanded skill bodies are generated output, never another authored copy. Freeze the returned plan for that invocation. Source edits during a pass take effect on a fresh invocation, not halfway around a loop.

Render all existing constructs:

- Skill: apply its instructions here with the original request and accumulated evidence. Repeated occurrences remain distinct even when sharing a body.
- Command: execute its exact LF argv, safely shell-quoted, in order. Stop on failure; never skip it or automatically replay a successful side effect after interruption.
- Loop: use the compiler's resolved return target and existing advance/iterate/blocked contract. State the specific target and entire repeated range. Invalid decisions require correction at that decision, not rerunning the work.
- XOR: apply the router with authored path descriptions; choose exactly one declared path, complete it, then rejoin. Nested loops stay within their compiled body. Invalid routing never defaults to a branch.
- Human occurrence: perform the review with the participant here and pause for their response. No substitute review agent or fabricated approval.

The transcript carries the current occurrence, loop pass, decision, and pending input. After interruption, inspect evidence before continuing; if the position is uncertain, ask here. This is instruction-driven execution: it provides neither the autonomous driver's mechanical enforcement nor its crash recovery. It creates no synthetic FlowProcess, step Process, or Task workflow arrival. Explicit LF command steps retain their normal effects and authorization boundaries.

The current model performs the work; authored agent preferences are disclosed but do not switch providers. Preserve native declarations and asset bases. Do not pretend to enforce another provider's controls, bypass permission denials, or fork a step silently. A step requiring unavailable native execution explains the specific limitation in this chat. Claude's existing generated user-only setting can remain because the selected Flow carries the resolved bodies.

## Export scope and lifecycle

Extend the existing `sync-skills` operation; its existing release-promotion integration gains Flow exports automatically. Default sync continues to export personal skills and builtin skills/Flows globally. Add explicit `--repo` to generate repository skill/Flow entries under the checkout's `.claude/skills` and `.agents/skills`; never put repository definitions into a shared home. There are currently no personal Flow sources; this change does not invent them.

Repository-only names need that sync after addition/removal. Content changes are picked up when a Flow wrapper renders its plan. Repository overrides of builtin Flow names use the global wrapper, which resolves the current checkout. No per-Flow manual skill authoring, watcher, or provider hook. The scope of “automatic” is generation during sync/promotion, not instantaneous discovery of newly authored filenames.

Keep generated ownership markers and source kind/name. Preflight destinations and collisions before mutation; preserve all third-party files and report skipped exports. Prune obsolete generated nested exports only after replacements are writable; a blocked replacement must not erase the working old export. Generated wrappers remain excluded from source discovery, preventing self-import. Sync remains idempotent.

## Implementation ownership

`definition_name::definition_key` owns portable keys for builtin registration, catalog selection, Flow lookup, loops, help and exports. Builtin shortcuts are derived from source namespaces before flattening; hyphen suffixes do not create aliases. `SkillSource` and `SkillOrigin` retain original names, paths and declarations. New captures use canonical names; stored graphs are not rewritten.

`flow_instructions::render_flow_instructions` consumes `compile_flow` output, emits the numbered topology and borrows distinct captured bodies. `Skill::source_text` owns captured text for both native invocation and chat rendering; rendering does not manufacture an invocation. `flow show --instructions` is local inspection, incompatible with JSON/process inspection. `SkillSyncOptions::repo` selects repository destinations. Sync preflights names/collisions, writes replacements before pruning, preserves blocked nested exports, and reports skipped Flow names without prefixes.

Implementation removed raw-name export paths and nested-path expectations; exact-string-only skill deduplication; literal Flow path lookup; slash-only builtin registration and runtime comparisons; namespace rows in native-facing listings; `build.rs`'s separate category discovery and empty-map emitter; `find_flow_path` and its redundant builtin-override fallback; `SkillInvocation::source_text` and dummy invocations used only to reconstruct text. Builtin registration, shortcuts and categories now share one discovery pass; repository Flow selection returns its canonical key with the original path. Hierarchical source paths, generated-only pruning, source precedence, derived unique shortcuts, session composition, native invocation names and recorded history remain. Scoped Session creation and operation checks also use canonical names; no storage migration.

Forbidden outcomes: an export-only rename, different definitions selected by spelling, two authored Flow representations, a second Flow parser, executing a Workflow as a Flow, ignored loop/command/review nodes, overwritten third-party bundles, or chat completion counted as Task completion.

## Internal slices and acceptance

One integrated native-skill change, implemented in internal slices and shipped as one PR. No follow-up Tasks are needed for the two requested outcomes.

All three implementation slices are present: canonical resolution/export, compiled instruction rendering, and unprefixed global/repository Flow wrappers. Remaining work is gate's automated acceptance and native-provider demo/review. Rendering fixtures do not prove model adherence.

Review repairs are present: writes precede pruning across both providers; blocked replacements preserve their old nested exports; pruning avoids third-party symlinks and export does not turn existing namespaces into bundles; unfamiliar native declarations remain text; counterpart lookup errors propagate rather than becoming absence; scoped operation comparisons and new Session names use canonical keys. Sync is not transactional: an I/O failure may leave earlier writes in place, but prevents pruning and permits retry.

Existing fixtures cover both filename layouts, same-root ambiguity, cross-scope overrides, same-key Flow/skill help and explicit/untyped lookup, recursive composition through alternate spelling, repeated skills with exact occurrence IDs, export pruning/collisions/idempotence, and rendered loop/XOR/review/command plans with original skill bodies. They live in `skill_catalog`, `flow`, `skills`, `flow_instructions`, and `cli_discovery`; builtin inventory coverage lives in `builtins` and `discovery_tests`. Fixture presence is not a full acceptance result.

Remaining automated acceptance belongs to gate: `cargo test -p loopflow --lib`, `cargo test -p loopflow --test documented_commands --test flow_discovery_tests --test discovery_tests --test cli_discovery --test flow_tests --test session_lifecycle_tests`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings`. The Session lifecycle suite covers the changed scoped Session skill names. Unavailable platform checks belong to capable CI; actual failures still require repair.

Native chat demo is judgment, not a headless gate dependency. Use harmless fixture flows in Claude and Codex, including a local override. Capture observed behavior; do not claim instruction rendering proves model adherence. Other providers can consume the portable format but are outside this first delivery's integration proof.

## Evidence

2026-10-08 publication review: inspected the complete change against base `04a4a296b`, including the uncommitted realignment. No new concrete repair was identified. Publish as a checkpoint with scratch retained, not a landing candidate: focused implementation evidence supports review, while merged-tree acceptance remains gate-owned and Claude/Codex adherence remains demo/review-owned. Hosted checkpoint CI defers the matrix until scratch is cleared; a published PR is not a verification pass.

2026-10-08 realignment: inspected implementation `1d85d836f`, simplification `72e46f7fb`, and sync merge `87758c3f8`. The merged upstream change `04a4a296b` alters macOS installer process discovery and CI, not Flow resolution or skill export; no approach change follows from that merge. The current `pursue`/`refresh` sources match the expanded sequence above. No bounded code mismatch was found in this inspection. Prior focused results remain evidence for the earlier source state, not a gate pass for the merged tree.

2026-10-08: repository inspection established the paths and behavior above; no provider reproduction was run. The naming incompatibility is grounded in the [Agent Skills specification](https://agentskills.io/specification#name-field), which requires dashed names matching their containing directories. [Claude's skill documentation](https://code.claude.com/docs/en/skills#control-who-invokes-a-skill) documents user-only invocation controls. [OpenAI's skill documentation](https://learn.chatgpt.com/docs/build-skills) documents explicit skill selection and lazy body loading. Neither establishes that every historical Claude release rejects the current nested export in the same way.

Check: realignment `git diff --check` PASS (prose only); prior Linux `cargo test -p loopflow --lib` filters `portable_`, `portable_repository_flow_overrides_builtin_in_either_layout`, `engine::skill_invocation::tests`, and `engine::builtins::tests`, plus `cargo test -p loopflow --test discovery_tests every_builtin_`, `cargo clippy --all-targets -- -D warnings`, and local `cargo fmt --check` PASS; macOS dependency build scripts stalled; merged-tree acceptance remains gate/CI-owned and native-provider adherence remains demo/review.
