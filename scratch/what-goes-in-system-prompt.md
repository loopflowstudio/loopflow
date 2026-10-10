# What goes in the system prompt

Status: draft. Decisions marked accepted are Jack's, 2026-10-09, in the design
Session. Everything else is proposal. Jack's subsequent LOO-444 steer authorizes
building proposed defaults and naming them at demo, not changing accepted channels.

Channel contract resolved (Jack, 2026-10-09, "option B"): the added-instructions
slot is byte-identical within a launch profile, and surface instructions,
participant name and reply guidance stay in it. A launch profile is the surface,
the reply settings and the participant; two launches sharing all three send the
same bytes. The launch cut now wires fixed additions and file/excerpt callbacks; the native
provider/resume/compaction matrix remains for gate. Jack's later `e1fdb81b-75ff-4cca-b789-243ad84bad14` steer authorizes
that production cut. Option A below resolves first-turn transport; no fixed-slot
or transport decision remains open.
Reconciled through `17c1a2909` on 2026-10-09 against locally available main
`906576f39`, including `3e1e6245c` (#1512) and `e69d5103f` (#1511). No newer
remote inspection is claimed. `5e8fdde2f` implements the checkout Wave-source
decision; `17c1a2909` removes obsolete SQLite fixture setup while retaining
stale-store counterexamples at the callback and CLI boundaries.

The earlier callback-only review describes the pre-production checkpoint.
`494210e37` connects the launchers and removes inlining/overrides;
`d3c01464e` reduces callbacks to saved delivery and preserves OpenCode resume
settings. Source inspection confirms those changes, not native acceptance.
The remaining work below is integration proof and review of proposed UX,
not another channel cutover.

First-turn transport resolved (Jack, 2026-10-09, "A is fine"): the first turn
stays a command-line argument on terminal launches. No editor, paste or stdin
transport is built. A first turn that would exceed the terminal's argument cap
fails before launch with a message naming the size and the cap; that is the one
permitted size refusal. The editor and paste probes are evidence only and are
not a production path. No further transport probes are authorized.

Wave source resolved (Jack Heart, 2026-10-09, comment
`cbc13fa5-0a1e-4dec-b638-52b29589edc1`): launch context and context callbacks read
Wave Markdown from this repo checkout, with ancestors by path segment as before
#1503. The saved delivery keeps the Wave name, not its database ID; listings name
real files. `gather_saved_wave_docs`, callback Wave-ID lookup and generated Wave
snapshots are removed. The `wave_documents` table, `lf wave edit` and other
readers are unchanged; LOO-449 owns their removal. This supersedes the stored-Wave
requirements and rename/name-reuse proof at `8b41e0327`.
Jack Heart resolved clipboard placement on October 10: skill, message, then tagged
clipboard text, counted toward the argument cap. Shrinking Wave memory remains open.
The October 10 snippet direction supersedes whole-or-listed delivery for oversized files.

## October 10 narrow cut and PR notes

Jack Heart's `c8e28b6c` selects clipboard in the first message and “the normal
snippet + block thing” for oversized memory. **PR review note:** interpreting
that phrase as marked start excerpts with path, full byte size and an instruction
to read the rest is the operator's interpretation, for Jack to confirm at review.
Small files remain whole. Listing/skill reservation and the 10,000 rendered-byte
cap remain; whole files are selected first, then excerpts in existing priority
order. No clipboard file reference remains. The saved compact-only active skill
and overflow listing retain their complete-pointer behavior.

Delete: clipboard private-reference writer and whole-or-listed-only assertions.
Preserve: reference escaping, native skill selection, first-turn order/cap,
current checkout reads, complete manifest and native hook ownership.
Review caught a native-skill message-wrapper change; it was removed so only the
clipboard suffix changes on that path. Native integration remains with gate.

## Problem

Jack wants meaningful native resume titles and current, compactable context,
without restoring giant terminal messages or cmux arguments. Loopflow must add
to the harness's own instructions, not replace them. The original design transcript
and out-of-scope search measurements remain in the preserved reconciliation input.

## What to build

Loopflow launches an agent as a guest in its harness: a small fixed
instruction slot, the skill and request as the first turn, and one capped
context block injected into the conversation at start and after compaction.

The harness keeps what it owns: its built-in prompt, its loading of the repo's
agent guide, its compaction, its titles.

| Content | Channel | Status |
|---|---|---|
| Operating guide, surface instructions, user name, reply guidance | Added-instructions slot, byte-identical within a launch profile (surface, reply settings, participant) | Accepted |
| The skill, then any launch message | First user turn | Accepted by Jack's later option-A steer (`c9b19504`): skill then message |
| Paths to `scratch/` and `wave/<address>/`; as much of Wave memory and scratch as fits; a listing with sizes for the rest and a line saying to read it | Context block | Accepted: "This seems fine" |
| The active skill, again | Context block, after compaction only | Accepted: "I like this" |
| Diff, `-d` docs | Not inlined; command or paths | Proposal |
| Clipboard | Tagged block after the message in the first turn | Accepted October 10 |
| Summaries, steers, Task brief | Complete private files under `.lf/prompts/`, referenced from the block | Proposed default |

Context block fill rule: preload small files whole, then use remaining bytes for
marked start excerpts of omitted files; retain all complete paths in the listing. Order: the Wave's `MEMORY.md`, then `scratch/<branch>.md`, then other
scratch files smallest first, until the cap.
Reserve space for paths, the complete listing and (after compaction) the skill
before choosing file bodies. Excerpts carry source path, full UTF-8 size and a
read-the-rest instruction; their fully escaped bytes count toward the cap. A listing or skill can itself exceed the cap:
complete manifest/saved-skill files with pointers are a proposed extension,
not accepted replacement of the promised listing/skill. Pointer-read behavior
and acceptance of the overflow pointers remain unresolved in `scratch/questions.md`;
the instruction-slot channel is settled by option B.

## The demo

`lf design "fix the flaky gate"` inside cmux, in a worktree with 200 KB of
scratch, on Claude and on Codex. The chat opens with the design skill and
"fix the flaky gate"; `codex resume` lists real text; the agent already knows
the Wave memory and sees the scratch listing; after `/compact` the skill and
current scratch are back. On Codex the native base instructions remain alongside the added developer
instructions; test preservation rather than a model-specific opening sentence.

`AgentConfig.system_prompt` holds only additions; `task_prompt` holds skill/request.
The context-block operation carries paths, whole documents or marked excerpts, listed UTF-8 sizes
and the compact-only saved active skill. Native delivery is connected; its full behavioral matrix remains unproved.

## Key functions

- `ContextDelivery::block(moment) -> ContextBlock` rereads the captured checkout
  and its Wave files by captured name; `moment` is start or after-compaction.
- `lf __context-block --delivery <saved-file> --moment <start|compact>` is the
  single hidden callback used by launchers and CLI fixtures. No ad-hoc source mode.
- Claude/Codex: `SessionStart` with startup and compact sources. Codex
  `PostCompact` is not a context-delivery hook.
- Codex app-server: additive `developerInstructions`, with session-scoped hook
  configuration/trust; the TUI bypass flag does not cover this surface.
- OpenCode: the same fixed `system` addition on each owned user turn, not only
  the first. The launch-scoped plugin supplies conversation context on terminal
  and server paths; native startup/compaction acceptance remains unproved.
- Wave content: read `wave/<name>/` Markdown from the checkout each time,
  including each path-segment ancestor, never siblings or children. Listings
  point to the real files; scratch also reads current local files.

## Constraints

- No Loopflow-added repo or scratch text in a provider system prompt. This
  interpretation preserves harness-owned guide loading; OpenCode's native guide
  placement creates a literal conflict with the broader wording, still needing
  review. Moving context alone does not prove safe first-turn transport.
- Added instructions add to the harness's prompt, never replace it. Codex moves
  from `model_instructions_file` to `developer_instructions`.
- No launch argument approaches cmux's 122,880-byte cap.
- One rule for interactive, headless and persistent launches. Where lf owns the
  message stream it sends the same block under the same cap.
- The cap is the harness's own effective limit, not a config number. Count
  provider units, not just Unicode scalars: both harnesses spill a 10,000-scalar
  emoji block. Whole-file/excerpt selection must prevent that native truncation too.
- Limits stop deciding what is sent. `memory_tokens` and `scratch_tokens`
  remain as size targets; `lf context` measures the folders against them.

## Done when

Focused block checks exist; complete-channel checks remain with gate (one Cargo
filter per invocation):

```sh
cargo test -p loopflow process_prompt
cargo test -p loopflow --test context_launch_tests
cargo test -p loopflow --lib context_block
cargo test -p loopflow --test context_block_tests
cargo test -p loopflow generated_titles
cargo test -p loopflow native_hooks_name_requests
```

Required behavior through lf's actual launch paths, not only the native probes:

- Claude and Codex start in cmux with 200 KB scratch and arguments comfortably
  below 122,880 bytes; first turn and native resume text contain skill then request.
  Large skill/request text must not simply move the oversized argument elsewhere.
- Added instructions are byte-identical across launches of one profile; no Loopflow-added file
  bodies or request appear there. Codex retains its native base and AGENTS.md,
  with no `model_instructions_file` override on terminal or app-server paths.
- Startup and manual/automatic compaction deliver current whole documents, marked excerpts or
  readable complete listings; compact refresh also restores the active skill.
  Boundary cases include Unicode, many files and an oversized listing/skill.
- Supported resume/engine replacement retains native Session identity, refreshes
  context and hook trust, and never resends the initial skill/request as new work.
- Every supported surface uses this contract without truncation or
  an old/new switch; the only size refusal is a first turn over the argument cap. `lf context` reports size targets without gating launch.

Headless gate owns model-request/transport checks; demo owns cmux presentation
and native resume-list judgment. Native probes alone satisfy neither boundary.

## Production cut and remaining acceptance (2026-10-09)

The old `format_content_sections`, `format_wave_sections` and file-body diagnostic
assembly are deleted. Added instructions contain the operating guide, surface and
participant; structured-reply guidance stays in the same slot. Native skill choice
no longer selects a separate context channel or suppresses operating guidance.
The explicit operating-guide opt-out remains, not an old/new delivery switch.
Codex terminal and app-server inputs use additive instructions; no production
`model_instructions_file` writer remains. First-turn argv and its one size refusal
are unchanged.

`ContextDelivery` captures repository, Wave name, Machine, complete active skill
and reference paths. Task seed/steers and summaries are private complete
files; clipboard is a tagged first-turn block, not a reference file; explicit docs remain paths, and changed-file context points to Git inspection.
The compact-only saved skill identifies the original asset directory. The existing
10,000-byte block/manifest builder rereads scratch and checkout Wave/ancestor files.
Metadata/skill overflow pointers remain proposed UX, not Jack's acceptance.
Diagnostics list sources and sizes instead of recreating file-body assembly.

Claude launch settings install SessionStart startup/resume and compact callbacks.
Codex terminal flags and app-server thread start/resume use the same declarations
and exact session-flags trust hashes. They preserve other settings and do not
publish global trust or bypass all hooks. The terminal spawn operation composes
capture and context hooks together, including callers below ordinary skill launch;
this replaces the temporary capture profile and its trust-flag parser probe.
Callback commands explicitly name the saved source descriptor and Machine; they
need no inherited Wave or Task environment. Sources remain private files after the
driver exits. Captures retain inputs on terminal as well as headless launches;
native resume restores settings/additions, never the initial request. Explicit
replay remains headless-only. Headless continuation retains the saved active skill.

OpenCode uses a launch-scoped native plugin on terminal and server paths. Its
conversation transform supplies startup context, detects the native summary after
compaction and refreshes the block; its system transform adds only the fixed slot.
The plugin retains the first owning conversation and excludes sibling Sessions.
Existing provider config/plugins are merged, not overwritten. Node checks cover
refresh, duplicate-addition avoidance, preserved native text and sibling isolation;
this is not native OpenCode integration acceptance.

### Remaining work

- The terminal launch fixture uses stand-in providers and invokes the emitted
  callbacks separately. The headless Codex fixture observes thread/turn inputs,
  not native hook execution. These establish wiring and refreshed callback output,
  not delivery into a native model request. The existing provider-only probes
  remain prerequisite evidence, not substitutes for the matrix below.
- Gate: actual lf/native fake-API startup, manual/automatic compaction, terminal and
  app-server resume/replacement on Claude, Codex and OpenCode; preserve native base,
  guides, naming/capture hooks, permissions, account isolation and first-turn text.
  Exercise explicit native declarations/assets and changed launch fixtures.
- Gate: captured reference/readability and late source changes, oversized Unicode,
  listings and skill; verify exact trusted hashes against supported native Codex.
  Saved delivery rereads the captured checkout and Wave path; native continuation
  remains unproved. Checkout edits, ancestors, real listing paths and stale stored
  documents are covered by focused source/callback fixtures. Wave configuration
  still reads saved GOAL frontmatter; changing that reader belongs to LOO-449,
  not this context-source cut. Checkout content refresh does not prove configuration
  refresh or native hook delivery.
- Demo: cmux's 200 KB launch, native resume-list text, whole-pointer reads and the
  proposed OpenCode native-guide interpretation. No transport probes or new size
  refusals are selected.

An isolated native Codex `hooks/list` accepted all three generated session-flags
hashes (two context hooks and capture):
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo444-trust-ynsnmwqi/`.
The actual lf/native fake-API startup attempt under macOS network isolation failed
before a model request with `Operation not permitted`; adding Unix-socket allowance
did not resolve it. This is unresolved environment/entry-point evidence, not a
provider pass or a proven product cause:
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo444-lf-wire-gcs_tci8/`.
Release's operation-entry lesson applies; callback-only tests cannot replace this.

Review moved capture-hook composition back to terminal spawn so lower callers
cannot lose native identity capture, connected OpenCode's generic terminal runner,
removed duplicate additions and excluded sibling context. Wave and ancestor reads
follow checkout paths under Jack's later source decision. No installation, publication or landing is claimed.
The pre-cut implementation plan and complete older checks remain at
`6149952c8:scratch/what-goes-in-system-prompt.md`.

Checks: `cargo test -p loopflow --lib context_block` (9 passed), `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` and `lf context --json` pass; native integration remains with gate and presentation with demo.

### Integrated upstream boundary

Main #1512 changes engine lifetime, not prompt delivery. Codex/OpenCode engines
now follow driver lifelines; replacing a provably dead driver ends its old engine
and resumes saved native history on a new one. Live handoff retains the lifeline.
The deleted surviving-engine recovery path is not a context-refresh integration
point. Context hooks and their complete source files must remain usable across
supported native resume and engine replacement without replaying the initial
skill/request or restoring the abandoned in-flight turn. Launch-scoped trust
must be established on the replacement engine as well as the first one.
These are preservation requirements for the production cut, not outcomes proved
by the provider-only probes. #1512's source and throwaway-child tests establish
no installed or real-provider lifeline acceptance.

Main #1511 launches lf Codex terminals with `--no-daemon` in
`spawn_session_command_with_env`, after command construction. Hook configuration
must reach that launched process and its selected provider home; configuring only
Codex's shared background daemon cannot satisfy this entry point. Preserve that
account isolation. Earlier native app-server probes do not establish terminal
hook delivery through this path.

Release is the only immediate child directory with memory in this checkout;
its goal and full memory were read. Its operation-entry lesson still applies:
all three Codex instruction writers and actual lf launch/resume paths need
coverage; provider-only passes cannot stand in for that integration.

## Delete — do not maintain

Compression removes the prototype callback's `--repo`, `--skill-file` and
`--reference` mode plus `build_context_block`; all refreshes use saved
`ContextDelivery`. Wave context reads no longer open the store.
`PreparedProcessPrompt.prompt` is removed: diagnostics still use `format_prompt`
on demand, not a second string assembled for every launch. OpenCode config
composition appends its plugin directly to existing settings, without temporary
Command objects or a generic config merge; resume applies caller settings before
appending the plugin so those settings cannot overwrite it. These are source
reductions, not native acceptance.

The config guide's detailed docs/diff/clipboard and third-party sections now match
the channel table: paths or private references, no obsolete preloading, budget
enforcement or implicit operating-guide opt-out. The October 10 cut changes clipboard assembly and context excerpt selection.

The whole channel cut remains one PR; source implementation is not native acceptance. The rejected transport runner `first_turn_transport.py` and its exclusive
`test_first_turn_transport.py` are deleted. Their complete editor evidence/code is
archived at `d81f12c42:scripts/benchmarks/skill-invocation/`; stdin/paste code is at
`d008a9761`. No transport probe remains to maintain or rerun. Shared native Codex
settings in `launch.py` remain used by context-delivery and skill-fidelity probes.

- Removed: `gather_saved_wave_docs`, `ContextDelivery.wave_id`, its database lookup,
  Wave snapshot generation and the exclusive rename/private-snapshot tests.
  Checkout-source fixtures retain ancestor ordering, sibling/child exclusion,
  stale database rejection, refresh and readable real paths. Review also corrected
  these fixtures to inspect gathered documents, not obsolete diagnostic inlining.
- Removed: `context_tests::import_wave` and registry/Git/skill setup used only
  by Wave document filtering. Exact path/content assertions retain Markdown order
  and exclusion coverage; launch and stale-store callback fixtures stay separate.
  The repository-memory assertion now reads the conversation block, not diagnostics.
  `lf context` refresh edits checkout memory while the imported copy stays stale.
  Wave configuration still uses saved GOAL frontmatter (LOO-449), so its CLI fixture
  retains registration: removing it yielded the repo limit 700, not Wave limit 400.
- Removed: duplicate context listing/header metadata and the separate long-root
  fallback. The complete manifest now owns both inline and pointed metadata.
- Removed: `INITIAL_TURN_PROMPT`, its naming special cases and exclusive tests.
  Attributed request selection and existing-name protection remain.
- Removed: `<lf:scratch>`, `format_wave_sections`, `format_content_sections` and their exclusive assembly tests. Diagnostics retain a source inventory, not provider file bodies.
- Removed: context bounding/excerpts, preserved-excerpt store, notice, goal/input
  ceilings and launch-time measurement. `measure_context` owns size-target reporting
  from documents only; no second submitted-source count or skill preview remains.
- Removed: all Codex `model_instructions_file` writers. Both terminal writers and app-server use additive instructions.
- Removed: the installed-skill context split; native invocation, declarations, arguments and captured assets survive.
- Removed: LOOPFLOW.md launch-excerpt guidance and unused
  `SkillInvocation::instruction_text`, formerly used only by launch budgeting.
  Recursive scratch remains available whole, excerpted or listed; translated skill/argument
  coverage remains.
- Removed: tests asserting scratch or memory bodies inside `system_prompt`; surviving checks assert refreshed conversation content.

Must survive: the size targets, `lf context`, `lf monitor usage --context`,
prompt logs under `.lf/prompts/`, source-boundary escaping.

## Forbidden outcomes

- A flag or config choosing between inline and injected delivery.
- The skill or request present in both the slot and the first turn.
- An unmarked excerpt or an excerpt without its complete source path and size.
- Anything in the added-instructions slot that varies between two launches of
  the same profile: Task, Wave, branch, paths, time, skill text, request.
- Surface instructions, participant name or reply guidance moved into the
  context block.

## Follow-ups (intent only; each gets its own design)

- Attribute tool reads to Scratch or Memory in `lf monitor usage --context` by
  the path read. Interactive Sessions need the harness's own transcript: lf's
  capture holds only their launch input.
- Before-compaction write to `scratch/`. Accepted: "Makes sense to do."
  Mechanism open: in a terminal session lf cannot make the agent take a turn.
- Search over past Sessions: `lf session search`, scoped to the Task by
  default, reading harness transcripts; raw excerpts, no model call. Index
  versus scan to be decided by measurement (Jack: "Probably worth performance
  testing this one specifically"); see Evidence.
- Not accepted: native skill delivery; `MEMORY.md` as an index; scheduled
  memory curation.

## Evidence (2026-10-09)

- Codex 0.161.0, lf-launched session: base instructions were Loopflow's
  144,605 characters with no "You are Codex". A plain session's base is 21,420
  characters. `AGENTS.md` arrives separately as a user message.
- Probe: `codex exec -c developer_instructions=…` kept the built-in base and
  added the text as a developer message. One small headless request.
### Prerequisite probes and contrary evidence

`context_delivery.py` in `scripts/benchmarks/skill-invocation/` runs real native
clients against local fake APIs with new Homes and external egress denied. It
checks actual model requests, not assistant claims. Claude excludes resume hooks
so their output cannot pass for after-compaction refresh. Codex proves that
PostCompact ran even though its context never reached the model.

- Claude 2.1.295: startup/compact output is user content, fresh bytes replace
  old bytes after manual compaction. Evidence:
  `/tmp/loo444-context-proof/claude-tx250ian/`. The older 2.1.294 system-field
  observation in the benchmark README remains evidence of that version only.
- Codex 0.161.0 app-server: additive instructions survive manual compaction;
  native base and AGENTS.md remain. Compaction regenerates the addition and
  replaces old hook context with fresh conversation content. The fixture uses
  the model's fallback native base, not a required “You are Codex” prefix.
  Startup/compact SessionStart context is
  developer conversation content; PostCompact output is ignored. Evidence:
  `/tmp/loo444-context-proof/codex-b2te08w4/`. An initial probe with the TUI
  bypass flag ran no hooks; fixture-only per-thread trusted hashes fixed the
  probe without saved config changes. This is not production trust integration.
- [Codex 0.161.0 output spill source](https://github.com/openai/codex/blob/rust-v0.161.0/codex-rs/hooks/src/output_spill.rs)
  sets the default at 2,500 approximate tokens. [Claude hook documentation](https://code.claude.com/docs/en/hooks#json-output)
  caps individual strings at 10,000 characters. Boundary-size behavior
  is exercised below; the earlier short-marker checks did not establish it.
- [OpenCode 1.18.33 request preparation](https://github.com/anomalyco/opencode/blob/v1.18.33/packages/opencode/src/session/llm/request.ts)
  appends `input.user.system` after its own prompt. The [prompt loop](https://github.com/anomalyco/opencode/blob/v1.18.33/packages/opencode/src/session/prompt.ts)
  passes the latest user message, whose `system` comes from that request. Thus
  first-turn-only delivery is not persistent. This is source verification,
  not an OpenCode live wire or compaction proof.

Review found two invalid acceptance shortcuts: a resume hook could conceal a
missing compact hook, and a hardcoded native-base opening depends on model
metadata. The probe excludes resume injection and checks preserved native base
content separately from additive markers. No production consumer is changed.

The probes share decoded message text, fake-API lifetime, native Codex settings
and PTY ownership. Marker checks inspect text, not serialized block metadata;
Unicode and quotes remain literal. Terminal replies scan only newly received
bytes plus a query-length overlap, preserving split/repeated queries without
rescanning the transcript. First-turn assessment reads only the first request;
later requests cannot repair missing launch text. Native guide messages remain
permitted before it. These probes predate Jack's option A and the first Rust cut.
Their argv concern now applies only to the remaining context-delivery writers. Both installed-skill probe entry points
now use the current `-i` selector, not removed `--tui`; gate owns lf entry checks.

### Rejected transport evidence (archived)

Jack's option A supersedes the investigation, not its counterexamples. Codex
0.161.0 exec accepted 225,023-byte stdin; TUI rejected it. Paste changed CRLF and
consumed literal paste terminators; the editor preserved Unicode/CRLF/terminators
but trimmed trailing whitespace (285,027 → 285,023 bytes). No lossless transport
or lf/resume/cmux acceptance follows. No further transport probes are authorized.
Code and complete evidence: `d81f12c42:scripts/benchmarks/skill-invocation/`;
earlier stdin/paste runner: `d008a9761`; all retained result paths and prior
reconciliation notes: `ebdfd89cd:scratch/what-goes-in-system-prompt.md`, Evidence.

### Hook-size counterexamples

`context_delivery.py` now compares the entire injected string at startup and
manual compaction. In `/tmp/loo444-hook-size-proof/`, 10,000 ASCII characters
arrive intact on Claude 2.1.295 (`claude-d0c91f0t`) and Codex 0.161.0
(`codex-85zy66ez`). At 10,000 Unicode scalars / 39,901 UTF-8 bytes, both spill
and truncate (`claude-avatk8kr`, `codex-e1y3mynh`). Codex retains both marker
checks despite cutting the middle, invalidating marker-only acceptance. A
2,500-scalar / 9,901-byte Codex block arrives intact (`codex-dhdmg8w5`). These
samples require provider-unit budgeting; they do not establish every boundary
or automatic compaction. Native spill files do not satisfy whole-file inlining.
The callback implements complete-list/skill overflow with readable pointers;
provider pointer-read behavior and the native delivery matrix remain unproved.
The old assembly deletion list is unchanged.

Historical probe/check results predate the Rust cut, not production verification.
Exact retained JSON paths, earlier scratch copies and compressed-probe receipts:
`ebdfd89cd:scratch/what-goes-in-system-prompt.md`, Evidence. No provider probes
were rerun during compression.
