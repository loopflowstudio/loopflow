## Try it!

```bash
lf pr                  # show current branch PR state
lf pr open             # create/update the current branch PR
lf pr submit           # prep for a human merge
lf pr land             # hands-off merge path
lf wt list
lf wt create next-thing
lf commit -m "message"
lf release run patch
lf pm show --wave designer
```

Flow operation items use the same command payloads:

```yaml
- gate
- op: pr land --create-pr
```

Validation run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
uv run python scripts/test.py
uv run python scripts/test.py --all
```

The full matrix passed through Python, Rust, website, Swift package, and e2e smoke. The Loopflow UI Xcode test built and launched, then the UI runner sat idle at 0% CPU in this headless session; I interrupted it and killed the orphaned runner.

## Intent

Remove the `lf op` drawer from the human command API and make the common mechanical commands first-class. Humans, builtin flows, webhook execs, docs, prompts, and smoke tests now use the same command grammar.

## Assumptions

`op:` remains the flow-step marker; only its payload changes to the new CLI grammar. Historical release notes keep old `lf op` mentions because they describe old shipped behavior. The implementation currently keeps several plumbing verbs as top-level commands (`next`, `advance`, `branches`, `sync`, `doctor`, `sync-skills`, `shell`) even though the scratch design suggests some should die; reviewers should confirm that product choice.

## Key decisions

Bare `lf pr` reports status, while `lf pr open`, `lf pr submit`, and `lf pr land` carry the mutating lifecycle. PR operations keep the existing conflict-recovery behavior. Webhook-planned commands now have a regression test that catches stale argv falling through to external subcommands.

## Not included

No `lf op` compatibility shim, no old flow-payload migration, and no edits to historical release artifacts.
