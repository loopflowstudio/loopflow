# Agent startup

```sh
mkdir -p /private/benchmark
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release -p loopflow --bin lf
cp target/release/lf /private/benchmark/lf-baseline
# Build the candidate with the same options and retain a separate immutable copy.
uv run python scripts/benchmarks/agent-startup/fixture.py \
  --source ~/.lf/loopflow.db --home /private/benchmark/dense --apply-drafts
uv run python scripts/benchmarks/agent-startup/native.py \
  --baseline /private/benchmark/lf-baseline --candidate /private/benchmark/lf-candidate \
  --home /private/benchmark/dense --repo /private/benchmark/repo \
  --provider claude --samples 16 --output /private/benchmark/claude
```

Use a disposable checkout with the same files for every variant. Run Codex with
`--provider codex`. Run one measurement driver per Home: each selects its provider
in that Home's configuration. Keep builds and unrelated benchmarks out of the
measurement window. Copy the binary before another build can overwrite it.

`fixture.py` opens the source database read-only and uses SQLite backup. It creates
a new private directory, copies no credential files, and optionally applies the
checkout's ordered drafts **only to that copy**. Before any launch it removes
copied managed-account routes and their absolute provider-home references; native
providers use their ordinary authentication. The CLI subsequently validates
its exact schema. Use `--apply-drafts` only for a released source snapshot that
does not already contain them; compatible experimental snapshots need no flag.
Supply the disposable checkout at `/private/benchmark/repo` before measuring.
These scripts do not create a Git worktree or alter the installed Machine.

`first_use.py` takes the same arguments and measures five fresh dense Homes per
variant. On macOS it clones a consistent private seed with APFS copy-on-write.
This means first use of the fixture, not eviction of OS or native-provider caches.
Connect is its first reconnect after preparation, not a first-ever login.

`handoff.py` replaces providers with shell stand-ins and measures only to their
readiness marker. It accepts the same baseline/candidate/Home/repo/output options
and `--samples 20`; it uses Claude. `--profile` captures macOS `sample` stacks.
Install `inferno` and `rustfilt` into a private tools directory, then render:

```sh
uv run python scripts/benchmarks/agent-startup/flames.py \
  --tools /private/benchmark/tools/bin --input /private/benchmark/profile \
  --output /private/benchmark/figures --label comparison
uv run python scripts/benchmarks/agent-startup/summarize.py \
  /private/benchmark/claude/numbers.jsonl --output /private/benchmark/summary.json
```

Only publish numeric JSON, metadata, sanitized folded symbols, and SVGs.
`*.private`, SQLite files, captures, and full `sample.txt` files stay private.

## October 8, 2026: measured improvement is small

Jack Heart requested an autonomous profiling pass through publication for review,
without landing or Task completion. Two small deletions survive this pass:

- Interactive startup uses the actual provider spawn result instead of launching
  the selected provider once with `--version` and again to open it. Automatic
  provider discovery still probes when no agent is configured. Missing-executable
  failures retain the installation hint and positive non-start receipt.
- An absent ambient Wave returns before Git discovery, runtime/thread creation,
  and registry reads. Explicit and inherited Wave IDs retain their existing
  resolver, including stale-identity errors. Nothing is cached.

On an M4 Max, release builds with debuginfo, native Claude 2.1.294 and Codex
0.161.0, the complete change improves warm bare launch by **41 ms paired median
for Claude and 24 ms for Codex**. Codex reconnect improves by **16 ms**.
Claude reconnect has **no demonstrated final improvement**: its median was
868 → 899 ms in the final run, with a paired difference interval spanning zero.
The smaller initial probe-only run was more favorable. This is a modest reduction,
not parity with direct provider startup.

### Warm readiness, 16 alternating samples each

Times are milliseconds, measured from process creation to a marker appearing in
the provider's editable prompt. Each row gives median, interquartile range, and
full range. Baseline/candidate order and route order alternate. Direct providers
run once per round on the same host, checkout, and native account environment.

| Provider / route | Baseline median [Q1–Q3]; range | Candidate median [Q1–Q3]; range | Candidate minus direct median |
|---|---:|---:|---:|
| Claude direct | 707 [680–741]; 649–898 | — | — |
| Claude bare lf | 1055 [1016–1074]; 994–1686 | 1002 [967–1060]; 915–1343 | +296 |
| Claude connect | 868 [846–893]; 808–984 | 899 [859–935]; 792–1129 | +193 |
| Codex direct | 290 [289–292]; 288–302 | — | — |
| Codex bare lf | 616 [609–622]; 582–650 | 590 [588–598]; 571–632 | +300 |
| Codex connect | 460 [452–463]; 437–657 | 444 [441–450]; 427–556 | +154 |

The overhead column subtracts medians from the same run; it is not an isolated
measurement of Loopflow CPU. Native launch/resume are different provider paths.
No samples or outliers were dropped from these datasets.

### First use of fresh fixtures, five samples each

OS pages, authentication, provider installation and workspace trust remain warm.
These samples distinguish fresh Loopflow storage/capture state from the repeated
fixture above. Five samples describe spread; they do not establish tail latency.

| Provider / route | Baseline median [Q1–Q3]; range | Candidate median [Q1–Q3]; range | Candidate minus direct median |
|---|---:|---:|---:|
| Claude direct | 667 [643–681]; 638–710 | — | — |
| Claude bare lf | 1091 [1045–1132]; 1013–1460 | 993 [989–997]; 945–1009 | +326 |
| Claude connect | 821 [791–831]; 790–879 | 818 [816–828]; 770–849 | +151 |
| Codex direct | 292 [291–293]; 290–311 | — | — |
| Codex bare lf | 692 [679–715]; 667–820 | 638 [637–711]; 632–817 | +347 |
| Codex connect | 479 [478–495]; 465–501 | 466 [461–478]; 459–486 | +174 |

### Where the time went

The [before/after bare-launch graphs](20261008/provider-probe-bare-baseline.svg)
and [candidate](20261008/provider-probe-bare-candidate.svg) show the version-probe
wait disappearing. The [reconnect baseline](20261008/provider-probe-connect-baseline.svg)
and [candidate](20261008/provider-probe-connect-candidate.svg) cover the other
entry point. The second deletion has separate
[bare baseline](20261008/no-ambient-wave-bare-baseline.svg),
[bare candidate](20261008/no-ambient-wave-bare-candidate.svg),
[connect baseline](20261008/no-ambient-wave-connect-baseline.svg), and
[connect candidate](20261008/no-ambient-wave-connect-candidate.svg) graphs.
Each SVG is standalone, titled, demangled, searchable and zoomable.
XML and artifact sanitization checks passed; rendered visual review remains for
review because this run has no rendering environment.

These are main-thread **wall-stack samples**, including blocked waits and
shutdown, not CPU flamegraphs or exact elapsed-time accounting. Attach misses
some initial startup; the very short first candidate capture missed most of it.
Each graph aggregates three separate profiles. Child Git/provider CPU and worker
threads are excluded; a parent waiting on a child is visible. Timing comparisons
use separate unprofiled runs.

Remaining visible work includes Git repository/branch discovery, context assembly
and tokenizer initialization, SQLite schema/ledger validation, account resolution,
and fsync-backed capture/client publication. Bare launch assembles context and
creates a Session; reconnect reads its retained context and native identity.
The stand-in run counted 17 → 16 Git children, 22 → 19 SQLite connections and
1260 → 1089 statements for bare launch; reconnect stayed at eight Git children
and fell from 15 → 13 connections and 790 → 676 statements. Counts include
shutdown. Returned rows are not scanned rows or SQLite VM steps.

Native direct startup itself is roughly 0.7 s for Claude and 0.29 s for Codex
here. Provider rendering, native configuration/authentication, hooks and possible
network waits belong inside that native interval; this pass does not attribute
them individually. The stand-in measurements require no provider network.

### Decisions, contrary results, and stopping point

The probe-only Claude comparison is noisy: bare 1019 → 1022 ms; connect
871 → 856 ms. Its paired reconnect improvement is 28 ms, but the fixed-seed
bootstrap interval is −1 to 47 ms. It does not justify a broad Claude speed claim.
The separate 20-pair Codex probe-only comparison isolates the deletion from the
Wave change: bare 640 → 628 ms (paired 11 ms, interval 2–24 ms); connect
470 → 464 ms (paired 8 ms, interval 4–17 ms). The complete Codex comparison is
616 → 590 ms bare and 460 → 444 ms
connect, with positive paired intervals of 16–27 and 8–22 ms respectively.

The Wave-only stand-in comparison has 20 alternating pairs: bare 445 → 432 ms,
paired improvement 14 ms (95% bootstrap interval 1–27 ms); connect 272 → 272 ms,
interval −11 to 8 ms. Keep it for its measured bare-launch reduction, not a
reconnect timing claim. The final Claude reconnect result is retained as contrary
evidence, not replaced by the more favorable earlier run.

The last attempted optimization retained the SQLite handle between capture
reservation and publication, removing an immediate close/reopen. Thirty matched
pairs gave bare 431 → 428 ms, paired improvement 3 ms (interval −4 to 9 ms).
Connect was an unchanged control: 283 → 277 ms, interval −5 to 11 ms. The signal
did not separate from noise, so **that code was removed**. Its raw numbers remain
in `rejected-retain-store.jsonl`.

The pass stopped after this attempt stopped producing a measurable gain, rather
than retaining speculative code. Repeated Git discovery remains a possible
future target; reusing its facts across owners needs a deliberate API change,
not a global cache. This pass does not claim every possible optimization was
exhausted or that the remaining 0.3 s is an irreducible limit.

Several measurement approaches were rejected before comparison:

- ANSI stripping reported a missing `r` in an input marker although Claude had
  retained that character from the previous screen. The PTY reader now uses a
  terminal emulator and requires the whole marker while terminal echo is off.
- Trust-dialog timeouts and forced PTY exits were not readiness. Clean provider
  exit is required for reconnect; forced closure left an active-client error.
- An initial comparison used a candidate overwritten by a concurrent test build
  without symbols. Those numbers are excluded. Retained immutable binaries and
  hashes prevent that ambiguity in the accepted comparisons.
- A stand-in's instantaneous `--version` cannot establish the native cost of
  the removed probe. Native matched runs determine that effect.

### Fixture and proof limits

The dense measurement snapshot retained about 281,000 Processes, 1,819 Sessions
and 1.33 million Session events. Its managed-account catalog was empty; both
providers used their existing native authentication. The measurements do not
cover managed-account switching, account brokers, Wave-bound Task launch, a
remote Machine, or cold provider login. A separate fresh read-only backup of the
installed database, including its six account rows, passed source-draft and CLI
schema validation; no launch was attempted against those copied accounts.
No source binary opened the installed database for writing.

Earlier exploratory probes, before account-route removal, reached copied absolute
managed-provider homes and reported rejected refresh grants. They were excluded.
There was no filesystem-write audit of those probes, so complete provider-home
isolation is unproved for them. The preparation repair prevents repeating that
ordering; it does not retroactively establish isolation.

Only benchmark-created Sessions are connected. Codex needs a persisted native
turn for reconnect, so preparation asks for one `OK` response without tools;
those seed turns are outside timing. All measured launches type and erase an
unsubmitted marker. Claude needs no seed turn. Neither provider's transcripts,
credential files nor private SQLite state appear here.

The missing-program error now follows account resolution and activation because
the real spawn is authoritative. A broken managed-account route can therefore
report its error before an absent executable. Actual spawn failure still records
positive non-start evidence, and the retained-Session recovery test proves retry.
Successful account selection, context assembly and control fencing are unchanged.

The [CI smoke test](../../../rust/loopflow/tests/agent_startup_tests.rs) runs both
entry points with an empty environment and a stand-in provider. It preserves
Session/native identity, the default skill's context, and 30,000 retained events.
It requires one provider invocation, at most 25 Git children, fewer than 2,000
SQLite statements and 25,000 returned rows, and a deliberately loose 30-second
readiness ceiling. Network-isolated execution passed. These are regression
alarms, not production latency targets.

Native numbers, profile counts, build hashes and full spread are under
[20261008/](20261008/); [summary.json](20261008/summary.json) is generated by
`summarize.py` with inclusive quartiles and 10,000 fixed-seed paired bootstraps.
All-target Clippy, formatting, and focused attribution/reconnect/smoke behavior
checks passed. No installed performance gain or hosted CI result is claimed.
