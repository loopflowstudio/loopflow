# PR4 — Project/KR claim provenance via a PM overlay (W2-124)

Serial continuation of the receipt contract. Shipped: `Receipt` type + memory-fact
receipts (#919), `lf receipt show` drill + `lf doctor` sweep (#971), repo+number+SHA
PR identity + inaccessible-evidence surfacing + trace/PM/PR resolver tests (**#984,
merged**).

This doc scopes the next serial slice and maps the tail, so no session mistakes the
merged slices for the whole contract.

## Remaining contract (each its own serial PR, in order)

1. Report provenance + automatic receipts — authored Wave reports (chat turns)
   carry the receipts for what they summarized, attached at authoring.
2. Correction without DB edits — re-cite/retract through an authored API only.
3. Shared Mac/iOS/chat drill-down over the resolver.
4. Survival — receipts keep resolving across land / restart / successors / migrations.
5. Real N/N orphan-free dogfood — `lf doctor` receipts clean on a live wave for a week.

## PR4 computable design

**User-visible outcome.** A user (or agent) can bind a Project or KR claim to its
evidence with `lf pm cite <claim-id> --receipt kind:reference…`, and later
`lf receipt show` drills that receipt to the same canonical record a memory receipt
does. `lf doctor` reports these claims' receipt health alongside memory facts. A
Linear re-sync never erases a citation.

**Source of truth.** A new local, rebuildable **PM receipt overlay** — a SQLite
side-table keyed by `(repo, wave, claim_id)` → `Vec<Receipt>`. Linear stays the
owner of claim *text*; the overlay owns only the *citation*. `lf pm show`'s read
model merges the overlay over the Linear-owned snapshot payload. Deleting the
overlay loses citations only, never Linear data — it rebuilds from the journal of
`ClaimCited` events (same replay discipline as `MemoryAdded`).

**Claim identity (the load-bearing decision).** A Project claim id is its Linear
project id. A KR has no Linear id of its own (`PmKr { text, holds }`, positional
under `PmProject.krs`), so a KR claim id is `<project_id>#<ordinal>` — stable across
text edits and re-sync as long as KR order is stable, which the Linear read
preserves. `lf pm cite` validates the id resolves to a claim in the current
snapshot before writing; an unknown id errors (no dangling citation).

**End-to-end proof.** `lf pm cite <project_id>#0 --receipt pr:owner/repo#N` →
`lf pm sync` (re-fetch Linear) → `lf pm show --json` still shows the citation on
KR 0 → `lf receipt show pr:owner/repo#N` opens the PR → `lf doctor` counts the KR
claim as resolving, zero orphaned.

**Affected surfaces / consumers.**
- `Receipt` reused as-is; new `ClaimCited { claim_id, receipts }` journal event +
  wire type (no serde defaults) with a round-trip fixture (Rust + Swift mirror).
- Store: overlay table + migration; `put_claim_receipts`/`claim_receipts` reads.
- `lf pm cite` (new); `lf pm show --json` merges overlay (new field `receipts` per
  project/KR); doctor `gather_receipt_audit` folds overlay claims into the sweep.
- Swift `PmShowResult`/`WaveProject`/`WaveKr` gain `receipts` (DTO mirror + fixture).

**Absent / error states.** No overlay row → claim shows zero receipts (doctor:
missing, warn during grace). Unknown claim id at cite time → error, no write.
Overlay read fails → doctor marks Pm inaccessible (already implemented in #984),
never silently orphaned. A KR whose ordinal no longer exists after a Linear
delete → doctor orphaned (the citation outlived its claim).

**Operational boundary.** `lf pm cite` and `lf pm show` stay local reads/writes
(no Linear round-trip on cite; sync is the only network path). Overlay merge is
O(claims) over the cached snapshot.

**Exclusions (later slices).** Report/chat-turn provenance and automatic
attachment (slice 2). Correction API (slice 3). Mac/iOS/chat render (slice 4).
Cross-machine overlay replication — the overlay is per-machine, rebuilt from the
wave journal on demand.

## Tests

- `ClaimCited` round-trip fixture (Rust + Swift).
- `lf pm cite` attaches, `lf pm show --json` reads it back, and it survives a
  simulated re-sync (overlay merge after snapshot replace).
- `lf receipt show` drills a KR-cited PR receipt.
- Doctor sweep over overlay claims: resolving ok; missing/orphaned/inaccessible warn.
- Unknown claim id at cite time errors without writing.
