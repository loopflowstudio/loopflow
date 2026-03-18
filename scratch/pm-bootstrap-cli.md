# PM Bootstrap CLI

## Human-reviewed intent (2026-03-18)

This wave item is not really about adding a large manual bootstrap command surface.
The real goal is to turn PM integration on with minimal decisions, then have loopflow automatically do the right thing during normal work.

## Problem

PM support currently assumes a single provider and mostly one-shot manual operations. That is the wrong shape for the actual workflow.

The desired setup is:

- **Linear** is the single **read/write** PM provider
- **Asana** is an **export-only** provider
- Once PM is enabled, loopflow should:
  - sync from Linear at the beginning of PR-oriented flows
  - write updates back to Linear at the end of PR-oriented flows
  - mirror/export updates to Asana at the end of PR-oriented flows

The user does not want to choose between push, pull, or union during setup. They want PM to be turned on and working.

## Product shape

### Provider roles

The design needs provider roles rather than a single `pm.provider`.

Conceptually:

- one **RW provider**: canonical sync source and destination
- zero or more **export providers**: receive mirrored updates, never drive local state

For the current use case:

- `linear` = RW provider
- `asana` = export provider

Roadmap items already support multiple provider IDs via `linear_id` and `asana_id`. Wave config should likewise support multiple active providers with explicit roles.

A possible config shape:

```yaml
pm:
  rw_provider: linear
  linear_project: "..."
  export_providers:
    - asana
  asana_project: "..."
```

The exact YAML can change, but the roles cannot be implicit.

## Bootstrap behavior

Bootstrap should be **low-decision** and **non-destructive**.

Default behavior:

1. Connect or validate the RW provider project (Linear)
2. Connect or create export-provider projects (Asana)
3. Match local items to provider items by normalized title
4. Write wave config and roadmap frontmatter IDs
5. Create missing items where needed to get into a working state
6. Avoid destructive replacement or deletions during bootstrap

Not the product center for v1:

- push vs pull vs union prompts
- destructive reconciliation
- ingesting from export-only providers

If explicit force modes are ever needed later, they can be separate escape hatches. They are not the default path.

## Runtime ownership

Ongoing PM behavior belongs with the reliable lifecycle path, not only ad hoc CLI commands.

That points toward `lfd` owning the durable semantics:

- beginning-of-flow sync from Linear
- end-of-flow writeback to Linear
- end-of-flow export to Asana
- best-effort comments/completion after PR activity and merge

`lf ops` can still own one-time local mutations:

- writing wave YAML
- writing roadmap frontmatter IDs
- creating or linking projects
- explicit setup/status helpers

But the important feature is the automatic lifecycle behavior after activation.

## CLI implications

The previous `init/link/status` design is now secondary.

Useful helpers may still exist, but they should serve activation of the provider-role model rather than define the product.

Possible direction:

```bash
lf ops pm init
lf ops pm status
```

Where `init` does the conservative attach-and-reconcile flow:

- detect configured provider roles
- connect/create provider projects as needed
- match items by title
- write IDs/config
- leave the system ready for automatic flow hooks

Provider-specific top-level commands are not required.

## Integration points in the current code

Existing pieces that fit this design:

- `RoadmapItemDocument` and `RoadmapItemFrontmatter` already support multiple provider IDs (`linear_id`, `asana_id`)
- wave YAML already has per-provider project fields (`linear_project`, `asana_project`)
- provider clients already implement `PmProvider`
- `lfd` already owns reliable lifecycle behavior elsewhere, which is a better long-term home for PM automation than manual CLI-only orchestration

Pieces that likely need reshaping:

- current config assumes a single `pm.provider`
- current ops flows assume one provider at a time
- current bootstrap design overemphasizes explicit command modes instead of automatic activation

## Scope

In scope for this design:

- provider-role model: one RW provider, many export providers
- low-decision bootstrap to get PM working
- automatic sync/writeback/export at the right lifecycle moments
- Linear as RW, Asana as export-only for the current use case

Out of scope for this design:

- import from export-only providers
- destructive multi-source reconciliation
- general many-to-many sync semantics
- provider-specific command trees as the main UX

## Open questions

- Exact YAML shape for provider roles
- Whether bootstrap lives mainly in `lf ops`, `lfd`, or a thin `lf ops` wrapper over `lfd`
- Which exact flow boundaries should trigger sync/writeback/export first: beginning of work, PR creation, land, merge, or all of the above
- Whether `status` is needed in v1 or whether activation + automatic behavior is enough

## Done when

- A repo can be configured with Linear as RW and Asana as export-only
- Bootstrap gets the system into a working state without asking push/pull/union questions
- Starting PR-oriented work syncs from Linear
- Finishing PR-oriented work writes back to Linear and exports to Asana
- Roadmap items carry both provider IDs where applicable
- The design centers automatic PM behavior, not manual YAML/frontmatter editing
