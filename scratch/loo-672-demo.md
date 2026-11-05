# LOO-672 — Task conversation demo

## Accepted native re-demo — November 5, 2026, 12:03 PST

Built source `990fab54d3` with the fresh Rust CLI/FFI and Swift app. Jack Heart
reviewed the opened candidate and said “ok lg2m ship it”, authorizing shipping.

Before any Task or Wave selection, six native providers were live: the repository,
the Review Wave, and four Tasks (two Claude, four Codex overall). Selecting Review
showed its already completed reading pass; returning to repo retained the same
conversation. Provider process identities were unchanged after selection. The
Stopped run Graph IDs matched their prelaunch set. Both repo and Wave reported
status without starting Task work. No Start/Open action was clicked.

Evidence: `demo-evidence/all-work-manifest.json`, `all-work-load-ps.json`,
`all-work-load-sessions.json`, `all-work-load-stopped.json`,
`all-work-after-selection-ps.json`, `all-work-repo.png`, `all-work-wave.png`.
The repo screenshot's old No session state is retained as
`store-load-unselected.png`. Native startup and rendered selection pass; exact
drafts, restart, Rex takeover, cold return and twenty-Task private memory were
not measured in this review. The Wave reading pass could not find its authored
files in its prepared checkout; it still read the shared Wave state. This is an
observed context limitation, not proof of full Wave context fidelity.

Jack asked the cache size: actual code keeps eight recent unfinished Task
conversations across windows plus visible exceptions; repository/Wave primaries
are outside that cap. Two concurrent preparations, no byte budget. This means
Task prefetch is bounded rather than literally every Task in a large repository.
This limitation was disclosed in conversation while shipping was authorized.

### Demo retention and ownership

- Durable root: `/Users/jack/.lf/demos/loo672-9cd2824ab8/durable` (private Machine, fixture repo, captured history and native
  provider-history copies).
- Runtime root: `/Users/jack/.lf/demos/loo672-9cd2824ab8/runtime` (complete copied bundle and app log).
- LF_HOME: `/Users/jack/.lf/demos/loo672-9cd2824ab8/durable/machine`
- LF_BIN: `/Users/jack/.lf/demos/loo672-9cd2824ab8/runtime/Loopflow LOO-672 Redemo.app/Contents/MacOS/lf`
- App PID: 61597; bundle `com.loopflow.mac.redemo.loo672.9cd2824ab8`.
- Native history copies for all six threads: `/Users/jack/.lf/demos/loo672-9cd2824ab8/durable/provider-history`. Original
  native threads remain in the configured provider homes; these are copies, not
  relocations. The private Machine's capture history also stays in the durable root.
- Full owner/process identities and history paths: `/Users/jack/.lf/demos/loo672-9cd2824ab8/durable/owners.json` and
  `demo-evidence/all-work-owners.json`.

| Session | Provider | Owner PID | Agent PID | Owner lf process ID |
|---|---|---|---|---|
| 1b004602486944d4b40986ff3ab09c74 | claude | 67405 | 67760 | 09c729a6-e7fb-4afa-bf23-e84b230b11ed |
| db27e9a514ea49a39f3a91d27bbd100e | codex | 67991 | 72330 | 2b1c3f90-f746-422b-916a-519ecc9daeb4 |
| 6bdbe914830f4d6eb9edd1dd83869ae2 | claude | 68093 | 68504 | af4e3437-0c45-43da-81d5-fb10a7b6fb1c |
| 3f0fabfb74b84607852ad459e06ed009 | codex | 66671 | 70857 | d279ee06-31e2-411a-9307-8523564a4c3d |
| 6d8569454f304c01b8a7997593929862 | codex | 66255 | 70735 | b4929b24-17ff-46b1-97ec-55489e91f7d8 |
| d312686713fe4bdd863c8e6d7c3ca1db | codex | 67363 | 71997 | 6319f3a5-6add-4b9f-8a34-d90a83ca40c4 |

Reopen a private-Machine conversation from the fixture repository with:

```sh
cd /Users/jack/.lf/demos/loo672-9cd2824ab8/durable/repo
env LF_HOME=/Users/jack/.lf/demos/loo672-9cd2824ab8/durable/machine LF_BIN='/Users/jack/.lf/demos/loo672-9cd2824ab8/runtime/Loopflow LOO-672 Redemo.app/Contents/MacOS/lf' '/Users/jack/.lf/demos/loo672-9cd2824ab8/runtime/Loopflow LOO-672 Redemo.app/Contents/MacOS/lf' resume <session-id>
```

Teardown is **not released**. No reviewing-assistant draft was typed or submitted.
The automatic reading passes settled their own initial turns. Any input Jack
entered remains unclassified until Jack answers the demo release question. Both
roots and all owners stay intact. Approval to ship is not release of editor input.

After Jack confirms every draft/request settled, use the resume command above
for each exact Session and exit its provider normally. An owner with an edited
draft remains running even after submission. If an eligible owner will not exit,
first re-read `env LF_HOME=/Users/jack/.lf/demos/loo672-9cd2824ab8/durable/machine LF_BIN='/Users/jack/.lf/demos/loo672-9cd2824ab8/runtime/Loopflow LOO-672 Redemo.app/Contents/MacOS/lf' '/Users/jack/.lf/demos/loo672-9cd2824ab8/runtime/Loopflow LOO-672 Redemo.app/Contents/MacOS/lf' ps --json` and verify its full process identity and PID
against owners.json, then `kill -TERM <verified-owner-pid>` only for that owner.
Confirm every demo owner has exited before removing the exact runtime directory:
`rm -rf /Users/jack/.lf/demos/loo672-9cd2824ab8/runtime`. Never delete the durable root. If any input
is given up or any owner survives, retain runtime and owners and document it.

## Headless demo decision — November 5, after the gate

No one was present; this pass stood in for Jack Heart. Since the accepted native
re-demo (`990fab54d3`, “ok lg2m ship it”) the branch changed only the warm Task
bound (8 → 16, Jack's request, `0426d19ae9`) and a test fixture trusting its
folder (`457a45cd68`). Neither changes what Jack observed: the bound is a constant
whose retention and prefetch tests derive from it (focused Desktop filter 182
passed; full Desktop 724 passed), and the fixture fix restored `transport_tests`
60/60. No new native run was made; the accepted experience stands. Decision:
advance. Open: Wave memory still says “Eight warm” (realign), and the unmeasured
cases above (drafts, restart, Rex, cold return, twenty-Task memory) remain
unproven, as disclosed when shipping was authorized.

Headless demo, November 5 13:15 PST: no code changed since the advance above (only a gate note, `14f845fb5c`); the accepted experience and proofs stand. Decision: advance.

Current runbook: `scratch/demo.html`. Earlier results below are historical.


Full record of the November 4 failed reviews, corrections and earlier candidates:
`78f4d8e6c0:scratch/loo-672-demo.md`. Jack Heart's decisions are in
`questions.md`; the design is `loo-672-ready-conversations.md`; the runbook is
`demo.html`.

## Jack Heart’s feedback — November 5

Jack: “otherwise looks good” for the repaired Task demo. The selected repository
in Jack’s screenshot still showed No session open / Open conversation. Jack
requested the same behavior for Waves and repositories, then clarified “auto
open everything *AND* prefetch everything”. Implement the expanded scope in
`loo-672-load-prefetch.md`; return for demo. No shipping approval. The expanded
scope is implemented at the tree (`loo-672-ready-conversations.md`, Primaries) and
unproven natively; the November 5 candidates predate it. Cases added the same day
for the next demo: a started Task nobody owns opens on load like any other, with no
Claim/Start gate; an unstarted Task's pane offers Start and no conversation gate;
Task, Wave and repository panes show the same placeholder when a conversation is
not open; selecting a completed Task you own with no history shows its record,
never a Start or Open placeholder, and its history appears in panes once revealed;
a Dashboard tile whose Task conversation failed to open reads the same as the
pane, **Retry** on the failure and Rust's Start label after a refused reveal, and
either attaches in the tile with the board still shown, on a Dashboard, the
repository page or a Wave view (in the tree since the eighth- and ninth-review
tile direction).

## Earlier candidate — November 5

Jack Heart requested launch. Rebuilt Desktop at `eae0d31694` and prepared fresh
private data at `/private/tmp/loo-672-redemo-346bmcxn`. Launched and raised its
window through native UI. Before any Task selection, all four sidebar rows
reported Working and `current-load-ps.json` recorded four live providers.
Manifest: `demo-evidence/current-manifest.json`. No human acceptance yet.

## Store-launch proof — November 5, 2026, 05:41–05:55 PST

**Result: load alone starts every working Task conversation, and they stay warm
after selection. Awaiting Jack Heart's re-demo.** Candidate
`/private/tmp/loo-672-redemo-nz7p4u4v` (own bundle id; run `launch.sh` from a
display session; [manifest](demo-evidence/store-manifest.json)), fresh private
data: four owned idle Tasks, no provider ever launched, the Stopped run Task
carrying its three stopped `TASK_RUN_ATTEMPTS` records and the “start it” comment.
`uv run python scratch/prepare-redemo.py` reproduces the setup from the debug CLI
and app builds without touching the main environment. The candidate was built from
`1fdbf09f7a` plus the store-launch diff committed as `78f4d8e6c0`; `83b3d64a93`'s
shared Task matching came after it and is unproven natively. `demo.html` still
describes the November 4 candidate (`7b23c50c20`, “Prefetch still needs repair”);
the next demo rebuilds it from a HEAD candidate and this evidence.

Observed with nothing selected
([store-load-unselected.png](demo-evidence/store-load-unselected.png)):

- Within seconds of launch all four providers were live: Claude for **Claude
  continuity** and **Stopped run**, Codex for **Codex continuity** and the
  absent-Session **Automatic conversation**, whose primary Session load created
  ([store-load-ps.json](demo-evidence/store-load-ps.json), four `agent_process`
  nodes; [store-load-sessions.json](demo-evidence/store-load-sessions.json), all
  `active` with agent process IDs and terminal surfaces).
- No Graph ran: `lf ps` held no Graph node and the Stopped run's Graph IDs equal
  `status_before_load` ([store-load-stopped-status.json](demo-evidence/store-load-stopped-status.json)).
- `lf desktop goto 2` selected **Claude continuity**: the pane adopted the
  running terminal mid reading pass with the same agent PID
  ([store-selected.png](demo-evidence/store-selected.png),
  [store-selected-ps.json](demo-evidence/store-selected-ps.json)). The other
  three kept their surfaces and `active` Sessions; `lf desktop capture` read all
  four terminals ([store-selected-captures.txt](demo-evidence/store-selected-captures.txt)).
  The Stopped run's untouched conversation reported the failed step and the
  recorded “start it” comment without acting on it.
- Found and fixed on the first candidate: leaving the Dashboard released the
  tiles' terminals because tile workspaces never entered the warm set.
  `prefetch` now admits a visible tile; `WorkspaceRetentionTests` covers it.

The three extra Graph records of November 4 were a ship restart, not opening:
`lf task run` on a graph with no interactive step approved Auto-Ship, and
`ops::recovery::recover()` restarted the ship on the next read. The fixture graph
now ends in `interactive: default`.

## History in one paragraph

The first November 4 review failed on Jack Heart's screenshot showing “No existing
conversation; start one explicitly”, an Open button and a Machine cleanup error;
Jack said “ok, get it fixed and let me know when ready ofr me to redemo”. Both
defects were repaired (explicit Start from Rust's resume explanation; local
presence through the local query). The November 4 re-demo then launched real Codex
and Claude from an explicit Start click, which Jack rejected: “but the conversation
should be auto stated and opened”. The November 4 native candidate (`7b23c50c20`)
created absent Sessions on load but started no provider until selection; the
November 5 store-launch repair above replaced it. Screenshots and process
observations from every stage remain under `demo-evidence/`.

## Desktop suite crash that depends on environment size

`scripts/test_desktop.sh` failed three of three times with exit 1 in
`defaultConfigurationLoadsAfterInitialAppearance` at 402 passes, EXC_BAD_ACCESS
in `ghostty_config_finalize`. The same binary passed 714 tests when the count of
unrelated environment variables changed (two or three added, or zsh with relative
paths). The test passes alone and within its suite; the branch adds no `setenv`
and does not touch the test. Inferred: libghostty holds an `environ` pointer that
an earlier test's `setenv` reallocates. Gate may hit this; a fix belongs in a
separate Task if Jack Heart wants one. Logs: `/tmp/loo672-desktop-full2.log`,
`/tmp/loo672-P{1,2,3}.log`.

## Required configured demonstration

With real Claude and Codex Sessions, from a fresh repository load with no saved
visits, verify working Tasks prefetch automatically without changing selection
(proven above), then:

- Repository and Wave primaries: with no saved history, load opens the repository
  and each Wave conversation unselected; selecting one reveals the already started
  Session and no operator pass or Graph launched; returning to an existing Task
  conversation keeps it. The fixture needs a Wave in the loaded repository.
- A → B → A: type an unsubmitted draft; verify exact draft and provider identity.
- Exceed the warm set, return cold: saved history precedes input readiness and no
  new turn starts. Measure first paint separately from usable input.
- Control a disposable Session in Rex: selection stays passive, last content
  remains, and only **Take back** transfers input. No Fork button.
- Close a pane and wait for refresh: it stays closed until deliberately selected.
- Relaunch the candidate: selected Task returns first, bounded preparation follows.
  Exercise stale-owner/reboot recovery only in disposable data.
- Visit twenty Tasks and measure Desktop + lf + provider private footprint, listing
  visible exceptions and protected providers. Eight is still provisional.
- Confirm rendered reporting of Claude's folder-trust dialog in a hidden pane and
  its clearing after an answer.
