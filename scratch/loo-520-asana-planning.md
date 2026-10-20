# LOO-520: Asana as a planning provider

Status: implemented for Tasks and verified against live Asana (2026-10-19); Projects
and Notion remain. Jack Heart asked for autonomous work through implementation to
a demo, without a design review.

## Outcome

A repository selects its planning provider the way it selects an agent harness:

```yaml
# .lf/config.yaml
planning:
  provider: asana
  project: "1209…"   # the repository's Asana project
```

`lf task create`, `lf task list`, `lf task complete/reopen/edit/comment` and Desktop
behave as they do for Linear; the Asana task mirrors the Loopflow Task. Linear
stays the default remote provider; Git stays the default transport.

## Shape

- `PlanningConfig` is the one selection seam: `Git | Linear | Asana { project }`.
  `ops::planning_client` resolves it into one `PlanningClient` enum
  (`Linear(LinearClient) | Asana(AsanaClient)`), mirroring `HarnessKind`.
  Linear-only operations reach their client through `PlanningClient::linear()`.
- Local Work stays authoritative. Export, field, state and comment receipts are
  shared; only the transport differs. Asana returns the same observations
  (`LinearIssue`, `LinearComment`, `LinearIssueObservation`) so acceptance,
  uncertainty and steering stay one path.
- Identity: Linear accepts a caller UUID; Asana accepts an OAuth app `external.gid`.
  Loopflow writes the export UUID as `external.gid` and addresses every task as
  `external:<uuid>`, so `external_issue_id` stays the Loopflow-assigned identity and
  a lost creation reply is resolved by exact lookup, never by a second create.
- Asana scope ("team"): the repository project gid. Identifier: the local
  `lf-<uuid>` identifier (Asana has none). Story listing rejects `external:`, so
  comments read by the task's Asana gid.
- State: `completed` ↔ `completed: true`; `unstarted`/`canceled` targets map to
  `completed: false` / `true`. Observed state is `completed` or `unstarted`.
- Comments ↔ Asana comment stories. Asana gets the visible text, without
  Loopflow's hidden markers. Delivery and read-back claim a story by that text,
  so nothing echoes back as a steer.
- Auth: `lf account connect asana`, the same loopback OAuth broker as Linear
  (`PmOAuthBroker` over a per-provider `PmOAuthSpec`), with the client ID and
  secret from Doppler (`ASANA_CLIENT_ID`, `ASANA_CLIENT_SECRET`). Asana keeps one
  long-lived refresh grant and needs the secret to refresh.

## Linear-only (unchanged)

Team claims, Initiatives/Waves, team delta acquisition, reteam, attachments,
follow-up relations, project content documents, chapter sweeps.

## Remaining

- Projects: Loopflow Project ↔ Asana mirror (section or project). Until then a
  Task in a Project awaits its Project mapping in an Asana repository.
- Inbound acquisition of tasks created in Asana (team delta equivalent).
- Notion provider: follow-up Task; the seam holds (see questions.md).
- Rename shared observation types off the `Linear` prefix; wire field `linear_id`
  → provider-neutral (touches Swift DTO mirrors).

## Demo (for Jack)

The scratch Asana project is "LOO-520 Asana planning demo" (gid `1219678216343882`)
in the Loopflow team.

```bash
doppler run -- lf account connect asana        # browser consent
mkdir -p /tmp/asana-demo/.lf && cd /tmp/asana-demo && git init -q
printf 'planning:\n  provider: asana\n  project: "1219678216343882"\n' > .lf/config.yaml
lf task create --title "Plan in Asana" --notes "Created by lf"
lf task list                                    # the Task, with its Asana link
lf task complete <id>                           # Asana task shows completed
lf task comment <id> "From lf"                  # appears as an Asana comment
```

## Deleted

`linear_observe::connected` is gone; shared paths now call `planning_client::connected`.
A duplicate `PlanningProvider` enum was removed too. No earlier Asana code remained
after 43a1f85ce3.

## Checks

- `cargo test --lib asana` (LF_* unset): 8 passed. `cargo clippy --all-targets -D warnings`: clean. Broader suites → gate.
