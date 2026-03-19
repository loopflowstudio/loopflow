---
asana_id: '1213718054761190'
linear_id: 6034b87f-bda3-4b74-ab5c-17775f9963b1
---
# 05: PM sync steps and flow

**Finish line:** `lf ops pm pull` is the deterministic PM→repo primitive, explicit push/export surfaces are documented as event-scoped writes, and future step wrappers build on those ops commands instead of re-implementing sync logic.

The core PM stance has shifted: PM is the source of truth for planning state, local wave files are a pulled mirror plus explicit planned edits, and `main` is not a trustworthy merge base for deciding what remote state should become. This item is about turning that stance into clean ops surfaces and later thin step wrappers.

## What to build

### Pull-first ops surface

- Add `lf ops pm pull --wave <wave>` as the deterministic PM→repo command.
- PM wins. Rewrite local numbered roadmap files to match remote order/title/body/existence.
- Do not compare against `main`. Do not attempt three-way inference. Pull is "make local look like PM now."

### Explicit push semantics

- Define push as explicit and event-scoped:
  - manual `lf ops pm push --wave <wave>` from known local diffs
  - PR open / failure / merge comments or completion from stable item IDs
  - no background "guess the diff from main" behavior
- Document that all default usage is pull; push only happens when the user or executor has a concrete payload to send.

### Shared rules

- Do not duplicate PM logic from `ops/pm.rs`.
- Keep provider-role resolution and frontmatter writes in the existing ops path.
- Pass wave selection through unchanged so future thin steps stay transparent wrappers.

## Open design questions

- **Push scope:** Should explicit push start with whole-item text/title updates, or stay narrower until lifecycle writes and ordering semantics are stable?
- **Wrapper naming:** Should future step wrappers be `pull-pm` / `push-pm`, or should flows call the ops items directly?
- **Lifecycle overlap:** For PR-oriented flows that already do run-boundary PM actions, which writes stay in lifecycle hooks vs explicit push commands?

## Constraints

- Ops commands stay deterministic and headless — no agent reasoning.
- Pull is the default day-to-day refresh path.
- Push must be explicit and based on specific local diffs or lifecycle payloads.
- Avoid creating a second default lifecycle that fights the executor's automatic PM hooks.

## Done when

- `lf ops pm pull --wave <wave>` rewrites local roadmap files from PM state
- Pull does not consult `main` or attempt three-way merge logic
- Push semantics are documented as explicit, event-scoped writes
- Future step/flow wrappers can call pull/push ops commands without inventing a second sync implementation
