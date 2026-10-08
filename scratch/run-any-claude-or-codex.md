# Run existing skills on either harness

LOO-420. Draft returned for design review; reconciled 2026-10-07. Jack Heart
selected native invocation for skills belonging to the selected harness,
translated ports for the other harness, and inlined Loopflow builtins. Jack requested the first
workflow step and no landing. No production implementation is retained.

## Required behavior

`lf audit` discovers `.claude/skills/audit/SKILL.md` without `.lf` configuration.
Claude receives its native invocation; Codex receives a translated port. Listing
and help identify the same selected source as execution. User skill files and
their bundled assets remain unchanged. Unfamiliar declarations become visible
limitations with runnable instructions, not silent omissions or load failures.

## Proposed owners and cut

One engine skill catalog owns roots, names, precedence, source paths and format.
Repository definitions precede personal definitions, then embedded builtins.
Within each scope: `.lf/skills`, `.claude/skills`, `.claude/commands`,
`.agents/skills`, `.codex/skills`, `.codex/prompts`. Explicit provider home
overrides must select the same files as their harness. Generated Loopflow exports
never override current embedded instructions. Bundled Markdown is not a second
skill. Namespaces and filesystem cycles need deterministic handling.

This ordering is proposed, not a recorded decision from Jack. Native dispatch
must resolve the catalog's selected file, including same-name collisions; a
matching slash-command name alone does not establish source agreement. Export
reuses catalog discovery while retaining its personal/builtin scope: repository
skills remain local, and discovering a third-party source does not authorize
overwriting it or exporting it to another harness unchanged.

Canonical Process prompt preparation owns native versus ported selection after
agent resolution. Captured skill provenance must survive Flow serialization;
Claude's `agent: Explore` must not become an lf harness selector. Native launch
must retain provider parsing of arguments, permissions, model and supporting files.
Codex app-server documents explicit skill input alongside text. Claude's native
print transport is proved below; it needs a separate context channel. Existing
Session turns must remain in their conversation; no new execution owner.

Ports need argument substitution, tool/model translation with explicit loss
notices, absolute supporting-file access, and capture of the actual instructions
and invocation. Permission hints are not equivalent to enforced tool policy.
Never overwrite an existing native skill to stage a port. No database migration
is selected. Storage changes must follow the final captured-input representation.

## Architectural counterexample and unresolved boundary

`run_prompt` passes `AgentConfig` to the headless harness. Terminal/IDE
`launch_session_with_env` receives model, environment and provider Session ID
separately, but skill invocation and context share one prompt string. Claude
native dispatch recognizes one command text block. Prefixing context or sending
two text blocks bypassed native expansion. Appending context expanded the skill but placed that
context in the native `<command-args>` replay receipt. The model sometimes hid
this contamination in its answer; marker-only assertions gave a false pass.

`UserPromptSubmit` preserved arguments and its context survived native resume,
including after deleting the hook file. However, the October 7 continuity probe
found that Claude 2.1.293 persists this attachment with `renderedRole: "system"`.
The earlier marker check missed this authority change. The hook cannot carry
repository references under the current design's constraint below.

The current IDE deep links carry only folder/path and prompt, with no separate
context or launch-settings channel. Forcing terminal launch would violate the
selected surface; appending context violates argument fidelity. A discovery and
prompt-formatting cut still cannot meet the full replacement requirement.

The revised design needs one launch input shared by headless, terminal and IDE:
selected source/dialect, exact invocation arguments, bounded context and recorded
provenance. Capture must retain both the invocation and separately delivered
context, while provider history establishes native expansion. Temporary context
files must survive the provider that reads them, including native handoff and
resume. Never promote repository reference material into system instructions to
make slash parsing succeed.

Codex 0.160.1 `turn/start` now has direct probe evidence: an explicit `skill`
item, exact invocation and separate user text context survive `thread/read` and
a process restart into `thread/resume`. Native history contains user-role skill
expansion with the selected path and full file bytes. Its experimental schema
also exposes `additionalContext` on start/steer; that field was not exercised and
is unnecessary for the proven separate-text path. Production still sends text
only; no Loopflow capture or active-turn steering proof follows from this probe.

Native IDE handoff is narrower than previously known. Official Claude docs expose
`claude --desktop --resume ID`; Codex documents `codex://threads/ID`. Claude
Desktop 1.15962.2's installed URL handler forwards only the new-session prompt and
folders. Its resume handler imports a CLI session by ID, without launch settings
or a new prompt. The import function calls `stripThinkingBlocksFromFile` on the
native transcript; the named call is source evidence, not an observed mutation.
Neither handler supplies the missing context transport. Creating and opening a
native conversation remains a candidate, not an accepted implementation or GUI
proof. No app was opened. Claude bundle SHA-256:
`33b378efe3eeade5cf20a6dbcf13027fc8aadcb56aacf4454ce70c19259731bf`.

Existing-conversation ownership is already present: `commands/run.rs::resume`
reloads the Session's stored skill; `continue_conversation` reserves a new input
under that Session and `begin_capture` claims its driver. The internal resume
field is not a public skill selector. `Harness::send_input/send_current` accept
strings, so exact skill provenance and separate context need to traverse those
existing owners. Do not replace this path with another Session or an independent
writer attached to a live provider. A caller choosing a different skill in an
existing conversation still needs an explicit invocation surface and admission
proof, including preserved Work attribution and current-driver authority.

Return to authored design review: select a Claude context channel that preserves
user-level reference authority and exact native arguments, then specify native
IDE delivery and existing-conversation selection. Hook context would require
changing the explicit authority constraint; that change is not selected. Neither
headless pre-execution before opening an IDE nor persistent provider configuration
has been selected. No requirement is waived, and lack of a display is not the
reason to stop dependent production work.

## Delete — do not maintain

These cuts require the replacement launch path; the retained probe replaces none
of them. Keep the production cut together after resolving the boundary above.

- `lf/discovery.rs`'s separate external-source catalog, rams alias, fuzzy npx
  lookup/fetch path, and recursive Markdown name scan. Already installed npx
  skills remain discoverable as ordinary `.agents/skills` folders.
- `engine/flow.rs`'s separate Markdown resolver and late `.agents` fallback.
- Exclusive rams/npx fetch fixtures; preserve precedence, nested-definition
  error, read-only listing and installed-skill behavior on the unified catalog.
- The builtin IDE path which invokes potentially stale exported builtin text;
  preserve context delivery and the existing launch surface.

## Remaining implementation and acceptance

1. Resolve Claude's reference-authority conflict and the shared launch-input/IDE
   handoff boundary above. Preserve the proven Codex typed-skill transport; prove
   Loopflow capture, same-conversation admission and live steering separately.
2. Replace discovery end to end, including export's source collection and help.
3. Carry skill provenance through preparation and retained Flow instructions;
   implement native launch and translated ports without parallel resolvers.
4. Prove unchanged third-party skill fixtures on both providers, including
   bundled references, arguments and declared controls. Keep configured provider
   evidence separate from deterministic transport fixtures.
5. Compare plain and lf startup/context costs and same-conversation use with
   repeated, matched baselines. Count context fetched during the turn as well
   as launch tokens: Intelligence's context-ablation pilot found that omitted
   material gets read from disk, and did not measure resumed history. Remove
   regressions before claiming the strict replacement outcome.
6. Build changed code and run focused tests; affected suites and full
   headless acceptance remain gate-owned. The probe does not satisfy them.

## Evidence

Source inventory at `626789dcd`: loading has a late repo-only `.agents/skills`
fallback that listing misses. Frontmatter retains only lf `agent`,
`default_agent`, `action_style`; other declarations are discarded. Full prompts
inline skill text and lose the source directory. Codex's transport currently
sends text input only. Claude terminal launch explicitly documents positional
slash invocation as unreliable. The incomplete production cut was removed after
review exposed the surface/fidelity conflict.

Installed lf 0.13.9 in a disposable ordinary directory reproduced the discovery
gap: `help` cannot resolve `.claude/skills` or `.codex/prompts`; `.agents/skills`
loads by name but `list skill --json` omits its bare name. This is installed
baseline evidence, not a candidate pass.

Claude 2.1.293 and Codex 0.160.1 ran native synthetic skills without conversion.
The [probes](../scripts/benchmarks/skill-invocation/README.md) own reproduction
instructions and limits; the original standalone run passed at `1f6e744c3`.
October 7 continuity receipts: Codex thread `01a11922-4b62-7011-9213-ce33542394a3`
retained two turns and unchanged first-turn history. Claude Session
`a162c8e3-4001-43fe-af9b-d6af17181c38` retained three exact invocations and its
original context attachment `969304e2-caf6-4596-894c-db6dd955b866` after deleting
the hook file. Its first answer/reasoning did not echo the context marker; the
resumed answer recovered it. Reinstalling the hook appended one attachment.
Both attachments have system rendering authority. Provider histories retain the
receipts; temporary skill/hook fixtures are removed. This proves neither hook
isolation across unrelated Sessions nor native GUI continuation.

Review finding: a successful provider exit and a plausible final answer cannot
prove native dispatch or faithful argument delivery. Provider input receipts are
the required evidence at that boundary. The probe now reads answers and receipts
in one pass, keeping them separate in its observations and acceptance checks.
Release's child memory records the same distinction between an operation's
outcome and its reporting process's success. Current production source still
matches the inventory above.

Check: `uv run ruff check scripts/benchmarks/skill-invocation` and `uv run pytest scripts/benchmarks/skill-invocation/test_continuity.py -q` pass (4 tests); `continuity.py --provider codex/claude --executable PATH` passes native transport checks, exposes Claude's system-role conflict; product implementation and gate remain open.

Protocol references: [Codex skill input](https://learn.chatgpt.com/docs/app-server#skills),
[Claude skill declarations](https://code.claude.com/docs/en/skills),
[Claude SDK dispatch](https://code.claude.com/docs/en/agent-sdk/slash-commands),
[Claude desktop handoff](https://code.claude.com/docs/en/desktop#coming-from-the-cli),
[Codex deep links](https://learn.chatgpt.com/docs/reference/commands#deep-links).
