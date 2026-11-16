# Synchronous storage ownership

## Scope

Move saved records, synchronous SQL, sidecar files, migration catalogue/schema
and released fixtures into `loopflow-storage`. Keep acquisition, provider/account
policy, configuration, native attachment, async attribution and complete capture
manifest projection in the application. Remove the old implementation as each
owner moves; no public raw connection or staging-only compatibility layer.

This scope comes from the accepted LOO-650 storage design and the November 15
decomposition direction. Carry the complete non-scratch source delta from
`fa357ee7f6` to `b71c361516`; retain all 121 migration files and 45 historical
SQL fixture bytes. Original union checkpoint:
`d0cbe2e372dd3661d17943f1bcfc2351448dc28e`.

## Dependencies

Stacks on the private library foundation. Its complete consumers are included
here: storage builds without test-support, application fixtures use the dev
feature, and native packages consume the private path dependency. LOO-650 keeps
execution separation, cache upkeep and final alternating measurements.

## Delete — do not maintain

Remove application SQL implementations, saved record copies, catalogue/schema
and test-image generation, and the application historical fixture directory.
Move mixed tests by behavior; retain public lifecycle/acquisition fixtures above
storage and SQL/frontier invariants below it. No duplicate fixture source,
application dev-dependency from storage, callback into application policy or
production transaction helper solely for tests.

## Contracts

Keep pinned read snapshots and complete saved-input comparison on writes.
Observe files/configuration/processes outside general SQL transactions. Preserve
exclusive upgrade preflights/export/process fences, lazy token keys, attachment
leases, once-only receipts, native identity and historical alias behavior.
Migration ordering and canonical schema bytes stay unchanged; Git/ref changes
and no-Git source packages still produce the right catalogue.

## Done when

Build the CLI and compile application fixtures independently on this branch.
Compile storage tests without test-support first, then application fixture tests
with the dev feature. Run focused migration, record/receipt and mixed lifecycle
checks. Validate migration authoring and native package classifiers. The
canonical schema remains
`0a506f2e3be6d80483304b52c6eca2b5ab3ce2ca87b66e5fae28bf9b49f4f596`.
The existing cross-root storage/test-support receipts establish categorical reuse,
not repeated whole-build speedup. Publish for review; shipping is authorized after technical checks pass.

## Source seam repair

The first independent architecture check reported `read projection missing from
map: tests/fixtures/migrations/`. At `b71c361516`, the old directory still held
only the unused `draft_manifest.json`; all 45 SQL fixtures had moved. Include
its existing deletion from `d529e8a24a` in this storage slice. No consumers refer
to the manifest. SQL migration/fixture bytes and production code remain unchanged.

All 869 paths in the original source delta matched their saved blobs before this
one later cleanup was included. `check_architecture.py` is rerun after the deletion.

Foundation follow-through: carry its existing workspace provenance watcher and
fresh-runner Cargo dependency fetch into this source before building. The
provenance regression fixture moves from application-owned SQL build inputs to
storage metadata using the existing final fixture at `d529e8a24a`. Its fake
metadata producer watches only its own build script, so a library-only edit
cannot spuriously refresh the application through metadata. The same runtime
clean-to-dirty assertion remains. Parent sync will reconcile this source with the
final merged foundation before publication.

Review repair: the slow-write stack filter recognized only `loopflow::`, so a
storage-only writer could report `unknown`. Recognize the storage crate in the
same filter and exercise its actual captured call site. No parallel diagnostic
path or SQL change is introduced.

## Independent verification

- Standalone storage test compilation without test-support, CLI build and application
  fixture check: pass; five focused operation/lifecycle tests pass. All-target clippy and formatting pass.
- `check_architecture.py`, `check_rust_test_inventory.py`: pass.
- Migration-history/authoring and third-party inventory Python checks: 12 passed;
  refreshed inventory has 275 components.
- Original migration/fixture comparison: all 121 and 45 SQL files unchanged;
  generated canonical schema matches the contract hash above.
- `cargo test -p loopflow-storage --lib call_sites_name_storage_callers`: 1 passed.

The real Cargo provenance fixture passes; restoring the old application-only
watcher fails at the `.dirty` assertion after a storage-only edit. Its metadata
producer does not rerun on library edits, so metadata cannot mask this regression.

Parent queue CI exposed a preview-cancellation readiness race; its verified
fixture repair must join through sync before this slice is published.
