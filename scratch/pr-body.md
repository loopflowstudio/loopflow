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
lf pm show --wave systems
lf pm task create --wave systems --project ops --title "Tighten the gate"
```

Flow operation items use the same command payloads:

```yaml
- gate
- op: pr land --create-pr
```

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
swift test --package-path swift -Xswiftc -gnone
uv run pytest python/tests/
uv run python scripts/test.py --all
```

Current local result: Python, Rust, website, Swift package, and e2e passed. The
Loopflow UI Xcode job builds and passes app/unit tests here, but the
`LoopflowUITests-Runner` exits before bootstrapping in this headless session.

## Intent

Remove the `lf op` drawer from the human command API and make common mechanical
commands first-class. Humans, builtin flows, webhook execs, docs, prompts,
Swift-launched session commands, and smoke tests now use one command grammar.

## Assumptions

`op:` remains the flow-step marker; only its payload changes to the promoted CLI
grammar. `sync-skills` remains hidden instead of documented because the install
path still needs a direct non-interactive sync command. Historical release
notes and recorded fixtures keep old `lf op` text because they are shipped
records or captured data.

## Key decisions

Bare `lf pr` reports status, while `lf pr open`, `lf pr submit`, and
`lf pr land` carry mutating lifecycle actions. Main's newer Linear task/project
surface was preserved under `lf pm` during the rebase. The retired human CLI
verbs (`next`, `branches`, `doctor`, `shell`, `cp`, `push`, `sync`, and queue
reconcile) are gone from clap; machine callers use library paths or validated
new argv.

## Not included

No `lf op` compatibility shim, no old flow-payload migration, and no edits to
historical release artifacts. The branch was rebased locally and not pushed.
