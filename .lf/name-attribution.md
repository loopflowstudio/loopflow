# Name attribution: repository learning

Recorded 2026-09-25 from Jack's use-names branch through `f18f47c8f`.
Placement remains unresolved: Jack has not named an owning Wave or Task.
[Choose design or runnable UX prototyping paths · LOO-297](https://linear.app/loopflow/issue/LOO-297/choose-design-or-runnable-ux-prototyping-paths-in-loopflows)
motivated the change; it does not establish ownership of this work.

## Decisions to retain

Jack's destination rule lives in [STYLE.md](../STYLE.md#voice) and the shared
[operating guidance](../rust/loopflow/src/engine/builtins/LOOPFLOW.md).
Stored conversation remains conversation; a newly authored summary uses names.
Historical quotations and machine fields such as `human: true` remain intact.

Three facts must stay separate:

- Personal preference belongs to the explicitly chosen `user.name` in personal
  config. Repository config cannot name every caller. See
  [configuration](../docs/config.md) and its
  [loader](../rust/loopflow/src/engine/config.rs).
- The current participant travels through launch context and `LF_USER_NAME`.
  Forwarded empty means unknown, so a remote Home cannot substitute its owner's
  preference. Detached work starts unnamed. Native resume must deliver the
  participant update to the model, including clearing a stale name; environment
  propagation alone is insufficient. The update grants no review approval.
- Each request's author belongs to its existing journal or provider record.
  Missing names stay unknown on replay. Provider IDs remain identity; names are
  display data. Neither a publisher nor a webhook editor necessarily authored
  the request. Explicit steering retains its requester separately.

No people registry, historical backfill, output replacement filter, or second
name store is needed. Chat DTOs and Swift projections carry the captured name;
their absence must not default to the current participant.

## Failures worth remembering

The first Linear query selected names while its pagination query selected only
IDs. Static response fixtures supplied names anyway and concealed the loss.
The repaired [pagination proof](../rust/loopflow/src/pm/linear.rs)
(`observe_issue_reads_every_comment_page_in_revision_order`) models GraphQL
field selection: before repair it observed `[None, Some("Jack")]`; afterward
it observed `[Some("Maya"), Some("Jack")]` in revision order. Check continuation
queries when extending provider records.

Native session resume bypasses fresh prompt assembly. Its
[resume path](../rust/loopflow/src/lf/commands/util.rs) now sends a participant
update through each provider's resume prompt. A fresh-launch generation cannot
prove that a resumed review conversation receives or obeys this update.

Local chat delivery had an anonymous test shortcut and an unchecked writer
beside the production path. One fallible
[`try_deliver`](../rust/loopflow/src/controller/wave/runtime.rs)
now serves HTTP and tests. Preserve destination ownership, append before
projection/broadcast, and retryability after journal failure. Provider ingestion
still owns provider IDs and deduplication; it is a distinct operation.

Capture names only for authored text. The Mac sender initially loaded a name
even for a bare interrupt, allowing a preference-read error to block cancellation.
Gate repaired that path and observed the regression fail before the change and
pass afterward through the existing loopback HTTP server. Control inputs that
create no authored record should not depend on display preferences.

## Dated evidence and limits

Existing evidence was inspected during this curation; tests were not rerun for
this documentation change.

- Nine `preferred_name` tests passed, covering fresh preference reads,
  correction, unknown callers, forwarded caller versus host, detached startup,
  prompt formats, and resume subprocess arguments for all three providers.
- Local CLI/API/journal demonstration retained Jack, Maya, and null after
  reopening, with identical conversation text: “You chose the prototype path.”
  Source-attribution tests covered Discord replay, Linear authors versus
  editors, and webhook persistence. Eight Swift contract tests passed.
- Compression verification passed 34 runtime, 13 HTTP, and eight journal tests,
  including failed writes and concurrent delivery order. Formatting and clippy
  across all targets passed. This is not a full repository gate.
- Real fresh CLI generations read isolated personal config without a name in
  the current request. Two Jack processes produced “Jack requested” in Task
  prose and “You requested” in conversation. After correction, the captured
  brief said “Maya requested keeping the design path and adding a runnable
  prototype path. Jack requested a design-only path last week.” The anonymous
  requirement remained unnamed. These are generated examples, not a live Task
  write. The exact builtin design skill used a temporary alias because `design`
  resolves to a review-bearing Flow; that Flow was not executed.
- The final tracked-text scan accounted for 876 remaining matching lines:
  688 historical, 160 maintained source/docs, seven prompt assets, and 21
  tests/fixtures. Retained terms were reviewed as machine syntax, quoted
  evidence, or established terminology. This dated count excludes scratch and
  is not a permanent invariant.

Detailed branch evidence remains in scratch while acceptance is open. Local
history at `ae531ed55` contains `scratch/use-names-slice-4.md`, generated
outputs, their harness, and the scan inventory; `f18f47c8f` contains
`scratch/use-names-compress.md`. This summary retains the decisions and limits
independently of scratch cleanup or checkpoint-history collapse.

Gate on 2026-09-25 passed 799 affected Rust tests, 229 Python tests, 78 website
tests (three viewport skips), architecture, Rust formatting/clippy, 112 focused
Swift tests, Swift boundaries, and the Mac build. The full Swift package attempt
stopped reporting completion and was terminated after 122.5 seconds; no
assertion failure established the cause. The stream test had not finished;
focused chat/contract/stream proofs passed afterward, but a complete package
pass remains unproven. Native rendering and hosted UI were not exercised.
The scan now has 877 matching lines: the additional line is this record's
quoted machine field, not new product prose.

## Unresolved acceptance

The live native review-session continuation remains unexercised. Demonstrate
an anonymously prelaunched review session opened by Jack, then another named
participant, then an unknown caller on a differently named Home. Confirm
historical authors survive and no review decision is implied by the update.

LOO-297 remains unchanged. Its source-client read failed with
`No Linear credential found. Run doppler run -- lf auth linear.` A separate
`no Task exists` result described only the local Task registry. Neither result
establishes absence from Linear or from every Home. Read the current issue
through an authorized credential path before repairing Jack's quoted sentence;
preserve the rest of the issue. No auth or placement changes occurred.

No Wave plan was reconciled and no Task was filed or closed for this curation.
Resolve ownership before moving these outstanding outcomes into Wave planning.
