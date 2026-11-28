Read when the change touches skill resolution, `extends: builtin`, `lf list skill` or the commit after a skill step.

A fixture repository with a local bare origin (`git init --bare origin.git`,
clone it, push `main`, work on a branch) and a private `LF_HOME` is enough; keep
the real `HOME` so `claude` answers on the account's login. Repository skills
from `~/.codex/skills` still show in `lf list skill` under that `HOME`.

A private home has no `default_agent`. With only `-a claude:haiku` on
`lf graph <name> --batch` or `lf -b <skill>`, the step commit printed
`load error: No agent selected` and committed `lf <skill>: checkpoint`
(2026-11-25); set `default_agent` in the private home to see a written message.

`lf -b implement` usually commits its own work, so its step commit only pushes.
To see a message written, use a throwaway repository skill that edits a file and
is told not to run git. Don't name it `note`: `lf -b note '<text>'` fails with
"invalid version format". A two-step Graph of that skill
(`.lf/graphs/<name>.yaml` with `- <skill>` twice) checks the Graph path in
under a minute; `lf graph show <id>` lists each step commit as a second
row under the skill's label.

A loop Graph shows the decision prompt: `- loop:` over a skill that appends a
line to `count.txt` and a decider skill that reads it ("done at 3 lines"), run
with `lf -a claude:haiku graph <name> --batch` in a branch checkout with a bare
origin, takes about two minutes for three passes. Each decider's opening turn
in `~/.claude/projects/<cwd>/*.jsonl` carries `Decision pass N. Iterating
repeats 2 steps, back to <first step>.` and the loop criteria. Manual
`lf commit` on that branch pushes; point `origin` at a missing path to see a
push failure keep the commit.

For a stopped Graph with successful worker turns, give its decider an exact
invalid report to repeat even on correction, such as
`{"decision":"advance","summary":"passed","reason":"failed"}`. With Codex,
run `lf -b --task <id> graph <name>` from the disposable Task checkout. Three
rejected turns leave the Graph's diagnostic in `lf task status <id>`.

To check a builtin loop decider's wording (`implement` in `build`'s inner loop
and in `quick`), copy the loop into the fixture repository as
`.lf/graphs/<name>.yaml` (`- loop:` over `- cmd: sync` and `- implement`) and
run `lf -a claude:haiku graph <name> --batch '<brief>'` from a branch with a
bare origin: no GitHub, a minute or two per pass. Read each pass's decision
from the `StructuredOutput` tool call in the newest
`~/.claude/projects/-private-tmp-<repo path>/*.jsonl`; a blocked loop exits 3
with `Error: Graph is blocked: <reason>`. Plan wording steers the decider
("each pass is reviewed before the next" made it advance after pass 1), so
write the plan the way kickoff would. An oversized brief may install and start
services (Postgres under `/tmp`): stop what the run started by PID.

`build`'s outer loop (`realign` deciding) runs the same way: a fixture
`.lf/graphs/<name>.yaml` with `- loop:` over an inner `- loop:` (`cmd: sync`,
`implement`), then `concept-review` and `realign`, plus a two-pass
`scratch/plan.md` whose first pass is built, takes about ten minutes on haiku.
`realign`'s opening turn reads `Decision pass 1. Iterating repeats 4 steps,
back to sync.` Add a `.gitignore` first: step commits sweep up `__pycache__`
from the agent's test runs, so `concept-review`'s commit looks like it wrote
code. Give the review one small code mismatch, one user-docs mismatch and
one separate outcome. Pass 1 returns `iterate` and queues the code fix in
`scratch/plan.md`. Only the plan and docs change in that `realign` step. Pass 2
returns `advance` once `implement` fixes the code and `concept-review` confirms
it. For the same review, standalone `lf -b -a claude:haiku realign` queues the
code fix and reports what remains without a loop verdict. In both runs, the
separate outcome goes under `## Deferred findings`.

To read the text a skill launch composes (`extends:` chains, merged
frontmatter), put a stand-in `claude` first on `PATH` that writes its argv and
stdin to files and prints one `{"type":"result",...}` line, then run
`lf -b -a claude <skill> 'probe'` under a private `LF_HOME`. The first stdin
line is the stream-json user message holding `<lf:skill:<name>>…`; an inherited
`agent:` shows as `--model` in argv. The batch never finishes against that
stand-in, and macOS has no `timeout`: wrap it in `perl -e 'alarm 25; exec @ARGV'`.
The four launches before the prompt are `auth status --json` and `--version`.
Have the stand-in `read` one stdin line before printing its result; printing
first lets lf exit before the prompt arrives and leaves the capture empty.
In zsh, an unset list in a variable needs `env ${=U}` to split. A bare
`lf -b <skill>` composes `docs:` into `<lf:reference>` blocks but never diff
files, so `diff_files:` is visible only from a Task step.
Cycles and missing targets refuse at launch and in `lf graph show` of a Graph
that names the skill.
