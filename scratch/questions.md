# Open choices (LOO-924, headless)

- Renamed stream trait: `Driver` (`claude::Driver`, …); `Conversation` already names `transcript::session::Conversation`.
- A fifth harness needs one file, one `HarnessKind` variant and one registry line; no link-time registration.
- Stacked 2026-11-16 on `jack-heart/cli-prox` (#1953). Jack: "yeah thats fine". #1953 has since merged (4a0691a9a0) and main is merged in, so the PR base is main. Earlier dependency investigations are in 3b735e67d6:scratch/questions.md.
- The deletion check for provider `SessionCommand` must be scoped to execution code: `loopflow::lf::SessionCommand` is the unrelated Clap command enum and remains.
- No `Placement` wrapper: `Launch` carries `mode` and `holder`; `Delegation::Here` carries the resolved `mode`. `Placement` already names the `machine:` setting and a pane container.
- Slice 3 finding (2026-11-16): `implicit_binding` cannot produce `Bound`; `select_execution_work` produces `Checkout` or calls `select_work` for inherited `LF_AS`, which produces `Declared`. Outside a Task checkout, `LF_AS=task:<id> lf <prompt>` has no CLI selector: `bin/lf.rs` passes `named_work=false` to delegate, but `run_bound_prompt` later uses `Declared.is_managed()=true` and `Holder::Server`. Resolving Place from that same managed fact would change today's bare placement into held/detached. Call made headless (concept-review, 2026-11-16): this Task keeps the entry as it is, so Place reads `named` and Holder reads the Work source; slice 3 proceeds. Still open for a person, outside this Task: should that entry be bare, as `server/README.md` and `transport/README.md` say of every unlisted launch, or stay `Holder::Server` because `LF_AS` is also lf's own declaration for workers it starts (`pr_landing.rs:354`)? Relevant paths: `ops/run.rs::{select_execution_work,select_work}`, `run.rs::{implicit_binding,run_bound_prompt}`, `bin/lf.rs` delegate call.

- Slice 3 counterexample (2026-11-16): headless `lf -b resume ID --message TEXT`
  uses only `connection.work_flag(cli).is_some()` for placement and Holder
  (`lf/commands/session.rs::resume` → `run.rs::resume`). Reopening a managed
  Session by ID without selectors therefore stays here + Caller; native resume
  by ID uses `store.is_work` and can be Server. The earlier design's claim that
  every reopened Session reads its stored Work was too broad. Preserve this
  behavior under "behave as before": headless resume passes no Session to
  `transport::holder`; add both resume modes to the final launch matrix.

- Slice 4 implementation (2026-11-16): use the design's two-runner option over
  the same `Launch`/`Harness`: native returns an OS `ExitStatus` (signals and
  shell status included); the stream loop returns answer prose and a typed
  provider-turn failure used by retry policy. No synthetic OS status or
  mode-tagged result. Native commands are prepared `std::process::Command`
  values, with an owned OpenCode observer companion that closes and joins on
  every exit path. The headless matrix waits here; transport fixtures own
  actual held-terminal exec coverage.

- Slice 4 implementation (2026-11-16): `Launch` carries the resolved Holder;
  `transport::holder` remains its single resolver after binding. A constructor
  does not reinterpret Work: headless selectors and native stored Session Work
  deliberately differ. The earlier draft's constructor-removal note gives way
  to the design's resolved `Launch { holder, .. }` form.
- Deletion-check scope (2026-11-16): the old `holder: crate::session::Holder`
  grep also matches function parameters, including the unchanged
  `ContextDelivery::hook_settings`. The removed authority is AgentConfig's
  field, not required Holder arguments passed to a launch.

- Concept review (2026-11-16, headless): `Launch::mode` is dropped in slice 6.
  The Task outcome says mode is "resolved once, at `transport::delegate`, into
  a launch value the harness receives". With two runners, the resolved mode
  selects the runner and the harness receives it as `open` or `driver`; the
  field had no reader but two refusals. Simpler option taken; reversible by
  restoring one field.
