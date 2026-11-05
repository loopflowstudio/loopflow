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
  it; in cmux it joins the caller's window. Otherwise, and for every repository or
  Wave open, a new container named for the Wave or repository. There is no flag to
  put a repository/Wave launch into the current session; add one if demo wants it.
- **Repository = one session.** A repository launch is one session with every
  Task tab; it no longer splits into per-Wave sessions, and includes no Wave
  conversations. A Wave launch includes its primary conversation tab.
- **Followers are Rex-only and per Machine.** The source follower ignores other
  Machines' Tasks; each Machine's launch follows its own. A Task that first starts on
  a Machine where the launch opened nothing arrives with the next open. `--arrive-task`
  forwarding was removed because it needed a remote container record.
- **Uncertain creation.** The follower marks a Task seen before creating its tab, so
  a lost reply is reported once and never retried.
- **Readiness.** Records live under a per-tab launch key and are deleted after the
  command reads them. "View again" is another fresh launch with `--session`;
  `--retry-observers` and its beside-the-pane retries were removed.
- **Template validation.** `task_layout` must hold Desktop's three Task panes (hide
  unused ones); Session/location/file capture is rejected when a launch reads it,
  not at `lf set` time.
- **cmux.** Surface commands are typed into a shell (`cd <checkout> && <argv>`),
  so they leave a shell in the checkout after the tool exits. Group and workspace
  references are parsed from CLI output; neither has native proof here because
  running cmux would act on Jack's live app.

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

- **Retained-terminal protection.** `hold.rs` read occurrence rows that no longer
  exist; nothing consumed it. Live viewers are attached Sessions the lifetime owner
  already sees. A stopped-but-retained Rex tab has no non-inventory association to
  its Task. Gap stated in docs; LOO-717 (PR #1811, still at `b81df60579`) owns the
  consumer. Options for Jack: Rex block metadata (if Rex adds it), a host query by
  argv/label at cleanup time, or accepting the gap.
- **Native app Split** in Rex/Herdr/Desktop remains owed to the demo (needs a focused
  app window; this run sat inside Jack's live Rex).
- **Bound-later conversations** report their selected directory on later attachments
  though the running provider keeps its original cwd (reversible, one line).
- **Wave memory.** `.lf/wave/product/MEMORY.md` still describes `--retry-observers`
  and registry reuse (line ~592). Memory is read-only in this step; propose replacing
  that paragraph with the fresh-launch contract at the next memory-maintaining step.
