---
requires: diff vs main
produces: simpler code
action_style: procedural
---
Simplify the branch's model and public interfaces. Delete what no longer earns its place.

## Map the model

Read the current design and diff. Follow each changed concept through its
core types, persistence, public API, clients/UI, tests and documentation.
Write down who owns each fact and which representations merely copy it.
Could a new reader explain the model and public contract in one screen?

Look for the structural reduction: one representation that removes several
special cases. A rename leaves the old architecture intact when the same
ownership boundary and adapters survive.

## Reduce from the owner outward

1. Remove duplicate concepts and facts from the core model. Derive what already
   has an owner. Replace mutually exclusive optional fields with one explicit
   choice. Retire state that only preserves obsolete launch inputs.
2. Make persistence and public interfaces express that model. Remove unused
   tables, fields, wrappers, aliases and fallback parsers. Do not add an adapter
   to keep an internal caller using a retired shape; migrate the caller.
3. Follow the change through clients, UI, wire types, fixtures, examples and
   docs. Compare mirrored fields against actual behavior. Delete tests of
   removed implementation details; retain proof of user-visible capability.

Stay on the branch's model path, including direct owners and consumers outside
its literal diff. Leave unrelated cleanup alone. Respect the full reviewed
design and its deletion path, not just the latest slice.

Preserve external contracts and installed callers that still need them. Removing
an in-repo caller does not retire deployed copies. Follow repository migration
rules for persisted data; require explicit evidence before deleting an upgrade
path. Internal compatibility without a real consumer should disappear.

Prefer a coherent reduction across layers to isolated cosmetic changes. Do not
invent work to satisfy a line-count target. If no meaningful reduction exists,
report the model path and suspected redundancies inspected and stop.

## Verify

If executable behavior changed, run the smallest existing behavioral proof for
that change. Reuse passing evidence when content is unchanged; gate and CI own
broader suites. If a behavior breaks, the reduction went too far. If a test only
encodes a removed representation, update it to prove the retained behavior.

Report the model before and after, removed interfaces or representations,
anything intentionally retained and why, and the focused proof. The outcome is
simpler code with the same user-visible capability.
