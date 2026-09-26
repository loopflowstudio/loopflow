# Preserve the demo Home across its draft's release

Observed 2026-09-25: installed roadmap uses a Home without current chapters.
The retained development Home `local-afee63d734c7482cb94d1071af26d9ea` holds the
real chapter records, but current `install local-preflight` rejects its older
canonical frontier (0.12.20 → 0.12.21). Read-only ledger inspection establishes
one applied `wave_chapters` draft. Its SHA-256 is identical to the body of the
new canonical release migration, excluding the release's `-- draft:` header:
`519e4984a449617dbee7f541e8517214f0f0d96891264621421510972306343b`.

Finish: existing local-promotion preparation recognizes exactly released draft
SQL already applied to a development Home, preserves all product data and Run
files, and records the canonical migration once without executing its SQL again.
Ordinary reads remain validation-only. Changed/reordered/unrecognized SQL and
schema disagreement still require explicit recovery; do not fake ledger success.

Implement inside the existing development-migration owner. Match the complete
pending canonical migration bodies, in order, against the applied draft prefix
using release provenance names and exact SQL checksums. Atomically transfer only
the matching ledger receipts; validate remaining draft prefix, full expected
schema and foreign keys before committing. No registry, live chapter rotation,
data export/import, new CLI flag or provider write. This is an exact promotion
of existing migration evidence, not permission to reset an incompatible Home.

Proof: simulate the real pre-release prefix plus chapter draft; retain authored
data across adoption and repeated application; malformed checksums and schema
drift must leave both ledgers/product data unchanged. Run existing development
migration tests, fmt and clippy. Then run supported local-preflight against the
retained Home (it validates an isolated copy). Do not apply an installation or
modify the live store in this implementation pass.

The native Flow review owns its files independently. Parent owns migrations.rs
and this bounded recovery contribution. Build resource preflight currently passes.

## Implemented and checked

Exact adoption is now in `apply_installed_development_sqlite`. No product SQL is
replayed during adoption; remaining unreleased draft IDs/checksums/timestamps
survive position normalization. Schema mismatch rolls back both ledgers.

Five focused tests pass: chapter preservation/idempotence, changed checksum and
schema refusal with unchanged data/ledgers, retaining an unreleased draft and
its data, plus the existing prefix/dependency tests. Initial compilation found
missing explicit test imports and unsupported usize SQL parameters; corrected
those before behavioral proof. All-target clippy and formatting pass.

Rebuilt `target/debug/lf`; its supported `install local-preflight --store
<retained Home>/loopflow.db --json` now returns `promote_and_migrate`, with 32
compatible executable references and the two current development drafts. This
operates on an isolated validation copy. The original live Home still has its
0.12.20 release frontier, original wave_chapters draft receipt and six chapter
records. No promotion or store mutation ran. Exact hashes and logs are in
[the receipt](demo-home-release-adoption-receipt.json).

Independent compression/review should check this migration-owner change along
with the next bounded slice before installation. Remaining demo work includes
Comments, tuple/disclosure implementation, final app/CLI builds and real readback
from the promoted Home; this passing preview alone is not a demo-ready claim.
