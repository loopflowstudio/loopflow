# LOO-498 — open judgments and reversible choices

Current design: the opening section of [rex-workspace.md](rex-workspace.md). Jack
Heart's November 5 concept review selected a fresh launch on every `lf open`, config
templates, DB-held sticky Desktop layouts and no LF persistence of hosts,
workspaces, occurrences or copies anywhere. He then said "ok, pursue": implement
through review publication, without Auto-Ship, merge or shared-store migration.
Older judgments are in `9187804aee:scratch/questions.md`.

## Reversible implementation choices (November 5 fresh-launch pass)

- **Arrivals use full configured layouts.** A newly started Task arrives as a normal
  Task tab with its splits, no focus, no new provider. Stated implementation
  choice of Jack's hierarchy, not a verbatim selection; keep it visible in demo.
- **Container defaults.** A Task opened inside a Rex session or Herdr workspace joins
  it. In cmux it joins the group its caller's workspace is in now, else the window.
  Otherwise, and for every repository or Wave open, it gets a new container named
  for the Wave or repository. No flag puts a repository/Wave launch into the
  current session; add one if the demo wants it.
- **One follower per broad launch.** The source follower adds every Machine's
  arrivals. A peer's container comes from its receipt, or from that Machine's
  first arrival. A remote first arrival creates a container named for the Task's
  Wave or repository, chosen by the peer's own scope resolution.
- **Uncertain creation.** The follower marks a Task seen before creating its tab, so
  a lost reply is reported once and never retried.
- **Readiness.** Records live under a per-tab launch key and are deleted after the
  command reads them. "View again" is another fresh launch with `--session`;
  `--retry-observers` and its beside-the-pane retries were removed.
- **Template validation.** `task_layout` must hold Desktop's three Task panes (hide
  unused ones); `lf set` and config reads reject Session/location/file capture.
- **cmux.** Surface commands are typed into a shell (`cd <checkout> && <argv>`),
  so they leave a shell in the checkout after the tool exits. The caller's group
  comes from `workspace-group list --json`; neither it nor workspace creation has
  native proof here because running cmux would act on Jack's live app.

## Explicit Rex regression acceptance

During pursuit Jack Heart shared another Session's report that installed
`lf open --host rex` returns “Rex opening is not supported yet”, and asked whether
this PR fixes it. Confirmed: origin/main has that exact stub in `open.rs`; this
branch routes Rex through the native workspace renderer. The isolated native test
`rex_native_fresh_launches_keep_earlier_arrangements_and_task_directories`
explicitly invokes `open --task INF-123 --host rex --server …`. Preserve this
coverage and state that installed behavior changes only after landing/install.
No separate Task was requested or created.

## Still open

Jack Heart's prohibition on LF host, workspace and occurrence persistence covers
lease tables too. It is settled and not a question.

- **Retained-terminal protection** (requirement, not waived). It is an external
  integration dependency on LOO-717, whose API must take a shape compatible with
  that prohibition. Facts and its current incompatibility are in rex-workspace.md
  "Remaining". This Task neither changes nor launches LOO-717.
- **cmux group JSON** comes from cmux source, not a live app. Running cmux here
  would act on Jack's live app.
- **Native app Split** in Rex/Herdr/Desktop is still owed to the demo. It needs a
  focused app window, and this run sat inside Jack's live Rex.
- **Bound-later conversations** report their selected directory on later
  attachments, though the running provider keeps its original cwd (reversible,
  one line).
