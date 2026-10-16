# Interrupted Codex reconnect

```sh
uv run python scripts/benchmarks/agent-startup/fixture.py \
  --source ~/.lf/loopflow.db --home /private/benchmark/startup-home \
  --receipts 234483
uv run --with pyte python scripts/benchmarks/agent-startup/reconnect.py \
  --baseline /private/benchmark/lf-baseline \
  --candidate /private/benchmark/lf-candidate \
  --home /private/benchmark/startup-home --repo /private/benchmark/repo \
  --output /private/benchmark/reconnect --samples 3
```

Prepare the disposable repository and immutable release binaries as described in
[the benchmark guide](../README.md). This driver creates its own native Codex
Sessions. It interrupts only a benchmark client's new process group after proving
its editor accepts text, lets the existing provider watchdog exit, then measures
ordinary `lf session connect`. It never connects a copied Session or removes
history. `--prepared` can reuse the driver's private Session map after interrupted
preparation; every Session must belong to the disposable benchmark checkout.

Jack Heart reported 38 seconds at “Preparing codex” on October 16, 2026 with
lf 0.14.2. The retained observation places the delay after a 7,008-byte resume
instruction file was written and before the app-server was launched. The reported
6,060 assembled tokens exclude carried native history. No stack was captured
at that original incident, so its exact cause cannot be established retrospectively.

## Measured cause

The controlled reproduction uses a private read-only backup of the installed
store: 3,790 Sessions, 788,632 Processes and 2,033,513 Session events. Copied managed
account routes are removed before launch. Native authentication is unchanged.
The fixture adds 234,483 synthetic receipt files, matching the installed directory's
observed file count, without copying live process identities.

An initial cleanly detached reconnect reached an editable prompt in 771 ms without
those receipts and 1,472 ms with them. After abruptly ending only that benchmark
client, the next actual reconnect took **56,315 ms**. A one-second macOS sample
placed all **789 main-thread samples** in:

```
session connect → resume_session → session_command_status_with_env
  → resume_session_agent_process → resume_session_attachment
    → process_evidence → read_process_receipt_files_at
```

The scan was reading and parsing every receipt to find one previous attachment's
Process ID. This happened at “Preparing codex,” before provider startup. The
repair reads the existing `<process-id>.json` directly. The shared identity rule
still checks receipt schema, Process and trace IDs, PID and OS birth; missing
receipts retain the existing completion/boot fallback, and unreadable or malformed
evidence remains unknown. Atomic receipt publication and attachment fences stay
intact. No cleanup, cache, context reduction or native-history change is involved.

The separate slow Wave status `realpath` observation remains a separate issue.
Neither it nor temporary-directory allocation was the sampled cause here.

## Matched release builds

Three alternating samples per binary and context, seconds; brackets give the full
range. All 12 launches reached a usable prompt and preserved every pre-existing
Session event plus native thread and input identity. No timed samples were dropped.

| Context / interval | Baseline median [range] | Candidate median [range] |
|---|---:|---:|
| Short: Preparing → Starting Codex | 52.98 [49.29–54.19] | 1.02 [1.00–1.17] |
| Short: process start → usable prompt | 53.72 [50.02–55.57] | 1.85 [1.74–1.92] |
| Long: Preparing → Starting Codex | 53.55 [49.68–55.22] | 1.04 [1.00–1.12] |
| Long: process start → usable prompt | 54.35 [50.42–56.02] | 1.79 [1.78–2.00] |

[Raw numeric rows](numbers.jsonl), [summary](summary.json),
[build/fixture metadata](metadata.json), and [sanitized diagnostic sample](profile.json).
Stages are observed terminal output, not precise internal function timers.
“Starting Codex” precedes account/provider startup; “Opening codex terminal” follows
bootstrap/history recovery. The whole-process readiness interval includes both.
The approximately one-second remaining preparation and provider/terminal startup
are not attributed further by this experiment. Three pairs demonstrate this large
reproduced effect; they do not characterize population tail latency.

## Acceptance and limits

The matched driver alternates binary order on the same fixture, checkout and
native Sessions. The 234,483 synthetic receipts stay fixed; each deliberate
crash also retains its own receipt, fewer than 0.01% additional files. Both binaries use `cargo build --release -p loopflow --bin lf`
from the same source base with only the receipt lookup changed. Setup and seed
turns are outside timing. Each measured reconnect types and erases an unsubmitted
marker; footer rendering alone is insufficient. Exit 130 is the normal interrupted
terminal path from the driver's Ctrl-C; the setup exit is deliberately SIGKILL.

The short history has one seed turn. The long context has one native turn carrying
800 synthetic inventory lines (56,766 bytes), retained in full. Provider seed
receipts reported 26,699 input tokens for the short case and 39,525 for the long
case; these are observed provider totals, not just the supplied message size. An attempt to add
headless turns to a completed seed Session was refused before launch; that failed
preparation produced no timed samples and was not presented as a three-turn case.
The driver reuses those known seed Sessions, checks native thread/capture identity,
and hashes every pre-existing Session event before/after reconnect.

This is a warm shared macOS host, not an exclusive machine or cold-cache result.
Native authentication uses the ordinary provider home; copied absolute account
routes are absent. Managed-account switching and the older LOO-461 cold-cache
acceptance remain separate. Private databases, transcripts, full profiles and
Session identities stay outside the repository. Numeric results and sanitized
profile symbols are retained here.

## Regression and review

Twenty journal tests pass, including exact identity lookup without permission to
list the directory, malformed/unreadable/missing receipts, trace/ID mismatch and
PID reuse. Thirteen affected attachment, process-status and Task-execution tests
pass under external-network isolation, including concurrent takeover fencing and
unknown provider identity. All-target Clippy, Rust formatting, benchmark Ruff and
the fixture isolation test pass. Hosted CI and release acceptance are separate.

Review kept one process-identity rule and the existing receipt filename. No new
cache, migration, configuration knob, process control or retention policy was
introduced. Inventory commands retain their all-receipt reader. Invalid exact
receipt evidence remains unknown rather than granting attachment replacement.

The supported CLI-only installer leaves Desktop and its bundled clients running.
Desktop explicitly selects its bundled helper through `LF_BIN`; CLI-only promotion
therefore cannot demonstrate adoption by an already-running older Desktop. A full
installation stops those clients. Release/installation results must retain that
boundary rather than treating CLI success as Desktop acceptance.
