# Flows in chat, portable skill names

Delivery preparation — 2026-10-09. Jack Heart authorized the direct-checklist revision. Generated recipes now combine headless LF commands, conversational reviews, and natural-language loop decisions. Jack Heart requested queue followed by landing. Compression and sync are complete; realignment found the documented recipe and naming behavior consistent with the code. Gate checks are complete with the full-suite limitation below; PR #1510 still shows the earlier renderer. The HTML walkthrough covers `c0136bb9a`, before the compression cleanup.

## Intended outcome

Export Flows as short native skill recipes that coordinate headless LF work and conversational steps, preserving exact names and supplying dashed fallbacks where hierarchy cannot be expressed.

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

Demonstrate with a harmless fixture whose first pass needs repair, second passes, and final step writes a marker. The transcript must show the repeated range, exactly one final marker, and the coordinating conversation retained throughout, with headless work returning to it. A second fixture pauses for a person's review and continues only after their response.

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

**Accepted — Jack Heart, 2026-10-08:** “no flow prefix”; “just dont write flows that collide with skills (ours OR theirs)”. Preserve skill-first export collisions, third-party files, and exact original Flow identity. Skip conflicting Flow exports with a report; never invent a prefix or suffix.

**Requested revision — Jack Heart, 2026-10-08:** “Run `lf -b implement`”, “Execute the skill 'demo' in this conversation”, and loop decisions “maybe needs to be put into natural language instructions”. Generate the checklist directly in `SKILL.md`. Remove the lookup command and captured-body appendix.

The `pursue` recipe:

```text
Carry out these steps using the current request. Wait for each command to finish
and inspect its result before continuing. Stop on command failure.

1. Run `lf -b implement`.
2. Run `lf -b compress`.
3. Run `lf sync`.
4. Run `lf -b realign`.
5. Review the objective, changes, and remaining findings in this conversation.
   If this boundary's requirements are satisfied, continue to step 6.
   If the last pass made meaningful progress and specific work remains,
   state the next action, repeat steps 1–4, then reassess here.
   If progress requires missing input or repeats a failure without new evidence,
   explain what needs resolving and stop. A deferred check alone is not a blocker.
6. Run `lf -b pr-publish`.
```

An authored conversational occurrence renders as “Execute the skill `demo` in this conversation.” Preserve its review boundary: wait for the participant's response before proceeding. The example does not add demo to builtin pursue or export Workflow nodes as Flows.

Implemented mapping: ordinary skill occurrences become headless LF commands; `human: true` occurrences execute the skill here. Existing `Step`/`ConcreteSkill` already carry that distinction, occurrence IDs, and return targets. Jack Heart authorized implementation after this mapping was proposed; no schema was added. Commands use exact selected skill identities and safe shell quoting; use explicit `lf -b skill NAME` where the short form would resolve to another command or Flow. Child runs receive the relevant request and next-pass direction through the existing LF message argument and working artifacts.

Render topology from `compile_flow`, never a second YAML parser:

- Default loop: natural-language criteria with the resolved backward range, including intervening commands and branches, followed by reassessment at the decision. No JSON verdict or arbitrary pass limit. Criteria preserve the loop-or-next distinctions between boundary completion, meaningful progress, repeated failure, and deferred checks.
- Custom or overridden decision skill: execute that skill in this conversation, preserve its actual criteria, then explain which outcome repeats the range, continues, or stops. Never replace custom policy merely because its name is `loop-or-next`.
- XOR: execute the router in this conversation with authored path descriptions; choose one declared path, complete its numbered steps, then rejoin. Preserve custom routing criteria. No JSON transport contract for chat decisions.
- Interruption: inspect existing results before repeating side effects. If position cannot be established, resolve it here.

Headless LF commands retain normal Process/Session records and authored agent settings. The coordinating chat creates no synthetic outer FlowProcess and does not advance a Task Workflow or declare Task completion. Conversation-local execution uses the current model and provider's actual skill-loading support; it must not pretend that naming a skill executed it. Native invocation controls and asset bases remain intact.

## Export scope and lifecycle

Keep existing sync/promotion and explicit `sync-skills --repo`. Global exports cover personal skills and builtin skills/Flows; repository exports stay under the checkout. No personal Flow source, watcher, or provider hook is introduced.

Direct recipes capture topology at sync time. Flow edits and repository overrides therefore require repository sync; the previous runtime-override claim no longer applies. Child commands load skills normally when launched. Fixtures cover repository override refresh without changing global exports. Provider selection of local exports still needs native demo evidence. Generated ownership markers retain exact source kind/name and keep exports out of source discovery.

Preserve idempotence, destination preflight, writes before pruning, blocked replacements, nested Codex exports, and third-party bundles/namespaces/symlinks. I/O failure may leave earlier writes, but prevents pruning and permits retry.

## Current system and key functions

`definition_name::resolve_name` selects exact identities before unique portable fallback. `portable_name` derives dashed projection. Naming, discovery, composition, loop targets, customization, help, and provider exports already implement the accepted correction; retain them.

`flow_instructions::render_flow_instructions(name, steps, flow_names) -> Result<String, LoadError>` renders the direct recipe from compiled `ConcreteStep::{Skill, Command, Xor}`. `skills::render_flow_skill` incorporates it during sync using the original exact Flow identity. Definition loading and compilation share the selected SkillCatalog, so global sync cannot accidentally capture checkout definitions or a different home. `SkillSyncOptions::repo` continues to select repository destinations. No database migration.

Headless skill commands build argv and shell-quote once, preserving agent arguments and escaping command, Flow and flag-shaped names. Export ancestry checks stop at the selected home/repository: aliases used to reach that scope are valid, while provider-directory and bundle symlinks beneath it remain protected during writes and pruning.

## Delete — do not maintain

Removed in the recipe revision:

- `flow_instructions::body_number`, body collection/appendix, chat JSON decision/route contracts, frozen-body and no-child-session instructions; replace their exclusive renderer fixtures with direct-recipe tests.
- The generated `lf flow show ... --instructions` indirection, its unreleased CLI flag/dispatch in `lf/mod.rs` and `lf/navigation.rs`, and its exclusive `cli_discovery` tests.
- Rejected body-bundling/runtime-freshness descriptions in `docs/authoring.md` and `docs/config.md`, and the removed flag in `docs/lf-reference.md` and `README.md`. The local walkthrough covers `c0136bb9a`; it does not yet include the command-construction and export-ancestry simplifications.

Preserve `Skill::source_text` and captured-source behavior for native LF invocation. Preserve compiler topology, autonomous decision schemas/runner, and naming/export protection tests: those remain live consumers, not deletion targets.

Forbidden outcomes: bodies copied into every recipe; a second Flow parser/runtime; generic prose erasing custom decision criteria; fabricated review approval; silently stale repo overrides; normalized fallback displacing exact names; prefixed Flow exports; overwritten third-party skills; chat completion counted as Task completion.

## Internal slices and acceptance

One integrated change ships in PR #1510. Naming and recipe generation are implemented, focused verification passed, and the walkthrough is refreshed; gate/demo acceptance remains. No follow-up Tasks.

**This slice:** implementation, focused review, and local walkthrough refresh complete. The renderer and lookup-CLI deletion are complete, authoring docs are updated, and Loopflow skill exports allow model invocation while retaining third-party controls. The walkthrough distinguishes local `c0136bb9a` from published `ad914c331`.

Focused fixtures must cover headless versus conversational occurrences, safe exact-name commands, a repeat range containing commands/branches, custom decision preservation, XOR rejoin, and no bodies/JSON appendix. Keep existing exact-name, collision, source precedence, capture, and pruning regressions. Demonstrate a two-pass repair and a conversational review without launching a substitute reviewer.

Gate: `cargo test -p loopflow --lib`; `cargo test -p loopflow --test documented_commands --test flow_discovery_tests --test discovery_tests --test cli_discovery --test flow_tests --test session_lifecycle_tests`; `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`. Expect all affected tests and checks to pass. Unavailable platform checks belong to capable CI; native chat adherence belongs to demo/review, not a headless gate prerequisite.

## Evidence and status

2026-10-08: naming correction is published at `ad914c331` in PR #1510. Earlier implementation and review evidence remains in git history. The refreshed local `scratch/pr-review.html` depicts `c0136bb9a`, explicitly distinguishing the unpublished recipe revision from the PR. Native-provider adherence and visual rendering remain unverified.

2026-10-08: inspected the current renderer, sync wrapper, and builtin loop-or-next policy. That policy already permits conversational assessment without a bound decision protocol. Loopflow skill exports now allow model invocation. Authored native controls remain untouched, and the recipe asks for participant invocation when a native dependency requires it. Flow entry points remain user-invoked in Claude.

2026-10-09 compression review: removed the remaining README lookup command and config runtime-freshness claim. Native macOS tests exposed export checks rejecting the system `/var` symlink; narrowed the checks to provider destinations beneath the selected scope. Regression fixtures cover shell/CLI-safe skill names, aliased homes, protected provider symlinks and idempotent pruning. Broader acceptance and native-provider adherence remain with gate/CI and demo/review.

Check: affected integration suites PASS (92 tests, 1 existing ignored), Clippy/fmt/diff PASS; library without inherited LF_CAPTURE_KEY: 1723 passed, 7 planning-fixture failures, 9 ignored; isolated serial planning run PASS (14 tests, including all seven failures). Full-run failures involved fixture paths/credential reads; required CI owns full-matrix acceptance. Native provider adherence remains unverified.
