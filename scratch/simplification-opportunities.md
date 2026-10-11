# Simplification Opportunities

October 10, 2026. Jack Heart requested a reduction assessment after reviewing the
net additions. Proposal only.
The ship Flow was interrupted before landing; its uncommitted gate repairs remain.

## Product intent

Agents conduct Work through the CLI. Desktop retains Work tabs across Machines;
planning synchronizes while execution stays on its recorded Machine.

## Opportunity 1: Finish the existing planning reduction before composing it

**Misalignment:** this branch carries the older peer enrollment and mixed
Linear/Git evidence model. The accepted LOO-412 direction selects one transport per
repository and one typed Work writer.

**Symptom:** `store/sqlite/planning_peers.rs` adds 9,899 lines, including 7,433 lines
in its test module. Its production paths include `use_peer_planning`, selection
conflicts, provider evidence acquisition, creation-receipt projection and separate
field projection. `planning_exchange.rs` also reconstructs provider-shaped fields.
These are substantially inherited dependency code, not Desktop control.

**Realignment:** consume LOO-412's completed canonical Work/writer and configured
transport cuts, preserving this Task's delegation and Desktop behavior. Delete
superseded enrollment, mixed-provider projection and their exclusive fixtures at
that integration boundary. Do not implement a competing rewrite here.

**Cascade:** fewer planning representations, identity conversions, SQL projections,
setup commands and fixture matrices. LOO-412 owns this cut. The previous integration had 33 conflicts; savings are unmeasured.
Preserve causal conflicts, uncertain effects, historical mappings and execution.

## Opportunity 2: Explain the operation's own decision

**Misalignment:** effect-free explanation sometimes narrates a parallel operation
path rather than rendering the decision execution already needs.

**Symptom:** `ops/task/{explain,planning_explain,checkout_explain}.rs` add 914 lines;
planning explanation repeats request matching, field lists and intended effects.
It already shares validators; those are useful foundations, not duplicate checks
to delete indiscriminately.

**Realignment:** where execution and preview duplicate a decision, return that
concrete read-only decision from its current owner and consume it in both paths.
Keep execution revalidation. Do not build a generic command-plan framework or
shrink coverage by deleting `--explain`.

**Cascade:** fewer parallel branch trees and fewer opportunities for explanation
to drift. Begin with Task edit/refile, not every command. LOO-397 already owns the
broader explanation work. Savings are unmeasured.

## Aligned areas

Keep Work-keyed workspaces, content-owned locations, the multiplexer and Ghostty
surfaces. Snapshot selection and live lifetime validation have distinct jobs.

Check: read-only source/design review; no product changes or test reruns. Against
base `5f44fd9e`, source files add about 23,050 net lines; trailing Rust test modules
account for about 8,853, leaving roughly 14,200 non-test source lines (comments and
blanks included). Counts are estimates, not a promised deletion budget.
