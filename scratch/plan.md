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

## Design

One type describes the change a message is written for, and renders the one
prompt section both writers use. Suggested home: `ops/change.rs`.

```rust
enum ChangeRange { Staged, Since(String) }   // --cached | origin/<base>...HEAD
struct ChangedFile { path, renamed_from: Option<String>, lines: Option<(u64, u64)>, working_note: bool }
struct Change { range: ChangeRange, files: Vec<ChangedFile>, patch: Option<String> }
impl Change { fn read(repo, range) -> OpsResult<Self>; fn prompt_section(&self) -> String }
```

The section always holds:

- **Every changed file**, one line each: added and deleted line counts and the
  full path. Read from `git diff --numstat -z <range>` and rendered by lf,
  because `--stat` abbreviates long paths (`.../x.rs`) and writes renames as
  `{a => b}/x`, neither of which can be pasted into a command. A rename prints
  `path (from old)`; a binary file (`-\t-`) prints `binary`.
- **The read command**, spelled for this range: `git diff --cached -- <path>`
  or `git diff origin/<base>...HEAD -- <path>`.

Then one of two things:

- The whole patch, when it is at most 100,000 bytes. Nothing was selected, so
  there is nothing for the writer to decide and it answers in one turn, as it
  does today.
- No patch, and a sentence saying so: the patch is N bytes, read the files
  that carry the change with the command above. The writer picks.

No file is ever cut, and no prefix exists. `[diff truncated]` goes.

PR ranges include notes added under `scratch/`, because the copy is written
before `lf scratch clear` runs (`ops/land.rs:159` then `:171`). Those lines
are marked from `git::scratch::added` as working notes that are removed
before the PR publishes: worth reading for why, not part of what ships. A
commit's stat lists scratch plainly; it is being committed.

The read instructions live in the section lf builds, not only in the skill
text, because `.lf/skills/pr-message.md` and other repositories' copies
extend or replace the builtin.

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
  design (the patch is inlined whole or not at all), recorded in case the
  fallback below is ever built.
- **Sizes of the last 40 merged PRs:** 24 are under 100 KB; the largest are
  1.8 MB, 3.2 MB and 4.6 MB (741 files). The old cut dropped something from
  33 of 40. A 741-file stat is about 50 KB.

## Choices made for review

1. **Changes up to 100,000 bytes are inlined whole.** Passed over: never
   inline, so there is one shape and no threshold. That adds a tool turn to
   every step commit and lets a hurried writer answer from the stat alone on
   changes it could have been handed complete. 100 KB is about 30k tokens,
   inside every configured writer's window.
2. **The importance-ordered fallback is not built.** Its condition, a writer
   that cannot run commands, matches no launch lf makes today (first finding).
   A writer whose vendor configuration denies the command still has the
   complete stat and writes from it. If a toolless harness arrives, it gets a
   `Capabilities` field and the fallback attaches in `Change::prompt_section`:
   product code, then docs, then tests, then generated and lock files, then
   scratch, whole files only.
3. **Scratch notes are marked, not hidden**, in the PR stat. The `pr-message`
   skill asks for "what people said that shaped it, what a run showed", and
   the notes are where that is.

## Surfaces

- `ops/commit.rs` `generate_commit_message`: used by `lf commit` and
  `commit_step` (every Graph step, 300 s timeout).
- `ops/pr.rs` `generate_pr_copy`: used by `lf pr submit`/publish and
  `ops/land.rs:344`. Its `## Diff stat` and `## Unified diff` headings merge
  into the one section.
- Builtins `ops/skill/commit-message.md` and `pr-message.md`: "Read the diff"
  becomes reading the listed files that carry the change. Keep the rest word
  for word. `.lf/skills/pr-message.md` extends the builtin and needs no edit
  unless it names the old headings.
- `docs/orchestration.md:58` describes the writers; add one line on large
  changes only if it reads naturally there.

## Absent and error states

- Nothing staged, or an empty PR range: unchanged; both callers return before
  the writer launches.
- `git diff` fails: the existing `OpsError::CommandFailed`.
- The writer reads nothing: it still has every path and line count. No retry.
- Paths with spaces or quotes: the stat prints the raw path from `-z` output;
  the instruction says to quote it.

## Exclusions

- `lf task diff`'s 1 MB whole-comparison cap (`ops/task/snapshot.rs:586`) and
  Desktop's workaround in LOO-515.
- Release-notes context limits in `ops/release.rs`.
- Changing which agent writes messages, or the 300 s step-commit timeout.

## What goes

- Both `truncate_chars` (`ops/commit.rs:359`, `ops/pr.rs:2571`) and
  `[diff truncated]`.
- The separate `--stat` and full-diff reads in `generate_pr_copy`, and the
  `## Staged diff`, `## Diff stat`, `## Unified diff` prompt headings.
- `pr_tests.rs:1716`'s assertion on `## Unified diff` moves to the new
  heading; its point (the next agent gets the same whole prompt) stays.

Must survive: the JSON reply contract, `message_launch` agent selection and
fallback agents, cached gate PR copy, base and commit-log sections of the PR
prompt.

## Slices

1. **`Change` and the commit writer.** `Change::read` and `prompt_section`,
   wired into `generate_commit_message`; commit.rs's `truncate_chars` goes.
   Unit tests on a `TestRepo`: a staged change over 100 KB whose meaningful
   file sorts last lists every file with counts, carries the read command and
   no patch; a small one carries its whole patch; a rename and a binary file
   render usable paths. Then the real run below with `lf commit`. This is the
   slice that could prove the plan wrong: it shows whether a cheap writer
   reads the right file when handed only the stat.
2. **The PR writer.** `generate_pr_copy` uses `Change` with
   `Since(origin/<base>)`, scratch notes marked; pr.rs's `truncate_chars` and
   the two git reads go; `pr_tests.rs` updated.
3. **Skill text and docs.** The two builtins and `docs/orchestration.md`.

## Demo

In a repository with a staged `aaa_generated.txt` of 200 KB and a ten-line
`zzz_retry.rs` that adds a retry:

```bash
lf commit            # the message names the retry in zzz_retry.rs
lf pr submit         # on a branch with the same change: the description does too
```

Before this change both describe only `aaa_generated.txt`.

## Checks

- `cargo test -p loopflow-execution --lib ops::change::` — slice 1 and 2 unit tests; implement runs it.
- `cargo test -p loopflow --lib pr_tests::` (the `tests/work` module) — implement runs the touched test.
- The demo with a real agent, both writers — qa or gate runs it; no fixture stands in for a writer choosing what to read.
