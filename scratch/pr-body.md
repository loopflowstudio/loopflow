Flows become native chat skills with a readable checklist: headless work runs through LF, while reviews and loop decisions stay in the conversation. Hierarchical names gain dashed fallbacks for Claude without collapsing distinct slash and dash definitions.

## What changes

- Generate Flow recipes during global or repository skill sync, including commands, loops, branches and conversational review steps.
- Prefer exact literal names before normalized fallback. Codex retains slash/dash pairs; Claude gives an explicit dashed definition its literal name.
- Export Flows without a prefix and skip names owned by Loopflow or third-party skills. Preserve authored native controls and existing bundles.
- Remove the intermediate instructions command and bundled skill bodies. Re-sync after Flow edits or repository overrides.

## Checks

Affected CLI, discovery, Flow and Session integration suites: 92 passed, one existing ignored. Formatting and all-target Clippy pass. The full library run (without inherited `LF_CAPTURE_KEY`) had 1,723 passes, seven planning-fixture failures and nine ignored tests. All seven failures passed in an isolated serial run (14 planning tests). Full-run errors involved missing fixture paths and fixture credential reads; required CI still owns the clean full-matrix result.

Native Claude/Codex selection and adherence have not been demonstrated; automated checks cover recipe generation and execution contracts.

## Try it

Suggested walkthrough: run `lf sync-skills --repo`, select an exported Flow, and inspect its direct checklist. A Flow with `human: true` should perform that step in the current conversation; a default loop should explain whether to repeat its numbered range or continue.
