# Open questions — prove-the-language

Headless run; assumptions made and proceeding.

- **Asana id 1216257471904678 maps to `3-vocabulary-completeness.md`** (confirmed
  via `asana_id:` frontmatter in that file). Treating the "three reference
  builds" acceptance test as the work item.
- **Kickoff = design deliverable, not implementation.** This session produces the
  design under `scratch/`; the probe run and the atom PRs are dispatched work.
- **Smallest falsifying probe = a CLI.** Assumed over server/mobile because it
  isolates the two most fundamental missing atoms (scaffold, run) from the
  client/server seam and platform-build machinery. Corrected in review if wrong.
- **New atoms are builtin markdown steps, not scripts.** Assumed because
  `build.rs` auto-registers `build/step/*.md` and every existing step is a prompt.
- **Probe runs in a throwaway temp dir**, not this repo, so greenfield git
  assumptions and product code never touch loopflow.

## Design-review reshapes (headless review pass — confirm in a human pass)

Made these executive calls against the code; a human should sanity-check them:

- **Dropped `design` from the `greenfield` flow.** Verified `design` is
  `interactive: true` (design.md:2) and headless flows park on interactive steps
  (`FlowAction::WaitInteractive`, flow.rs:272), so the original scaffold→design→…
  chain would stall the probe at step one. Reshaped to
  scaffold→implement→run→gate, with `scaffold` seeding `scratch/<branch>.md` from
  the GOAL. *Soft point:* this loads a design-brief responsibility onto
  `scaffold`. Fallback (documented in the doc) is a non-interactive `design`
  override inside the flow. A human should pick.
- **Split "done when" into a CI merge gate vs a manual acceptance experiment.**
  The full agentic greenfield build can't be a `cargo test` gate (paid, slow,
  nondeterministic). Merge gate is now the registry test only; the probe run is a
  one-shot experiment whose result goes on the roadmap. Confirm this is the
  intended contract before implementation wires a "probe green" check into CI.
- **Metric made binary (empty `.lf/steps/`), not a stall count.** A headless
  agent improvises around gaps, so counting stalls isn't measurable. Confirm the
  team is fine losing the "gap count = 2" narrative in favor of the observable
  empty-directory proof.

## Blocker — roadmap write-back deferred

`lf op pm show --wave goals` fails: `wave/goals/GOAL.md has no pm.asana_project`
(needs `lf op pm init --wave goals`). The design's roadmap write-back (record CLI
result on 1216257471904678; file server-probe + mobile-probe) is owed but blocked
on connecting the project — a setup + external write, out of kickoff scope in a
headless run. Next pass: connect the project, then `lf op pm update` per the
design's "Done when".
