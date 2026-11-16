# Private library foundation

## Scope

Extract the existing model, builtin definitions and terminal state into private
path-dependency crates. Retire crates.io publication while keeping native
packages, archive hashes, release readback and application behavior. The
application imports each owner directly; retained public type paths are aliases
to one implementation. Keep the pinned Ghostty archive and its build logic with
the terminal crate. Preserve runtime checkout identity.

This scope comes from the accepted LOO-650 design and the November 15
decomposition direction. The source is the complete non-scratch delta from
`9cafc410eeab03d772b8cac7d641d6bd7edfd435` to `fa357ee7f6`.
Original union checkpoint: `d0cbe2e372dd3661d17943f1bcfc2351448dc28e`.

## Dependencies

Starts from main. Synchronous storage follows on a dependent Task. LOO-650
retains execution separation, automatic cache-root upkeep and final repeated
build-cost measurements. No later slice is required to run this foundation.

## Delete — do not maintain

Remove the application implementations moved to model/definitions/terminal,
including duplicate terminal fixture source. Remove crates.io publication,
registry secrets and its exclusive release tests. Keep one implementation of
each value, builtin and native terminal behavior; add no staging flags or shims.

## Done when

The CLI builds and its test consumers compile on this branch. Model,
definition and terminal behavior tests pass with direct owner imports. Release
retirement checks keep the native package flow. Existing reuse/source-identity
probes and isolated debug builds remain available. Reuse can retain another
checkout's debug paths; isolated developer builds are the source-debug escape.
No whole-build speedup, production cache validation or safe concurrency budget
is claimed. Publish this independently reviewable foundation; no merge is authorized.
