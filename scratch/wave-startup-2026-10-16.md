# Wave startup delay

Jack Heart reported 38 seconds at “Preparing codex” while loading this Infrastructure Wave Session, and proposed large context as a contributor.

Observed October 16, 2026, America/Los_Angeles, installed lf 0.14.2:

- Session `7242191576e1424ebadf2b9350718594`; connect Process `b89aa2f9-75c4-4eeb-8f58-ce469416eded` began 09:16:39.
- Trace `7f96e888-2b04-4515-b033-84d63ddcb54e`; saved resume-instructions file created 09:16:40, 7,008 bytes.
- AgentProcess `5c3cf536-df74-4b6e-936a-f327bd83772c`, Codex app-server, recorded start 09:17:17.
- Thus 38 seconds elapsed before provider launch; most elapsed after instructions were written. This does not identify the exact slow function. Large provider history/model prefill cannot explain time before the provider starts; local history/context bookkeeping remains a hypothesis.
- Recorded context inventory reports 6,060 assembled tokens but carried/provider input is unknown. Do not call this the total conversation size.
- Source reviewed at exact released tag v0.14.2 (`d9edc347f9617150b025b911152cc7ba86d51ecc`), `rust/loopflow/src/lf/commands/util.rs`. Preparing precedes local attachment/store checks; Starting Codex precedes native harness launch. No stage-level trace or controlled context-size comparison yet.

Existing LOO-461 owns outstanding startup verification. It remains unselected; no additional startup worker launched. Preserve this evidence for stage timing and a realistic populated-store comparison, rather than reducing context blindly.

Receipts: `/tmp/wave-startup-origin.json`, `/tmp/wave-startup-agent.json`, `/tmp/wave-startup-context.txt`; saved prompt under `.lf/prompts/20261016-091640-7f96e888-2b04-4515-b033-84d63ddcb54e-DPah9q-resume-instructions.context.md`.

## Follow-up investigation, 09:36–09:40

Jack Heart asked whether investigation was continuing. It had stopped between turns; this pass resumed read-only diagnosis.

- A fresh `lf wave status infrastructure` remained running beyond three minutes. A one-second sample put all 880 main-thread samples in `Store::list_waves → CanonicalRepo::discover → realpath → __getattrlist`. This is filesystem path resolution, not a sampled SQL or tokenizer bottleneck. Receipt: `/tmp/startup-wave-query-sample.txt`.
- `Store::list_waves(Some(repo))` loads every Wave, then canonicalizes each nonmatching stored repository path. Frozen post-cleanup state retains 32 missing historical repository paths. An independent read-only path-resolution pass found missing temporary paths taking 0.714, 2.707, 2.954, 4.537, 3.555 and 4.478 seconds each; existing repository paths resolved below the displayed millisecond precision. Other missing paths were fast. This demonstrates a status-read bottleneck; it does not yet establish the exact original resume call stack.
- Three actual Session lookup components (legacy feedback, current capture and attachment) against the frozen post-cleanup backup used indexes and took 0.0006–0.0007 seconds each. This rules out those isolated queries in that snapshot, not all database work or live lock contention.
- A disposable Python temporary-directory experiment took 10.056 seconds for its first preparation/create and 0.001/0.000 seconds on the next two; resolution and removal were immediate. The first measurement includes Python's temporary-root discovery/probe. Do not misreport it as a measured Rust `tempdir` call or assume equal causes. All three experiment-owned directories were removed.

The original 38-second startup cause remains unproved. Native startup allocates its socket directory under the same macOS temporary root; local filesystem latency is now a concrete lead. No context was removed, no production code changed, and no live records were deleted. Next decisive proof is a sampled actual controlled resume using an isolated Session, separating path resolution, attachment/store work and provider handoff.
