# Flows in chat, portable skill names

Implemented correction — 2026-10-08. PR #1510 now preserves exact literal identities, with normalization only for fallback lookup and native projection. Gate acceptance and native-provider demo/review remain. Unprefixed Flow exports and skill-first export collisions remain accepted. Sync timing remains the reversible assumption in `scratch/questions.md`.

## Intended outcome

Export available Flows as native chat skills that follow their compiled sequence in the current conversation, preserving exact definition names while supplying dashed fallbacks where hierarchy cannot be expressed.

Jack Heart's intent: “automatically got skills in claude code (or codex, etc) for every flow that just instructed to invoke the skills in order, basically. (make sure to handle loops)”. Naming direction: “we accept / where we can but do - everywhere too”. Latest clarification supersedes the earlier collapsed-key interpretation: “if you define wave/session and wave-session, then wave-session wins for the wave-session literal (if you CAN use wave/session, i.e. cli and codex, then you can use both)”.

Placement: unresolved; no Wave was supplied, so no Wave memory was selected or curated.

## Required experience and pending demo

```text
Sources: .lf/skills/wave/session.md   # hierarchical definition
         .lf/skills/wave-session.md  # distinct dashed definition
CLI:     lf skill wave/session      # selects hierarchical definition
         lf skill wave-session      # selects dashed definition
Claude:  /wave-session              # dashed definition wins flattened export
Codex:   wave/session and wave-session remain separately selectable

Claude:  /pursue
Codex:   pursue in the skill picker
```

The `pursue` recipe expands to implement → compress → sync → realign → loop-or-next → pr-publish. Iterate returns to implement and repeats the intervening steps. Advance reaches publication. Blocked explains the missing input in this conversation and waits for new direction. No arbitrary iteration limit.

Demonstrate with a harmless fixture whose first pass needs repair, second passes, and final step writes a marker. The transcript must show the repeated range, exactly one final marker, and the same conversation throughout. A second fixture pauses for a person's review and continues only after their response.

## Naming and resolution

**Accepted correction — Jack Heart, 2026-10-08:** exact literal names are identities, not aliases of one collapsed key. Both hierarchical and dashed definitions may coexist. Exact spelling wins; normalization is a fallback only when no exact definition exists. A hierarchical definition still has a dashed way to invoke it when that dashed name is unoccupied.

Keep literal names in discovery, captured definitions, and listings. Derive portable spellings with `/` → `-` only for fallback lookup and provider projection. Do not reverse-split hyphens, lowercase arbitrary native names, or rewrite asset paths:

- Preserve repository/personal/builtin and provider-root precedence for identical literal names. Search the selected literal catalog exactly before considering normalized candidates. A fallback must not hide an exact definition; distinct spellings can therefore select different sources.
- `wave/session.md` plus `wave-session.md` is valid, not ambiguous. Only a fallback with multiple distinct candidates and no exact match needs an ambiguity diagnostic naming the candidates. Deduplicate repeated discovery of the same physical source without erasing legitimate literal names.
- Skills and Flows retain separate kinds. Explicit CLI `skill` or `flow` restricts candidates first. Bare YAML references and `flow: NAME` remain untyped; `step: NAME` selects a skill. Untyped lookup checks exact names before normalized fallback, with Flow-first precedence when both kinds have the same exact name. A malformed selected Flow never falls back to a skill. Help's collision notices must describe the actual selected definitions.
- Existing unique bare-name shortcuts and command precedence remain; normalized keys must not turn every hyphen suffix into a shortcut. Workflow-only names still produce the Workflow diagnostic, never become exported Flows.
- Recursive-composition detection uses resolved definition identity, not flattened spelling: composing distinct slash/dash Flows is valid; a real cycle through fallback aliases still fails. Customization writes/returns the selected source. Loop targets match exact occurrence IDs, then exact skill names, then unique normalized skill-name fallback. Occurrence IDs themselves are never normalized.

Preserve source paths, resource bases, native provider invocation names, and recorded history. Historical captures stay readable without renaming stored graphs. No database migration. The builtin catalog may keep its existing dashed public names and derived slash shortcuts, but aliases must never displace an exact authored definition.

Native export is a projection after source selection. Codex preserves separately addressable slash and dash definitions, plus dashed fallback exports only where the literal dashed name is unoccupied. Claude uses flat dashed names: an exact dashed definition wins over a hierarchical fallback; report any unrepresentable hierarchical export as skipped without failing the entire sync. Same-target third-party files remain untouched. Resolve export choices explicitly rather than relying on iteration order. Pruning must retain desired nested Codex exports and not confuse them with obsolete flattened Claude exports.

## Flow skills

**Accepted correction — Jack Heart, 2026-10-08:** “no flow prefix” and “just dont write flows that collide with skills (ours OR theirs)”. Generate a native skill under the Flow's projected name only when that target name does not collide with a Loopflow or third-party skill. Compute collisions for each provider's addressable names, including fallback projections and existing native destinations. Skip the Flow export and preserve the skill; this is a reported skip, not a fatal sync error. Never invent a prefix or suffix to escape a collision. Each wrapper must load its original exact Flow name, not its flattened display name, so separate slash/dash Flows cannot execute the wrong definition. Validate names for the receiving provider rather than banning supported slash names everywhere.

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

`definition_name::resolve_name` selects exact literal names before unique portable fallback; `portable_name` only derives `/` → `-` spellings. Skill and Flow discovery retain literal names with source precedence per identical name. Untyped lookup selects across both catalogs before applying Flow-first precedence for an identical name. Captured names, recursion, loop targets, listings and scoped-operation comparisons now use selected identities. Stored graphs are not rewritten.

`flow_instructions::render_flow_instructions` consumes `compile_flow` output, emits the numbered topology and borrows distinct captured bodies. `Skill::source_text` owns captured text for both native invocation and chat rendering; rendering does not manufacture an invocation. `flow show --instructions` is local inspection, incompatible with JSON/process inspection. `SkillSyncOptions::repo` selects repository destinations. Sync preflights names/collisions, writes replacements before pruning, preserves blocked nested exports, and reports skipped Flow names without prefixes.

Builtin registration keeps its dashed public names and source-derived shortcuts. Provider projection selects destinations explicitly: Codex retains literal hierarchical entries and available dashed fallbacks; Claude selects the dashed literal over hierarchical candidates. Wrappers load the original Flow name. Desired nested exports survive pruning, and third-party bundles, namespaces and symlinks remain protected.

**Delete — do not maintain:** removed collapsed catalog identities, slash/dash-pair ambiguity tests, normalization-only recursion/loop comparisons, flat-only Codex export/pruning assumptions, and same-source claims in help/docs. Exact-name coexistence and fallback tests replace them. The local walkthrough stays untouched by request. Compression also removes `resolve_flow_name`’s duplicate source scan and separate captured/live skill selection in launch preparation. No remaining code deletion target is identified; preserve compiled chat plans, source precedence, export ownership protections, body freezing, and Flow control behavior.

Forbidden outcomes: rejecting the valid slash/dash pair; fallback overriding an exact name; losing either definition on slash-capable surfaces; choosing an export by iteration order; Flow wrappers loading the wrong exact name; prefixed Flow names; a second Flow parser; overwritten third-party bundles; or chat completion counted as Task completion.

## Internal slices and acceptance

One integrated native-skill change, implemented in internal slices and shipped as one PR. No follow-up Tasks are needed for the two requested outcomes.

**Remaining acceptance:** gate checks and native-provider demo/review. PR #1510 remains the single delivery boundary with the corrected naming explanation. Jack Heart requested that `scratch/pr-review.html` remain local and unchanged for the separately requested refresh after this Flow finishes. Compiled instruction rendering and unprefixed Flow wrappers remain in place.

Review repairs are present: writes precede pruning across both providers; blocked replacements preserve their old nested exports; pruning avoids third-party symlinks and export does not turn existing namespaces into bundles; unfamiliar native declarations remain text; counterpart lookup errors propagate rather than becoming absence; scoped operation comparisons and new Session names use resolved literal names. Sync is not transactional: an I/O failure may leave earlier writes in place, but prevents pruning and permits retry.

Implemented fixtures: both layouts alone and together with distinct bodies; exact lookup selecting each; unique fallback when exact is absent; genuine ambiguous fallback; source precedence for the same literal; exact skill versus fallback Flow and exact Flow versus skill; composition of distinct slash/dash Flows versus actual alias cycles; exact and fallback loop targets; both Codex exports surviving repeated sync; dashed Claude literal winning over flattened hierarchy; unprefixed Flow exports yielding to native skills; wrappers invoking original exact Flow names; and generated-only pruning preserving third-party files. Existing rendering/control fixtures remain. Fixture presence is not a full acceptance result.

Remaining automated acceptance belongs to gate: `cargo test -p loopflow --lib`, `cargo test -p loopflow --test documented_commands --test flow_discovery_tests --test discovery_tests --test cli_discovery --test flow_tests --test session_lifecycle_tests`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings`. The Session lifecycle suite covers the changed scoped Session skill names. Unavailable platform checks belong to capable CI; actual failures still require repair.

Native chat demo is judgment, not a headless gate dependency. Use harmless fixture flows in Claude and Codex, including a local override. Capture observed behavior; do not claim instruction rendering proves model adherence. Other providers can consume the portable format but are outside this first delivery's integration proof.

## Evidence

2026-10-08 naming correction: exact-first lookup now reaches capture, composition, loop targets, customization, help, and provider projection. Review identified that nested Codex destinations must not write inside existing bundle assets or turn planned namespaces into bundles; preflight now preserves those boundaries. Slash/dash coexistence and idempotent provider exports have focused fixture evidence. The local HTML review artifact remains untouched.

2026-10-08 compression review: exact-first selection now reuses discovered names instead of rebuilding catalogs or cloning the combined name list. Export preflight separates unsafe ancestry, existing bundles, and new namespaces. Launch scope selection and prompt assembly share captured-first skill resolution: a removed or malformed replacement source cannot invalidate a retained invocation. The existing retained-source fixture now enters through outer prompt preparation and checks both cases. Prior implementation/realignment evidence is retained in `42528085e` (including `1d85d836f`, `72e46f7fb`, and sync merge `87758c3f8`); it is not merged-tree gate acceptance.

2026-10-08 realignment: inspected `acf724a2c` against the accepted correction, including catalog selection, composition, provider projection, collision handling and nested-export fixtures. No additional implementation mismatch was identified. Corrected authoring guidance that still described Flow-first lookup without the exact-name boundary, and distinguished untyped YAML `flow:` from Flow-only CLI inspection. No Wave is bound, so no Wave memory was curated. The existing PR branch contains the naming correction; publication of this documentation checkpoint remains with the Flow's publication step.

2026-10-08: repository inspection established the paths and behavior above; no provider reproduction was run. The naming incompatibility is grounded in the [Agent Skills specification](https://agentskills.io/specification#name-field), which requires dashed names matching their containing directories. [Claude's skill documentation](https://code.claude.com/docs/en/skills#control-who-invokes-a-skill) documents user-only invocation controls. [OpenAI's skill documentation](https://learn.chatgpt.com/docs/build-skills) documents explicit skill selection and lazy body loading. Neither establishes that every historical Claude release rejects the current nested export in the same way.

2026-10-08 publication review: inspected the branch range from base `04a4a296b` through `7a8d47ac6` and the pending realignment notes. Exact-first selection, separate Codex exports, original-name Flow wrappers and skill-first export collisions match the accepted correction; no additional repair was identified. Publication remains a review checkpoint, with gate acceptance and native-provider adherence pending. The local HTML walkthrough is excluded from staging and unchanged.

Check: `git diff --check` PASS (prose-only realignment); retained Linux `cargo test -p loopflow --lib` filters `portable_` and `retained_skill_context_keeps_its_source_after_catalog_selection_changes` (17 tests), `cargo clippy --all-targets -- -D warnings`, and local `cargo fmt --check` PASS from `acf724a2c`; broader acceptance remains gate/CI-owned and native-provider adherence remains demo/review-owned.
