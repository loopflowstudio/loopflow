# Run existing skills on either harness

LOO-420. Draft returned for design review; reconciled 2026-10-07. Jack Heart
selected native invocation for skills belonging to the selected harness,
translated ports for the other harness, and inlined Loopflow builtins.
Provider probes and documentation are implemented. Jack Heart selected
`--agent` / `-a` on 2026-10-07 and authorized that rename in this checkout.
The selector rename and app-launch removal are implemented locally; native skill
discovery and dispatch remain unchanged.

## Agent selector

`lf -a codex audit` and `lf --agent claude:opus audit` select a harness and
optional model. Replace the misleading `--model` / `-m` selector, including PR
commands, generated child commands and reproducible-command output. Keep config's
`agent` key, provider-native model flags and stored model fields unchanged.
Released-binary fixtures keep the released flag. No migration or alias is needed.
Prove CLI parsing, forwarding through Flow/Session launch and native model delivery.

## Terminal and headless execution (decision, 2026-10-07)

Jack Heart directed removal of all `--ide` code after the installed launch and
reconnect probe. This supersedes the app-surface requirement and the proposed
headless-to-app handoff. Remove the flag, `session.launch`, `ProcessTarget`, app
URLs, shortened native-skill seed, automatic skill export on app launch and the
handoff writer. Keep terminal launch, headless runs, native provider identity and
Session connect. Preserve existing history, including its recorded surface and
handoff events; this removal neither repairs nor deletes old app Sessions.

### Installed app-launch evidence

Jack Heart requested testing `--ide` as a universal launcher, including Process
records and `lf session connect`. Installed lf 0.13.9 (binary digest
`4cf8c9e9a39a916dbe57be9abc3e6eb688ed3ad776ff9243b68537d69592103a`)
ran `lf --ide --model PROVIDER --no-loopflow : MARKER_PROMPT` for Claude and
Codex in separate disposable data directories and ordinary workspaces. These are
released CLI flags, not the renamed candidate flags. Each prompt prohibited tools
and requested only a unique marker. Both launch commands exited zero.

Each store recorded one successful launcher process and one interactive Session,
with null native thread, endpoint, PID and birth time. Claude Session
`session_787c00c79b9142e38145b3b927fe147c` and Codex Session
`session_8896e66e4c034126ab7ec36aca19b899` both projected `unknown` through
`session connect ID --json`. Actual `session connect ID` exited 1 for both:
“Conversation has no connection and no confirmed engine exit; retry requires
exact process evidence or an observed restart of the same host.” No forced
replacement, process termination, or installed-state repair was attempted.

Receipts remain under
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo420-ide-y9bws_o_/`.
The scoped native-history search found no marker transcript in the ordinary
Claude/Codex history roots; visible app rendering or execution was not verified.
Exit zero establishes OS launch acceptance, not a working app conversation.

The removed source explains the missing link: the IDE branch called `begin_provider_spawn`,
opened a URL carrying only folder/prompt, recorded `interactive_opened`, then marked
handoff. It passed neither the capture environment nor a native Session ID to
the app. Terminal spawn owns those receipts. Connect cannot infer an engine's
identity or death from a successful launcher exit.

## Required behavior

`lf audit` discovers `.claude/skills/audit/SKILL.md` without `.lf` configuration.
Claude receives its native invocation; Codex receives a translated port. Listing
and help identify the same selected source as execution. User skill files and
their bundled assets remain unchanged. Unfamiliar declarations become visible
limitations with runnable instructions, not silent omissions or load failures.

## Interaction and recovery proposal

`lf audit "src/payment"` selects one source and preserves `src/payment` as the
native argument. Selecting Codex changes delivery to a translated port of that
same source. Help names the source and delivery mode; it does not fetch or export
anything. A bare invocation needs neither Task nor Flow.

For an existing conversation, propose `lf --session ID audit "src/payment"`
(new syntax, not implemented). Explicit selection keeps its native history and
Work attribution. A command issued by an agent must not implicitly reinvoke its
own busy conversation. The current driver admits the invocation as a new turn;
it queues behind an active turn unless the provider proves native skill steering.
Neither a second provider writer nor a reconstructed conversation is a substitute.
This proposal still needs admission and capture design, including recovery of an
input whose delivery acknowledgement was lost without blindly submitting it again.

Interactive runs stay in the terminal; batch runs stay headless. Both must retain
native conversation identity and capture exact skill invocation and context.

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
Codex app-server accepts explicit skill input alongside text in the retained
probe. Claude's native print transport is proved below; it needs a separate context channel. Existing
Session turns must remain in their conversation; no new execution owner.

Ports need argument substitution, tool/model translation with explicit loss
notices, absolute supporting-file access, and capture of the actual instructions
and invocation. Permission hints are not equivalent to enforced tool policy.
Never overwrite an existing native skill to stage a port. No database migration
is selected. Storage changes must follow the final captured-input representation.

## Architectural counterexample and unresolved boundary

`run_prompt` passes `AgentConfig` to the headless harness. Terminal
`launch_session_with_env` receives model, environment and provider Session ID
separately, but skill invocation and context share one prompt string. Claude
native dispatch recognizes one command text block. Prefixing context or sending
two text blocks bypassed native expansion. Appending context expanded the skill but placed that
context in the native `<command-args>` replay receipt. The model sometimes hid
this contamination in its answer; marker-only assertions gave a false pass.

`UserPromptSubmit` preserved arguments and its context survived native resume,
including after deleting the hook file. However, the October 7 continuity probe
found that Claude 2.1.293 persists this attachment with `renderedRole: "system"`.
The earlier draft treated that rendering field as proof of model authority.
That conclusion is withdrawn: current [Claude documentation](https://code.claude.com/docs/en/glossary#system-reminder)
says a system reminder can appear inside a user message or, on some models, as a
system-role message. The probe captures neither the final request role nor its
mapping. Hook suitability remains unknown; the user-level constraint below stays.

The revised design needs one launch input shared by headless and terminal:
selected source/dialect, exact invocation arguments, bounded context and recorded
provenance. Capture must retain both the invocation and separately delivered
context, while provider history establishes native expansion. Temporary context
files must survive the provider that reads them, including terminal reconnect and
resume. Repository reference material must retain user-level authority.

Codex 0.160.1 `turn/start` now has direct probe evidence: an explicit `skill`
item, exact invocation and separate user text context survive `thread/read` and
a process restart into `thread/resume`. Native history contains user-role skill
expansion with the selected path and full file bytes. Both turns supply the same
context anew: unchanged prior history proves retention, but the resumed answer
does not prove recall without resupply. Its experimental schema
also exposes `additionalContext` on start/steer; that field was not exercised and
is unnecessary for the proven separate-text path. Production still sends text
only; no Loopflow capture or active-turn steering proof follows from this probe.

Existing-conversation ownership is already present: `commands/run.rs::resume`
reloads the Session's stored skill; `continue_conversation` reserves a new input
under that Session and `begin_capture` claims its driver. The internal resume
field is not a public skill selector. `Harness::send_input/send_current` accept
strings, so exact skill provenance and separate context need to traverse those
existing owners. Do not replace this path with another Session or an independent
writer attached to a live provider. A caller choosing a different skill in an
existing conversation still needs an explicit invocation surface and admission
proof, including preserved Work attribution and current-driver authority.

The next technical check is Claude's actual model-input mapping for hook context,
including resumed input. Inspect the selected provider/model's request construction
or a credential-free synthetic request capture; transcript rendering alone cannot
settle it. If it preserves user-level references, the hook remains a candidate.
If it promotes them, another channel is required. Persistent provider configuration
remains an unselected candidate; app handoff is removed from scope by Jack Heart.

## Delete — do not maintain

These cuts require the replacement launch path; the retained probe replaces none
of them. Keep the production cut together after resolving the boundary above.

- `lf/discovery.rs`'s separate external-source catalog, rams alias, fuzzy npx
  lookup/fetch path, and recursive Markdown name scan. Already installed npx
  skills remain discoverable as ordinary `.agents/skills` folders.
- `engine/flow.rs`'s separate Markdown resolver and late `.agents` fallback.
- Exclusive rams/npx fetch fixtures; preserve precedence, nested-definition
  error, read-only listing and installed-skill behavior on the unified catalog.

## Remaining implementation and acceptance

1. Resolve Claude's reference-authority uncertainty and the shared terminal/headless
   launch input above.
   Preserve the proven Codex typed-skill transport; prove
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
Both attachments have system rendering metadata. Provider histories retain the
receipts; temporary skill/hook fixtures are removed. This proves neither hook
isolation across unrelated Sessions nor native GUI continuation.

Review finding: a successful provider exit and a plausible final answer cannot
prove native dispatch or faithful argument delivery. Provider input receipts are
the required evidence at that boundary. The probe now reads answers and receipts
in one pass, keeping them separate in its observations and acceptance checks.
Release's child memory records the same distinction between an operation's
outcome and its reporting process's success. Current discovery and native skill dispatch still
match the inventory above; the app-only launch path has been removed.

The probes share Claude launch/message/hook construction, consume decoded
receipts directly and omit Codex's unused event buffer. Offline regressions
retain contaminated arguments and reject answer-only evidence. Production deletion
targets still require the unresolved replacement; no launch requirement is waived.

Check (2026-10-07): integrated-tree architecture, fmt, Clippy, Ruff, 76 website tests, 414 network-isolated Python/probe tests plus the standalone network-denied macOS sandbox fixture pass; materialized Rust nextest passes 2,284 tests (17 skipped). The initial non-materialized Rust run was invalid; nested macOS sandbox execution failed. Installation and native-skill fidelity remain unproved.

Protocol references: [Codex skill input](https://learn.chatgpt.com/docs/app-server#skills),
[Claude skill declarations](https://code.claude.com/docs/en/skills),
[Claude SDK dispatch](https://code.claude.com/docs/en/agent-sdk/slash-commands),
[Claude desktop handoff](https://code.claude.com/docs/en/desktop#coming-from-the-cli),
[Codex deep links](https://learn.chatgpt.com/docs/reference/commands#deep-links).
