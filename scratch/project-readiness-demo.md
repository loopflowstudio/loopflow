# Project readiness demo — October 5, 2026

Review for LOO-366 at source `42e6c2706b1b35b2852e438ff94a49d060faccda`.
[Accepted design and remaining acceptance](keep-every-wave-ready-for.md).

## Observed configured behavior

The agent inspected the installed `/Users/jack/.local/bin/lf`: `lf --version`
returns 0.13.4. Installed `lf wave --help` exposes neither `ensure` nor
`bind-project`. `/Applications/Loopflow.app` reports version 0.13.4; no mounted
Desktop interaction was demonstrated.

Installed `lf wave status intelligence --json` returned Project
`999bdbdd-c045-41a6-8ffc-a97c4a40b0b3`, named `Chapter demo-20260924`, with
status `backlog`, empty Flow and two retained KRs. Tasks are unavailable with
`Wave intelligence has no In Progress Project`. This is installed readback,
not a fresh direct Linear observation or successful activation.

The installed demo was blocked; no production Project changed.

## Feedback and next action

Jack Heart previously authorized autonomous repair of this exact Intelligence
Project without another review Session. No new human observation, acceptance or
design change was supplied during this demo. The accepted shared binding and
exact-ID policy remain unchanged; no navigation verdict is recorded.

Recommended next action: finish the design's outstanding gate proofs and deliver
through the published installation path. Once these commands are available:

```bash
lf wave bind-project intelligence 999bdbdd-c045-41a6-8ffc-a97c4a40b0b3 --json
lf wave ensure intelligence --json
```

Repeat ensure and reopen Intelligence in Desktop. Confirm the same UUID,
In Progress status, unchanged KRs/Tasks and usable planning without adding Flow.
Configured fixtures, mounted retry and crash recovery remain unproved.

## Disposable app follow-up — October 6, 2026

Jack Heart requested a disposable Home and demo app. The agent built this branch's
Swift app and a temporary Rust test harness reusing its stateful Linear fixture
and production `project::bind_project` / `project::ensure`. The harness-only edit
was removed from the checkout after compilation; product source is unchanged.

The app is `/tmp/loo366-demo/Loopflow Project Demo.app`, bundle ID
`com.loopflow.mac.loo366demo`; Home and repository are `/tmp/loo366-demo/home`
and `/tmp/loo366-demo/repo`. No installed Home was copied. The bundled adapter
sends ensure to the fixture harness and forwards permitted reads to the source
CLI. This exercises real operations and SQLite projection, but not public CLI
ensure dispatch or real Linear. Synthetic credentials belong only to the fixture.

Observed by the agent:
- Primary-window opening selected Wave A and rendered its Backlog Project,
  authored KR and Tasks while B had no plan. The command log contains no ensure
  from that opening. `WorkSurfaceView.swift:111` renders this path;
  `WaveDetailPane.swift:175` prepares only the Portfolio path. The former also
  renders a Flow section for an empty string. Its first template error was caused
  by the adapter refusing flow-list; that read is now permitted.
- Adapter ensure activated A. Repeated ensure retained UUID
  `00000000-0000-4000-8000-000000000001`, name `Summer work — customer requests`,
  empty Flow, KR and Task bodies. B created UUID
  `fbcd01fc-f625-4e8e-a883-4808b120cb5b` and reused it on repeat.
- A simulated provider outage made ensure fail with `fixture interrupted
  connection`; source status retained the cached Project. Recovery succeeded
  with the same ID. Assertions compared before/after Project fields and Task
  bodies; they did not seed or prove local Task/PR/Session/Flow execution identity.
- Portfolio automatic capture produced no image; mounted Portfolio activation
  remains unproved. The persistent app was reopened for Jack Heart to inspect.
  Its log reports AttributeGraph cycles; cause and effect remain unresolved.

Evidence under `/tmp/loo366-demo/`: `activated.png` (despite its filename this is
**pre-activation** main-window evidence), `commands.jsonl`, `before.json`,
`after.json`, `offline-status.json`, `offline.err`, `provider.json`,
`harness-source.rs`, `adapter.py`, `server.log`, `app.err` and build logs.
[Code walkthrough](pr-review.html) includes the primary-path mismatch.

Recommended next implementation: share preparation with primary Wave opening
and retry, suppress absent Flow presentation, then repeat mounted opening,
reopening and failure recovery. No new human acceptance or agreed design change
was supplied. The prior installed-path blocker remains; this local proof does
not repair production Intelligence or establish configured acceptance.
