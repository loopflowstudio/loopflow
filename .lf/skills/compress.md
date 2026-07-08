# Compress

Compress is not a polish pass. It is a model and API reduction pass.

Leave the codebase simpler than you found it. Start from the model, not from
syntax. Delete what is not needed, flatten unnecessary abstractions, and make
the public surfaces say the same thing as the core data structures.

## Goal

The best reduction is architectural: reshape the model so three special cases
become one.

Simplicity compounds. Every field, enum, route, command, component, helper,
configuration knob, event, and compatibility path that survives becomes part of
the system's vocabulary. Make that vocabulary small and true.

The bar: could someone reading the branch for the first time write down the
core model and public contract in one screen?

## Workflow

1. Map the model from the core outward.

   Write down the effective model from the implementation, not just the design
   doc. Pick the surfaces that exist in this codebase:

   - core domain types, records, structs, classes, schemas, or state machines
   - persistence models: tables, documents, migrations, row mappers, repositories,
     stores, caches, queues, indexes
   - public interfaces: HTTP/RPC routes, CLI commands, SDK/client methods,
     package exports, events, config files, file formats, plugin hooks
   - UI/application mirrors: view models, stores, reducers, component props,
     generated or hand-maintained client models
   - tests, fixtures, docs, prompts, examples, and release notes

   Mark concepts that are duplicated, denormalized, historical, transport-only,
   or renamed-but-still-the-same-old-thing.

   If an active design doc or issue names the target ontology, use it as the
   north star. Reduce toward those nouns. Do not reintroduce explicitly rejected
   old product nouns just because they would make an ambiguity easier to
   describe.

2. Review the public API.

   Read the public contract for this codebase: routes, commands, exported
   functions, SDK methods, component props, configuration, events, fixtures,
   examples, and contract tests. Ask:

   - Does the public API expose the same nouns as the core model?
   - Are there response envelopes that only wrap one durable object?
   - Are old field names, compatibility aliases, route aliases, or parse
     fallbacks still present?
   - Are callers, clients, components, or adapters compensating for an unclear
     provider shape?
   - Do wire/schema/model fields match real behavior, or are fields only being
     carried through serialization and fixture round-trips?

3. Review storage and ownership.

   Follow each core concept through its owner and mirrors. Look for:

   - two tables, types, stores, reducers, services, or components representing
     one product object
   - one object carrying fields owned by another object
   - snapshot structs that only preserve old launch/config inputs
   - status enums that describe the same lifecycle twice
   - optional fields that are really mutually exclusive modes
   - adapters that translate between two shapes the system could make one

4. Find coherent reduction slices.

   For touched files and their direct model/API mirrors, ask:

   - What field is no longer read?
   - What field is duplicated on a parent and child object?
   - What type exists only to preserve an old name?
   - What helper exists only because callers see the wrong shape?
   - What route, command, export, event, config key, or component prop is just
     compatibility?
   - What tests only assert compatibility wiring?
   - What class or function only translates old nouns to new nouns?

   Do not stop after one tiny cleanup if the same concept leaks elsewhere.
   Either take the whole coherent slice or leave it for a larger pass and say
   why.

5. Reduce from the center out.

   Prefer this order, skipping layers this codebase does not have:

   - core domain model and state machines
   - persistence and ownership boundaries
   - public API / exported contract
   - generated or hand-maintained client/UI/application mirrors
   - commands, components, adapters, and display state
   - tests, fixtures, docs, prompts, and examples

   Apply changes directly. Reshape rather than layer. If a compatibility adapter
   seems useful, first try to delete the caller that requires it.

6. Verify.

   Run the smallest tests that exercise the changed model, then broaden. If a
   test breaks because it encodes the old model, update or delete the test. If a
   behavior breaks, the reduction went too far.

## What To Reduce

**Duplicated concepts.** If two names describe the same product object, pick one
and migrate every mirror.

Example: `LaunchResult` that only contains `Session` -> return `Session`.

**Renamed old concepts.** A rename is not a reduction when the old boundary
survives. If `LegacyJob` becomes `ExecutionJob` but still duplicates `Job`,
either fold it into `Job` or make it clearly private implementation state.

Search for old and new nouns together. If both remain in active code, treat that
as a compression target unless one is clearly private implementation state below
the product/API boundary.

**Denormalized fields.** If a child repeats a parent field without independent
meaning, remove it or derive it.

Example: role on both `Run` and `Session` -> keep it in the owner.

**Snapshot/config fossils.** Delete snapshots that only preserve old launch
inputs. Flatten fields that are real state; delete fields that are not.

Example: `JobSnapshot { command, target, old_scope }` -> `Job { command,
target }` plus deletion of stale `old_scope`.

**Compatibility seams.** Old field names, deprecated routes, fallback parsers,
wire defaults, alias methods, and hidden compatibility commands should disappear
unless the design explicitly requires external compatibility.

Example: `old_id` accepted beside `id` -> migrate in-repo callers and remove
`old_id`.

Classes and functions whose only job is translating old nouns to new nouns are
also compatibility seams.

**Mirror drift.** Hand-maintained models across languages, layers, clients,
schemas, UI state, fixtures, or docs should be compared field-by-field. A field
that exists in every mirror but no longer drives behavior is still stale.

**Implementation nouns in product APIs.** Transport/backend details can exist
below the boundary, but they should not name public product objects.

Example: `TerminalSession` -> public `Session`, with `tmux` as connection
metadata. In another codebase, `S3Document` might become `Document`, with S3 as
storage metadata.

**Stale language.** Helper names, tests, prompts, and docs should teach the new
model. Old words in non-public places still train future contributors to rebuild
the old shape.

## Scope

**Stay on the branch's model path.** You may touch files outside the literal
diff when they are direct owners, mirrors, fixtures, or clients of the concept
being reduced. Do not wander into unrelated cleanup.

**Reshape, don't layer.** Restructuring is good. Adding adapters, wrappers, or
compatibility shims is not reducing; it is adding.

**Preserve behavior.** Reduction changes structure and names, not user-visible
capability. If tests break because they encode the old model, fix the tests. If
runtime behavior breaks, back up and simplify less aggressively.

**Be ambitious or do nothing.** Compress should not default to one trivial
sub-50-line cleanup. If there is a real structural reduction, take the coherent
slice across layers. If there is not, make no code changes and report the review
that proves it.

A no-op report must name the model path inspected, the mirrors checked, and why
each suspected reduction was not meaningful.

**Be aggressive.** Make every type, field, and route prove it earns a place in
the final vocabulary.

## Output

Simpler code that passes tests, plus a short report of:

- core model before and after
- APIs/routes/DTOs removed or collapsed
- fields/types left intentionally and why
- tests run

If nothing can be reduced, say so and name the model/API review that proves it.
