# Proposal: bounded Session inventory enrichment

2026-09-29 · LOO-298 · Prepared for Jack Heart. **Private, unapplied,
uncompiled; no product tests run.** Main owns integration, builds and proof.
All contribution writes are inside `.lf/tmp/session-summary-proposal/`.

## Disposition

A three-file patch removes repeated Flow decoding and removes FlowGraph
construction entirely from Session surface projection. **It is a partial cut:**
the existing Session-history identity reader remains unchanged. Its exact
validation dependency is returned below, rather than introducing a second
identity truth or silently weakening resume validation. No schema, DTO, search,
paging, mode, Desktop reconciliation or native-authority change is proposed.

## Current source and proposed data flow

The current `list` selects Sessions in SQL using the existing filters, title/ID
order and offset. Each selected row calls `surface`, which calls `managed_review`
(and fully decodes the managed Task capture), then separately reads its member
Flow and constructs a display graph solely to validate the stored numeric node.
Historical Sessions sharing a Flow repeat that work. Provider identity already
comes from `input_provider_session` → `summary_for_input`; there is no native
sidecar fallback in this path. The older discovery research is stale there.

The patch adds one **read-local** `SurfaceFlows` value, discarded when the list
returns. It loads each referenced Flow once through the existing full validated
reader, then retains only name, captured node count, current input and finished
state. It also retains unreadable/missing results for that read. The single
`surface_with_flows` implementation serves list and individual surfaces; the
latter receive a fresh read-local value. No persisted cache or claim is added.

Task's managed pointer is read by one narrow indexed join per selected Task,
without decoding an unrelated managed capture. Remote placement is projected
only when that pointer matches the Session's own readable Flow. The existing
`managed_review` is retained for action lookup, which still rereads and validates
the exact Flow/version/input. Launch, resume, complete, rename and bind fences
are unchanged. A list is not a database-wide snapshot; cached facts are consistent
within their first read, and later mutations still require fresh action checks.

`QueuedInvocation::node_id` already counts all preorder nodes, including every
XOR alternative. The patch lifts its existing counter into one private helper
and exposes `node_count` inside the crate. Surface checks the stored ID against
that contiguous range instead of building labels, child graphs and return edges.
It does not infer location from skill/title, synthesize an unknown node, or change
storage IDs. NULL node/iterations remain NULL; missing Flow remains Unknown;
unreadable own capture keeps its reason and disables actions; absent captured
node still errors. The full Flow decoder still validates cursor/claim/cwd and
capture bytes once per distinct Flow. The row/captured-ID equality is already
required by the `retain_flow_invocations` schema constraint.

Exact replacement/removal callers:
- `human_session::list`: passes a shared read-local projection.
- `human_session::surface`: delegates to that same projection with fresh scope.
- Passive `surface` no longer calls `managed_review`/`task_flow`, and no longer
  calls `FlowGraph::new`/`node_at`. Active `owned_target` keeps `managed_review`.
- New crate-private `SqliteStore::managed_flow_id`: reads the existing Task pointer.
- `QueuedInvocation::node_id`: uses its lifted counter; node ordering is unchanged.
- No Session history, provider connection, process receipt or Work-label caller
  is deleted. Work ancestry and remote placement still use their current owners.

For N listed members of F distinct Flows, full capture decoding changes from
up to two per member to one per distinct Flow, plus narrow Task pointer reads.
All-distinct Flows still require one capture each. SQL row selection is unchanged;
there is no claim of bounded capture bytes, dense latency, or complete discovery.

## Exact history dependency

`input_provider_session` first reloads the AgentSession, calls
`summary_for_input`, decodes every selected `SessionEvent` payload, and passes
those values to the shared `provider_session_from_history` reducer. The fallback
sorts original JSONL ordinals and deserializes each retained `EventEnvelope`,
including usage and attempt-finished envelopes that cannot change native identity.
Fresh publication supersedes an imported sidecar; only with neither present does
the event fallback validate the complete retained envelope set. Unknown account
remains unknown. Native start/usage rows can also be decoded by the summary query
although their payloads ordinarily have no observation `source`.

A simple SQL filter for three identity event types is **not behavior-equivalent**.
For example, schema-1 `type: usage` with missing required usage fields is selected
by today's summary predicate and rejected by its fallback EventEnvelope decoder.
A provider-only predicate would skip it and return a native identity. An
unsupported-schema nonidentity envelope similarly remains relevant to that error
contract. The same reader serves NativeRun history, unpublished-launch recovery,
resume and process-related readers, not just passive listing. An `EXISTS` check
also loses provider-reference validation and fresh/imported/account ordering.

`predicate_probe.py` replays the extracted current SQL predicate over three
JSON values and confirms all are selected; its minimal pointer join covers
present/corrupt/missing targets without decoding payload. This is **SQL-expression
evidence only**, not a Rust rejection reproduction, full-schema test, or product
proof. The rejection conclusion follows the inspected required serde fields and
schema check in `run_record.rs`. No candidate history query was applied.

There is no schema/DTO dependency for the supplied Flow patch. Removing all
history decoding while preserving *today's full validation contract* requires
validated identity/validity metadata on the existing owner, with recorder,
import/replay/conflict and invalidation writers changed together. Alternatively,
a metadata-only **display** contract can deliberately leave unrelated historical
errors to detail/actions, retain the current strict action reader, and share the
same identity reducer. That is a visible error-projection distinction, not a
safe mechanical predicate substitution. This contribution selects neither a new
persistent summary nor an altered error contract. No new permission or lifecycle
is needed; main should make that boundary explicit before expanding this patch.

## Review, overlap and proof obligations

The private patch extends the existing nested/post-XOR membership test to compare
one inventory with exact surfaces across cursor changes. It adds retained NULL
location, first out-of-range node rejection, fresh-read corruption and disabled
actions while retaining all Session rows. The existing numeric capture proof
also checks the shared count. These tests are **authored, not executed**. No
mock call counts, factory, provider launch, timeout extension or weakened native
check is introduced.

Desktop proposal overlap is one file, `ops/human_session.rs`: its bind/preview
hunks are separate from this patch's list/surface/test hunks. The validator replays
both proposals in both orders against saved originals and obtains identical
combined text. The Desktop patch is not applied here. True/false/all filtering,
full inventory, off-roadmap labels and terminal retention remain main's proof
obligations after combining it; no Rust/Swift wire changes are needed for this cut.

Review found that a whole FlowGraph was unnecessary once keys became numeric;
the final patch reuses storage's count instead of adding another node traversal.
It also keeps exact corruption diagnostics by retaining full Flow decoding once,
rather than extracting name/state alone and declaring unreadable work usable.

Main's focused commands (not run here), after resource preflight and hunk review:

```sh
uv run python .lf/tmp/cut-i/run.py session-summary-rust cargo nextest run -p loopflow --lib --test session_cutover_tests --no-fail-fast -E 'test(captured_nested_membership) | test(stored_session_membership_resolves_nested_and_post_xor_graph_nodes) | test(review_session_retains_feedback_and_history_across_replacement_and_corrupt_neighbors) | test(session_work_labels_follow_stable_ancestry_without_a_roadmap) | test(inventory_scopes_before_paging_and_keeps_worktree_repository_identity) | test(import_stores_each_old_session_once_with_its_name)'
uv run python .lf/tmp/cut-i/run.py session-summary-actions cargo nextest run -p loopflow --test session_cli_tests --no-fail-fast
uv run python .lf/tmp/cut-i/run.py session-summary-clippy cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Before integration, add or identify a focused remote-placement projection/action
case and a refreshed managed-pointer change. The proposed nested test is
Taskless, and the existing remote DTO fixture cannot establish SQL-to-route
behavior by itself. Use the unchanged native
identity-order/fresh-publication proof if the later history implementation changes
its reader. No migration matrix or Swift wire test is justified solely by these
three hunks' production boundary; combining the Desktop change has its own
already-recorded proof plan. Real accounts, Desktop, final dense performance,
installed migration and the retained native decision failures remain unproven.

## Patch identity and validation

Apply hunks only from `session-summary.patch` in this directory. Private proposed
full files are not replacements for moving working source. `initial.json` records
original source/context hashes; `original/` preserves bytes; `proposed/` contains
only the three proposed Rust files. `validate.py` regenerates the patch and checks
hunk counts, unique old contexts, exact in-memory replay against original and
current files, Desktop overlap, read-only applicability and private rustfmt.

Patch SHA-256: `187a8fe3b8163c2660fd8e60fe6639127db15d02cb0da8bd69ea366762503677`. **3 files / 8 hunks**.
Opening and validation HEAD: `915aa82ffda9dd7a87408c087e38a2febe3d3cf1`.
The closing check found main's `tests/session_cutover_tests.rs` changed from
`25e823ff…` to `5b86ddfa…`; it receives no proposed hunk. Full hashes are in
`validation.json`. All three patched source files still match their originals;
main's concurrent harness/agent/Flow edits remain outside the patch.
Recheck at application. `validation.json` supplies every proposed-file hash.

Private `rustfmt --edition 2021 --config skip_children=true --check`, independent
hunk replay and `git apply --check` pass. These establish syntax/format/context,
**not type checking or behavior**. No cargo, Swift, Xcode, product test, branch
binary, installed Home, provider, PM/Git mutation, new worker or process control
ran. Read-only Git inspection was used.

Measured production delta: **+122 / −44 = net +78**.
Line SequenceMatcher with autojunk disabled, saved originals versus private
proposals; trailing Rust test modules excluded, no rename credit. This includes
moving the shared counter and the read-local projection; it is a cost measurement,
not a claim of code-size reduction or completed Session-summary conversion.
