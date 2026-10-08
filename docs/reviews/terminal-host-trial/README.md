# Repeat the isolated terminal trial

```sh
uv run python scripts/terminal_host_trial.py --host pty --self-test
uv run python scripts/terminal_host_trial.py --host pty --host herdr \
  --herdr /path/to/pinned/herdr --output /tmp/new-terminal-trial
```

Run on macOS with `sandbox-exec`, standalone Homebrew Git, Python through `uv`,
and an existing lf binary (`--lf /path/to/lf` selects it explicitly). The runner
copies binaries into a disposable environment; it does not install or update lf
or any host. A new output directory is required. Herdr is prepared separately;
missing capability returns failure with a receipt, never a passed host column.
No cmux automation is implemented.

The [findings page](../terminal-host-trial.md) distinguishes the real herdr pane
and CLI from the synthetic provider. The output's overall success means the
smoke check matched its expected results, including named blocked boundaries.
It does **not** mean Task execution or publication succeeded.

## Isolation and scope

Every runtime child uses a private HOME/LF_HOME, explicit environment, fixture
PATH and macOS sandbox. File data reads outside the fixture and system/runtime
paths, unlisted executables, network connections and security-daemon lookups are
denied. Only Unix sockets inside the private root are allowed. Before launching
lf or herdr, harmless sentinels test outside read/write, loopback, unlisted exec,
and inside/outside Unix socket behavior. No account material is used by the probes.
Build/download networking is outside that runtime phase.

The Claude-named executable is an original, visibly synthetic protocol fixture;
it never contacts a provider. Other providers, credential helpers and publishers
are blocked substitutes. `gh CLI not found` therefore establishes the publication
boundary reached, not the actual signed-out GitHub experience. Seeded private
planning rows establish registered placement only. A fresh Task separately fails
at missing repository `pm.linear_team`. The seeded Task Flow fails before provider
execution because it requires a managed account, after three attempts.

macOS refuses native setuid `ps` in this sandbox. A fixture reads real process age
with libproc for lf's narrow age query; the full `lf ps` inventory is explicitly
unavailable. Session and Flow reads, including step Process IDs, are retained.
The controller tracks only owned child PID/birth pairs and process groups for
cleanup; its process-table read contains no command arguments or environment.

Herdr runs a unique server and real TUI client with private state/socket and no
integrations. Public socket operations create/focus/read/close owned panes.
They never create worktrees, inject status or touch installed host sessions.
Raw TUI output is captured; pane text/snapshots supply headless observations.
Resize, exact input/return, ordered two-step output, bounded capture and owned
process cleanup are checked. Notification delivery, visual colors and real-agent
state inference require separate signed-in/rendered checks.

## Receipts

Each JSONL line is `{file, receipt}` from a named output file. Paths and identities
are from disposable roots that were cleaned up. JSON escapes terminal controls;
base64 preserves captured PTY/TUI bytes. Only the overflow probe's repetitive
synthetic payload is omitted, with its byte count and hash retained.

- [final-02 PTY](final-02-pty.jsonl) and [final-02 herdr](final-02-herdr.jsonl):
  final scenarios, confinement, prepared executable identities, persisted Session/
  Flow/Process reads, host snapshots, pane text and cleanup. Each scenario has
  evidence and outcome fields; unmeasured observations are null.
- [final-01 PTY](final-01-pty.jsonl) and [final-01 herdr](final-01-herdr.jsonl):
  earlier shared pass, including forced timeout/overflow cleanup probes. Later
  review corrected checkout-publication attribution and cleanup error metadata;
  final-02 repeated the affected host paths. The probes themselves did not change.
- [development attempts](development-attempts.json): prior failed and successful
  harness attempts are retained as summaries, not quietly retried out of history.
  Full development captures remain temporary; final behavior is evidenced above.
- [build receipts](build.json): pinned revision, compiler/archive and binary hashes,
  three attempts, elapsed build time and diagnosis. Host source/build diagnostics
  are intentionally excluded from the repository.

The source pin was `herdrdev/herdr@4dc23bb15d4a2fd2c093abfb509f903c3015bf56`
(version 0.9.3). The ordinary build command was
`cargo build --locked --release --bin herdr -j 4`, using a private HOME/CARGO_HOME
and Zig 0.16.0. The installed 0.15.2 compiler failed. After a 600-second timeout,
Jack Heart explicitly requested continued work; the same source/cache completed
in 148.504 seconds with a longer limit. Download time is not included in those
build durations. The earlier short archive transfer was resumed and hash-checked.
No upstream provider tests or integration installers ran.

The copied lf reported 0.13.9, SHA256
`4cf8c9e9a39a916dbe57be9abc3e6eb688ed3ad776ff9243b68537d69592103a`.
Its source revision is unknown. This is installed-binary evidence, not validation
of the older checkout's Rust implementation. `lf --version` also emitted an
unavailable Exec-ledger warning; the exact output is retained.

Final-02 first fixture output: 0.900 s PTY / 1.133 s herdr after command launch.
The fixture deliberately waits 350 ms, plus 250 ms after interactive input return.
Herdr startup, preparation and command timestamps are separate summary fields.
All timings are single local samples with capture overhead; no p95, cold install,
provider startup or real-use acceptance claim follows.

## Review and remaining evidence

The implementation review caught a missing Task binding on checkout publication,
a cleanup field that ignored wait errors, and a missing-executable exception that
could escape receipt capture; all were corrected. Negative verdict checks reject
reordered/missing Flow steps, absent input return and unexpected CLI failures.
Final-02 passes both hosts; final-01 additionally passes forced timeout/overflow
and separate-child cleanup. Formatting and static checks pass.

Fixture process detection changed between samples: one early run had no detected
agent; later snapshots recognized the Claude-named fixture while its state stayed
unknown during input/return. The receipts retain both. This is sampling evidence,
not a real provider status result. Agent labels and Loopflow Waiting are separate.
The resize appears asynchronously in later snapshots (27 to 35 viewport rows);
it is not compositor proof. The later title comparison below isolates the terminal-title input; visual precedence
and real-provider naming remain unproved.

Reference entry points consulted on 2026-10-07:
[herdr install](https://herdr.dev/docs/install/),
[socket API](https://herdr.dev/docs/socket-api/),
[agent documentation](https://herdr.dev/docs/agents/),
[cmux API](https://cmux.com/docs/api),
[cmux notifications](https://cmux.com/docs/notifications).
They guided protocol access; the observations above come from the trial or Jack's
attributed reports. No host source, tests, skills, help text or config was imported.


## Title and status derivation

```sh
uv run python scripts/terminal_host_trial.py --host herdr --observe-titles \
  --herdr /path/to/pinned/herdr --output /tmp/new-terminal-title-trial
```

The opt-in comparison runs after the original smoke scenarios. It replaces the
synthetic provider with one original script under two executable names, then runs
that script as `trial-agent`, as `claude`, and through `lf -m claude trial-one`.
First content, title alpha, and title beta are different strings. The script waits
two seconds before requesting input and two after returning it; these delays allow
host sampling, not performance measurement. It emits only OSC 2 title changes,
using the [published control sequence](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Operating-System-Commands);
no host status or provider screen pattern is injected. Overall success still means
fixture transport and cleanup succeeded, not that the host inferred provider state.

[title-01-herdr.jsonl](title-01-herdr.jsonl) retains 18 raw receipts: confinement,
versions, executable hashes in the summary, initial snapshot, all three commands,
pane reads and five-stage snapshots, and owned-process cleanup. The title fixture
is `TITLE_PROBE` in the runner; the summary's original provider hash describes the
preceding baseline fixture, before replacement. The same pinned herdr and lf
hashes as final-02 were used. No survivors or cleanup errors remained.

| Controlled input | Direct `trial-agent` | Direct `claude` | `lf` → `claude` |
|---|---|---|---|
| Identical first printed content | No agent | Claude recognised | Claude recognised |
| OSC 2 alpha, awaiting input | `terminal_title=trial-title-alpha` | Same | Same |
| OSC 2 beta, after input return | `terminal_title=trial-title-beta` | Same | Same |
| Workspace / tab labels throughout | `repo` / `1` | `repo` / `1` | `repo` / `1` |
| Input and return state | unknown | unknown | unknown |
| Exit snapshot state | unknown | idle | idle |

This is causal evidence for executable-name-sensitive recognition, and for terminal
output supplying the pane title without changing the workspace/tab label in these
samples. The same original title bytes pass through lf. It does not identify every
recognition source, prove a screen-reading algorithm, establish name priority after
manual rename, or reproduce a real agent's Working/Waiting display. Missing
`terminal_title` in earlier snapshots meant no observed title, not an API that
cannot expose one. Earlier brief samples without recognition remain valid.

For cmux, public first-hand behavioral reports provide bounded evidence without
using host implementation, help, skills or configuration:

- The [May 8 report #3749](https://github.com/manaflow-ai/cmux/issues/3749)
  observed eight live Codex panes with empty status, then a Running entry after a
  manual hook invocation. That demonstrates a hook-to-status path and shows that
  a visible agent title need not supply status. It is historical, not a guarantee
  that October's cmux has no other source or that lf caused every empty state.
- [August 15 report #10217](https://github.com/manaflow-ai/cmux/issues/10217)
  records cmux 0.64.22 showing a Codex name after provider `/rename` and sidebar
  Running/Idle independently of an absent title spinner. Provider names can reach
  tab labels; title decoration and sidebar state are separate observations.
- [July 20 report #8514](https://github.com/manaflow-ai/cmux/issues/8514) records a
  topic-bearing auto-name disappearing after compaction in cmux 0.64.17. This is
  another observed naming path, not proof of how Jack's title was generated.

Only the reports' observed behavior is used here; their source-derived explanations,
configuration excerpts and proposed implementations are not adopted. Jack's report
and LOO-428 independently establish his wrapper interaction and loss of recognition
when bypassing it. Together these justify retaining host wrappers and testing
hooks, names and status separately. They do not prove that the first injected
message generated “Loopflow operating guide.” Exact title-source and precedence
attribution on Jack's installation is blocked by the no-live-cmux/no-provider-history
boundary; the findings page gives the permitted next keyboard checks. No host code
or configuration was imported, and no integration was installed.
