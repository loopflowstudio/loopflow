## Usage

```yaml
# ship-roadmap flow — land is now a direct ops call, not an agent session
- ingest
- kickoff
- review-design
- build
- review
- ops: land --create-pr
```

```bash
# gate writes PR copy that land consumes automatically
lf gate                    # → scratch/pr-title.txt, scratch/pr-body.md, scratch/.pr-copy-ref
lf ops land --create-pr    # reads cached copy, no --title/--body needed
```

## Summary

Adds `ops:` as a first-class flow item type. Flows can now execute mechanical operations (land, rebase, release) directly via Rust functions — no agent session startup cost. Gate writes PR copy files to scratch/ for handoff. Land reads them, validates freshness via SHA check, and clears scratch/ after consuming. Release notes are now narrative via a `release-notes` step invoked as a subprocess.

## Changes

- `FlowItem::Ops(OpsItem)` variant with YAML parsing, expansion, and execution in both CLI and daemon flow runners
- `gate` step extended to write `scratch/pr-title.txt` + `scratch/pr-body.md` + `scratch/.pr-copy-ref`
- `lf ops land` reads cached PR copy, validates against HEAD SHA, falls back to CLI args
- `release_run` invokes `lf release-notes` subprocess for narrative notes
- New `release-notes` builtin step
- Python `FlowStep.from_raw()` handles ops items; Swift `FlowProgressPills` renders with distinct styling
- Updated `ship-roadmap`, `ship`, `release` flows to use `ops:` items
- Tests: flow parsing, land cached PR copy, release tag idempotency
