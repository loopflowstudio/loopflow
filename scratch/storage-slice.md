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
not repeated whole-build speedup. Publish for review; no merge is authorized.
