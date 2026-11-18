# LOO-923 open choices

Kickoff, 2026-11-18:

- Predecessors: the Task comment of 2026-11-16 says "Begin substantive implementation only after those measured outcomes are available." LOO-921 is idle on its shared-daemon question and LOO-922 is unstarted; LOO-923 was claimed and run on 2026-11-18. Assumed: the extraction proceeds and reports base-against-candidate numbers from `run.py`'s private caches; the comparison with LOO-921/LOO-922 results is added when they exist.
- First extraction is harness formats, not `ops`. `ops` as a crate downstream of execution would save more per ops-only edit (about 20 s of 57.5 s) but lengthens the chain. Is the one-crate-longer chain acceptable for that saving?
- `provider_auth` stays in execution for now because it also holds tracker credentials. Does it belong with the harness, or split by what the credential is for?

Concept review, 2026-11-18 (`scratch/concept-review.html` has these numbered):

- The crate depends on `loopflow-model` only. Assumed: the store-joined half of `transcript/` stays in execution, which makes the crate about 7k lines instead of 15k. Not measured: whether a crate that size moves the ops-edit build outside noise.
- Named `loopflow-harness`, not `loopflow-providers`: `Provider` already means the credentialed service and the rental host in code.
- One struct per harness; `harness::Launcher: loopflow_harness::Harness` in execution. `Capabilities` stays with `Launcher`.
- Line counts for the moved half (about 7k) are from `wc -l` on whole files plus an estimate for the split `mod.rs` and `live.rs`; no build was run in this review.

Write-docs, 2026-11-18:

- `AGENTS.md`'s "never a middleman" rule now also covers `rust/loopflow-harness/**`. Assumed: a decoder sits in the headless stream path, so the rule follows it. The transcript readers are passive; should the rule name only the decoders?
- "Add a provider" is now two implementations in two crates (`Harness`, then `Launcher`) where it was one file. Is a five-step recipe across two crates an acceptable cost for the format-only test loop, which 6 of 215 PRs would have used?

Realign, 2026-11-18:

- `transcript::live::TurnUsage` and `loopflow_model::chat::types::TurnUsage` are two types with one name. Slice 2 puts both within reach of `loopflow-harness`. Assumed: the live one keeps its wire shape and its fields; whether it should become the model's type is not decided here.
