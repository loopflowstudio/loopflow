# v0.14.17

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.17 lets a Task finish without you walking it there. Click **Auto-Ship** or run `lf ship`, and Loopflow runs the rest of the Task's Workflow, repairs CI, merges the PR and marks the Task done. Tasks can also run on rented machines from Daytona, Modal, RunPod, AWS or Fly, including GPU and spot capacity, and a Wave can send its Tasks there by default. Session history is now read from each provider's own transcript instead of from copies Loopflow kept, so `lf history` can show and search past conversations, and `~/.lf` no longer stores a second copy of every stream. This release changes the store format: `lf < 0.14.17 can no longer open the store (raised by: scope_tasks_capacity, rental_spot, session_facts_only, ship_it)`.

## Ship a Task with one click

Landing a Task used to involve several separate mechanisms: a Workflow format kept apart from graphs, `lf ci watch` and per-Task automation holds for CI repair, and a pause that didn't match the other states. Now approving a Task carries it the rest of the way, and a single landing engine handles everything after that. (#1652)

- **`lf ship` and Auto-Ship.** The Task moves to `shipping`, and Loopflow runs every remaining route, standing in at each `interactive:` stage. Choosing any route into `end` also ships the Task. If the PR merges, the Task completes no matter who merged it. A failed ship process backs off before restarting and names the failure. When the Machine's work is next read, any ship process that has died is started again.
- **`lf ship --stop`** ends the ship process and disarms any auto-merge it armed. In Desktop, an Undo toast follows ship and stop.
- **Task state has two separate axes.** A Task is `unstarted · idle · active · moving · shipping · deferred · done · abandoned`, and a Session is `available · busy`. Pause is now Defer everywhere: `lf task defer` replaces `lf task pause`. Abandoning a Task is one write that keeps its reason.
- **Workflows are graphs.** A Workflow's definition is a graph in `.lf/graphs/`. Its `interactive:` steps are stages, and the slices between them are routes. `.lf/workflows/` and the builtin workflow files are gone. The builtin `ship` graph is now `deliver`. `feature` and `code` now end at a post-merge `verify` stage, which uses a new `verify` skill.
- **One landing engine owns CI repair.** CI repair now happens only inside a waiting landing (`lf land --wait-and-fix`, `lf ship`). `lf repo ci watch` and its launchd watcher are gone, along with the minute-by-minute Task check, per-Task CI repair holds, and `lf task automation`, `lf task automate` and `lf task reconcile`.
- **Desktop.** Auto-Ship sits on the Task heading, and Defer and Abandon appear on hover. A Workflow bar in the main pane shows the stages and the routes you can start, with a live `At <stage> · yours` line. Opening a Task no longer shifts the sidebar.

## Rent a machine for a Task, and give it back when the Task is done

The goal is for infrastructure Tasks to run on rented CPUs that close once the Task is done. During the cycle, Daytona and Modal sandboxes first replaced v0.14.16's RunPod pods. RunPod then came back with a new provider boundary, alongside AWS and Fly, plus GPU and spot options. The final set is five providers behind one embedded bridge. (#1704, #1707, #1709)

```bash
doppler run -- lf ondemand add code --provider daytona --vcpus 4 --memory-gb 8 --max 2 --max-usd-per-hour 0.40
lf machine use code infrastructure     # the Wave's new Tasks rent from code; its Session stays here
doppler run -- lf task run LOO-xxx     # rents code-1, hands the Task off, runs there
lf ondemand list
```

- **Providers:** `--provider daytona | modal | runpod | aws | fly`. Daytona and Modal boot an Ubuntu 24.04 image that already has gh, Node, Claude Code, Rust and uv. Modal is reached over a TCP tunnel to key-only sshd, and Daytona through its SSH gateway. RunPod, AWS and Fly boot stock Ubuntu 24.04 and install the same tools before `lf` is admitted, so their SSH wait is 20 minutes instead of 5. `lf` itself is installed over SSH after boot: either the published release or `LF_RENT_LINUX_BIN`. (#1704, #1707)
- **The deadline holds even if your laptop is asleep.** Daytona (`ttl_minutes`) and Modal (`timeout`) enforce it themselves. On RunPod, AWS and Fly, a guardian starts on the machine before any setup runs. Fly destroys the Machine when the guardian exits. A RunPod guardian deletes its own pod, retrying until the delete succeeds, and shortens the deadline if the price RunPod actually charges uses up the budget sooner. An AWS guardian powers the instance off, which terminates it. A guardian can only shorten the deadline, and machines found already ended are still deleted. (#1707)
- **GPUs and spot capacity.** `lf ondemand add --gpu` rents one GPU on RunPod (A40, A6000, L4, RTX4090, L40S, A100, H100), AWS (L4, A10G) or Modal (T4, L4, A10G, L40S, A100, H100). `--vcpus` and `--memory-gb` are now minimums, and the cheapest matching shape sets the actual size and price. `--spot` (AWS, RunPod) admits, caps and sets the deadline at the on-demand price, and warns that the provider may end the machine early and that unpushed work is lost. (#1707)
- **Prices.** Published per-second rates set the price. When the provider reports its actual charge (RunPod's `costPerHr`), that charge replaces the published rate once the rental is confirmed. AWS output names both regions, for example `aws (eu-west-1, priced as us-east-1)`. An on-demand whose shape has no price is refused when added, and `lf ondemand list` shows it as unpriced instead of failing. (#1707)
- **A Wave can rent by default.** `lf machine use <on-demand or pool> [wave]` (or `--repo`) sets where that scope's Tasks run. Whichever is nearer wins: a capacity default or a Tasks Machine. `lf task run` then places an unowned Task exactly as `--rent` or `--pool` would. `--session` is refused, and capacity defaults are not copied to your other Machines, because pools and on-demands are local to each Machine. (#1704)
- **Give-back waits 15 minutes** after a rental's last Task is completed or abandoned, so a follow-up run can reuse it. A sandbox that never becomes usable is deleted immediately. (#1704)
- **Removed:** `lf ondemand availability`, and the RunPod CPU catalog, stock checks and image digest pinning from v0.14.16. (#1704)

## Read and search past conversations

Before this release, Loopflow couldn't show what was said in a past Session, and when Claude removed an idle transcript, the conversation disappeared without notice. `lf history` now reads conversations directly from the provider's transcript. It also reports what each provider still keeps, so you can raise `cleanupPeriodDays` before something you need is gone. Loopflow stores no conversation text. (#1700)

```
lf history retention
lf history show <SESSION> --transcript
lf history search "cleanupPeriodDays" --since 30d
```

- **`lf history show SESSION`** now ends with the conversation: for each provider conversation the Session used, it lists your requests, launched skills, agent messages, one line per tool call and the reason for any failure. `--transcript` prints only the conversation, and `--json` returns it as structured output. If a transcript is gone, the output lists the recorded turns (time, status, final answer) and says why.
- **`lf history retention`** counts readable and facts-only Sessions by provider, account and age, lists the next removals, and shows each provider's retention policy. A new **Retention** card on the Desktop Telemetry dashboard reads its `--json` output (the `RetentionSummary` wire type).
- **`lf history search TEXT`** finds text in recent conversations. It accepts `--since` (default `7d`), `--wave`, `--project`, `--task`, `--limit` and `--json`. It scans transcript files on every call and reports `scanned_bytes`, so wide windows over a lot of Claude history may be slow.
- Transcripts are read from Claude, Codex and Pi transcript files, and from `opencode session export`. Codex, OpenCode and Pi are reported as having no provider expiry, per their documentation; this has not been checked against live data.

## The store keeps facts, not copies

v0.14.16 shrank the store but kept each run's full provider stream in `events.jsonl` and in SQLite. This release removes both copies. Every provider now has one decoder, and a single writer records what Loopflow reads back. Usage totals come from a single source, and neither the store nor `~/.lf/runs` grows with transcript text anymore. (#1698, #1702, #1706)

- **One decoder per provider.** Claude, Codex, OpenCode and Pi each have one decoder that feeds the live view, Session receipts and availability. Previously, three separate parsers each read the stream and could drift apart. A failed receipt write now returns to the driver instead of being logged and ignored. Receipt payloads and the conversation stream are unchanged. (#1702)
- **One usage row per provider turn.** Claude's row matches what the live view shows, including cost, model, peak input and context window. Readers sum only these native rows, so stream usage copies that earlier versions stored no longer count twice. (#1698)
- **Headless steps stop writing `events.jsonl`.** Prompt context (first and peak request tokens, tool-result count and size, compactions) is attached to each turn's receipt. `lf history usage --context` and `lf history show <input> --context` read it from the store. Running/Stalled for a Task step now comes from CPU samples stored in SQLite, not from the file's last line. (#1706)
- **`lf history show --events` is removed.** To debug a stream, read the provider's own transcript, now available through `lf history show --transcript`. (#1706)
- Retention is simpler. The per-capture prune pass, its 24-hour settle window and its cursor file are gone. `lf disk clean` still merges per-receipt native usage into one row per turn, and its summary no longer reports stream rows or captures. (#1698)

## Operational notes

- **Store format.** `lf < 0.14.17 can no longer open the store (raised by: scope_tasks_capacity, rental_spot, session_facts_only, ship_it)`.
- **`session_facts_only` rewrites every store in one pass.** It deletes stream rows from SQLite after first carrying their usage into native rows. Claude turns that had no stream copy keep only their token counts, so their cost and model show as unknown. Stream usage from an attempt with no observed provider session is dropped, so that Session may show lower or unknown usage. Existing `events.jsonl` files stay on disk, but nothing reads them anymore.
- **v0.14.16 RunPod rentals.** `scope_tasks_capacity` deletes RunPod on-demands and rentals in the v0.14.16 shape (templates that name an `image`, or hardware tagged with `kind`), which no current reader can parse. Current-shape RunPod rows are kept. A v0.14.16 pod that is still running is ended at its deadline by its own guardian. Recreate the on-demand with `lf ondemand add --provider runpod`. (#1709)
- **Custom Workflows need rewriting.** Graphs with `human: true` steps, or anything under `.lf/workflows/`, must be rewritten as `interactive:` stages in `.lf/graphs/`. Scripts that call `lf task pause`, `lf task automation`, `lf task automate`, `lf task reconcile` or `lf repo ci watch` must be updated. (#1652)
- **Uninstall the launchd CI watcher before upgrading** on any Machine that used it. Only a running landing repairs CI now. (#1652)
- **Live verification of rentals.** Daytona and Modal were rented, reached and ended by hand through this code. RunPod, AWS and Fly have been tested only against mocked HTTP and SDKs. The automatic Wave-default path, and give-back without `--force`, have not yet been exercised end to end. Give-back needs `lf history export` on the rented machine, which this release provides. (#1704, #1707)

## Small changes

- **Desktop: a Wave's sidebar lists only its started Tasks**, plus its subwaves, in plan order. ⌘1–9, ⌃⇥ and sidebar search now skip unstarted work. The Wave board still shows unstarted Tasks with **Start**, and an empty Wave reads "No working Tasks in this Wave". (#1703)
- **Usage updates no longer scan `session_events` while holding the store's write lock.** A new index lets each usage update find its row directly, so other writers no longer wait while that scan runs. (#1713)
- `scripts/check_cost.py` and `scripts/benchmarks/storage/replay_history.py` are deleted. `scripts/context_ablation.py` reads `lf history show --transcript --json`. (#1706)