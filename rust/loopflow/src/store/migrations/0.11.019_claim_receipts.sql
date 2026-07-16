-- PM receipt overlay: one Project/KR claim's evidence receipts.
-- Linear owns claim text (the pm_snapshots payload); this side-table owns only
-- the citation, keyed by (repo, wave, claim_id). claim_id is a Linear project
-- UUID or `<project_id>#<ordinal>` (a KR). Deleting rows loses citations only,
-- never Linear data — the overlay rebuilds from the wave journal's ClaimCited
-- events. receipts is a JSON array of Receipt (kind, reference, wave).
CREATE TABLE claim_receipts (
    repo TEXT NOT NULL,
    wave TEXT NOT NULL,
    claim_id TEXT NOT NULL,
    receipts TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (repo, wave, claim_id)
);
