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

Implement, slice 2, 2026-11-18:

- `HarnessKind` is no longer `#[non_exhaustive]`, against the style guide's rule for public enums that may grow. Assumed: in a private workspace crate an exhaustive match is worth more, since a fifth harness then fails to compile until it has a launcher. Keep the attribute and a wildcard arm instead?
- `recognizes(&OsProcess)` stayed on `Launcher`. The alternative keeps it on `Harness` by passing the command, executable path and arguments in, which reads the process table for every candidate before asking. Not done.
- The live `TurnUsage` was not renamed: the rename reaches the generated server schema.

Implement, slice 3, 2026-11-18:

- The comparison ran with `--cohorts 3`, not five. The one-minute load average was 33-43 on 16 CPUs from other checkouts' builds when it started, and five cohorts at one Cargo job is about three hours on an idle Machine. Assumed: three samples and their recorded load say as much as five would under that load. Rerun with five on a quiet Machine before the numbers decide the next extraction?
- `probe.py` did not gain `rust/loopflow-harness`. Its workspace is the storage chain (model, definitions, terminal, storage, test-support), none of which depends on the crate; a member it never builds would check nothing.
- `scripts/test.py` and `scripts/macos_inputs.py` needed no change. The runner selects Rust on any `rust/` path and runs nextest over the workspace; the Desktop inputs name wire files, none of which moved. What did need saying is the filter: the crate's tests are not under `harness::`, so gate uses `package(loopflow-harness) | test(harness::)`.
- `harness_test` times a second edit after an untimed first test build. Without that, the baseline's sample is mostly execution's dev-dependencies compiling once, which no edit loop repeats.
- `report.py` no longer writes `scratch/measure.html`; the committed page under `results/` is the one to open.

Realign after slice 3, 2026-11-18:

- The measurement landed on the outcome the plan named in advance: an ops edit moved -0.8 s and a storage edit -0.2 s, inside their ranges. Assumed: the crate still ships, for the decoder test build (35.6 s to 1.2 s, 4.76 GiB to 0.32 GiB) and because the Task asks for one representative extraction with its measurement. Ship it, or drop the boundary and file `ops` as the next Task with nothing merged?
- `origin/main` moved to `a0548bfe28` after the comparison ran; the merge touched storage and `store/session_projection.rs` only and had no conflict. Assumed: the committed results stand without a rerun, since neither revision in the comparison includes #1977.

QA, 2026-11-18 (`scratch/qa.html`):

- A successful Codex turn was not reached: the account was over its usage limit until 3:36 PM. Rerun `lf -b -a codex : '<prompt>'` with the branch `lf` before merge?
- Seen on the installed build too, not changed: `lf session usage` shows `-` for Pi Sessions whose `turns` row holds tokens and cost. Its own Task?
