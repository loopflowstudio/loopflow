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

## Blocker — roadmap write-back deferred

`lf op pm show --wave goals` fails: `wave/goals/GOAL.md has no pm.asana_project`
(needs `lf op pm init --wave goals`). The design's roadmap write-back (record CLI
result on 1216257471904678; file server-probe + mobile-probe) is owed but blocked
on connecting the project — a setup + external write, out of kickoff scope in a
headless run. Next pass: connect the project, then `lf op pm update` per the
design's "Done when".
