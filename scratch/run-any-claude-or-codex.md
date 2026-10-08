# Run existing skills on either harness

LOO-420. Draft returned for design review, 2026-10-07. Jack Heart selected native
invocation for skills belonging to the selected harness, translated ports for
the other harness, and inlined Loopflow builtins. Jack requested the first
workflow step and no landing. No production implementation is retained.

## Required behavior

`lf audit` discovers `.claude/skills/audit/SKILL.md` without `.lf` configuration.
Claude receives its native invocation; Codex receives a translated port. Listing
and help identify the same selected source as execution. User skill files and
their bundled assets remain unchanged. Unfamiliar declarations become visible
limitations with runnable instructions, not silent omissions or load failures.

## Owners and cut

One engine skill catalog owns roots, names, precedence, source paths and format.
Repository definitions precede personal definitions, then embedded builtins.
Within each scope: `.lf/skills`, `.claude/skills`, `.claude/commands`,
`.agents/skills`, `.codex/skills`, `.codex/prompts`. Explicit provider home
overrides must select the same files as their harness. Generated Loopflow exports
never override current embedded instructions. Bundled Markdown is not a second
skill. Namespaces and filesystem cycles need deterministic handling.

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

`run_prompt` passes `AgentConfig` to the headless harness, but terminal/IDE
`launch_session_with_env` receives only a prompt string. Claude native dispatch
recognizes one command text block. Prefixing context or sending two text blocks
bypassed native expansion. Appending context expanded the skill but placed that
context in the native `<command-args>` replay receipt. The model sometimes hid
this contamination in its answer; marker-only assertions gave a false pass.

`UserPromptSubmit` hook context preserved the original argument in the configured
Claude stream-json probe. The current IDE deep links carry only folder/path and
prompt, with no separate context or launch-settings channel. Forcing terminal
launch would violate the selected surface; appending context violates argument
fidelity. A source change limited to discovery and prompt formatting therefore
cannot meet Jack's strict replacement requirement across the existing surfaces.

The revised design needs one launch input shared by headless, terminal and IDE:
selected source/dialect, exact invocation arguments, bounded context and recorded
provenance. Capture must retain both the invocation and separately delivered
context, while provider history establishes native expansion. Temporary context
files must survive the provider that reads them, including native handoff and
resume. Never promote repository reference material into system instructions to
make slash parsing succeed.

Unresolved: identify and prove a native IDE handoff that carries context without
rewriting the skill, changing arguments, forcing a terminal, or introducing a
hidden model turn. Creating a provider Session and opening its existing native
conversation is a candidate to investigate, not an accepted implementation.
Existing-conversation invocation also needs an explicit dispatch path through its
current driver; a new Session or copied transcript cannot establish continuity.
Jack has not waived either requirement. This is a design-review return, not a
claim that unavailable GUI testing blocks ordinary implementation.

## Delete — do not maintain

- `lf/discovery.rs`'s separate external-source catalog, rams alias, fuzzy npx
  lookup/fetch path, and recursive Markdown name scan. Already installed npx
  skills remain discoverable as ordinary `.agents/skills` folders.
- `engine/flow.rs`'s separate Markdown resolver and late `.agents` fallback.
- Exclusive rams/npx fetch fixtures; preserve precedence, nested-definition
  error, read-only listing and installed-skill behavior on the unified catalog.
- The builtin IDE path which invokes potentially stale exported builtin text;
  preserve context delivery and the existing launch surface.

## Remaining implementation and acceptance

1. Resolve the shared launch-input and IDE handoff boundary above. Prove the
   Codex app-server skill item and capture/replay of separate native context.
2. Replace discovery end to end, including export's source collection and help.
3. Carry skill provenance through preparation and retained Flow instructions;
   implement native launch and translated ports without parallel resolvers.
4. Prove unchanged third-party skill fixtures on both providers, including
   bundled references, arguments and declared controls. Keep configured provider
   evidence separate from deterministic transport fixtures.
5. Compare plain and lf startup/context costs and same-conversation use. Remove
   regressions before claiming the strict replacement outcome.
6. Build changed code and run focused tests; affected suites and full
   headless acceptance remain gate-owned. The probe does not satisfy them.

## Evidence

Source inventory at `626789dcd`: loading has a late repo-only `.agents/skills`
fallback that listing misses. Frontmatter retains only lf `agent`,
`default_agent`, `action_style`; other declarations are discarded. Full prompts
inline skill text and lose the source directory. Codex's transport currently
sends text input only. Claude terminal launch explicitly documents positional
slash invocation as unreliable. Scratch was empty on entry; no prior accepted
design artifact was supplied. The incomplete production cut was removed after
review exposed the surface/fidelity conflict; existing production behavior is
unchanged.

Installed lf 0.13.9 in a disposable ordinary directory reproduced the discovery
gap: `help` cannot resolve `.claude/skills` or `.codex/prompts`; `.agents/skills`
loads by name but `list skill --json` omits its bare name. This is installed
baseline evidence, not a candidate pass.

Claude 2.1.293 and Codex 0.160.1 ran native synthetic skills without conversion.
The repeatable [probe](../scripts/benchmarks/skill-invocation/README.md) records
provider versions, model-output markers and Claude's native argument receipts.
It does not prove third-party fidelity, permissions, model selection, bundles,
app-server, IDE handoff, lf capture or current-conversation behavior. Single-shot
elapsed times cannot prove startup parity with an existing slash-command session.

Review finding: a successful provider exit and a plausible final answer cannot
prove native dispatch or faithful argument delivery. Provider input receipts are
the required evidence at that boundary. Release memory's misleading green cron
receipt lesson applies; release-specific state is unchanged.

Check: `uv run ruff check scripts/benchmarks/skill-invocation/probe.py` and
`uv run python scripts/benchmarks/skill-invocation/probe.py --claude /Users/jack/.local/bin/claude --codex /Users/jack/.local/bin/codex` passed; product implementation and gate remain open.

Protocol references: [Codex skill input](https://learn.chatgpt.com/docs/app-server#skills),
[Claude skill declarations](https://code.claude.com/docs/en/skills),
[Claude SDK dispatch](https://code.claude.com/docs/en/agent-sdk/slash-commands).
