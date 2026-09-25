# Ordered implementation cycle

Human authorized repeated **implement → compress → review-slice**, each as a
separate subagent. Parent invokes bounded `lf --task LOO-291` skills sequentially
in the existing worktree and inspects each handoff. No new Task, worktree or
managed Flow is created. Governing design: `scratch/main-view-task.md`;
prototype source reference: `scratch/visual-study/accepted-reference.json`.

Cycle 1 subset: shared Session naming. Seed names from invoked skill, otherwise
recover the historical magical musical animal generator. Persist exact Session
title and generated/human provenance with existing canonical ownership. Provide
shared rename/readback and operating guidance for improving generated names;
preserve manual names including races. Mirror the contract to Swift. Prove via
isolated real CLI/shared read with no live client mutation. A small usable CLI
slice is valuable; native breadcrumb/rename integration follows in the next cycle.

All phases preserve existing dirty work; no blanket staging or checkpoint of
unrelated contributions. No publication, installed app replacement, PM mutation,
Session transfer or Task completion. Review must report remaining full-design
obligations rather than approve the whole Task from one passing subset.

Phase receipts: cycle-01-implement.md, cycle-01-compress.md,
cycle-01-review.md. Each records changed paths, exact proof commands/results,
concrete findings, and the next bounded slice. Parent owns progression.

## Live ledger

- Cycle 1 implement completed (2026-09-25): shared title/provenance, recovered
  magical-musical pair generator, rename/suggest CLI, Swift mirror and operating
  guidance. Focused Rust/CLI/race/golden and 34 Swift tests pass; see its receipt.
  Generator history contained no animal list. Recovery across replacement Runs
  and remote Home behavior remain review targets; no full-design approval.
- Cycle 1 compress started after implement exited successfully. Independent
  review-slice follows its completed handoff; no phase runs concurrently.
- Cycle 1 compress completed: required Run-ID plumbing simplified; CLI suggestion
  messaging now uses provenance. Five CLI and 15 focused Rust tests pass. It
  identified replacement-Run name loss and unproven agent-command/Home cases.
- Cycle 1 review-slice started after compress exited. It must reproduce those
  gaps and repair bounded defects before UI work depends on the naming contract.
  Parent launcher log: `/tmp/loo291-parent-cycle01-review-output.log`.
- Cycle 1 review-slice completed: fixed `$LF_RUN_ID` rename inside Ask/Flow,
  human names lost on replacement Runs, a rename/replacement race, and opaque
  remote-Flow errors. 6 CLI + 15 lib tests, clippy pass. Flow-specific proof
  remains source-only. See cycle-01-review.md.
- Cycle 1 review completed: Run-ID boundary lookup, replacement name carry-over
  and rename/preparation serialization fixed. Six CLI and 15 Rust tests pass;
  five reviewed source hashes independently matched. Flow-specific behavior and
  remote read truthfulness remain explicit gaps, carried into cycle 2.
- Cycle 2 implement starts the named native Session slice. See cycle-02-brief.md;
  parent updates only the design's current slice, retaining the full target.
- Human steering during cycle 2: the continuation branch's second loop after
  demo is correct. Keep building and defer the two-loop UI discussion until the
  working build is ready. Final Advance → queue → land is confirmed; final
  Iterate returns to implement. Session implementation continues.
- Cycle 2 implement completed: native breadcrumb/rename, shared membership,
  88 Swift tests and focused Rust/CLI/DTO proof pass; remote/Home and replacement
  coverage remains for review. The parent is integrating the continuation branch
  locally before compress/review so those phases assess the intended runtime.
