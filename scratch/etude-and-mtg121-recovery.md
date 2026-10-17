# Etude and MTG-121 recovery — October 17, 2026

Jack Heart requested unbreak for both failures.

Etude: `/Users/jack/src/etude-sdk` is a symlink to `/Users/jack/src/etude`. Git confirms the main checkout and `etude.logging-and-errors` share `/Users/jack/src/etude/.git`, with origin `loopflowstudio/etude`. Installed 0.14.5 reproduced missing locator for retained repository `repo_abfe3d45f37b47bb8480884a940546c2`. Jack’s attempted bind omitted the final `6c2`. Supported `lf repo identity --bind` with the full identity from the confirmed main checkout succeeded. Identity, Session listing and `lf --explain` now succeed from the new worktree. Replayed actual bare `lf` in a PTY there: native Codex 0.163.0 reached its usable prompt and answered “What would you like to work on?” The verification conversation was exited normally; no repository work was requested or changed.

MTG-121: Task `task_e5c6c24e65ed4174b8bf6d25d8201ea2` retains its full preparation plan and separate $5 GPU cap. Fresh Task status has no checkout, Sessions, Flows or execution owner; `task location --peers` returns only this Machine, unrecorded. `task admission --claim` reproduces the missing authority refusal. A read-only SQLite check confirms no authority, started marker, checkout or retained task-creation intent. No live database edits were made. A narrow explicit admission recovery operation is being implemented/tested in this checkout; see historical-task-admission-recovery.md. No GPU spend incurred by this recovery.

## Verified recovery

The five admission unit tests passed, including immutable cross-Machine authority, simultaneous claims/lost replies, explicit historical recovery/idempotence, started-work protection and delegated-owner protection. The public CLI regression passed through actual establishment and checkout after an initial missing-authority refusal. Formatting, Clippy across all targets, CLI build and diff checks passed. No migration was added or released SQL altered.

The tested candidate’s `lf task admission MTG-121 --establish --json` succeeded, designating the existing local Machine and Manabot repository. Installed 0.14.5 independently read the same receipt and prepared `/Users/jack/src/manabot.mtg-121-reduce-remote-gpu`. This recovery does not require replacing the installed binary. The Task’s current brief now records recovery, preserving the full acceptance, conservative budget/capacity limits and original plan. Its preparation content was copied into `scratch/mtg-121-prepared-plan.md` in the new Task checkout.

Installed `lf task run MTG-121 pursue` launched Flow `b3a2aee5-34c3-415d-89a0-15c82d24e62b`; fresh Task status reports implement Process `816e9cbc-7371-4613-aa8d-27e5f78914f8` running. Worker output confirms it is reading the Task/design and beginning implementation. The launch reason explicitly preserves the $5 maximum including failures, separate ledger, single rental/capacity checks, deadlines, deletion and MTG-135/137/150 separation. No paid GPU was started by this recovery. Further worker spend must be read from its own ledger, not assumed zero indefinitely.

The existing Manabot `auto-code` Flow contains a removed `flow: code` reference. The already-requested `pursue` Flow is valid and was used explicitly; no global Manabot Flow definition was changed. That compatibility cleanup is separate from the verified MTG-121 admission/launch repair.

Review: explicit authority establishment is an operator choice, not a distributed election from negative peer reads. It cannot overwrite a foreign authority or relocate recorded work, and the claim/planning write share a transaction. LOO-414’s broader remote execution/automatic admission acceptance remains open.
