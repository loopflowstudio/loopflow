# Assumptions — LOO-412

2026-10-07. Jack Heart authorized only the Task-on-another-machine slice and
publication for review. The earlier remote-work research and rename evidence
remain at `e50dbd749e3207599f9937ce45653f4c6f33a5cd:scratch/questions.md` and its
companion design; no later slice is selected.

- A target has the repository cloned and a compatible installed `lf`. Machine
  registration/version handling belongs to LOO-411.
- An existing Linear issue supplies its Project identity. The cold-clone test
  exposed SQLite's current-Project admission requirement. Preserve that authority:
  importing absent planning seeds only an unselected Wave with the issue's exact
  active Project, under the planning lock. An existing selection or unfinished
  rotation is never changed. No new provider Project is created.
- SSH transfers the existing planning record only into absent target planning.
  It retains its timestamp and never replaces local invalidation/removal evidence.
- An existing target checkout behind the required source commit asks for `lf sync`
  and retains all bytes. Source uncommitted/unpushed work is reported before SSH;
  this slice does not commit, push or reset it.
