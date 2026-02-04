# Open Questions

## Wave: rust

### 2026-02-04: Ingest blocked

The rust wave's Phase 1 roadmap has no pickable items:

| Item | Status | Notes |
|------|--------|-------|
| 01a-prompt-parity | ✅ Done | Completed |
| 01b-ops-parity | 🔜 In progress | Already in scratch/rust-parity.md |
| 01c-testing-and-rollout | ⏸️ Blocked | Depends on 01b completion |

**What's blocking progress:**
- 01b (ops parity) must complete before 01c can be picked
- The ops parity work is already active in `scratch/rust-parity.md`

**Options:**
1. Continue ops parity work in current scratch doc until complete
2. Split remaining ops parity work into smaller pickable chunks
3. Mark 01b complete and pick 01c if ops parity is actually done

**Question:** Is ops parity (01b) complete? If so, the roadmap status should be updated to ✅ Done, unblocking 01c-testing-and-rollout for picking.
