# LOO-498 — failed demo and corrected launch design

## November 5, 2026: observed failure and Jack Heart's direction

The branch repeatedly opened LOO-498's active conversation as a custom passive
`lf session view`. Jack reported “everythime you launch a demo this pane freezes”.
Source inspection found ignored input and a hidden Ctrl+R transfer shortcut. A
read-only hint was added in `21bbb00329`; its three PTY checks passed, but that did
not establish the cause of the reported freeze.

The subsequent `$demo` built that commit and opened a fresh Rex tab through Rex's
CLI using the real LF Machine. It reported connected. Both the native screenshot
and Jack's supplied screenshot showed overlapping text, stale prompt fragments,
and inconsistent spacing. Jack said “still ooks super werid”, then explicitly
said to leave this Task alone because he was using it. No acceptance was given.
The operator stopped host interactions; no input transfer or teardown occurred.

Jack clarified the expected behavior: launch a Rex tab in the right worktree and
run ordinary `lf resume`. He then requested dramatic simplification of the code
and questioned unnecessary Session commands. Current implementation direction is
[rex-workspace.md](rex-workspace.md): remove the observer command, readiness
files/protocol, and custom input transfer. Preserve ordinary resume, layout and
worktree handling. Do not use this active Task or any of its panes for a demo.
Further hands-on proof needs separate work; no live demonstration is accepted.

## Last failed opening — retain these resources

- Commit: `21bbb00329`; runtime SHA-256:
  `70b984d899ff1b10cc004c6d356f0e00fe25b47121394b2ee95b0c91e0548cec`.
- Evidence root: `/Users/jack/.local/share/loopflow/demos/loo498-20261105-input-hint/data`.
- Fixed runtime: `/Users/jack/.local/share/loopflow/demos/loo498-20261105-input-hint/runtime`.
- LF_HOME: `/Users/jack/.lf`; runtime `lf` is LF_BIN. This used real data, not a
  private Machine. The runtime is a copy, not an installed update.
- LF Session: `310fb7b6015843e498316cd921a348b9`.
- Rex session: `session:01a19d85-39a8-7aae-8c83-fa6adc2e1317`.
- Launcher tab: `window:01a1aeec-835a-7b56-b977-8e2d862a70a1`;
  block `block:01a1aeec-834d-70a0-9cc5-892cc17febdc`.
- Conversation tab: `window:01a1aeec-98d3-79fa-accd-f2cc7aa0838d`;
  block `block:01a1aeec-98d2-75d4-9958-b130d347fb40`.
- Evidence: `before.json`, `after.json`, `launcher.json`, `open.json`,
  `open.stderr`; screenshots are in the conversation, not saved image files.
- The earlier tab `window:01a1adc5-3ed3-7cec-b32d-f640c130b637` was preserved.
  Its observed shell was `block:01a1ade8-92ba-7626-9e1d-79a772f4d43d`.

These are historical identities, not current process liveness or signal authority.
No draft/request settlement or disposal authorization exists. Leave all resources
untouched. Do not infer release from absent blocks, finished providers or age.

## Earlier retained roots and evidence

Under `/Users/jack/.local/share/loopflow/demos/`, retain both `data` and `runtime`
for each of:

- `loo498-20261105-review`: same real Machine; saved Desktop layout was one
  Conversation pane. Native receipt proved connection, not usability.
- `loo498-20261105-opening-repair`: fixed child executable selection and transient
  Rex CLI discovery. Native Split started zsh in the Task checkout; text reflow
  was visibly broken. Repairs were published in `1a788c6a6f`, synced `e9dcba147b`.
- `loo498-20261105-live-tools`: manual tool panes and a mixed-binary failed opening.
- `loo498-20261105`: private real-data snapshot at `data/lf`; its original native
  session later disappeared. This is not evidence that all input was settled.

Retain `/private/tmp/loo498-real-demo-j8rn_nmv` and
`/private/tmp/loo498-demo-c8knqzcq` too. The latter's synthetic Task was `30d671`.
Earlier exact receipts, owner identities, safe conditional teardown and history
reopening instructions are in `21bbb00329:scratch/workspace-demo.md`. They require
fresh ownership evidence and input settlement before use. Never delete durable
history or stop processes by name. User's current instruction is to leave the
active Task and its native panes alone.

## Simplification proof

`441ed217fd` removed the observer path (916 net Rust lines); `8c77f061a4`
synced current main without conflicts. Isolated Rex tests now prove ordinary
workspace resume keeps the same provider and unfinished input, with the Task's
working directory. The separate manual-resume cwd test also passes. No running
user Task, native pane, installed binary or shared database was used for this
validation. Layout/config/host behavior and the independent cwd repair remain.

Checks: opening library 14 passed; normal-viewer library 8 passed; work_open
integration 22 passed initially, its corrected three-pane shell selection test
passed separately, and its unchanged tmux test passed standalone after an initial
2-second host timeout; isolated native Rex 2 passed. No timeout relaxation or
retry loop was added. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.

The review page describes the simplified flow without asking Jack to reopen his
active conversation. Hands-on acceptance is still outstanding and must use
separate work. No Graph continuation, merge or teardown was selected.


## November 6, 2026: separate native demo — conversation startup failed

Jack Heart invoked demo after the simplification and user-documentation rewrite.
The operator used a separate Task, 99543b (task_99543b5a41a84b7cb6e76f5d625a52ef),
in `/Users/jack/src/loopflow.rex-native-demo`. No existing conversation was resumed,
closed or modified. Task creation and checkout launched no Graph. The demo brief
prohibits implementation, Graph execution and other-Task operation.

The copied branch CLI at commit 21ae187407 read the real Machine successfully.
A Rex CLI-created launcher ran `lf open --task 99543b --json`, without a host flag.
Rex was detected and the fresh tab contained Conversation, Task & workflow and
Shell. The conversation block reported the demo checkout through OSC 7. This is
not proof of a manually created split's actual cwd.

Conversation startup then failed: the pane showed “Nothing to resume here;
starting a new conversation”, then disappeared. The Task view and shell remain
visible and readable. Native screenshot evidence is in this conversation; no
filesystem screenshot export was available. No provider input or split inheritance
was accepted by Jack. The initial opening receipt is not evidence of usability.

Process and history evidence identified a mixed runtime: the branch outer resume
was d1181700379fcf6a97efa8cffc3ab41d2a1e233e4053ba5159fd983bb682cde5;
`ops/human_session.rs` selects `resolve_lf_binary` for its child, which on a real
Machine selects the installed executable. Its frozen runtime was
`~/.lf/bin/lf-e497ed1b11a25358a978332cb156d9428dd53436aa67d07d048d6edeec5d76eb`
(version 0.15.1+5519340b4). The child recorded failed at 16:14:05 UTC; the LF log
says “execution owner did not take the provider wait; waiting here” and “Execution
owner closed before answering”. The precise cause of provider exit remains unknown;
the runtime mismatch is established, not proven causal. Recommended next work:
trace that ordinary-resume exit and ensure configured branch demos use one runtime
through the whole launch. Do not restore a custom viewer or add Session commands.

### Retained evidence and owners

- Durable evidence: `/Users/jack/.local/share/loopflow/demos/loo498-20261106-native/data`.
- Fixed runtime: `/Users/jack/.local/share/loopflow/demos/loo498-20261106-native/runtime`.
- Real `LF_HOME=/Users/jack/.lf`, per Jack's prior real-data preference; no private
  Machine, installation or database migration. Both roots remain intact.
- LF Session: `efb749e7cffb42bc88ff86a25575e1c6`; final Session list says closed.
- Rex session: `session:01a19d85-39a8-7aae-8c83-fa6adc2e1317`.
- Launcher: window `01a1b1a2-907f-7697-b4aa-f2c77e55d23c`,
  block `01a1b1a2-9063-769f-a929-909716148b2a` (both retain their Rex prefixes).
- Demo tab: `window:01a1b1a2-ad99-7dcf-b4bf-e9e3e6056a3d`.
- Removed Conversation block: `block:01a1b1a2-ad92-73c8-aa90-6b99fddaa172`.
- Retained Task view: `block:01a1b1a2-ad94-7582-a218-8515cde088d8`;
  PID 77240, parent login 77152, started November 6 at 08:13:11 local.
- Retained shell: `block:01a1b1a2-ad99-749b-a66f-020a7c96ce6c`, login 77153.
- Conversation owners 77150, 77242, 78145 and 78445 no longer appeared in `ps`.
  They are historical identities, never future signal authority.
- Evidence files: create.json, checkout.json, before.json, launcher.json, open.json,
  open.stderr, after.json, after-exit.json, history.json, sessions-final.json,
  ps-final.json. Input artifact: `~/.lf/runs/f5/f5acc3d427ec456bb81cb9e7eae3650f`.
- History remains readable using `lf session history efb749e7cffb42bc88ff86a25575e1c6
  --json` against the real Machine. No usable provider history was demonstrated.
- No input settlement or teardown authorization was received. Leave native shells
  and roots intact. After Jack answers the runbook's settlement question, recheck
  the exact retained owner identities before closing demo-owned tools. Use `q`
  in Task view and `exit` in demo shells; do not signal by name. Never remove the
  real LF_HOME, checkout, or durable history. Runtime removal is deferred until
  every owner is verified exited and all input is settled.

LOO-498 remains feature → demo; no live Graph was observed. The branch was synced
with main c8d7a7f4a8; its AGENTS conflict kept main's prose reduction and this Task's
isolated-host testing rule (864778108a, pushed). The demonstrated binary predates
that documentation/skill-only merge. No Rust change or repeated build was needed.
The runbook is `scratch/demo.html`; this review is failed, not accepted.


## November 6 follow-up: ordinary Task conversation available

Jack Heart supplied a screenshot of Task & workflow and said the demo was still
not what he wanted: “some weird thing that isnt just a task session”. The operator
opened one ordinary conversation pane for the same separate demo Task; LOO-498's
running conversation and all earlier panes were left alone.

A direct installed-LF resume first exposed the missing-history error: Claude had
no conversation f5acc3d4-27ec-456b-b81c-b9e7eae3650f. Supported `session new
--prepare` also refused because that transcript did not exist. Neither operation
repaired it. The fresh conversation was therefore launched through the existing
`lf --task 99543b skill task-session` command, explicitly instructed to greet Jack
and wait. It used a copied installed CLI, not the branch workspace-opening path.
This is proof of the requested ordinary conversation experience, not proof that
the branch startup failure is repaired.

Native capture and screenshot now show Claude's response “The Task conversation
is open” and an ordinary input prompt. `lf session list` reports active/available.
The native process cwd is `/Users/jack/src/loopflow.rex-native-demo`. No Graph ran.
The new conversation was bound as this demo Task's primary so later resume selects
it instead of the failed history. Jack's input/resize acceptance remains pending.

- Rex tab: `window:01a1b1ad-2bba-7670-ae79-afeebb9e531e`, named
  **99543b · Task conversation**; single block
  `block:01a1b1ad-2b9f-74ce-be41-f9d7db51a41b`.
- LF Session: `session_8ebcd369c5e9479fbfe989fea0f7b65d`.
- Provider history: `3fff580e-c9f9-44b7-9439-34dd5f6ec09d` (Claude).
- Observed owners: viewer 54390, terminal owner 62309, provider 62413;
  recheck identities before any eventual teardown.
- Same durable/runtime roots and real LF_HOME as the preceding attempt.
  `runtime/native-lf` copies the installed build; `runtime/new-session.sh` launches
  it. Evidence: new-session-launch.json, plain-sessions.json, plain-bind.json,
  plain-capture.txt. Screenshot is in the conversation, not an exported file.
- Failed resume retained tab: `window:01a1b1ac-4f1b-7a60-a5d9-c3f17ea66718`,
  block `block:01a1b1ac-4f09-7109-b0b6-807eb556218a`, exited 1.
- Reopen history through `lf resume session_8ebcd369c5e9479fbfe989fea0f7b65d`.
  No draft settlement or teardown requested; keep all owners and roots intact.

Jack Heart subsequently replied “okworked”. This confirms that the ordinary Task
conversation worked for him. It does not establish individual split/cwd or resize
checks, repair of branch fresh-conversation startup, or approval to ship. The demo
input has not been released for cleanup; leave the conversation and roots intact.

Branch fresh-conversation startup was verified later; see the November 6 native proof below.


## November 6 headless passes and gate

Headless demo decisions on eaf36a5d3, 6b3818bc5 and 3b5061b18 ran no live demo and advanced on “otherwise lg2m ship it”. Their “not proven live” notes are superseded below.

Gate on eaf36a5d3 and the architecture map fix, final at fb810b8e4:
- `scripts/test.py --reuse-passing` with the Task filter: website 72 passed; rustfmt passed; Rust 639/640. `check_architecture.py` failed on unmapped `process:rex`, then passed once it was mapped.
- `served_reads::what_an_app_serves_in_process_matches_its_lf_command` fails identically on origin/main 12d897b6b (macOS `/var` vs `/private/var`). Linux CI cannot see it.
- `scripts/test_desktop.sh`: the process dies in `ThemeTests.defaultConfigurationLoadsAfterInitialAppearance` (libghostty Breakpad), the same on origin/main; it passes alone. CI owns the Swift result.
- `cmux_task_joins_the_group_its_caller_is_in_now` failed once in a suite run on the two-second host deadline and passed alone and in the gate.

## November 6 native proof (tree 502a14596 plus the new test)

Fresh-conversation startup works when one runtime runs the whole launch. All runs used isolated servers with `LF_*` unset; the active Task's panes and the retained demo roots were not touched.

Native tests, one at a time, `LOOPFLOW_TEST_REX`/`LOOPFLOW_TEST_HERDR` set, herdr 0.9.3:
- `--test integration rex_native_fresh -- --ignored`: 1 passed (11.83s).
- `--test transport_tests rex_workspace_resumes -- --ignored`: 1 passed (6.70s).
- `--test transport_tests rex_manual_resume_elsewhere -- --ignored`: 1 passed (5.62s).
- `--test integration herdr_native -- --ignored`: 1 passed (12.84s).
- `--test transport_tests rex_open_starts_a_fresh -- --ignored` (new): 1 passed (5.77s). `lf open --task` on a Task in a linked worktree, stand-in provider: pane draws, provider `pwd -P` is the worktree, OSC 7 names it, typed input arrives, one launch.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.

Real Claude, by hand: isolated Rex server, private LF_HOME, LF_BIN pinned to a copy of the branch binary (`lf 0.15.1+502a14596`, SHA-256 `b91f70766d851c8abd036590a974ef01486275c6547961437eded227e793f886`), guarded `rex` on PATH. Task 0f9e97, placed with `lf task checkout`, no conversation:
- `lf open --task 0f9e97 --host rex --server … --no-focus --json`: exit 0; tab held Conversation, Task & workflow and shell.
- Pane printed “Nothing to resume here; starting a new conversation.”, then Claude's folder-trust dialog. After Yes, Claude ran its first turn and stayed up.
- `lsof` on provider PID 50510: cwd `/private/tmp/loo498-fresh-ybHz/repo.0f9e97-fresh-conversation-demo`, the Task worktree. Rex reported the same path by OSC 7.
- Typed “Reply with exactly the word marmalade-7 and nothing else, and run no tools.”; Claude answered `marmalade-7`.
- `lf open --task 0f9e97` again: new tab, same Session, same PID 50510, `marmalade-7` still in the scrollback. No second provider.
- Second Task 775232 from the same repository: no trust dialog, provider started. Its name, “Trust declined”, records a planned experiment that never ran.

The provider was real but the Machine was private, with ambient Claude credentials. A branch build on the real Machine was not rerun; that path hands its child to the installed runtime (`human_session.rs`, `resolve_lf_binary`), so the November 6 mix can recur on a dev build until `lf` is installed.

November 6 failure, from retained `history.json` and `lf.log`, read only:
- Trust dialog ruled out: account selected 16:14:05.164, provider session observed .179, `terminal.json` failed .202. The end came 38 ms after opening; a dialog would have waited. `loopflow` is also trusted, and this run's second worktree shows a trusted repository's worktree skips the prompt.
- 54 seconds passed between capture (16:13:11) and opening; this run took under 8 seconds. Unexplained.
- `result_ref` is null and no owner log line beyond “Execution owner closed before answering” was found, so the exact cause stays unknown. The mixed runtime remains the only difference established; it was not reproduced.

Other observations:
- `lf open --task` on an unplaced Task (`worktree: null`) opens only Task & workflow, by design in `layout.rs`, with no conversation. See [questions.md](questions.md).
- No `default_agent` and several harnesses on PATH: the pane shows “No agent selected” and stops. Clear, and unchanged.
- Claude reported plain `lf` in the pane failing with “a newer lf migrated this store”: `~/.local/bin/lf` (5519340b4) predates the branch store. `$LF_BIN` worked. This is branch-versus-installed skew and ends with `lf install`.

Not proven headless: a native Split from the app shortcut (raw `session.new_split` is not that path) and resize. Both need the Rex app.

Evidence: `/Users/jack/.local/share/loopflow/demos/loo498-20261106-fresh`. Both Claudes left through `/exit` (Sessions closed, `lf ps` empty), the four demo Rex sessions were destroyed, server 1947 stopped by its recorded group, and `/tmp/loo498-fresh-ybHz` removed. Provider transcripts for those paths and their `~/.claude.json` project entries remain.

## November 6 compress: main's #1834 restored

`94cb745eb` merged LOO-717's older tip (`a1067c99c`, before #1834) onto a tree that already held #1834, and kept the older files. No conflict records it, and later syncs treat #1834 as merged, so #1834 was being reverted by this PR: 82 of its 83 files matched `c8d7a7f4a^` byte for byte, including `builtins.rs`, `graph/compiled.rs`, `graph/mod.rs`, `prompt/mod.rs`, `PROMPTS.md`, the skills and goldens, and the deleted `optimize` Graph. None held branch work; `AGENTS.md`, the 83rd, keeps its intended testing rule. Restored from merge base 76a5a3012. Working tree against main went from +11,626/−4,542 to +6,504/−487. A blob comparison over the other changed files found no other main commit lost this way; it cannot see partial hunk losses in files the branch also edits.

Checks on the final tree, `LF_*` unset:
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `check_architecture.py`: passed.
- `--lib builtins:: prompt:: graph:: lf::commands::open::`: 225 passed. `golden_prompt`: 1 passed. `documented_commands`: 4 passed. `world_recipes` selection: 7 passed. `work_open_tests`: 24 passed, 2 ignored.
- Native, one at a time, after the `isolated_rex` helper: `rex_workspace_resumes`, `rex_open_starts_a_fresh`, `rex_manual_resume_elsewhere`, `rex_native_fresh`, `herdr_native`: 1 passed each. A final binding rename touched only `rex_manual_resume_elsewhere`, which was rerun after it.
- `session_lifecycle_tests`: 35 passed, 6 failed with “No agent selected”; committed HEAD fails the same six, measured in a throwaway worktree. The fixture installs a stand-in `opencode` and assumes no ambient harness. With none on PATH, 41 passed. Not fixed here.

## November 6 realign: native tests on 4b4c78b24

Commit 4b4c78b24 is the compress edits plus main 81a79796f; `623855b89..HEAD` adds only an SSH test, `scripts/testing.md` and Wave memory. Isolated servers, `LF_*` unset, one at a time, herdr 0.9.3: `rex_native_fresh`, `rex_workspace_resumes`, `rex_open_starts_a_fresh`, `rex_manual_resume_elsewhere`, `herdr_native` each 1 passed. Real Claude was rerun on this commit; see the next section.

CI on 4b4c78b24: the run succeeded with rust-test, rust-lint, macos and e2e-smoke skipped. `.github/CI.md` calls this deferred, not passing: a PR head with scratch defers its tests to the merge queue. No CI test result exists for this tree.

## November 6 headless demo: real Claude on 4b4c78b24

Isolated Rex server, private LF_HOME, one pinned copy of the branch binary (`lf 0.15.1+4b4c78b24`), real Claude with ambient credentials. The tree outside `scratch/` is unchanged from 4b4c78b24. LOO-498's conversation, its panes and the retained roots were not touched. No person watched this run, so nothing here is Jack Heart's acceptance.

Task a6d291, placed with `lf task checkout`, no conversation. The launcher was a shell in the Rex session:
- `"$LF_BIN" open --task a6d291 --json`, with no host or server flag and `REX_SERVER` empty: exit 0. Rex was found from inside the shell, and the tab landed in the caller's session with Conversation, Task & workflow and shell. Earlier real-Claude runs passed `--host rex --server`.
- Pane printed “Nothing to resume here; starting a new conversation.”, then the trust dialog. After Yes, Claude ran its first turn.
- `lsof` on provider PID 85253: cwd is the Task worktree. OSC 7 on Conversation and Task & workflow names the same path, before and after reopening.
- Typed “Reply with exactly the word quince-4 and nothing else, and run no tools.”; Claude answered `quince-4`.
- Same command again: exit 0, new tab, one Session, same PID 85253, `quince-4` on screen with no replay. The Conversation pane moved to the new tab; the first tab kept its Task & workflow and shell. Each opening adds those two, as `docs/workspaces.md` says.
- Typed into the reopened pane; Claude answered `pear-9`.
- `/exit`: PID gone, Session closed, `lf ps` empty.

Not proven:
- `rex split -b <Conversation>` started in `/Users/jack`, and so did a split from a shell that had run `cd /tmp`. This is the omitted-cwd path: `rex split` does not inherit, and it is not the app's `pane.split`. It says nothing about the Split shortcut either way. The OSC 7 reports Split inherits from are correct.
- Split shortcut and resize need the Rex app in focus, by hand, in a separate Task.
- A branch build on the real Machine still hands its child to the installed runtime until `lf install`. The cause of the November 6 exit stays unknown.

Evidence: `/Users/jack/.local/share/loopflow/demos/loo498-20261106-head`. Rex server 53061 stopped by its recorded group and `/tmp/loo498-head-tYqw` removed. Provider transcripts and the `~/.claude.json` project entry for that path remain.

Stand-in decision: advance to deliver. The accepted target, an ordinary Task conversation from `lf open`, held on this tree, and what remains needs a person or an install, which another implement pass cannot supply. The two choices in [questions.md](questions.md) stay open.

## November 7 headless demo: Task with no checkout, on 8a8ae78ef

Isolated Rex server, private LF_HOME, one pinned copy of the branch binary (`lf 0.15.1+8a8ae78ef`), real Claude with ambient credentials. LOO-498's conversation and panes and the retained roots were not touched. No person watched, so nothing here is Jack Heart's acceptance. Focused test `opening_an_unplaced_task_places_it_and_starts_its_conversation`: 1 passed.

Held, for `"$LF_BIN" open --task d32d29 --json` from a shell in the Rex session, no host or server flag, Task created with no worktree:
- Exit 0. The worktree and branch appeared, and the tab held Conversation, Task & workflow and shell. The pane printed “Nothing to resume here; starting a new conversation.”, then the trust dialog.
- `lsof` on provider PID 85879: cwd is the Task worktree. Claude answered `damson-7`.
- Same command again: one Session, same PID, `damson-7` on screen, no second worktree. Claude answered `medlar-3` in the new pane. `/exit`: PID gone, Session closed, `lf ps` empty.
- No Graph started. An abandoned Task (73625d) opened as Task & workflow alone, 0 shells, no worktree. Broad `lf open` created no worktree: e6be5f, unstarted, got none and no tab.

Failed: broad `lf open` starts agents. Run from the repository with three placed Tasks, it launched three Claude providers:
- 200837 and d8456d had no conversation. Each started fresh and took an unprompted turn from the Task-session prompt, in bypass permissions: 16–17 assistant messages and 6 tool calls each (`broad-transcripts.txt`).
- d32d29's Session had ended with `/exit`. Its provider was relaunched.
- The Task's accepted outcome says “Restore familiar state without eagerly starting agents for every historical Session.” LOO-709 removed `lf resume --all` for this reason, and broad open now does the same per repository. The cause is in `launch()`: every non-arrival gets `resume: true`, so the explicit Task case and the broad case share one branch. This predates 8a8ae78ef. 8a8ae78ef widens it slightly: a Task opened once stays placed, so it is started by later broad opens.
- `docs/workspaces.md` says “Opening does not start the Task's Graph or send a message.” The fresh start's first turn contradicts the second half, for `--task` too.

Smaller, seen by accident: with no `default_agent` and several harnesses installed, `lf resume --task` exits 1 and Rex closes the Conversation pane. The tab is left with Task & workflow and shell, the receipt still says `opening`, and the error is shown nowhere. Reproduced by hand in the Task's shell.

Not rerun: Split shortcut and resize, which still need the Rex app by hand. Two-Machine behavior.

Evidence: `/Users/jack/.local/share/loopflow/demos/loo498-20261107-unplaced`. Server 47264 stopped by its recorded group; the broad open's nine processes outlived it as detached owners, and were stopped by verified PID. `/private/tmp/loo498-unplaced-GiNF` removed. Provider transcripts and `~/.claude.json` project entries for those paths remain.

Stand-in decision: iterate to implement. Rule to build is in [questions.md](questions.md).

Built after that demo, not yet seen in Rex: broad gating, doc correction, harness failure. Fixture Rex only: `cargo test -p loopflow --test integration -- --test-threads=1 work_open_tests::` 27 passed, 2 ignored; `cargo test -p loopflow --lib lf::commands::open::` 17 passed. Rerun the three-Task broad open in the demo.
