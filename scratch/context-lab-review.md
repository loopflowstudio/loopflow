# Context Lab design review

## What changed structurally

Context Lab introduces one read path and one intervention path:

```text
agent launch capture
  → local trace/context ledger
  → Rust SessionSetQuery + atomic ContextLabSnapshot
  → native Swift flame / lanes / table / evidence
  → explicit lf trace body reader
  → existing Task workspace + refine skill
```

Rust owns population filters, trace joins, canonical source/revision identity,
token attribution, representative selection, and the flame hierarchy. Swift
owns view state, linked selection, saved queries, rendering, and the guarded
handoff. This reinforces the existing daemonless `RegistryQuery` boundary; it
does not introduce a second telemetry store, editor, agent host, or git path.

The design intent still matches `scratch/instruction-workbench.md`: research a
set of real sessions first, move from aggregate pressure to immutable evidence,
then start a separate refinement session against the canonical source.

## Key choices

- The session set is the primary object. Instruction rows are derived from its
  measured context rather than maintained as an admin catalog.
- Flame identity is `kind → canonical source → content hash`. Historical
  revisions remain immutable and naturally accumulate evidence.
- Prompt and conversation bodies remain closed until **Open trace**. The graph
  and evidence rail carry only measurements, hashes, availability, and exact
  addresses.
- Refinement refuses copied text and in-place historical mutation. It must map
  one current canonical source hash into an existing Task worktree.
- Provider-total-only turns remain in coverage denominators but contribute no
  flame or lane width. This keeps missing assembly attribution missing and makes
  flame widths reconcile exactly with measured supplied context.

## Live evidence

The installed branch app queried the real Loopflow repo over 30 days. During the
review the population contained 37 sessions, 102 launches, and 110 turns. The
largest aggregate band was Wave memory; operating instructions were third. This
contradicted the starting hunch that `LOOPFLOW.md` would necessarily dominate
and validated ordering by observed supplied-token load.

After excluding eight provider-total-only turns from attributed geometry:

- supplied context: 841,082 tokens;
- aggregate flame root: 841,082 tokens;
- sum of root children: 841,082 tokens;
- lanes with assets but no supplied total: 0.

The high-context operating-guide representative at trace
`556f8b57-a406-4455-be8a-99b0acd2b60d`, launch
`e3f2b14f-054a-4c12-85a1-079344ff1293`, turn
`73d5bc08-ed47-4f77-aa78-054deda5c501` opens a 5.1 KB system prompt, 56.4 KB
task prompt, and 270 KB normalized conversation. The first body read occurs only
after the explicit trace action.

## Review changes already made

- Rebased through PR #906 so the development binary can read the current live
  ledger schema.
- Preserved the explicit development-to-production-ledger opt-in through the
  native app launcher.
- Resolved a Task worktree launch path to its canonical main-repo filter.
- Let `lf trace` accept the trace id carried by Context Lab as well as an exec id.
- Removed unmeasured provider-total-only assets from flames and lanes.
- Made missing canonical-source refinement visibly disabled and made trace body
  backgrounds explicit for readable native rendering.

## Risks and bottlenecks

1. **The refinement loop is not yet live.** The PM cache contains W2-71 under
   Intelligence / Context, but `intelligence` is not registered and no W2-71
   Task Session or worktree exists. Context Lab cannot manufacture that control
   ownership in Swift.
2. **Historical operating-guide rows are intentionally read-only.** They predate
   source-path capture and show “No canonical file source.” A normal run through
   this branch binary must prove that `LOOPFLOW.md` appears as an editable new
   revision with the correct effective hash.
3. **Project and Task filters lack ledger attribution.** Reconstructing them from
   filenames or worktree names would create a second, weaker identity system.
4. **Task creation is omitted.** The current sheet selects only an inactive Task
   Session with a durable worktree; it cannot create a Linear Task and return a
   workspace receipt in one human-confirmed operation.
5. **Native startup logs AttributeGraph cycles.** No visible Context Lab failure
   has been attributed to them yet; the final demo must settle that rather than
   normalize noisy runtime diagnostics.

## Done-when audit

- **Research truth:** live 30-day query, atomic filters, coverage denominators,
  flame/lane/table models, cancellation, and exact token reconciliation hold.
- **Evidence truth:** canonical revisions, representative roles, availability,
  and explicit exact-trace opening hold. A fresh canonical `LOOPFLOW.md` capture
  is still required.
- **Refinement truth:** stale-hash/worktree guards and the structured seed exist;
  a real Intelligence Task launch, diff, and backlink have not been experienced.
- **Learning truth:** revision comparison is live for naturally observed hashes;
  the edit → ordinary run → new canonical hash journey remains undemonstrated.
- **Shipping proof:** focused Rust/Swift tests and native builds pass. The final
  full matrix, accessibility pass, installed-app journey, and independent demo
  remain open.

## What is intentionally not included

- a public instruction-admin CLI;
- a copied prompt database or embedded Markdown editor;
- an LLM-authored quality score;
- remote telemetry or automatic prompt-body opening;
- guessed Project/Task identity.
