# What goes in the system prompt

Status: PR #1518 approved by Jack Heart on October 10 after clipboard/excerpt fixes
(comment `96ee2917`). Other unselected defaults remain proposals. Jack's subsequent LOO-444 steer authorizes
building proposed defaults and naming them at demo, not changing accepted channels.

Channel contract resolved (Jack, 2026-10-09, "option B"): the added-instructions
slot is byte-identical within a launch profile, and surface instructions,
participant name and reply guidance stay in it. A launch profile is the surface,
the reply settings and the participant; two launches sharing all three send the
same bytes. The launch cut now wires fixed additions and file/excerpt callbacks; the native
remaining provider/resume/compaction coverage is disclosed below. Jack's later `e1fdb81b-75ff-4cca-b789-243ad84bad14` steer authorizes
that production cut. Option A below resolves first-turn transport; no fixed-slot
or transport decision remains open.
Reconciled on October 10 with main `df5169ab9` (#1521), including #1519/#1520's
AgentProcess ownership. Clipboard/excerpts and source/callback simplifications
remain at `95e6703d9`, `b7300bf0c` and `8951376e5`; native evidence is below.

The earlier callback-only review describes the pre-production checkpoint.
`494210e37` connects the launchers and removes inlining/overrides;
`d3c01464e` reduces callbacks to saved delivery and preserves OpenCode resume
settings. Source inspection confirms those changes, not native acceptance.
Remaining native coverage is disclosed below, not a delivery blocker under Jack
Heart's October 10 steer. Proposed UX remains identified for review.

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
snapshots are removed. Main #1521 (LOO-449), now integrated, removes `wave_documents` and `lf wave edit`;
Wave configuration also reads the checkout. This supersedes the stored-Wave
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
clipboard suffix changes on that path. Remaining native coverage is disclosed below.

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

Focused block checks exist; these automated checks cover construction and callbacks,
not the outstanding native-provider matrix (one Cargo filter per invocation):

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

### Remaining coverage and PR notes

Jack Heart's October 10 comment `96ee2917` records real branch launches outside
sandbox: Codex headless, Claude headless and Claude interactive received the repo
guide, a whole small scratch file, a marked 50 KB-file excerpt and the request in
`lf:message`. Jack attributes the earlier `Operation not permitted` to the sandbox.
This is reported native launch evidence, not a gate rerun or installed acceptance.

Interactive Codex, manual/automatic compaction, resume/replacement and OpenCode
remain unverified. Jack explicitly made these disclosures nonblocking. Native
AgentProcess takeover, pointer reads, cmux's 200 KB presentation and resume-list
judgment are not established by callback or request-construction fixtures.
Linux-only planning reconnect stays with CI on this macOS checkout.

The excerpt rule remains the operator's interpretation of Jack's “normal snippet
+ block thing,” recorded for confirmation at review. Overflow listing/skill pointers
and the OpenCode native-guide interpretation remain proposed defaults.

Main `df5169ab9` supplies the single checkout-file Wave reader and configuration;
this branch's duplicate reader is removed. Preserve main's path ordering and
missing-directory/error behavior. The terminal callback fixture refreshes Wave and
scratch bytes without the removed document cache.

Earlier isolated hook-trust and sandbox evidence remains at
`7b28ab1ca:scratch/what-goes-in-system-prompt.md`, Remaining work.

Review moved capture-hook composition back to terminal spawn so lower callers
cannot lose native identity capture, connected OpenCode's generic terminal runner,
removed duplicate additions and excluded sibling context. Wave and ancestor reads
follow checkout paths under Jack's later source decision. No installation, publication or landing is claimed.
The pre-cut implementation plan and complete older checks remain at
`6149952c8:scratch/what-goes-in-system-prompt.md`.

Gate review found stale old-channel expectations in Flow, startup, resume and
context DTO fixtures. Fixtures now inspect first-turn text and execute emitted
callbacks at provider turn time, retaining references before later scratch edits.
The actual resume regression was an unconditional manifest read: missing optional
capture payload blocked a still-valid native conversation. Resume now preserves
native history when that file is absent; malformed saved data still reports errors.
Existing missing-manifest launch/resume coverage exercises both paths.

The initial materialized run overlapped a source-tree Cargo rebuild, replacing its
shared `target/debug/lf` with a different migration frontier. Its affected failures
are invalid verification evidence; serial materialized reruns own the result.
`TESTING.md` now forbids overlapping builds on that shared target.

Python's four prune cases read the installed registry despite `LF_HOME` and fail
on its absent `planning_completed` column; no installed-store writes or repair
were attempted. The nested macOS sandbox observation returns `Operation not permitted`.
These five host-dependent cases stay deferred to isolated CI; the full Python run
is not green. Architecture-map drift was fixed and its 21 tests pass.

Checks (October 10): `scripts/test.py --reuse-passing` ran affected suites once (initial materialized Rust 2,284/2,392; 108 failed, then serial `materialize_rust_tests.py -- cargo nextest ... -E <failed cases>` passed 105/108 and the final repaired 3/3); focused integration 22/22, architecture pytest 21/21, benchmark/alignment pytest 38/38 and Node pass; website 76/76, headless Swift 410/410 plus boundary check pass; `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, `git diff --check` and `lf context --json` pass. Python initial 402/408: architecture repaired; four installed-store prune cases and one nested-sandbox case deferred to isolated CI, alongside Linux-only planning reconnect. Logs: `.lf/tmp/gate/run-97150/`, `/tmp/loo444-materialized-repairs.log`, `/tmp/loo444-resume-repairs.log`; no single all-green gate or installed/native-matrix acceptance claimed.

### Integrated upstream boundary

Main #1519/#1520 supersede #1512's driver/engine vocabulary and ownership.
An LfSession owns the conversation; AgentSessionId names native history;
AgentProcess names the provider OS process, separately from its attached LfProcess.
Codex can adopt a saved AgentProcess connection under the current attachment claim.
Its lifeline and provider history are distinct from that attachment; LOO-447 owns
Claude/OpenCode takeover and stop. Context delivery must cover supported Codex
handoff and dead-provider replacement, not restore the deleted driver-generation model. Live AgentProcess adoption
and dead-provider replacement are separate acceptance cases. Saved sources and hook trust must survive
those transitions without replaying the initial skill/request. The current
app-server start/resume request carries the generated hook configuration; source
inspection establishes wiring only, not native refresh on a live takeover.

The review-requested ordering repair is implemented in `6b8f13e3f`; the earlier
return-to-implement feedback is resolved at the request-construction boundary.
It puts the explicit native skill reference before
request/clipboard text in `CodexHarness::send_input`. Its focused regression
checks the first outbound turn, exact arguments, original declaration/asset paths
and a later turn without replay. This proves app-server request construction,
not native expansion, delivery or resumed-provider acceptance; gate retains those
boundaries. Review kept the existing seed owner and native resolver unchanged.

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

The predecessor is removed: `INITIAL_TURN_PROMPT`, system-channel file assembly,
launch budgeting/bounding, stored-Wave context lookup/snapshots, the installed-skill
channel split, Codex instruction overrides, clipboard references and rejected
editor/paste/stdin transport runners. Do not restore them. Complete deletion and
fixture history: `95e6703d9:scratch/what-goes-in-system-prompt.md`, this heading.

Keep one saved `ContextDelivery` callback path. Preserve size-target reporting,
prompt logs, source escaping, native declarations/assets and first-turn bytes.
Excerpt selection now indexes only nonempty UTF-8 prefixes within the remaining
byte budget, avoiding whole-source boundary allocation and empty-prefix cases.
Whole-file selection still precedes excerpts; metadata and escaping count in full.
Review corrected the CLI guide's obsolete promise of whole GOAL.md delivery and
the terminal fixture's stale assertion that oversized scratch has no preloaded text;
it now requires a marked excerpt while retaining the complete source.
Cross-repository documents now keep readable source paths instead of display-only
`[repo] path` labels; repository memory stays only in the refreshed-source list,
not a second saved reference. The terminal fixture executes the emitted Codex
callback strings, deleting timestamp-based descriptor selection and reconstructed
commands. These source/callback checks do not establish native delivery.
No remaining predecessor deletion is identified; native acceptance remains above.

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
