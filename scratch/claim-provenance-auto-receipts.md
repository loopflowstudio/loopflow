# PR4 — claim provenance + automatic receipts (W2-124)

Serial continuation of the receipt contract. Slices #919/#971/#984 gave the
receipt *type*, the *drill* (`lf receipt show`), the *sweep* (`lf doctor`), and
repo+SHA PR identity with inaccessible-evidence surfacing. What remains is the
rest of the contract — this doc scopes PR4 and maps the tail so no session
mistakes the merged slices for completion.

## Remaining contract (each its own serial PR)

1. **PR4 (this) — Project/KR/report claim provenance + automatic receipts.**
2. Correction without DB edits — fix a bad receipt through an authored API only.
3. Shared Mac/iOS/chat drill-down over the resolver.
4. Survival — receipts keep resolving across land / restart / successors / migrations.
5. Real N/N orphan-free dogfood — `lf doctor` receipts clean on a live wave.

## PR4 scope (smallest stable slice)

Memory facts already carry `Vec<Receipt>`. Extend the same binding to the other
curated claims — KRs, Projects, executive reports — and make authoring attach
receipts instead of leaving claims bare.

- **`ClaimCited` binding.** A `{ claim_ref, receipts: Vec<Receipt> }` record that
  points a KR/Project/report claim at its evidence, journaled like `MemoryAdded`.
  Reuse `Receipt`; do not fork the type. Wire type (no serde defaults).
- **`lf memory cite <fact> --receipt kind:reference…`** — attach receipts to an
  existing/authored memory claim (companion to `add --receipt`).
- **`lf pm cite <kr|project id> --receipt …`** — bind a KR/Project claim to its
  evidence; stored in a PM snapshot overlay so a Linear re-sync never clobbers it.
- **PM snapshot overlay.** Local, rebuildable side-table keyed by claim id →
  receipts; the read model merges it over the Linear-owned payload.
- **Automatic receipts.** When loopflow authors a claim it already knows the
  evidence for (a report citing the turns/runs it summarized), attach those
  receipts at authoring — no manual `--receipt`.
- **Doctor extension.** The `receipts` sweep already audits memory facts; extend
  it to KR/Project/report claims via the overlay, same missing/orphaned/
  inaccessible/cross-wave discipline.

### Done-whens

1. `lf pm cite <id> --receipt pr:owner/repo#N` binds a KR claim; survives re-sync.
2. `lf receipt show` drills a KR/report receipt exactly like a memory receipt.
3. An authored report attaches receipts automatically for what it cited.
4. `lf doctor` sweeps KR/Project/report claims, not only memory facts.
5. Overlay is rebuildable — deleting it loses no Linear data, only local citations.

### Tests

- `ClaimCited` round-trip fixture (Rust + Swift mirror).
- `lf pm cite` / `lf memory cite` attach + read-back; overlay survives a re-sync.
- Doctor sweep over overlay claims: resolving ok, orphaned/inaccessible warn.
- Automatic attachment: an authored claim carries the expected receipts.
