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
