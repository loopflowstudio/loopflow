# Open choices (LOO-924, headless)

- Renamed stream trait: `Driver` (`claude::Driver`, …); `Conversation` already names `transcript::session::Conversation`.
- A fifth harness needs one file, one `HarnessKind` variant and one registry line; no link-time registration.
- Stacked 2026-11-16 on `jack-heart/cli-prox` (#1953). Jack: "yeah thats fine". #1953 has since merged (4a0691a9a0) and main is merged in, so the PR base is main. Earlier dependency investigations are in 3b735e67d6:scratch/questions.md.
- The deletion check for provider `SessionCommand` must be scoped to execution code: `loopflow::lf::SessionCommand` is the unrelated Clap command enum and remains.
- No `Placement` wrapper: `Launch` and `Delegation::Here` carry `mode` and `holder` as fields. `Placement` already names the `machine:` setting and a pane container.
- Unverified: whether a binding found by `implicit_binding` can be `WorkSource::Bound` while `delegate` saw `named_work = false`. Slice 3 checks each entry; a differing holder is recorded, not silently changed.
