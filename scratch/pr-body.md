Flows become native chat skills with a readable checklist: headless work runs through LF, while reviews and loop decisions stay in the conversation. Hierarchical names gain dashed fallbacks for Claude without collapsing distinct slash and dash definitions.

## What changes

- Generate Flow recipes during global or repository skill sync, including commands, loops, branches and conversational review steps.
- Prefer exact literal names before normalized fallback. Codex retains slash/dash pairs; Claude gives an explicit dashed definition its literal name.
- Export Flows without a prefix and skip names owned by Loopflow or third-party skills. Preserve authored native controls and existing bundles.
- Remove the intermediate instructions command and bundled skill bodies. Re-sync after Flow edits or repository overrides.

## Checks

Affected CLI, discovery, Flow and Session integration suites: 92 passed, one existing ignored. Formatting and all-target Clippy passed before the generated-artifact repair.

CI on `2abf01d29ed5d4e8f16fe91dbdcd83c26cf3c113` found stale prompt goldens and portable architecture HTML after slash-to-dash command renaming. The Rust job otherwise passed 2,304 tests. Regenerated both artifacts and added their checks to the testing guide.

Repair verification:
- `cargo test -p loopflow --test golden_prompt` — passed.
- `uv run --project website python scripts/render_architecture_html.py --check` — passed.
- `uv run --project website --extra test pytest website/tests/test_portable_architecture.py -q` — 1 passed.

CI on the repaired head is pending. Native Claude/Codex selection and adherence have not been demonstrated; automated checks cover recipe generation and execution contracts.

## Try it

Suggested walkthrough: run `lf sync-skills --repo`, select an exported Flow, and inspect its direct checklist. A Flow with `human: true` should perform that step in the current conversation; a default loop should explain whether to repeat its numbered range or continue.
