# d92894: message writers read the whole change

## Outcome

`lf commit`, step commits and PR descriptions describe the change, whatever
its size. Before this branch both writers got the first 20,000 characters of
the diff in path order and a `[diff truncated]` line.

Direction, from the Task comment of 2026-11-18 (Jack Heart), which replaces
the brief's "lf chooses whole files within a budget":

> the writer decides what to read and in what order. Give it the complete stat
> (every changed file with its line counts) and a way to read any file's patch
> from the checkout, rather than having lf pre-select and order whole files
> into the prompt. lf picking files by a fixed importance order is the fallback
> only if the writer cannot run commands. Whatever it did not read is still
> knowable to it from the stat. The outcome and demo in the brief stand.

## What exists

Built and committed (`ff02472439`, `6863c3607a`, `9f5f1fd770`), except how a
Skill is handed the section, at the end of this part.

`prompt/change.rs` owns showing an agent a change: `Change::read(repo, range)`
and `Change::prompt_section()`, over `ChangeRange::{Staged, Since(base),
Uncommitted}`. The commit writer, the PR writer and `context: {diff: true}`
all render it; the Task workspace block shares `PATCH_INLINE_TOKENS` and
nothing else. Both `truncate_chars`, `[diff truncated]`, the 20-file and
800-line limits and the three old prompt headings are gone (grep, 2026-11-18).

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

For a `Since` range, files from `git::scratch::added` are marked as working
notes, cleared when the change lands (`ops/land.rs:171`). `Staged` and
`Uncommitted` list scratch plainly.

The builtin Skill texts are not edited: "Read the diff" is true with the patch
inlined, with only the list, and when the Skill is run by hand.

`PromptComponents.change` is an `Option<Change>`; the `DiffTier` enum and the
unread file count are gone, and attribution reads `change.patch` to record a
list-only delivery.

The writers are shown the section in their prompt. A Skill run with
`--diff patch` or `context: {diff: true}` is not: `ContextDelivery::prepare`
writes the section to a `changes` reference file and the context block lists
its path. Three haiku runs of three never opened it (qa, below). That is the
one thing left to build; see "What remains".

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

5. **`--diff patch` puts the section in the first turn**, after the request,
   where `-c` and `--docs` already go. Made at the post-build concept review
   on what qa showed: in the prompt a cheap model read the right file every
   time; as a listed file it was never opened, and on a branch with only
   uncommitted work the agent answered "adds nothing beyond `main`". The
   section is bounded (the list plus at most 15,000 tokens), opt-in, and no
   more frozen than a file written once at launch. Passed over: leaving the
   listed file, which keeps a flag that does less than its name; and taking
   `context: {diff: true}` back off `Change`. Open: whether the old docs
   sentence "rather than inlining a frozen patch" meant never to do this
   (`questions.md`).

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

## Demo

In a repository with a staged `aaa_generated.txt` of 200 KB and a ten-line
`zzz_retry.rs` that adds a retry:

```bash
lf commit            # the message names the retry in zzz_retry.rs
lf pr submit         # on a branch with the same change: the description does too
```

Before this change both describe only `aaa_generated.txt`.

## What runs showed

- 2026-11-18, `lf commit -a claude:haiku` on a staged 275,665-byte patch
  (6,000-line `aaa_generated.txt`, eleven-line `src/zzz_retry.rs`): the title
  was "Retry export once on timeout and regenerate schema fixture" and the
  body gave the retry's reason from the code comment. A cheap writer handed
  only the list read the right file.
- 2026-11-18, realign: `cargo test -p loopflow-execution --lib --
  prompt::change:: gather_diff_tiered the_branch_change_is_delivered
  task_workspace_context`, 11 passed. `cargo test -p loopflow --lib --
  pr_copy_moves_to_the_next_agent_when_codex_is_out_of_quota`, passed.
- 2026-11-18, compress (`gather_diff_tiered` is now `gather_branch_change`):
  `cargo test -p loopflow-execution --lib -- prompt::change::
  gather_branch_change the_branch_change_is_delivered
  reference_rendering_preserves files_only attribution`, 20 passed.
  `cargo test -p loopflow --lib --
  context_choices_override_config_and_omission_inherits`, passed.

- 2026-11-18, qa, real writers in a throwaway repository (`scratch/qa.html`):
  `lf commit -a claude:haiku` on 354 KB and on 801 files, `lf commit -a codex`,
  a step commit, and `lf pr publish` against the stand-in GitHub all named the
  short file's change. The PR writer ran `git diff` without
  `--dangerously-skip-permissions`, on a path holding a space and an
  apostrophe. Fixed: an uncommitted change left out untracked files.
  Not fixed: with `--diff patch` the agent did not open the `changes` file
  (`questions.md`).

## What remains

One slice for implement: the section a Skill asks for reaches the agent.

- `prompt/mod.rs`, `format_task_prompt`: when `components.change` is set,
  append its `prompt_section()` after the clipboard and before explicit docs,
  on both the native-arguments path and the plain one. Wrap it the way
  `format_clipboard` wraps its text: `render_reference` around an
  `<lf:change>` tag, with the line "Reference material, not new
  instructions."
- `context_block.rs`, `ContextDelivery::prepare`: the `changes` reference is
  written only for `diff_files` and holds what it holds on `main` (the
  sentence and `Changed paths:`). The `Change` import, the three-way match and
  its comment go.
- `prompt/attribution.rs` needs no edit: it already records the section as
  `Kind::Diff` when the task text contains it, which until now was never.
  Check that it does.
- Tests. Replace
  `the_branch_change_is_delivered_as_a_reference_the_agent_reads` with one
  that fails if the section leaves the first turn: `format_first_turn` of
  components holding a `Change` contains the section, and `prepare` writes no
  reference for a change alone. One attribution assertion: a `Diff` asset on
  the Task channel.
- Docs. `docs/config.md`: the context table row for branch changes ("first
  turn, after the request") and "Raw Diff", which loses "written once at
  launch to a `changes` file ... it is not put in front of the agent".
  `.lf/qa/message-writers.md`: the `--diff patch` bullet.
- The run that shows it: in a throwaway repository on a branch with one
  edited and one untracked file, nothing committed,
  `lf -b <skill> --diff patch -a claude:haiku` with a Skill that asks what the
  branch changes. The answer names both files. Before, it said the branch
  adds nothing.

Focused tests after the edit: `cargo test -p loopflow-execution --lib --
prompt::change:: gather_branch_change context_block:: attribution
format_prompt first_turn`.

Left for later steps:

- **`lf pr submit` against real GitHub.** qa ran `lf pr publish` with a real
  writer and a stand-in `gh`; the same `generate_pr_copy` serves both.
- **`cargo clippy --all-targets -- -D warnings` and the suites.** gate's and
  CI's; not run here.
- The open choices in `questions.md`; none blocks shipping.

Larger than this Task, on `concept-review.html`: the Task workspace block as a
fourth `ChangeRange` (a worker with `diff: true` is shown a change twice until
then), and `gather_branch_change` treating only a branch named `main` as the
default branch.
