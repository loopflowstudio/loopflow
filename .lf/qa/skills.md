Read when the change touches skill resolution, `extends: builtin`, `lf list skill` or the commit after a skill step.

A fixture repository with a local bare origin (`git init --bare origin.git`,
clone it, push `main`, work on a branch) and a private `LF_HOME` is enough; keep
the real `HOME` so `claude` answers on the account's login. Repository skills
from `~/.codex/skills` still show in `lf list skill` under that `HOME`.

A private home has no `default_agent`. With only `-a claude:haiku` on the run,
the step commit gets its message only because the run's `-a` reaches the child
`lf commit --step`; a home without either commits `lf <skill>: checkpoint`,
which is also the quickest way to see the fallback.

`lf -b implement` usually commits its own work, so its step commit only pushes.
To see a message written, use a throwaway repository skill that edits a file and
is told not to run git. Don't name it `note`: `lf -b note '<text>'` fails with
"invalid version format". A two-step Graph of that skill
(`.lf/graphs/<name>.yaml` with `- <skill>` twice) checks the Graph path in
under a minute; `lf process show <graph>` lists each step commit as a second
row under the skill's label.
