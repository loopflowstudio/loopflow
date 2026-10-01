# Context budgets

Implementation design, 2026-09-30. Jack Heart requested queryable, configurable
budgets and active upkeep of Wave memory and Task scratch (LOO-356).

Use the existing context-budget module for defaults, resolution, measurement,
excerpt feedback, and input enforcement. Add `context_budgets` to existing
personal/repo config and Wave GOAL.md frontmatter; merge each field, with Wave
above repo above personal above defaults. Expose every token and byte ceiling.
`lf context` previews a selected skill (realign by default), using local Task
context when available, without launching a provider or refreshing Linear.
Show original and submitted usage, overages, and each limit's source. Oversized
total input must remain inspectable even though launch rejects it.

Prompts carry the budget snapshot before excerpts. Update realign, compress,
kickoff and implement to curate within it and re-query after edits. Recompute
feedback each launch; no second state store or sticky cleanup queue. Preserve
live decisions and unresolved evidence; prune obsolete stacked-parent notes.

## Delete — do not maintain

- Replace standalone context ceiling constants and call-site assumptions with
  the shared resolved limits; retain exact source preservation and trace decisions.
- Remove separate source-measurement bookkeeping and repeated per-comment budget
  resolution. The report owns source measurement; each pending steer batch shares
  one resolved configuration. These cuts are complete.
- Do not restore retired record-learnings; realign owns memory curation.

## Implemented and reviewed

`lf context [--wave NAME | --task ISSUE] [--skill NAME] [--json]` uses the
same assembly and budget resolution as launches. Config supports all existing
ceilings with per-field provenance. Launches and live steers enforce resolved
limits; writer prompts receive usage, overages, and curation guidance.

Review found two necessary corrections: ordinary memory reads now use the
execution checkout so curation reaches the next launch; query reads a Task seed
without a live Flow claim, allowing another Task to be inspected without taking
its execution authority. Source excerpts still retain complete local copies.
Totals in the prompt explicitly precede the feedback notice; CLI totals include
that notice and structured reply guidance. Native instruction files delivered by
the provider remain outside Loopflow's assembled-input measurement.

Compression review consolidated bounding and reporting for present and absent
sources, reused the checkout-aware memory reader in the query, and removed
repeated Task/config reads within live steer batches. Budget-resolution failures
now have a distinct log message from source-preservation failures; either leaves
undelivered direction available for retry.

Local query evidence: the current Task resolves to intelligence. Infrastructure
memory in this checkout measures 22,272 tokens / 104,980 bytes against 8,000 /
65,536; the submitted excerpt is 7,946 tokens / 37,013 bytes. This establishes
measurement and feedback, not successful semantic curation.

## Remaining acceptance

- Gate complete: affected suites and focused repairs pass; no check deferred.
- Jack Heart's supervising session reviewed the built query, configuration and
  writer feedback and directed shipment in Linear comment
  `f5cd6421-355b-46a5-9b0d-f91ebc7ab0a5`. The supervisor owns real curation of
  infrastructure memory after landing. Live curation is not a gate prerequisite;
  this branch does not claim semantic preservation proof for infrastructure
  memory or LOO-303 scratch.

Gate review found the new CLI family missing from the architecture ownership
map and Wave-resolution test registry. Added its existing config, assembler and
local-source owners; no new store. The registry now classifies the authored-file
preview separately from registered-Wave operations. A CLI regression proves
unregistered Wave preview, per-field provenance, source and total overages, and
fresh measurements after edits.

LOO-303 query could not read the stored Task checkout: `git diff --name-only -z
746418d8d934d9c9c6f8830f3e95cdd06d8d1df2..HEAD` failed with `No such file or
directory (os error 2)`. Demo needs an available copy of that evidence; no other
Task checkout was repaired or modified.

Checks: `uv run python scripts/test.py --base de074a2 --reuse-passing` — Rust 2,028 passed / 1 registry failure / 17 skipped, website 78 passed / 3 skipped, architecture missing CLI entry; repaired failures pass with `uv run python scripts/check_architecture.py` and `cargo nextest run -p loopflow --test global_commands --test wave_resolution_matrix -E 'test(context_budget_preview_reads_authored_wave_without_registration) | test(registry_is_complete)'` (2 passed); final `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, and `git diff --check` pass; portable architecture regenerated without a diff; no CI deferral, semantic curation remains supervisor-owned after landing.
