# d92894: message writers read the whole change

## Outcome

`lf commit`, step commits and PR descriptions describe the change, whatever
its size. Today both writers get the first 20,000 characters of the diff in
path order (`ops/commit.rs:268`, `ops/pr.rs:539`) and a `[diff truncated]`
line.

Direction, from the Task comment of 2026-11-18 (Jack Heart), which replaces
the brief's "lf chooses whole files within a budget":

> the writer decides what to read and in what order. Give it the complete stat
> (every changed file with its line counts) and a way to read any file's patch
> from the checkout, rather than having lf pre-select and order whole files
> into the prompt. lf picking files by a fixed importance order is the fallback
> only if the writer cannot run commands. Whatever it did not read is still
> knowable to it from the stat. The outcome and demo in the brief stand.

## Model

lf shows an agent a change in three places today, each with its own code and
size rule:

| Where | Patch whole when | Otherwise |
|---|---|---|
| Commit and PR writers (`ops/commit.rs`, `ops/pr.rs`) | never past 20,000 characters | a prefix |
| `context: {diff: true}` (`prompt/mod.rs` `gather_diff_tiered`) | under 15,000 tokens, 20 files, 800 lines | `git diff --stat` |
| Task workspace block (`ops/task/snapshot.rs` `task_workspace_context`) | under 15,000 tokens | "Read the named worktree paths" |

This Task gives the idea one owner, and does not add a fourth rule. One type
describes a change as shown to an agent and renders the one prompt section.
The first two rows move onto it; the third shares its number.

Home: `prompt/change.rs` (both `prompt/mod.rs` and `ops/` reach it).

```rust
pub const PATCH_INLINE_TOKENS: usize = 15_000;
pub enum ChangeRange { Staged, Since(String), Uncommitted }
//                     --cached | <base>...HEAD | HEAD
pub struct ChangedFile { path, renamed_from: Option<String>, lines: Option<(u64, u64)>, working_note: bool }
pub struct Change { range: ChangeRange, files: Vec<ChangedFile>, patch: Option<String>, patch_bytes: usize }
impl Change { pub fn read(repo, range) -> Result<Self, _>; pub fn prompt_section(&self) -> String }
```

The section always holds:

- **Every changed file**, one line each: added and deleted line counts and the
  full path. Read from `git diff --numstat -z <range>` and rendered by lf,
  because `--stat` abbreviates long paths (`.../x.rs`) and writes renames as
  `{a => b}/x`, neither of which can be pasted into a command. A rename prints
  `path (from old)`; a binary file (`-\t-`) prints `binary`.
- **The read command**, spelled for this range: `git diff --cached -- <path>`,
  `git diff origin/<base>...HEAD -- <path>` or `git diff HEAD -- <path>`.

Then one of two things:

- The whole patch, when it is under `PATCH_INLINE_TOKENS`
  (`prompt::count_tokens`). Nothing was selected, so the writer answers in one
  turn, as it does today. Skip the tokenizer when the patch is over 200,000
  bytes; it cannot fit.
- No patch, and a sentence saying so: the patch is N bytes, read one file at
  a time with the command above, starting with the files that carry the
  change. The agent picks.

No file is ever cut, and no prefix exists. `[diff truncated]` goes.

For a `Since` range, `Change::read` marks files from `git::scratch::added` as
working notes: cleared when the change lands (`ops/land.rs:171`), worth
reading for why, not part of what ships. `Staged` and `Uncommitted` list
scratch plainly.

The reading instructions live in the section lf builds. The builtin Skill
texts are not edited: "Read the diff" is true with the patch inlined, with
only the list, and when the Skill is run by hand with no section.
`.lf/skills/pr-message.md` and other repositories' copies keep working
unchanged.

## Findings

- **Every writer can run the read command.** `claude -p` with no permission
  flags ran `git diff --cached -- a.txt` in a fresh repository and returned
  the added line (probe, 2026-11-18, haiku). Unattended runs already get a
  floor (`docs/config.md`, Yolo): Claude skips prompts, Codex gets
  `workspace-write` with `approval_policy = "never"`. OpenCode and Pi are
  shell agents too. `harness::Capabilities` has no "no tools" case.
- **Prompt size is not bounded by argv.** Batch prompts go over stdin
  (Claude stream-json, `harness/claude.rs:1637`) or the Codex app-server turn.
- **Patch sections and numstat entries match one to one** across the last 300
  first-parent commits, renames and binaries included. Not needed by the
  design, recorded in case the fallback below is ever built.
- **Sizes of the last 40 merged PRs:** 24 are under 100 KB; the largest are
  1.8 MB, 3.2 MB and 4.6 MB (741 files). The old cut dropped something from
  33 of 40. A 741-file stat is about 50 KB.
- **The commit writer always skips permission prompts
  (`ops/commit.rs:286`); the PR writer follows `yolo`.** `lf land` and
  `lf pr submit` already refuse when the PR writer moved HEAD or the worktree.
  Left as is.

## Choices made for review

1. **One size rule, the existing one: under 15,000 tokens the patch goes in
   whole.** Passed over: kickoff's 100,000 bytes, which fits 24 of the last 40
   PRs but is a new number beside two copies of 15,000 and would double what a
   `--diff patch` Skill can receive; and never inlining, which adds a tool
   turn to every step commit. PRs between the two sizes take the reading
   path, which costs a turn and loses nothing.
2. **`context: {diff: true}` moves onto `Change` in this Task.** Its 20-file
   and 800-line limits go; a Skill given a large branch gets full paths, line
   counts and the read command instead of an abbreviated `--stat`. Passed
   over: leaving it for its own Task, which ships the writers with three
   rules still standing. It is the last slice, so the writers do not wait on
   it.
3. **The importance-ordered fallback is not built.** Its condition, a writer
   that cannot run commands, matches no launch lf makes today. A writer whose
   vendor configuration denies the command still has the complete list and
   writes from it. If a toolless harness arrives, it gets a `Capabilities`
   field and the fallback attaches in `Change::prompt_section`: product code,
   then docs, then tests, then generated and lock files, then scratch, whole
   files only.
4. **Scratch notes are marked, not hidden**, in a branch change. The
   `pr-message` Skill asks for "what people said that shaped it, what a run
   showed", and the notes are where that is.

## Surfaces

- `ops/commit.rs` `generate_commit_message`: used by `lf commit` and
  `commit_step` (every Graph step, 300 s timeout).
- `ops/pr.rs` `generate_pr_copy`: used by `lf pr submit`/publish and
  `ops/land.rs:344`. Its `## Diff stat` and `## Unified diff` headings merge
  into the one section.
- `prompt/mod.rs` `gather_diff_tiered`: keeps its signature and its range
  choice (none on the default branch or detached; `origin/<base>...HEAD`;
  `HEAD` when nothing is committed). Its text becomes
  `Change::prompt_section()`, `DiffTier::UnifiedDiff` when the patch is
  inlined and `StatOnly` when it is not, so `prompt/attribution.rs` is
  untouched.
- `ops/task/snapshot.rs` `task_workspace_context`: its `MAX_PATCH_TOKENS`
  becomes `PATCH_INLINE_TOKENS`. Nothing else there changes.
- `docs/orchestration.md:58` and `docs/config.md` ("Include raw diff"): one
  line on large changes where it reads naturally.

## Absent and error states

- Nothing staged, or an empty PR range: unchanged; both callers return before
  the writer launches.
- `git diff` fails: the existing `OpsError::CommandFailed` in the writers;
  the Skill context path keeps treating a failed read as no diff.
- The writer reads nothing: it still has every path and line count. No retry.
- The writer reads the whole patch at once and overflows: the commit gets
  `lf <step>: checkpoint`, the PR writer returns its existing error. The
  section names the size and the per-file command to steer away from it.
- Paths with spaces or quotes: the list prints the raw path from `-z` output;
  the instruction says to quote it.

## Exclusions

- Moving the Task workspace block onto `Change`. It compares base, index,
  worktree and untracked files and reports a state per file; that needs a
  fourth range, and untracked files have no numstat.
- `lf task diff`'s 1 MB whole-comparison cap (`ops/task/snapshot.rs:584`) and
  Desktop's workaround in LOO-515.
- Release-notes context limits in `ops/release.rs`: a list of PRs and
  commits, not a patch, and it already reports what it left out.
- Which agent writes messages, the writers' permission stance, the 300 s
  step-commit timeout.

## What goes

- Both `truncate_chars` (`ops/commit.rs:359`, `ops/pr.rs:2571`) and
  `[diff truncated]`.
- The separate `--stat` and full-diff reads in `generate_pr_copy`, and the
  `## Staged diff`, `## Diff stat`, `## Unified diff` prompt headings.
- In `prompt/mod.rs`: `DIFF_TIER_THRESHOLD`, `DIFF_MAX_FILES_FOR_FULL`,
  `DIFF_MAX_LINES_FOR_FULL`, the `--shortstat` and `--stat` reads and
  `parse_shortstat_total_lines` if nothing else calls it.
- `MAX_PATCH_TOKENS` in `task_workspace_context`.
- `pr_tests.rs:1716`'s assertion on `## Unified diff` moves to the new
  heading; its point (the next agent gets the same whole prompt, all 1,200
  lines) stays. If that fixture's patch is over 15,000 tokens, shrink the
  fixture, not the rule.

Must survive: the JSON reply contract, `message_launch` agent selection and
fallback agents, cached gate PR copy, base and commit-log sections of the PR
prompt, the builtin Skill texts word for word, `gather_diff_tiered`'s range
choice and `DiffTier` as attribution reads it.

## Slices

1. **`Change` and the commit writer.** `prompt/change.rs` with `read` and
   `prompt_section`, wired into `generate_commit_message`; commit.rs's
   `truncate_chars` goes. Unit tests on a temporary repository: a staged
   change whose patch is over the rule and whose meaningful file sorts last
   lists every file with counts, carries the read command and no patch; a
   small one carries its whole patch; a rename and a binary file render
   usable paths. Then the real run below with `lf commit`. This is the slice
   that could prove the plan wrong: it shows whether a cheap writer reads the
   right file when handed only the list.
2. **The PR writer.** `generate_pr_copy` uses `Change` with
   `Since(origin/<base>)`, scratch notes marked; pr.rs's `truncate_chars` and
   the two git reads go; `pr_tests.rs` updated.
3. **Skill context and the shared number.** `gather_diff_tiered` reads a
   `Change`; its three limits and stat reads go; its tests keep their cases
   (none on main, uncommitted fallback, whole patch) and gain one for a large
   branch that gets the list and the command. `task_workspace_context` reads
   `PATCH_INLINE_TOKENS`. Docs lines.

## Demo

In a repository with a staged `aaa_generated.txt` of 200 KB and a ten-line
`zzz_retry.rs` that adds a retry:

```bash
lf commit            # the message names the retry in zzz_retry.rs
lf pr submit         # on a branch with the same change: the description does too
```

Before this change both describe only `aaa_generated.txt`.

## Checks

- `cargo test -p loopflow-execution --lib prompt::` — `Change` unit tests and the Skill context cases; implement runs it.
- `cargo test -p loopflow --lib pr_tests::` (the `tests/work` module) — implement runs the touched test.
- `cargo test -p loopflow-execution --lib ops::task::tests::task_workspace_context` — implement runs it after slice 3.
- The demo with a real agent, both writers — qa or gate runs it; no fixture stands in for a writer choosing what to read.

## Docs written first

`docs/orchestration.md` (after the `commit-message` agent paragraph) and
`docs/config.md` ("Raw Diff", and the "Branch changes" row of the delivery
table). They describe the intended behavior; implement builds to them.

The Raw Diff example settles a rendering the model left open: a first line
naming the range and file count, the read command with a quoted `'<path>'`,
right-aligned `+N  -N` columns, `binary` in place of counts, and a closing
sentence with the patch size in bytes. The exact wording is implement's.
