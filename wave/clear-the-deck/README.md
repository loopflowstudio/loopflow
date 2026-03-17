# Clear the Deck

## Vision

Remove product and deployment choices that create maintenance surface without teaching us anything. This wave is for cuts that simplify how `lfd` is configured, deployed, and executed. It does not own auth expansion or iOS distribution work; those live in `wave/trust/` and `wave/concerto/`.

## Strategy

Finish the remaining collapse in two passes.

First, collapse the public deployment story to the shapes that already exist in code: native/local and container/studio. `LFD_MODE=native|container` already centralizes several downstream decisions; build on that instead of inventing profile names the product cannot yet explain.

Second, decide whether sandbox stays at all. The current adaptive path only earns its keep if it clearly beats Docker on latency or isolation. If it cannot, demote it to an explicit experiment or delete it.

Hidden overrides are acceptable as escape hatches. Documented defaults are not allowed to sprawl.

## Goals

- Users choose from a small, honest deployment surface instead of a bag of orthogonal config knobs.
- The default container execution path is obvious in both code and docs.
- Deploy and operator docs describe only blessed paths.

## Risks

- New deployment nouns could drift from the current `native|container` machinery and accidentally fork behavior.
- Simplifying sandbox too aggressively could remove a useful path before a replacement is ready.
- Escape hatches can quietly become the real product if the blessed paths stay incomplete.

## Metrics

- Documented deployment shapes: 2
- Documented deploy-selection knobs outside those shapes: 0
- User-visible executor backends in blessed docs: 1
- Remote deploy setup steps before first healthy `lfd`: 10 or fewer
