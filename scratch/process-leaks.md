# LOO-823 process leaks

Outcome: account for resident lf processes, fix fixture/demo leaks and observer
lifetimes, and show affected suites leave no new lf, provider or Dev-app processes.
Existing processes are report-only. No signaling processes this work did not
start, name-pattern kills, synthetic CPU load, or step-tree reaper in lf.

PR #1877. Earlier design, experiments and verification are in
`f5cb942098e0ae9af447dfd350a38ff49d38be53:scratch/process-leaks.md`.
The same commit keeps the preceding `questions.md` and
`simplification-opportunities.md`; earlier alternative input-ledger designs are
in `551becf586687262921e0681a169b5b7973fc75b:scratch/process-leaks.md`.

## Direction, 2026-11-11

Jack Heart, word for word:

- "with bare lf, noting holds its at all"
- "with task sessions, i think we can just hold them until the task is closed basically"
- "wave/repo should behave exactly like open tasks tho"
- "whatever gets to the cleantest possible world fastest. transition costs ok"
- "3a - here" selects this PR; "4a" means done and abandoned.
- "6a" selects a per-Machine agent count in config; "no opinion on 5" leaves
  editor-empty detection undecided. Resident-limit suspension is deferred.
- Demo review: "1a 2b". Resume still replaces idle owners on another build and
  loses the draft; delete the sixteen-conversation background cap.
- Comment `2bcb32d3-8e6b-474d-8fcb-2cbaead48ce1`:
  "Review answers 1a 2b: drop the background conversation cap; owners obey a close request instead of judging Task closure (scratch/process-leaks.md)"
- Later concept review: "1b 2c". This replaces the stored automatic request:
  cleanup signals verified owners directly; explicit shell Close stays stored.
  The choice to end installed-build owners by PID was separate from this run;
  this plan contains no record confirming that action.

## Current implementation

At `89181c49ac`, the direct-signalling changes and both QA repairs are built;
bare-direct launch followed in `59fdcdaee0` (see the demo review section).
Interrupt hooks run in reverse registration order (`agent::exit_on_interrupt`), so a
signalled shell owner writes `jobs_settled` before its Process record completes.
`ops/task/retirement.rs::close_conversations` uses
`ops/human_session.rs::end_owner`, shared with rollover. The migration has no
Process close columns or automatic cancellation triggers;
`transport/lifetime.rs` records viewers and polls only explicit shell Close.
Initial attachment retains cooked input, and abandoned checkouts use the existing
retirement queue. The parentless-launcher repair follows the Session's current
AgentProcess as well as ancestry.

The earlier feedback's “Next implement” list is covered by these changes and the
focused checks below. The direct-signalling walkthrough passed S2–S4 after the
launcher repair. Those results do not replace gate or the broader suite process
census. No product decision remains for this scope; the Rex icon demo is owed.

## Implementation contract

Open Task, Wave and repository conversations (a scope's ongoing conversation)
remain running until quit or explicit replacement; closing their terminal or
detaching ends neither. Every other interactive launch is bare: no owner, the
provider is the terminal's foreground process-group leader, and it ends as that
provider would when its terminal goes, mid-turn included. A done Task's conversations end after fifteen minutes
without viewing once live work settles, on a cleanup pass. Completion and a new
owner's launch are lower bounds on that grace. This applies without a checkout
and after checkout retirement. Abandonment ends unviewed terminal conversations
even mid-turn. Headless Runs retain their existing lifetime.

Task cleanup owns the decision and signals verified owners in that pass, using
rollover's termination path. It first sends a best-effort notice and saves the
screen; an older owner that cannot receive the notice still ends by signal.
No automatic close request is stored. Owners record viewing changes and retry
failed writes; once recorded, they wait for the next change. Only shell owners
poll explicit Close. They do not read Task state. The cleanup inventory includes closed Tasks with live conversations,
including missing or retired checkouts, and abandoned local checkouts whose
cleanup previously met live work. Abandoned checkout retries use the existing
retirement queue and backoff, leaving it after successful deletion. Reopening
revokes that checkout intent; this is separate from owner signalling.

A return, reopen, membership change or new work observed before cleanup's decision
keeps execution. The accepted return-between-decision-and-signal race ends with
“Task completed”; resume continues history. A store read failure cannot authorize
closure. Cleanup recognizes current Session launchers through the Session’s AgentProcess
relationship as well as Process ancestry; an exec handoff can start without a
parent link. Independent execution and live requests still hold cleanup.
Input uses the bounded byte queue with no ledger or store writes. Initial
attachment leaves cooked input queued until input control is acknowledged;
recovery still discards uncertain input.

Explicit shell Close remains on `desktop_shells.close_requested`. Its owner waits
for requested work to settle, then closes ordinary jobs. Automatic shell cleanup
signals owners directly after the Task's viewing grace and work checks. Shell
viewing adds no Task hold.

Completed-Task retirement checks merged delivery before deleting a checkout;
a PR-less or unresolved checkout stays. Abandoned checkout retries use the
existing unmerged-delivery cleanup and retain dirty files. Resuming retains
provider history and the saved screen, never an editor or replayed input.
Ordinary shell jobs settle before checkout deletion. `docs/sessions.md` and the
transport README own this behavior.

### Delete — do not maintain

These runtime cuts are complete; no deletion target below remains in use.

- Idle ending; native-input ledger and retries; `BACKGROUND_TASK_CONVERSATIONS`.
- `store/sqlite/owner_close.rs`, Process close columns/index, seven close-request
  cancellation triggers, whole-store revision guard and automatic owner polling.
- Automatic shell close state and its cancellation tests. Preserve explicit Close,
  viewer deadlines, requested-work holds, job settlement and checkout protections.
- `transport/lifetime_tests.rs` and queued-ending veto tests, which exclusively
  exercised the replaced policy. Keep viewer recording recovery and direct-signal
  integration coverage.

Released `processes.native_input` remains for LOO-840's staged removal;
`desktop_shells.close_requested` stays in use and must stay out of that removal.
LOO-829 removes the ignored detached-timeout config key after release.
Resident-limit suspension remains deferred.

## Readings that cannot be recovered

2026-11-10 ~17:30: 343 lf processes; 113 `session observe-status` with gone or
replaced Desktop clients. PID 2046's stdin writer was inherited by 15 pane
processes. 24 `/tmp/lf-7501-*` owners/stand-ins were up to eight days old; their
fixture had no Drop. 78 owners were held by native input. Nine Desktop shells
included six on done Tasks whose retirement never reached closing because the
conversation's input hold blocked it. No existing process was signaled.

The initial audit's argument-based selector counted shell commands naming an
lf path. The corrected selector uses `ps comm`. QA's 2026-11-11 05:20 audit
counted 368, matching independent fresh readings before and after (368, 368).
`scratch/process_audit.py` generates `scratch/process-audit.html` read-only.

`scratch/qa.html` records the 05:13–05:55 walkthrough: open Task draft kept
40 minutes; done Task without checkout ended at completion +15:00; checkout
Task ended likewise, and its resumed conversation ended +15:00 after leaving.
The observed opening-input loss was #1882's path; that change is now in this
branch. A post-return checkout reason stayed stale until the next sweep;
ended providers retained open process rows that readers did not treat as live.

Two 110-test transport runs passed before the final close-request design and
left no new fixture processes. The latest sampled 797 descendant PIDs and 94
fixture-home paths; none remained. These runs do not cover this implementation.
A controlled 17-second provider-start delay reproduced the old upgrade startup
failure; both corrected upgrade cases passed with the delay. The temporary
wrapper was removed.

The previous publish gate stopped before suites at resource preflight (16.8
GiB free, 32 GiB reserve); log `/tmp/loo823-publish-gate.log`. No product failure
or process-cleanup result follows from that attempt.

2026-11-11 realign: remote main remained `49037487d`, already merged here.
PR #1877 still says "Native-input retention is unchanged"; its validation predates
the close-request design. That description needs replacement at publish.

2026-11-11, 10:59–11:29 QA world `/tmp/lf-qa-823g` (pinned copies of `89181c49ac`'s
`lf`/`lf-world`, 30-second `lf task retire` passes): an open Task conversation and a
repository conversation each kept a draft through 27 minutes unviewed and answered
it on return. A done Task completed 11:00:22 and was revisited 11:08:30. It was alive at
completion +15 and ended at 11:23:33 (+15:03 after that departure). The focused suites
(20 lib + 8 transport passed) left no new process; the only new provider PID belonged
to an installed `lf` run started before them. Audit 11:05: 288/287 against `ps` 287.
It listed the QA world's three live owners in the fixture kill list. `binary_kind` now
limits "test fixture" to `lf-7501-*` and gives other temporary binaries a row with no
kill command. Rerun 11:07: 24 fixtures, 3 other temporary, 288 against `ps` 288.

## Focused checks, 2026-11-11

Previous close-request checks and their commands are in
`b936afed1b42e416b160835604cb2ec43a85268b:scratch/process-leaks.md`.
They do not cover direct signalling. With inherited `LF_*` unset and stdin closed:

- `cargo test -p loopflow --lib -- ops::task::retirement::tests:: store::sqlite::task_retirement::tests:: desktop::shells::store::tests:: owner_close_upgrade_keeps_explicit_closes_and_rejudges_automatic_closes`: 20 passed on the final migration/retry queue; `/tmp/loo823-cleanup-final.log`.
- `cargo test -p loopflow --test transport_tests -- completed_task_closes_a_conversation_resumed_after_checkout_retirement completed_task_without_checkout_ends_a_conversation_with_typed_input returning_task_viewer_resets_cleanup_without_another_sweep silent_native_turns_survive_until_settlement native_background_watches_survive_until_settlement a_locked_store_does_not_delay_input_or_replay_it_on_return`: 6 passed; `/tmp/loo823-focused-final.log`. The first two also passed after the retry-queue refinement.
- `cargo test -p loopflow --test transport_tests completed_task_cleanup_signals_shell_owner_and_settles_jobs`: passed; `/tmp/loo823-shell-final.log`. Initial fixture attempts lacked required Task title/description/placement fields; the corrected fixture reaches cleanup, receives “Task completed”, settles its ordinary job, and retains its PR-less checkout without storing Close.
- `cargo test -p loopflow --lib -- snapshot_preview_keeps_typing_until_initial_attachment_is_acknowledged viewer_state_retries_after_a_store_failure_without_another_viewer_change`: passed in `/tmp/loo823-implement-tests.log`.
- `cargo test -p loopflow --test integration -- task_abandonment:: documented_commands:: a_task_run_starts_its_conversation_and_abandon_stops_it`: 6 passed; `/tmp/loo823-focused-final.log`.
- `cargo test -p loopflow --lib --test transport_tests -- conversation_cleanup_owns_grace_membership_and_work cleanup_recognizes_current_session_launchers_without_process_parents` plus the seven transport cases above: 2 policy and 7 transport tests passed after the live-walkthrough repair; `/tmp/loo823-launcher-focused.log`.
- `cargo build -p loopflow --bin lf --bin lf-world`, `cargo fmt`, `uv run python scripts/check_migrations.py`, `git diff --check`: passed. Gate owns broader checks below.
- Compress: `cargo test -p loopflow --lib --test transport_tests -- transport::lifetime::tests:: store::sqlite::task_retirement::tests:: completed_task_ returning_task_viewer_resets_cleanup_without_another_sweep silent_native_turns_survive_until_settlement native_background_watches_survive_until_settlement a_locked_store_does_not_delay_input_or_replay_it_on_return desktop_shell_reconnects_and_settles --test-threads=4`: 10 unit and 7 transport tests passed; `/tmp/loo823-compress-tests.log`. The lib-test link warned that `__eh_frame` exceeded compact-unwind's 16 MiB limit; build and tests succeeded. The last filter matched no test; explicit shell Close is checked separately below.
- Compress: `cargo test -p loopflow --test transport_tests desktop_shell_reconnects_without_replay_and_close_settles_its_jobs`: passed; `/tmp/loo823-compress-shell.log`. `cargo fmt --check` and `git diff --check` passed; `lf context` remains under both limits. Gate owns the broader suites and process census.
- Realign: `git diff --check` passed; `lf context` fits both targets. Prose-only reconciliation reused the focused results above; no suites rerun.
- Shell settlement race: interrupt hooks ran in registration order, so the owner's Process record (registered at startup) completed before its later shell-jobs hook wrote `jobs_settled`; `fixture.ends` could observe the exit first (`/tmp/loo823-bare-transport2.log`: 111 passed, 1 failed at `assert!(settled)`). `agent::exit_on_interrupt` now runs hooks in reverse, destructor order. Full `transport_tests` after the fix: 112 passed (`/tmp/loo823-hooks-transport1.log`). A second run hung in `desktop_shell_twenty_lifecycles_leave_no_owned_jobs_or_sockets` cycle 12 at `launch.wait()`: the shell's argv wrote its ready file before the launch's own attach connected, the test's viewer took control first, then the launch's attach replaced it and never exited. The four Desktop-shell fixtures now `wait_controlled` before taking over. Two more full runs at load average ~175: 112 passed (`/tmp/loo823-hooks-transport3.log`), and 111 passed with `a_headless_run_survives_without_viewers` past its 30 s `launched` wait (`/tmp/loo823-hooks-transport2.log`, 227 s run). That test passed 5/5 alone in 14–19 s; it doesn't touch interrupt hooks. The cleanup and twenty-lifecycle tests passed in every full run after the fix.
- Compress, with `wait_controlled` and the `closed_task_cleanup` migration rename: full `transport_tests` 112 passed, 0 failed (`/tmp/loo823-compress-transport1.log`); `closed_task_cleanup_upgrade_…` passed; `cargo fmt --check`, `git diff --check` clean. No further cuts: deletion targets are gone, surviving paths reviewed.

The first live direct-signal world was interrupted at 09:58 when a rebuilt CLI
met its older draft-migration checksum. Its own processes were stopped and a
fresh world with pinned copies of both binaries replaced it. Its earlier S4 and
startup-input readings are not grace-period completion results.

2026-11-11, 10:03–10:20 direct-signal world: S3 closed at 10:18:34,
fifteen minutes after its return/departure, and S4 ended during abandonment.
Fresh S2 and an extra fresh Task conversation stayed beyond grace: their provider
launcher's Process parent was absent, although the Session's current AgentProcess
named it. Cleanup treated that launcher as independent work. This invalidated
the ancestry-only ownership check; cleanup now reads that existing Session
relationship too. The failed walkthrough stopped its own processes. The repaired S2 repeat ended at 10:40:18, completion/departure +15:03,
with no checkout and the same parentless-launcher shape. S3 resumed history and
answered fresh opening input. At 10:41 no process from either private world or
its drivers remained. `scratch/qa.html` keeps the timeline and limits.

## Publish verification, 2026-11-11

`lf sync` merged `763da5c92f` as `4fec5e787b`, resolving the hook migration
without restoring native-input recording. Its focused background-work test passed.

- `uv run python scripts/check_migrations.py`: passed; only `owner_close_requests.sql` (now `closed_task_cleanup.sql`) is unmerged.
- `scripts/test.py --reuse-passing --rust-filter …`: architecture, test inventory and formatting passed; macOS `syspolicyd` at 281% CPU stopped CLI compilation. No product failure. `/tmp/loo823-publish-gate.log`; 37 sampled descendants, none left.
- Documented pressure fallback: candidate build, the same filtered nextest plan (573/573), Clippy, packaging, fixture regression, inventory (14/14), website, Swift app build/tests (807 tests, 99 suites) and Swift platform checks passed. `/tmp/loo823-publish-manual.log`; exact commands and phase results in `/tmp/loo823_manual_checks.py` and `/tmp/loo823-manual-results.json`.
- `cargo test -p loopflow --lib store::sqlite::steers::tests::`: 18/18 passed; the initial filter used the filename instead of this module. `/tmp/loo823-publish-steers.log`; no surviving descendants.
- `cargo test -p loopflow --test transport_tests ssh_shells:: -- --ignored`: 3/3 passed, including twenty SSH lifecycles; `/tmp/loo823-publish-ssh.log`; none of 452 sampled descendants remained.

The broader census sampled 2,557 descendants and found two `two_people` owners
with stand-in providers after all checks passed. `Pair` stopped its servers but
removed both homes without ending the detached conversations. Its Drop now uses
the shared recorded-process cleanup before the homes go; the existing test also
checks that both Machines' owners exited. The two leaked owners from this run
(PIDs 83753 and 94525) were signalled only after matching their recorded start
times and executable. Both providers and the two defunct children also exited.
No pre-existing process was signalled.

- `cargo test -p loopflow --test two_people`: passed after repair; none of 74 sampled descendants remained, no new lf process. `/tmp/loo823-publish-two-people.log`.
- Query plan check: the live store had 1,099 Tasks, 14,807 Sessions and 160,541 Processes. New candidate reads use Session/Process indexes; pending inventory scans Tasks per live conversation. An in-memory schema with 10,000 Tasks and 400 conversations returned 400 candidates in 0.237 s.

The final edit changes only fixture teardown and its assertion; the passing
product, Swift, website and package checks above still cover the same code.
Installed Desktop windows, real providers and the scheduled launchd sweep remain
untried here. The private-world walkthrough used stand-ins and manual cleanup;
its recorded results and limitations remain above and in `scratch/qa.html`.

- Final `cargo clippy --all-targets -- -D warnings` and `git diff --check`: passed after the fixture repair. `/tmp/loo823-publish-final-clippy.log`.
- Final audit: 284 lf processes in the opening snapshot and 284 when reads finished; `scratch/process-audit.html`.

## Remaining

- Demo: bare `lf` with Claude and with Codex in Rex shows the provider icon.
- Gate: affected suites and the suite process census. Under load ~175,
  `a_headless_run_survives_without_viewers` once missed its 30 s `launched` wait
  (`/tmp/loo823-hooks-transport2.log`); 5/5 alone. Watch it there.
- Publish: replace PR #1877's body ("Native-input retention is unchanged" and
  pre-direct-signal validation are stale); lead as the demo review says.
- LOO-840 (`task_1ddbd3851a8e424e9ce887fa8f39353b`) removes only `processes.native_input`, exporting released data first; shell Close stays. LOO-829 removes the ignored config key after release.

## Concept review, 2026-11-11

At that review there was nothing to change; the later demo review (below)
added bare-direct. The direct-signal implementation follows answer "1b". Conversation
closure needs no stored intent; explicit shell Close and checkout retirement
still do, because the request and unfinished deletion must survive a restart.
Keep the return-between-decision-and-signal counterexample above. Resume restores
history without reopening the Task or restoring its editor.

The review page now shows the built interaction instead of asking the superseded
close-request question. No next implement direction; gate and the process census
remain as listed above. Resident-limit suspension is still deferred.

## Historical concept-review finding, 2026-11-11

`scratch/concept-review.html`, read at `a99272166`, measured the whole-store
revision sum moving eight times in one second at rest (11346900 → 11346908).
That showed why an unrelated store write could repeatedly cancel closure.
Installed-build owners also could not read the proposed Process close column.
Direct signalling removes both dependencies. The review notes and replaced
implementation checklist remain at
`89181c49ac:scratch/process-leaks.md`; the latest direction is quoted above.

## Demo review, 2026-11-11 (after loop 3)

Jack Heart, word for word, reading PR #1877:
- "is this doing what i expect> i thought this was supposed to make it so bare lf closes right away"
- "and task sessions were always held wehn open"

As published, bare conversations "stay running until you quit their agent"
(`docs/sessions.md`), which keeps them longer than main's two-minute idle ending.
The earlier reading of "with bare lf, noting holds its at all" as "bare-direct
later, existing bare owners run until quit" was wrong: nothing holds a bare
owner, so it ends when its last viewer leaves.
- Asked whether closing a bare terminal mid-turn ends the agent: "for a bare session, it jsut does whatever claude or codex would do"

Managed conversations: "if its a managed task session, then neither ends the
session" (terminal close or detach). The PR body leads with closed Tasks ending,
bare ending with its terminal, and the removed two-minute rule; "Task sessions
held while open" is not new behavior. An interim "bare owner ends with its
terminal" direction was replaced by "1b" below.

Bare-direct, built after "1b"/"2a": `bin/lf.rs::bare_conversation` skips
delegation for plain `lf`, inline prompts and skill targets typed at a terminal;
`ops/human_session.rs::opens_bare` does the same for resuming a Session with no
primary scope. `os_process/foreground.rs` makes the provider its own group and
the terminal's foreground (from a close-on-exec duplicate of lf's stdin), takes
the terminal back on exit, and follows Ctrl+Z. One path for every harness
(`util.rs` native launch, `agent::run_interactive`). Native hooks are full only
when an owner holds the launch. Graphs, headless runs and Desktop relay surfaces
keep owners. Test:
`transport_tests::bare_lf_runs_its_provider_in_the_foreground_and_ends_with_its_terminal`.
Only a terminal on stdin goes direct; piped or scripted launches keep the
owner's PTY. Existing owner tests that resumed with pipes needed no change.
`codex_original_request_channel_and_turn_survive_all_viewers` now starts under
`session serve`; `detached_owner_keeps_no_descriptor_its_terminal_inherited`
launches `-b`. `nested_cli_execution_survives_its_calling_provider_and_viewer`
is deleted: a nested bare run "ends with its parent" (server-first).

- `transport_tests`: 111 passed, 1 failed, 8 ignored (`/tmp/loo823-bare-transport2.log`). `completed_task_cleanup_signals_shell_owner_and_settles_jobs` failed `settled`; cause and fix under Focused checks ("Shell settlement race").
- `context_launch_tests::` 21/21; lib `harness::context|context_block|harness::codex|os_process` 65/65; `documented_commands::` 4/4; `cargo clippy --all-targets -- -D warnings` clean on macOS. Linux `siginfo_t::si_pid()` branch compiles only in CI.
- Not tried: Ctrl+Z under a job-control shell; Desktop opening a bare Session.
- Compress (bare-direct): SAFETY comments per `unsafe` in `foreground.rs`, single wait loop in the bare test. `transport_tests bare_lf_runs_its_provider…` passed, clippy `--all-targets` clean; `/tmp/loo823-compress-bare.log`.

Answered (concept review, scratch/concept-review.html): before bare-direct every interactive launch
spawned `lf session serve` via `transport::delegate`, bare included. (1) Tie a
bare owner's life to its terminal now, or launch bare with no owner in this PR
(server-first's "Bare launches go direct"). (2) Is "managed" a scope's ongoing
conversation only, or any interactive Session naming a Task/Wave (e.g. a skill
typed in a Task checkout)?
- Jack Heart: "1b". Bare launches start no owner in this PR: the provider runs
  as the terminal's foreground child; owner, relay, saved screen, takeover and
  Ctrl+] go for bare; bare keeps context hooks only. Jack Heart: "2a".
  Only a scope's ongoing conversation (Task, Wave, repository) is managed; a skill
  typed in a Task checkout is bare.
- Jack Heart: "does this mean we get proper claude/codex icons in rex for bare lf now".
  Rex 0.1.0 keys the pane icon on the `tcgetpgrp` leader's invoked path and re-reads
  when that leader's executable changes (2026-11-05 reading). Bare-direct must leave
  the real provider as the foreground process-group leader: exec it, or start it in
  its own process group and hand it the terminal. No argv0 or symlink naming.
- Jack Heart: "shoudl be independent of provider too". One bare launch path for
  every harness (Claude, Codex, Pi, OpenCode, …); no per-provider branch.

## Bare-direct moves to LOO-820, 2026-11-11

LOO-820 ("Bare launches run the agent directly in the terminal") already owns
bare-direct, with "lf resume of a bare Session whose process still runs
elsewhere starts a second provider on the same thread; lf stops nothing".
Jack Heart: "let 820 own it". This PR drops the bare-direct pass (the
`bare_conversation`/`runs_in_terminal` launch branch, `os_process::foreground`,
the bare transport test and their docs) and returns to the published bare
lifetime: bare conversations run until their agent quits. The "1b", "2a", Rex
icon and "independent of provider" answers above now apply to LOO-820. The PR
body must not claim new Task-open holding; it leads with closed-Task cleanup,
the removed two-minute rule and ledger, and the leak fixes.

Jack Heart: "lets push wahtever is left to 824 or 820 so we can land this and get out of this complicated overlap".
Moved by comment: bare-direct, resume-elsewhere, bare Graph targets, Desktop
bare-pane owner → LOO-820; resident-limit suspension ("6a", "no opinion on 5")
→ LOO-824; bare Codex observer → LOO-826. LOO-823 takes no new scope.

Jack Heart, next messages: "i am a little unsure on the bare direct though", then
"if we already have it working we should just kee it here and hsip it here".
Bare-direct launch stays in this PR. LOO-820 keeps the remainder: bare turn-hook
and exit-record removal, resume-elsewhere, bare Graph targets, Desktop bare-pane owner.
Jack Heart: "bare direct is great".
Jack Heart: "826 was already shipping so i think you shouldnt ahve deferred anythign to it". Bare Codex observer → LOO-820; LOO-826 comment withdrawn.
