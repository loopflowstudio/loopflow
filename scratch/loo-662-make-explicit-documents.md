# LOO-662 — Make explicit documents reach the agent

## Problem and status

2026-10-27. Jack Heart authorized repairing explicit document delivery and shipping
the fix through the selected workflow. [LOO-662](https://linear.app/loopflow/issue/LOO-662)
owns the accepted outcome; the mechanisms below are the kickoff design, not an
additional product approval. Implementation is checkpointed at
`0a153351613043ba3c80cc0af9a91f1bbfbb78f0`; publication acceptance is recorded at the end of this plan.
The 2026-10-27 reconciliation confirms the delivery cut and retained
preview/capture receipts without finding a bounded code repair. The
earlier Graph stopped because its older lf could not read the upgraded database;
that failure does not invalidate the source receipts or establish installed success.

A person supplying `--docs` expects the contents to be available to the agent.
Installed 0.14.18 showed a 244,852-byte synthetic document in preview, omitted it
from durable submitted context, and produced “not supplied” when Codex was asked
for its final marker without tools. An external absolute file instead panicked.
These are two defects with different causes.

Intelligence owns exact submitted context and evidence. Its Project has no KRs or
metric targets in the Task status response; use the Task's observable acceptance,
not a fabricated coverage metric. Infrastructure owns coordinated release,
installation and final installed verification. This Task does not publish another
release or change released migrations.

## Demo

In a disposable repository, create a short text file and a roughly 60,000-token
UTF-8 file with distinct, unpredictable final markers. Keep one file outside the
repository. For each selection run:

```sh
lf --context --json --docs long.txt skill default 'Report the supplied document’s final marker without tools or edits.'
lf -b -a codex --docs long.txt skill default 'Report the supplied document’s final marker without tools or edits.'
```

The preview's submitted task text and the capture's task channel contain the same
complete rendered document. Codex returns its marker without reading files.
Repeat with an absolute in-repository path and the external absolute path. Prepare a disposable repository conversation with `lf connect --fresh --prepare
--json`, then send it two `lf -b connect ID --message ... --docs ...` turns; the new
turn supplies that document without replaying the initial skill or old document.

## Findings and choices

The following root-cause findings describe the pre-repair source at
`61111f788fc7cfcbdfc8412bb1210a7359fe8cbe` (base release 0.14.19).
The local `origin/main` reference still names that commit at reconciliation;
this is local reference evidence, not a fresh upstream observation. The repair
preserves the skill-frontmatter docs introduced by LOO-655 and leaves LOO-496's
hook transport unchanged.

* `prompt::gather_context` already gathers command/config docs and skill-frontmatter
  docs. `prompt::process::prepare_prompt` merges these sources, then builds
  `AgentConfig.task_prompt` from skill, message and clipboard only. Its native-skill
  branch assembles that text separately, so fixing only `format_first_turn` would
  leave native skills broken.
* `ContextDelivery::prepare` retains `DocumentSource::Docs` as paths, alongside
  saved generated summaries. `block` gathers only ambient documents; `render_block`
  reads explicit references for byte lengths and lists their paths, but never
  inserts their bodies. Even a working hook cannot satisfy the no-tools marker
  request. Raising its 10,000-byte ceiling would not address the 244KB input.
* `commands::run::preview_prompt` independently prints every gathered document as
  a `ContextReference`. Its apparent success says nothing about submitted channels.
  `attributed_context` correctly captures only bodies found in the actual system
  or task text. Adding absent documents to capture alone would falsify evidence.
* `should_exclude` uses `strip_prefix(repo_root).unwrap_or(path)` and passes the
  result to `matched_path_or_any_parents`. The latter requires a path under its
  matcher root. External absolute paths violate that precondition. Preserve the
  basename and `.lf` exclusions, and invoke the repository matcher only for paths
  inside its root. An unrelated repository's `.gitignore` is not this matcher's
  authority. The current-candidate probe succeeds with an empty matcher but exits
  101 with `ignored.txt` in `.gitignore`: the crate returns early for empty rules.
  Regression fixtures must include a nonempty ignore file.
* Cross-repository targets are already converted to absolute readable paths in
  `gather_doc_targets`; keep that behavior. Dedup currently visits ambient sources
  first. Explicitly selecting scratch or Wave memory can therefore lose its
  `Docs` classification. Selection intent must survive same-path deduplication.
* Codex submits text over app-server RPC; the native terminal also uses that
  transport. Its existing large-input launch test covers input larger than this
  document. Claude headless uses stream input and terminal startup uses a file
  on stdin. Pi/OpenCode terminal paths retain explicit size errors; this change
  must not quietly turn those errors into partial delivery or filesystem pointers.
* Headless resume in `commands::run::resume` builds a new task prompt, while
  preferring saved ambient `conversation_context`. Supplying new docs in the
  current task text works through that boundary without rewriting saved history.

Candidate reproduction (2026-10-27): source-built 0.14.19, isolated repository and
Machine at `/private/tmp/loo662-kickoff-v9fez10l`. `long-preview.stdout` contains
FIR-3826 only in `references`, not `task_prompt`. Capture
`a198215fa7a443429ffca21d80c3a4fb/context.json` contains neither the marker nor
`long.txt`. The real headless Codex launch exits successfully and returns FIR-3826
after saying the document was absent from its message. Session telemetry records
one tool result and 60,110 tool-output tokens, versus 21,433 first-request tokens.
This is retrieval after omission, not successful no-tools delivery. Preserve that
counterexample when evaluating marker-only assertions. The external-path panic
receipt is `external-nonempty-ignore.stderr` in the same evidence directory.

**Chosen mechanism (2026-10-27, kickoff):** render explicitly selected documents
as complete, escaped reference blocks in the ordinary submitted task channel.
Keep scratch, repository memory, Wave context, generated summaries and diff
navigation on their existing ambient paths unless explicitly selected as docs.
Use the same document renderer for ordinary and native-skill invocations. Keep
the active skill before the live request; clearly label document bodies as
reference material, preserving `render_reference` escaping so embedded skill
mentions do not become new invocations.

No provider-specific document side channel, database migration, new persisted
document store or extra network call is needed. Files own their current authored
bytes. Gathered `Document` values own the snapshot for that invocation; the
existing capture owns the exact submitted bytes and provenance afterward.
Preview derives its submitted channels from that same assembly. File changes
after submission cannot rewrite captured input.

### Alternatives

1. **Make hooks preload explicit files:** rejected. Shared hook limits are far
   below the accepted document size, resume/compaction has different semantics,
   and hook-only payloads do not enter initial submitted-context capture.
2. **Require the agent to read pointers:** rejected. It contradicts the accepted
   no-tools demonstration and makes delivery contingent on model behavior.
3. **Add gathered files to capture without sending them:** forbidden. It hides
   the defect by claiming context the provider never received.
4. **Put docs in system instructions:** unnecessary authority and more complex
   resumed-turn behavior. The current task channel already carries user-supplied
   reference text and supports large Codex requests.

Success means document selection is boring: CLI, config and skill declarations
all send the same bytes, and an investigator can explain the launch without the
original file. Failure would be duplicate bodies, metadata mistaken for delivery,
or short-file tests passing while long input disappears. Acceptance below targets
those specific failure modes.

## Architecture and deletion cut

Before this change: selection → gathered docs → preview reference bodies; launch → path-only
hook references. Capture derives from the body-free submitted channels.

Implemented target: selection → gathered docs → one shared explicit-document renderer → task
channel → provider transport and exact capture. Preview shows this task channel
and only lists remaining ambient references separately. Hooks keep their existing
ambient refresh responsibilities.

Delete — do not maintain:

* Remove explicit-doc path-only preparation from `ContextDelivery::prepare` for
  newly prepared input. Preserve generated summary references, saved skills and
  diff navigation. Existing serialized `ContextDelivery.references` remain readable;
  do not rewrite old Session records or pretend old launches had document bodies.
* Remove explicit documents from preview's additional-reference collection once
  they are in submitted text; do not display them twice.
* Replace the two independent task-body assembly branches in
  `prompt/process.rs` with shared document inclusion, while retaining native
  `SkillInvocation` and its exact arguments.
* Replace the out-of-root matcher fallback in `should_exclude`; do not add an
  external-file allowlist or refuse valid absolute selections.
* Remove `push_doc_file`'s silent exact-file read path. Exact selections use a
  fallible read; glob traversal owns its filtered reads and empty-selection warning.
* Update tests/goldens that exclusively assert explicit docs are path-only.
  Preserve ambient refresh, hook trust, clipboard, native skills, reference
  escaping, deduplication and read-only preview behavior.

The deletion cut is complete. Selection gathering now returns documents to one
canonical-path deduplication loop in `gather_doc_targets`; separate per-target
cross-repository sets, glob mutation parameters, and the glob's unused fallible
result are removed. Repeated cross-repository directory/glob/absolute selections
count once before the 100-file ceiling. Ordinary/native invocation formatting
shares the reference tail, and resume merges owned context values without cloning
or merging a fresh context into itself. No deletion targets remain.

No `Legacy`/`New` pipeline, duplicate capture asset store, dual delivery, hook-size
increase, arbitrary document truncation or automatic successful-delivery claim.

## Scope and boundary states

Consumers: `prompt/mod.rs`, `prompt/process.rs`, `context_block.rs`,
`lf/commands/run.rs`, preview reference filtering, existing attribution and provider
transport tests. Update `docs/config.md` context guidance and affected prompt goldens.
Preview keeps its existing wire fields; no Swift DTO change or store schema
change is expected. Audit direct `PromptComponents` constructors in tests when
preserving explicit intent through dedup.

Absent docs produce no extra section. Empty UTF-8 files remain valid selections
with a labeled empty body. Exact missing/unreadable/nontext selections must name
the path and reason rather than silently claim delivery; use a normal error for
an exact file that cannot be supplied. Existing glob/directory traversal and
exclusions remain, with visible diagnostics when a selection yields no usable
files. Keep the 100-file ceiling and applicable native-instruction dedup evidence.
Do not broaden this repair into a new exclusion-policy engine.

Canonical/normalized containment must handle absolute paths inside the repository,
`../` paths and symlink aliases without a matcher panic or bypassing exclusions
for an in-root source. External files retain universal exclusions but are not
tested against unrelated root-relative ignore patterns. Preserve cross-repo
matching against the selected related repository before absolute rendering.

If the provider rejects its context window or an existing transport limit, return
that failure with the attempted input preserved. No retries with shortened docs.
The operational target is lossless delivery of the accepted approximately 60k
token input over existing transports, with no added subprocess/network lookup for
document assembly. Hook ceilings remain unchanged.

Excluded: ambient context redesign, broader compaction guarantees, remote telemetry,
provider memory services, document attachment/search APIs, account configuration
changes, unrelated Session repair, release publication and migration edits.

## Concept review — 2026-10-27

**Outcome: nothing to change; no open product choice.** Jack Heart's accepted
no-tools delivery outcome already settles the consequential mismatch: a selected
document must be submitted reference text, not a path whose retrieval depends
on the agent. The implemented model matches that experience. No additional
implementation direction or approval is introduced by this review.

The interaction is `--docs long.txt` → preview the submitted reference → launch
with that reference → inspect its immutable capture. A later
`lf -b connect ID --message TEXT --docs next.txt` supplies the current selection
to that turn. Re-select a changed file to supply its new contents; the earlier
capture remains evidence of the earlier input. Preview and launch each gather
their own snapshot, so equality presumes unchanged files between those commands.

The concepts and APIs are sufficient: `gather_doc_targets` produces `Document`
snapshots; `format_task_prompt` supplies shared ordinary/native reference text;
`preview_prompt` exposes that assembly; `attributed_context` records the actual
submitted channels. Files own authored content, the invocation owns its snapshot,
and capture owns historical evidence. `ContextDelivery.submitted_docs` holds
only canonical identities for ambient suppression, accumulated by `resume`.
Removing those identities would reintroduce duplicate ambient delivery; turning
them into another document store would add an unnecessary lifecycle. Existing
provider transports and capture persistence suffice, without a schema change.

Failure and recovery preserve that model. An exact missing, unreadable or
nontext file fails with its path; fix the selection and invoke again. Excluded
or empty traversals remain visible, and the file ceiling fails rather than
silently choosing a subset. External paths use only applicable exclusions, not
an unrelated root's matcher. Provider rejection remains a failed attempt with
its attempted input preserved; recovery never silently shortens the document.
On supported resume, old documents remain in native history while new selections
enter current task text. Retired or attached Session refusals do not justify
changing Session ownership or claiming a resumed turn ran.

`docs/config.md` already expresses the resulting usage contract. No skill needs
an instruction to retrieve selected documents: requiring that action would undo
the accepted experience. Preserve the existing distinction between active skill
instructions and escaped reference text, native-instruction deduplication, and
ambient-only refresh. No usage or skill rewrite is needed here.

The retained counterexample remains decisive: a correct marker recovered by a
tool does not prove delivery. A differing submitted body, an old capture altered
by a file edit, or replay of an old document into a new turn would invalidate
the implementation. The next proof remains the already specified gate acceptance;
Infrastructure owns installed verification. This review adds no test run or
installed-success claim. The accompanying view is `scratch/concept-review.html`.

## Remaining work and retained evidence

The delivery cut is implemented. Ordinary and native skills share document
assembly; explicit selections win same-source dedup; preview omits their redundant
references. `ContextDelivery.submitted_docs` retains canonical identities only to
suppress ambient hook duplication, including across resumed turns. Historical
serialized deliveries omit that optional field and remain readable. Empty document
wrappers carry source attribution. These path identities are suppression metadata,
not another document payload or evidence of historical delivery. No schema or
provider transport changed.

Acceptance is complete as recorded below. Remaining work follows the selected
delivery workflow; Infrastructure owns installed verification. The focused boundary is
`context_launch_tests::explicit_documents_`; the gate command below includes it.

Gate owns the affected suites and `explicit_documents_*` CLI acceptance tests.
Those regressions are written to cover short/long relative, absolute and external selections,
config/frontmatter docs, native skills, terminal delivery, exact preview/capture/
provider text, immutable captures, and new-turn document selection. The integration acceptance tests pass after the fixture corrections recorded below.

Configured source proof (2026-10-27) is retained at
`/private/tmp/loo662-verified-0fx2pjp5`. Historical probe scripts
`/tmp/loo662-verify.py` and `/tmp/loo662-native.py` retain the initial failed resume
setups described below; they are not passing end-to-end acceptance commands.
Relative and absolute launches
of the original 244,852-byte input each submitted 245,926 task bytes, matching
preview and capture, and returned FIR-3826. First requests measured 78,801 and
78,799 tokens; both recorded zero tool results/output tokens. The external file
returned MAPLE-662-9b1f with matching preview/capture and zero tool output. A real
native terminal also returned OPAL-662-36a9 from its selected document.

The initial resume probes exposed existing lifecycle boundaries: exited standalone
Sessions retire, and attached Sessions refuse another owner. Receipts retain both
refusals. The successful resume proof uses a disposable repository conversation
prepared by `lf connect --fresh --prepare --json`, then two headless turns on the
same native history. Its new turn returned ASPEN-662-c72a, captured the new document
once without the prior document or initial skill, and recorded zero tool output.
All probe providers exited. This changes proof setup, not retirement/attachment
semantics; the regression follows that supported lifecycle. The terminal receipt
uses a Loopflow skill; native-skill invocation remains separately covered by the
focused unit pass and the pending deterministic acceptance test.

Infrastructure still owns coordinated release, installation and installed
verification. No installed-success claim follows from these source receipts.

## Done when / gate acceptance

The decisive scenario starts from a file selection, reads preview, launches the
same invocation, reads its capture, and inspects provider-received text. Exercise
short/approximately 60k-token files as relative, absolute in-root and external
paths; compare complete rendered bodies, first/middle/final markers and exact
captured channel text. Resume with a different document and assert only the new
selection is added to the new turn. Both Loopflow and native skill paths pass.

Regressions are present under the existing `context_launch_tests` module, named
`explicit_documents_*`; this headless acceptance command has a precise target:

```sh
cargo test -p loopflow --test integration context_launch_tests::explicit_documents_
```

Expected: no external-path panic; preview's submitted channels match provider
input and capture; no truncation, duplicate body, file-read tool dependency or
mutation of unrelated native state. Preview alone leaves no capture/runtime files
and launches no provider. Capture attributes docs to document sources rather than
anonymous assembly. Existing ambient hooks still refresh after file changes.

Gate runs affected checks once with inherited `LF_*` unset and stdin closed:

```sh
uv run python scripts/test.py --reuse-passing --rust-filter 'test(prompt::) | test(context_block::) | test(lf::commands::run::) | test(context_tests::) | test(context_block_tests::) | test(context_launch_tests::) | test(session_cli_tests::) | binary(golden_prompt)'
```

This includes formatting and all-target Clippy. Add provider-specific tests to
the filter only if implementation changes those transports; keep the full matrix
in CI. Configured-provider marker receipts prove the real source path separately
from the deterministic gate scenario. Infrastructure performs the final installed
verification after coordinated release; this Task reports that remaining owner
without asserting installed success.

Check (2026-10-27): `git diff --check` and `lf context` pass; retained `cargo test -p loopflow --lib prompt::` pass (99 tests) and `cargo fmt --check` remain applicable to unchanged code; affected suites/Clippy remain with gate, installed verification with Infrastructure.

## Publication acceptance — 2026-10-27

`lf sync --manual` completed without conflicts against main. Review of the full
range found two stale acceptance expectations: native skill metadata does not
interpret Loopflow `docs:`, and an explicitly selected Wave goal correctly changes
from ambient Wave classification to Docs. The fixtures now preserve those
contracts; no production repair was needed. Docs clarify Loopflow frontmatter.

Check: affected gate run `.lf/tmp/gate/run-21088` passed architecture, formatting,
173 Rust tests and 72 website tests; two expectation failures repaired, with
focused rerun passed both tests (nextest 937f7fa2-7d30-4545-9c9b-da5f5e8fe40a);
`cargo clippy --all-targets -- -D warnings` passed; `cargo fmt --all` passed. Production code is unchanged
from the retained configured-provider proof. Installed verification remains
Infrastructure-owned.

The focused rerun also corrected stale ambient-order and path-spelling assertions.
Attribution now proves the captured source resolves to the selected file and its
label matches the submitted reference; it accepts macOS canonical path aliases.
