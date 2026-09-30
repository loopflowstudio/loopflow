Conversation identity and Task history previously depended on several stores and sidecar readers. This change gives each process, conversation and started Flow one SQLite owner, preserving conversation identity through driver handoff and retries while making execution history searchable and bounded.

## What changes

- Exec records actual processes and causal ancestry. AgentSession owns conversation identity, captures, outcomes and usage. FlowSession owns the captured graph, progression and exact successful completions; Task and taskless execution share its driver.
- Session rename and write-once bind use the same owner, including retained completed Tasks. Binding affects future usage, and passive inspection does not start work. SQL readers filter and page before loading payloads; cursor pages retain ID order after renames.
- Linear Project status owns Chapters and default Flows. Rotation preserves started Tasks and their checkout, PR and execution; uncertain backlog stays unresolved.
- Rust/Swift models, fixtures, terminal retention and documentation move together. Retired Run owners, historical import, the separate Task provider launcher and child-pass FlowSessions are removed. Three direct migration groups retain current database state.

## Checks and limits

The full release-materialized local Rust run completed without fail-fast: 2,004 passed, seven failed, 17 skipped. All seven failures were repaired and passed focused follow-ups. The full Swift run recorded 290 passes and one obsolete import-fixture failure; all eight observation tests passed after repair. Original failed receipts remain failed. Python passed 310 tests; website passed 78 with three skips. CLI smoke, Swift/Xcode app and test-runner builds, boundary checks, eight distinct fixture captures, formatting, final all-target Clippy, architecture and immutable-migration checks passed.

[Gate evidence and review findings](https://github.com/loopflowstudio/loopflow/blob/31997bcade4ca7e48d774687ab644170484d84bb/scratch/integrated-gate.md) retain the exact scope, fixture-isolation incident and repair limits. A misconfigured sync fixture invoked a real conflict agent in a disposable repository; its output reported no push. Provider stubs now contain that path; native credential effects were not audited. Required hosted CI must pass on the landing candidate.

The integrated production-prefix estimate is +6,202 lines (+22,792 / −16,590), including SQL and excluding tests/docs. [Earlier debug density measurements](https://github.com/loopflowstudio/loopflow/blob/253aa0c40/scratch/density/README.md) used 20,000 Sessions, 5,000 Flows and 100,000 Execs: bounded warm CLI reads were 291–313 ms against a 300 ms empty-store baseline. These measurements name their earlier candidate and do not establish release or Desktop latency.

Configured provider/Desktop continuity and live Linear rotation remain unproven. This PR does not install or migrate the main Home. Release conversion still requires a frozen database/filesystem rehearsal, quiesced writers, a matching executable and backup, and verification of current Tasks, routes and native identities before writers reopen.
