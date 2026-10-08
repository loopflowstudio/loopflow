# Agent startup

Open the [interactive flamegraph viewer](index.html) to compare all three changes.

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

Jack Heart requested autonomous profiling and optimization with matched evidence
for every retained change. Three small deletions survive this pass:

- Interactive startup uses the actual provider spawn result instead of launching
  the selected provider once with `--version` and again to open it. Automatic
  provider discovery still probes when no agent is configured. Missing-executable
  failures retain the installation hint and positive non-start receipt.
- An absent ambient Wave returns before Git discovery, runtime/thread creation,
  and registry reads. Explicit and inherited Wave IDs retain their existing
  resolver, including stale-identity errors. Nothing is cached.
- Prompt construction takes the directory already resolved by CLI dispatch.
  It no longer launches Git a second time to find that same directory. Task-bound
  execution retains its selected directory; no repository facts persist across
  commands. A separate follow-up comparison below measures this third deletion.

On an M4 Max, release builds with debuginfo, native Claude 2.1.294 and Codex
0.161.0, the first two deletions improve warm bare launch by **41 ms paired median
for Claude and 24 ms for Codex**. Codex reconnect improves by **16 ms**.
Claude reconnect has **no demonstrated final improvement**: its median was
868 → 899 ms in the last native run, with a paired difference interval spanning zero.
The smaller initial probe-only run was more favorable. This is a modest reduction,
not parity with direct provider startup.

### Native readiness with the first two deletions, 16 alternating samples each

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
The requested cold-cache comparison remains unmeasured; first use of a fixture
does not close that acceptance gap.

| Provider / route | Baseline median [Q1–Q3]; range | Candidate median [Q1–Q3]; range | Candidate minus direct median |
|---|---:|---:|---:|
| Claude direct | 667 [643–681]; 638–710 | — | — |
| Claude bare lf | 1091 [1045–1132]; 1013–1460 | 993 [989–997]; 945–1009 | +326 |
| Claude connect | 821 [791–831]; 790–879 | 818 [816–828]; 770–849 | +151 |
| Codex direct | 292 [291–293]; 290–311 | — | — |
| Codex bare lf | 692 [679–715]; 667–820 | 638 [637–711]; 632–817 | +347 |
| Codex connect | 479 [478–495]; 465–501 | 466 [461–478]; 459–486 | +174 |

### Cold-cache verification: deferred to an isolated macOS gate host

No cold-cache result is claimed. On October 8 this run had macOS 26.0.1,
a non-root account (uid 501), `/usr/sbin/purge`, and other active agent processes.
No dedicated benchmark VM was provisioned; `tart`, `limactl`, and
`qemu-system-aarch64` were absent from PATH. A shared virtualization process was
present, which establishes no isolated host or cache-reset authority. Running
`purge` or rebooting this shared Mac would disturb unrelated work; neither was
attempted. Copying a fixture, `F_NOCACHE` on its database, or evicting just the CLI
cannot establish cold executable, library, Git and native-provider pages.

**Verification owner: LOO-436 gate on a dedicated disposable macOS benchmark
host**, with exclusive cache-reset/reboot control and native provider login.
This environment was not available to the current implementation run. Preserve
the existing warm and first-use evidence; do not relabel either as cold.

On that host, prepare one private dense Home and benchmark-only native Sessions
before timing, using the same release binaries and provider versions for every
route. Disconnect unrelated workloads. Alternate baseline/candidate and direct/
bare/connect order for at least 16 rounds. Before *each* cold invocation, flush
writes and reset the dedicated host's filesystem caches (or perform a full reboot,
then run exactly one timed invocation); record the reset method and successful
exit/boot identity outside the timer. Avoid inspecting the fixture or executing
the binary between reset and timing. Measure raw-mode marker readiness with
`measure.py`, then run the identical invocation immediately for its warm pair.
A reset once per round is insufficient: the first route would warm the others.
Report OS/file-cache cold separately from authenticated/trusted provider state;
this is not a cold-login test. A VM additionally needs an explicit host-cache
policy: a guest reboot alone leaves the host's backing-file cache warm. Retain
numeric results, reset receipts and build hashes; keep credentials and transcripts
private. The credential-free CI smoke cannot satisfy native cold readiness.

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
The third deletion has [bare baseline](20261008/reuse-directory-bare-baseline.svg),
[bare candidate](20261008/reuse-directory-bare-candidate.svg),
[connect baseline](20261008/reuse-directory-connect-baseline.svg), and
[connect candidate](20261008/reuse-directory-connect-candidate.svg) graphs.
Each SVG is standalone, titled, demangled, searchable and zoomable.
XML and artifact sanitization checks passed; rendered visual review remains for
review because this run has no rendering environment.

These are main-thread **wall-stack samples**, including blocked waits and
shutdown, not CPU flamegraphs or exact elapsed-time accounting. Attach misses
some initial startup; the very short first candidate capture missed most of it.
Each graph aggregates three profile attempts; the third-deletion bare baseline
has two nonempty captures because the first attach collected no stacks. Child Git/provider CPU and worker
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
470 → 464 ms (paired 8 ms, interval 4–17 ms). The two-deletion Codex comparison is
616 → 590 ms bare and 460 → 444 ms
connect, with positive paired intervals of 16–27 and 8–22 ms respectively.

The Wave-only stand-in comparison has 20 alternating pairs: bare 445 → 432 ms,
paired improvement 14 ms (95% bootstrap interval 1–27 ms); connect 272 → 272 ms,
interval −11 to 8 ms. Keep it for its measured bare-launch reduction, not a
reconnect timing claim. The last native Claude reconnect result is retained as contrary
evidence, not replaced by the more favorable earlier run.

An earlier rejected optimization retained the SQLite handle between capture
reservation and publication, removing an immediate close/reopen. Thirty matched
pairs gave bare 431 → 428 ms, paired improvement 3 ms (interval −4 to 9 ms).
Connect was an unchanged control: 283 → 277 ms, interval −5 to 11 ms. The signal
did not separate from noise, so **that code was removed**. Its raw numbers remain
in `rejected-retain-store.jsonl`.

The follow-up tested passing dispatch's resolved directory into prompt
construction. Thirty alternating pairs from the same current source base gave
bare handoff **402 → 390 ms**, paired improvement **7.5 ms** (95% bootstrap interval
**3.5–14.2 ms**). Git children fell **16 → 15**; SQLite work stayed at 19 connections
and 1,089 statements. Reconnect was unchanged: **242 → 239 ms**, paired interval
**−3.3 to 8.3 ms**, still eight Git children and 676 statements. Keep this reduction
for bare launch only. These are stand-in handoff timings, not another native
Claude/Codex comparison; do not add 7.5 ms to the earlier native gains.

| Route / variant | Median [Q1–Q3], ms | Full range, ms |
|---|---:|---:|
| Bare baseline | 402 [395–405] | 352–421 |
| Bare candidate | 390 [384–399] | 367–921 |
| Connect baseline | 242 [238–251] | 232–263 |
| Connect candidate | 239 [234–250] | 227–276 |

All 120 launches succeeded and remain in `reuse-directory-handoff.jsonl`, including
the candidate's 921 ms outlier. The paired bootstrap uses the same fixed-seed
method as earlier runs. Both immutable release binaries were rebuilt at the
current merged source revision; hashes and fixture counts are in
`reuse-directory-metadata.json`. The stand-in comparison ran through
`scripts/test_network.py`: external egress denied, local IPC available. A prior
blanket network denial prevented required local IPC during preparation; it
produced no timed samples. Own builds finished before measurement; unrelated
Clippy activity was observed on this shared host. Alternation and the unchanged
control bound interpretation; this is not exclusive-host evidence.

A final experiment deferred ancestor `.git` metadata probes until Git discovery
failed. Thirty alternating release pairs gave bare **380 → 385 ms**, paired
improvement **−3.1 ms** (95% interval **−9.0 to 4.8 ms**), and reconnect
**238 → 234 ms**, paired **6.5 ms** (interval **−2.8 to 12.8 ms**). Neither result
separates from noise. **That code was removed**; the patch, all 120 numeric rows
(including a 2.58-second baseline outlier), and its binary hash remain under
`rejected-defer-git-probes.*` and `reuse-directory-metadata.json`.

**Stopping condition: attempted reductions stopped producing measurable gains.**
The directory pass-through was a new gain, so investigation continued until the
filesystem-probe experiment failed. Together with the earlier rejected store
reuse, it bounds this pass; it does not prove every possible optimization is
exhausted. Remaining Git calls serve placement, Task attribution, canonical
Session repository identity, provider writable roots and account context at
different ownership boundaries. Sharing those facts more broadly would require
changing those APIs. Context/token counting, SQLite validation and durable
capture publication remain required work. Native startup retains the provider
cost described above. No persistent cache or skipped context was introduced.

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

The follow-up converter stalled with the Python parent blocked writing stdin
and `rustfilt` blocked writing stdout. Its input now uses a temporary file;
all four new SVGs generated successfully from the retained captures. One initial
smoke assertion expected `AGENTS.md` in Claude's appended context; native-file
deduplication deliberately removes it. The surviving assertion checks default
skill context and the captured root from a nested-directory launch.

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

The directory pass-through passed the public bare/connect smoke and checkout
Task-attribution test. The smoke also launches from a nested directory and checks
both repository context and the captured root, covering the path now passed by
dispatch. The final review found no new owner, cache, migration or account-routing
change; the missing native rerun, cold-cache evidence and rendered SVG judgment
remain distinct from the handoff improvement.

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
