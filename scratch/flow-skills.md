# Flows in chat, portable skill names

Draft — 2026-10-08. Jack Heart requested both changes and accepted the naming rule below. Flow export mechanics remain proposed; implementation has not been requested.

## What to build

Export every available Flow as a native chat skill that follows its compiled sequence in the current conversation, with one portable dashed name for each definition and hierarchical source organization.

Jack Heart's intent: “automatically got skills in claude code (or codex, etc) for every flow that just instructed to invoke the skills in order, basically. (make sure to handle loops)”. Naming direction: “we accept / where we can but do - everywhere too”. After raising “i do like hierarchy but i also like one canonical way HMMM”, Jack Heart accepted hierarchical sources with dashed public names and slash input normalization (“ok i guess”).

Placement: unresolved; no Wave was supplied. No PM lookup is needed for this design.

## Proposed experience and demo

```text
Author:  .lf/skills/wave/session.md
Name:    wave-session
CLI:     lf skill wave-session       # wave/session also resolves
Claude:  /wave-session
Codex:   wave-session in the skill picker

Claude:  /flow-pursue
Codex:   flow-pursue in the skill picker
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

Generate one `flow-<definition-key>/SKILL.md` per authored Flow. The prefix distinguishes a Flow from a same-named skill without changing LF's existing Flow names. Export only one flat directory per generated native name; validate portable names and report invalid names or collisions without silently truncating or suffixing them.

Each generated wrapper says:

```text
Carry out this Flow in the current conversation using the current request.
Run `lf flow show pursue --instructions` to read its resolved instructions.
Follow that plan here, including its loop decisions and review boundaries.
Do not launch `lf run` or new Sessions to carry out its skill steps.
```

`--instructions` is a proposed read-only view on `flow show`, mutually exclusive with process inspection and JSON. Resolve in the current checkout using `load_authored_flow` and `compile_flow`; emit a complete numbered plan and each distinct resolved skill body once, with original resource base and relevant declarations. This avoids stale repository overrides and dependence on another skill being discoverable in the provider menu. Loading/applying these bodies is the in-chat skill invocation; merely typing another slash command in assistant prose is insufficient.

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

## Current system and implementation

`engine/skills.rs::sync_skills` exports only `SkillCatalog` entries, with raw names in paths/frontmatter. `build.rs::canonical_builtin_name` changes `_` to `/`; builtin suffix lookup and session composition also use slash keys. `skill_catalog.rs::insert` deduplicates exact strings. `flow.rs::find_flow_path` looks up literal paths; `DefinitionLoader::resolve` owns Flow-first selection. `lf/navigation.rs::definition_help` probes both kinds for collision notices. The compiler already owns nested expansion, loop targets, XOR, and review flags.

Core values remain `SkillSource`, `FlowDefinition`, `ConcreteStep`, `DefinitionKind`, and original source paths. Add a shared canonical-key function and renderer, not a chat execution engine:

```rust
pub fn definition_key(name: &str) -> String;
pub fn render_flow_instructions(name: &str, steps: &[ConcreteStep]) -> Result<String, LoadError>;
```

Thread repository scope through `SkillSyncOptions`; keep source/native spelling in provenance where canonical display differs. Resolution must return ambiguity errors instead of discarding them as absence.

**Delete — do not maintain:** direct raw-name export path construction and its nested-path expectations in `skills.rs`; exact-string-only deduplication in `skill_catalog.rs`; separate literal-path matching in `flow.rs`; slash-only builtin registration/lookup assumptions in `build.rs` and `builtins.rs`. Replace their consumers in the same cut. Retain original source layouts, source precedence, composed session bodies, generated-only pruning, and third-party preservation tests. Update guides and help examples to dashed public names; retain deliberate slash-input tests.

Forbidden outcomes: an export-only rename, different definitions selected by spelling, two authored Flow representations, a second Flow parser, executing a Workflow as a Flow, ignored loop/command/review nodes, overwritten third-party bundles, or chat completion counted as Task completion.

## Internal slices and acceptance

One integrated native-skill change, implemented in internal slices and shipped as one PR. No follow-up Tasks are needed for the two requested outcomes.

1. **This slice:** replace name comparison and registration assumptions across discovery/resolution/help/export; preserve slash inputs and native provenance. Cut over obsolete nested exports and their tests together.
2. Add instruction rendering from compiled steps and `flow show --instructions`; prove decisions, branch rejoin, exact occurrence targets, and review boundaries.
3. Add builtin/repository Flow wrappers and scoped sync, document proposed usage, then exercise both native chat surfaces.

Gate: `cargo test -p loopflow --lib`, `cargo test -p loopflow --test documented_commands --test flow_discovery_tests --test discovery_tests --test cli_discovery --test flow_tests`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` must pass. Add behavior fixtures to the existing modules: both filename layouts in isolation; both together; cross-scope overrides; same-key Flow/skill help and explicit/untyped lookup; recursive composition through alternate spelling; repeated skills with exact occurrence IDs; export pruning/collisions/idempotence; and rendered loop/XOR/review/command plans with original skill bodies.

Native chat demo is judgment, not a headless gate dependency. Use harmless fixture flows in Claude and Codex, including a local override. Capture observed behavior; do not claim instruction rendering proves model adherence. Other providers can consume the portable format but are outside this first delivery's integration proof.

## Evidence

2026-10-08: repository inspection established the paths and behavior above; no provider reproduction was run. The naming incompatibility is grounded in the [Agent Skills specification](https://agentskills.io/specification#name-field), which requires dashed names matching their containing directories. [Claude's skill documentation](https://code.claude.com/docs/en/skills#control-who-invokes-a-skill) documents user-only invocation controls. [OpenAI's skill documentation](https://learn.chatgpt.com/docs/build-skills) documents explicit skill selection and lazy body loading. Neither establishes that every historical Claude release rejects the current nested export in the same way.

Check: design-only inspection; implementation and native-provider checks deferred to the implementation/gate/demo steps above.
