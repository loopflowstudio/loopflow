# LOO-673 batch 2 demo: drag, abandon, unassign (2026-11-02)

Branch `jack/loo-673-desktop-polish-fix-jacks-running-feedback` PR #1791; the CLI comparison below ran at `cf8000104a`, the Dev app runs `70bf7652b0`.
Page: `scratch/demo.html`. Intake and audit: `scratch/jack-loo-673-desktop-polish-fix-jacks-running-feedback.md`. Remaining duplicate derivations: LOO-696.

## Proven on the real store (CLI, no app)

Branch `target/debug/lf` vs installed `lf 0.15.1`, read-only against Jack's shared store:

- Repository default selection (the sidebar's Working list) on the installed release: LOO-672, 673, 515, **312**, **330**, **558**, 691, 705. LOO-312, LOO-330 and LOO-558 have no owner.
- Same selection on the branch: LOO-672, 673, 515, 691, 705. The three unowned Tasks are gone; they sit in Backlog.
- `state:working` and `state:autonomous` resolve on the branch (Autonomous: LOO-500, 620, 693); the installed release resolves neither.
- `lf wave show product`: branch groups WORKING 3 / AUTONOMOUS 1 / BACKLOG 33; installed prints NOW / WAITING / NOT STARTED / LATER.

## Not yet observed by Jack

Drag (item 13) and abandon (item 14) are proven by Swift tests only (`draggedRowLandsOnRelease`, `droppedRowMovesBeforeTheWrite`, `abandonLeavesTheSidebarAtOnce`). On 2026-11-02 Jack allowed agents to start Dev apps. The first launch from `cf8000104a` died at launch (dyld: missing `Sparkle.framework`; crash report `Loopflow-2026-11-02-131738.ips`), and the agent wrongly reported it open after seeing the process start; Jack said "i dont see it". Main had fixed the embedding in #1794 and #1796. After rebasing, the Dev app from `b86ae49d40` stays up with a window on screen. Store frontier checked: installed lf 0.15.1, latest release on the branch v0.15.0, no migration in the diff.

Jack then asked for `lf sync`, which merged main (`70bf7652b0`, through #1797) and pushed. That merge has no Swift or migration change. The Dev app from `b86ae49d40` then exited without a crash report, cause unknown; it was relaunched from `70bf7652b0` and was up with a window after 45 seconds. Production Desktop exited at 13:05, before any of these launches; a separate `lf unbreak` run handled it and production has run since 13:22. After the rebase and merge: 116 Swift tests in the four touched suites pass; Rust `work::selection`, `durable::` and `waves::` lib tests pass with inherited `LF_*` cleared (3 `durable::` tests fail if some stay set: a fixture-environment trap). On #1791, 2 checks pass and 15 skip.

Jack said "not open" after that. Crash report `Loopflow Dev-2026-11-02-133856.ips` and `lf doctor`: pids 5201 and 42459 both died by SIGILL 5 minutes after launch. Cause: since #1773, `lf_write` parses the whole `lf` command line in-process; a debug build's clap frame is 375 KB (pc is the stack probe at offset 0x14 of `Commands::augment_subcommands`, data abort at sp) on a 512 KB Swift task thread. `LoopflowLibrary.call` now runs on an 8 MB thread; `LoopflowLibraryTests` crashed before and passes after. Pid 79544 from the fixed build was still up with a window 5m44s after launch. Release builds were not measured.

Jack's 14:05 screenshot of that build: "Invalid selections watch frame ... WorkSelectionState from invalid String value working", and "No working Tasks in this list". A batch 2 bug: Rust gained `working`/`autonomous` selection states without the Swift mirror, and no fixture carried them. Fixed; `selection_states.json` now lists every state, checked against the Rust schema and decoded in Swift. Agent could not screenshot the window (no screen-recording permission), so Jack's eyes are the check.

## Jack's observations

(pending)

## November 2 evening exit investigation

Jack Heart reported another apparent crash. The polish Dev process (pid 483,
launched 14:11 PST) recorded `reason: quit` at 16:02:32. The shared Dev bundle
was subsequently replaced for LOO-500's Wave lifecycle demo; its executable was
modified at 18:21:13. That checkout's `scratch/wave-lifecycle-demo.md` records
failed private-world launches and LLDB evidence at 18:25:58: EXC_BAD_ACCESS in
`Commands::augment_subcommands` on a Swift cooperative worker. Its
`LoopflowLibrary.swift` still uses `Task.detached` and lacks this branch's
8 MB thread fix (`e6a8273bc6`). This matches the earlier stack-exhaustion failure;
the fix has not yet been verified in the LOO-500 build.

Later launches against the shared home failed the unmerged `wave_removal`
migration guard. That is separate from the private-world crash; the guard
refused access and no migration was applied by this investigation. The final
shared-home attempt recorded SIGTERM at 18:25:42. No Dev process was running
when checked at 18:28. Do not equate dump file modification time with crash time:
Ghostty exports previous crashes on the next launch. The intended demo (polish
on real data or Wave lifecycle on its private world) needs resolving before
replacing this shared app again.

## Next action

Jack tries the three runbook steps in the Dev build; failures go back to LOO-673 as batch 2 rework, acceptance lets #1791 land (15 of 17 CI jobs skip on the PR; recheck before landing).
