# LOO-535 · Demo worlds for LOO-514, LOO-515 and LOO-500

Status: implemented 2026-10-20 and reconciled onto #1614 (stand-in `claude`);
demo and Task comments remain. Jack Heart requested these worlds
2026-10-20 as autonomous follow-ups to lf-world (#1612).

## Problem

LOO-514 (Wave backlog / conversation / open-Task dashboard), LOO-515 (brief,
PR and files beside the Task conversation) and LOO-500 (adding and deleting
Waves) each need a hands-on Desktop demo. Today the only places to try them are
Jack's real store or hand-written mocks. Each of those Tasks needs a world
built through production writers that already holds the states its demo goes
through, so its author can run `run-debug --world` and do the loop.

## Demo

```bash
cargo build -p loopflow --bin lf-world
target/debug/lf-world list            # three new rows, each naming its Task
target/debug/lf-world build wave-backlog /tmp/w514
uv run scripts/loopflow-dev.py run-debug --world /tmp/w514
```

- **wave-backlog** (LOO-514): the `desktop` Wave opens with a ranked backlog of
  readable unstarted briefs, four open Tasks (recent Flow, failed Flow,
  Session-only, merged-but-open), two completed, and a Wave conversation that
  discusses the backlog. A second Wave makes switching meaningful; a third is empty.
- **task-review** (LOO-515): selecting a Task shows a long Markdown brief, a
  Changes view over real commits in a deep tree, and a **Pull request** tab
  that renders title, state, checks, reviewers and per-file patches. Other
  Tasks show merged, draft, no PR, an empty brief and a PR whose read fails.
- **wave-lifecycle** (LOO-500): Waves list holds an empty Wave, a Wave with
  Tasks/Sessions/conversation, and a Wave whose work is all completed. This is
  the before-state for add → use → remove (LOO-500 builds the remove path).

## Risk findings

1. **The PR view's body is live GitHub.** `TaskPullRequestView` →
   `PullRequestStore` runs `gh pr view <url> --json number,url,title,…,statusCheckRollup`,
   then `gh api --hostname <h> --paginate --slurp repos/<o>/<r>/pulls/<n>/files?per_page=100`
   and a `--jq` head check (`swift/LoopflowMac/PullRequestStore.swift`). The
   store only decides whether the tab appears (`publication.github.url`). A
   fake URL with real `gh` renders only an error line.
   → **The world supplies its own `gh`** (new rung, below).
2. **`gh` resolves through PATH.** `queryGitHub` runs `/usr/bin/env gh` with
   `GUIProcessEnvironment.enriched`, which *prepends* `~/.local/bin`,
   `/opt/homebrew/{bin,sbin}`, `/usr/local/{bin,sbin}`, `~/.cargo/bin`, but only
   those absent from the inherited PATH. So `env.json` sets
   `PATH=<world>/bin:<those six>:/usr/bin:/bin:/usr/sbin:/sbin`. Nothing gets
   prepended, and the world's `gh` comes first; the bundled-`lf` dir is
   prepended later and holds only `lf`. The six-entry list is copied from
   Swift; a comment names its source. If it drifts, the real `gh` wins and the
   view shows a visible gh error, not wrong data. Agents launched inside the
   world get the same `gh`, so a world can never reach real GitHub. That is
   the same isolation `lf clone` gives.
3. **Recording PRs without GitHub.** `request_task_pr_publication` +
   `attach_task_github_pr(PrInfo{…})` give an Open PR with no network call,
   provided the Task has no `linear_id` (world Tasks don't) and no merge
   request is stored. The body must carry `> **Task:** <title> · <id>`.
   Merged has no GitHub-free op. The world does a real merge into `main`,
   pushes to `origin.git`, and calls `store.settle_task_pr_merged(pr, at)`
   with that commit. Rust's own PR observation degrades ("could not resolve
   GitHub owner/repo") for a bare local origin and keeps stored rows.
4. **Wave conversation = primary Session row.** `agent_sessions.primary_scope='wave'`,
   inserted by `store.ensure_primary_session(scope, None, LfSession)`, which
   *reserves* a new row (`reserve_session_in`). There is no Wave form of
   `bind_session`. The world builds the LfSession the way `primary::wave_session`
   does and gives it the transcript's AgentSession, so native history opens. It
   does not admit the transcript first and then re-mark it.
   Resolved (kickoff, 2026-10-20): the selection writer is the tail of
   `provider_conversation::admit` (`ops/human_session/provider_conversation.rs:268-282`):
   `publish_prepared_input(…, Independent)`, then
   `write_provider_session(capture_dir, id, account)`, then the account route pin
   when present. Extract that tail into one crate-visible function taking the
   reserved LfSession, provider, conversation id and account; `admit` and
   `World::wave_conversation` both call it. No second copy.
5. **Wave delete does not exist.** No CLI delete/archive; `delete_wave` is
   test-only; Desktop's "Delete Wave" menu is never wired (`WavesView.swift:192`
   passes no `onDelete`); Desktop's `CreateWaveSheet` writes GOAL.md without a
   store row, so the Wave is invisible until `lf wave ensure`. All of that is
   LOO-500's to design. The world gives that demo its starting Waves.
6. **Live turns come from #1614's stand-in `claude`.** `<world>/bin/claude`
   resumes a world Session from its transcript and echoes each input line, so
   every recipe's Sessions (including the Wave conversation, which starts and
   resumes as Claude per `.lf/config.yaml`) open live without an account.
   Because `bin/` leads `PATH`, a real model answer inside a world needs the
   person to run a real provider explicitly; lf-world never copies credentials.
7. **Known Desktop defects the worlds will surface, not fixed here:** a
   completed Task lists under "Started" (`WaveTaskInventory` splits only on
   `runtime.started`; LOO-514 owns it), and AttributeGraph cycles on the
   populated Wave view. Liveness/Waiting comes from real provider streams, so
   no recipe fakes a waiting Task.

## Approach

### New rung: Providers

`Rung::Providers`: rows and artifacts, plus stand-ins for external services
that Desktop or `lf` read. The first is GitHub's PR reads.

- `World::create` writes `<world>/bin/gh`, a POSIX sh script with a header
  comment. It answers exactly three shapes from `<world>/github/pulls/`:
  `pr view <url> --json …` (`<n>.json`), the same with `--jq` (`<n>.heads`),
  and `api --hostname github.com --paginate --slurp repos/demo/project/pulls/<n>/files…`
  (`<n>.files.json`, already slurped `[[…]]`). A PR number marked unavailable
  exits 1 with gh-shaped stderr (`HTTP 502: Server Error`). Anything else exits 1
  with `lf-world's GitHub answers only PR reads: <args>`.
- `WorldEnv` gains `PATH` (risk 2). `env.json` stays the one environment
  contract for run-debug, screenshots and tests.

### World API additions (`rust/loopflow/src/world/mod.rs`)

- `files(task: Option<&Task>, files: &[(&str, &str)], message)`: write into
  the repo (pushed to origin, so later checkouts base on it) or a Task's
  worktree, then commit. `uncommitted(task, files)` writes without committing.
- `pr(task, number, title, body, PrSurface)`: `request_task_pr_publication` +
  `attach_task_github_pr` with `PrInfo` from the worktree head. Then it writes
  the stub's files from real git (`diff --numstat` and per-file patches
  `base..head`, base/head oids). PR additions therefore match the Changes view's
  committed diff. `PrSurface` holds state/draft, checks
  (name, conclusion), review decision, reviewers, or `Unavailable`.
- `merge(task, pr)`: `git merge --no-ff` of the Task branch on `main`, push,
  `settle_task_pr_merged` with the merge commit, and rewrite the stub file to
  `MERGED`.
- `wave_conversation(wave, title, turns, started_days_ago)`: shares the
  transcript writer with `session()` (factor it out of `session()`), then
  `ensure_primary_session(Wave)` with a `wave_session`-shaped LfSession and the
  extracted admit tail (risk 4).
- Task briefs come through `task_create`'s report as today.

### Recipes (`recipes.rs`), each `demos:` naming its Task

| recipe | rung | demos |
|---|---|---|
| `wave-backlog` | artifacts | LOO-514: Wave backlog, conversation and open-Task dashboard together |
| `task-review` | providers | LOO-515: brief, changes, files and PR beside the Task conversation |
| `wave-lifecycle` | artifacts | LOO-500: adding a Wave and removing empty and populated ones |

- **wave-backlog:** Waves `desktop` (plan with 2 KRs), `systems` and empty
  `research`. `desktop` has 7 unstarted Tasks with multi-paragraph briefs
  (ranked in creation order), 4 open Tasks, 2 completed, a Wave conversation
  of about 8 turns referencing backlog items by title, and one taskless
  Session. The open Tasks are: Flow ended 3 minutes ago; failed gate; Session
  only, no Flow; PR merged, Task still open, via the PR path on the artifacts
  rung with no stand-in needed for the board. `systems` has 2 Tasks.
- **task-review:** the repo is seeded with about 60 files, up to 5 levels deep
  (`src/search/index/…`, `docs/`, `tests/`), including one ~1,500-line file
  for scroll retention. Tasks in Wave `desktop`:
  1. Open PR #12, long Markdown brief (headings, lists, links, code, about 120 lines), 3
     commits over 6 files, checks 2 pass / 1 fail, review required, 1
     reviewer, plus an uncommitted local edit, so local ≠ published. Task
     conversation (bound Session) asks about the failing check.
  2. Draft PR #13, checks pending.
  3. Merged PR #11, Task still open ("merged does not complete").
  4. Merged PR #10, Task completed.
  5. No PR, local commits only, brief empty.
  6. PR #14 whose read fails (Unavailable).
- **wave-lifecycle:** `research` empty; `desktop` with 2 unstarted, 1 open
  (Session + Flow), 1 completed and a Wave conversation; `archive` with only
  completed work. The repository conversation is left alone.

### Delete — do not maintain

Nothing. `mock-waves` loading/error stay (no data produces them); inline
JSON for view logic, failure/transitional states, terminal/focus proofs and
DTO fixtures stays by the Task's direction. No earlier GitHub stand-in exists to
remove.

### Forbidden outcomes

- A Swift seam or env flag that makes `PullRequestStore` read files instead
  of `gh`; the app runs unchanged.
- Writing PR/Task/Session rows by raw SQL where a production op exists.
- A recipe that copies credentials or reaches real GitHub/Linear.
- Fake liveness (a "running" process with no process).

## Absent and error states

- Stand-in with no file for a number: same as GitHub's not-found (exit 1,
  stderr), shown as the view's error line.
- `env.json` without PATH (older worlds): app uses real `gh` and shows its
  error. Worlds are rebuilt, not migrated.

## Exclusions

- LOO-514/515/500 product work itself: views, the Started-bucket fix, Wave
  delete.
- Historical (superseded-sequence) PRs: one PR per Task here; add when
  LOO-515's design needs it.
- Linear-linked Tasks; live waiting/attention.

## Done when

- `lf-world list` names all five recipes; each new `demos:` names its Task.
- Gate (headless, from the repo root with inherited `LF_*` unset, `< /dev/null`):
  `cargo test -p loopflow --test integration world_recipes` passes, extended so that:
  - every recipe builds and `lf` reads Waves and Sessions (existing);
  - a `one-task` Session connects through the stand-in `claude` and echoes
    input (#1614's test, kept);
  - `wave-backlog`: `lf wave show desktop --json` has ≥7 unstarted and ≥4
    started-incomplete Tasks, and `lf session list --all --json` has one
    `primary_scope == "wave"` Session for `desktop`;
  - `task-review`: Task 1's `pr.publication.github.url` is set; running the
    world's `bin/gh pr view <url> --json <Swift's field list>` returns
    JSON whose `additions` equals the sum of committed additions from
    `lf task diff <id> --files --base parent --json`; PR #14 exits non-zero;
    Task 3 has a merge commit and is not completed;
  - `wave-lifecycle`: `lf wave list --json` includes `research` with zero Tasks.
  - `cargo fmt`, `cargo clippy --all-targets -- -D warnings`.
- Demo (display, person): each recipe opens in `run-debug --world` and its
  Task's surfaces populate as listed under Demo; the PR tab renders from the
  stand-in. Optional evidence: `screenshots.yaml` shots for `task-review`
  and `wave-backlog`.
- LOO-514, LOO-515 and LOO-500 each get a comment naming their recipe and
  the build/run commands.

## Implementation decisions (2026-10-20)

- `wave_conversation(wave, turns, days)` calls `primary::ensure`, the same
  path as `lf wave session`, then writes the transcript in that Session's
  Wave worktree. No title argument: production names it after the Wave.
- The extracted tail is `provider_conversation::select`. When the Session has
  no reserved input (a fresh Wave primary reserves none until connect), it
  calls connect's `append_input` first. `admit` and the World both use it. The World
  trusts its committed `agent: claude` and does not re-check the provider.
- The World repo commits `.lf/config.yaml` with `agent: claude`; otherwise a
  fresh world resolves Codex and the Wave primary wouldn't match its
  Claude history.
- `session()` resumes in the Task's checkout when there is one. Claude
  project directories are named with every non-alphanumeric character as `-`.
- The gate compares PR #12 against `lf task diff --files --json`: equal
  base/head oids, PR filenames equal to committed paths, and additions equal
  to the sum over files. That snapshot has no additions field.
- The `gh` stand-in returns the whole stored object for any `--json` list.
- Reconciled with #1614 (2026-10-20): `bin/` holds both stand-ins, written by
  one `write_executable`. `PATH` keeps this branch's fixed list (risk 2), not
  #1614's `bin:<builder PATH>`: an inherited PATH can carry the person's real
  `gh` ahead of nothing Desktop would prepend, and makes `env.json` depend on
  where lf-world ran. #1614's `LF_*` scrub and its stub-provider test stay;
  the test uses `Built`.

## Remaining

- Demo (display, person): `run-debug --world` on each recipe; the PR tab
  renders from the stand-in.
- After landing: comment on LOO-514, LOO-515 and LOO-500 naming the recipe
  and the build/run commands.

## Checks

- `cargo test -p loopflow --test integration world_recipes` 2 passed (77 s), clippy and fmt clean after compress 2; realign 2026-10-20 found no code change and `origin/main` unchanged since base `051a856109`, so no rerun.
