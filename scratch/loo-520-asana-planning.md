# LOO-520: Asana and Notion as planning providers

Status: Tasks implemented for Asana and Notion, each verified against the live
provider (2026-10-19); Projects remain. Jack Heart asked for autonomous work through
implementation to a demo, without a design review, then asked (2026-10-19) to put
Notion on this Task too, built on today's Work ↔ planning model rather than the old
`PmProvider` data model.

## Use cases (Jack Heart, 2026-10-19)

The tracker serves two jobs; acceptance walks both end to end, not provider calls:

1. **Interchange with coworkers.** A coworker creates, assigns, edits, comments on,
   completes or deletes a task in the tracker and Loopflow follows; Loopflow's
   progress (in progress, PR, shipped) shows up where the coworker looks.
2. **Meeting capture.** People talk (formally or not) and capture Tasks for later,
   in Loopflow chat or in the tracker; either way the Task exists in both places.

Conflict policy (Jack, 2026-10-19): the latest edit sent wins. An offline local
edit delivered after a tracker edit overwrites it; the overwritten value stays as
a receipt. This replaces today's tracker-wins adoption for every provider.

## Gaps against the use cases (2026-10-19 audit)

docs/planning.md states the target UX. Done on this branch:

- Tracker-created items become Tasks: the repository check reads changes through
  `PlanningClient::changes` for every provider; Asana/Notion items take a
  deterministic identity (v5 from the Asana gid; the Notion page ID). Standalone
  Linear issues arrive too, and the check runs before any Task exists.
- Deletion in the tracker removes the Task from the plan on positive evidence
  (Linear trash, Asana 404, archived Notion page via its URL).
- Starting work (checkout) queues a `started` state delivery; draft migration
  `task_state_started` widens the delivery target check.
- PR state reaches Asana/Notion as one comment per state.

Remaining before the docs are true:

- Projects in Asana/Notion (sections / `Project` select). Project acceptance is
  Linear-shaped (`pm_projects`, Initiatives, `ACCEPTED_WAVE_PROJECTS`); an item
  observed with no project clears local membership, so export stays gated.
- Latest-sent-wins conflicts: `observe_attempts` sets `conflict_json` and
  acceptance adopts the provider; flipping it changes Linear's tested policy.
- Asana identifier footer: belongs with Task codes (`scratch/task-codes.md`).
- Unverified: assignment between coworkers.

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

- `PlanningConfig` is the one selection seam: `Git | Linear | Asana { project } |
  Notion { database }`. `ops::planning_client` resolves it into one `PlanningClient`
  enum (`Linear | Asana | Notion`), mirroring `HarnessKind`.
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
- Rich text: descriptions and comments travel as `html_notes`/`html_text`, via
  `asana::rich_text` (ported from the deleted `asana_html.rs`, made line-faithful).
  Live Asana stores that HTML byte for byte except `<hr/>` → `<hr />`, so supported
  markdown round-trips exactly; h3+, `* ` bullets, list numbering and fence
  languages normalize once on first read-back, then stay fixed.
- Comments ↔ Asana comment stories. Asana gets the visible text, without
  Loopflow's hidden markers. Delivery and read-back claim a story by
  `comment_markdown` (that text after the rich-text round trip), so nothing
  echoes back as a steer. Story reads page at 100.
- Auth: `lf account connect asana`, the same loopback OAuth broker as Linear
  (`PmOAuthBroker` over a per-provider `PmOAuthSpec`), with the client ID and
  secret from Doppler (`ASANA_CLIENT_ID`, `ASANA_CLIENT_SECRET`). Asana keeps one
  long-lived refresh grant and needs the secret to refresh.

## Notion

- Scope: the repository database (`database:` takes the ID or its URL;
  `notion::database_id` normalizes both). A Task is a page; the description is
  the page body as blocks (`notion::blocks`, line-faithful like `asana::rich_text`).
- Identity: Notion takes no caller IDs, so the export UUID goes in a `Loopflow ID`
  rich-text property and every request finds the page by an exact filter query.
- The database adapts on first use: Loopflow adds `Loopflow ID`, and `Status`
  when no state property exists. State reads Notion's status groups (To-do,
  In progress → `started`, Complete; a Complete option named Cancel… → `canceled`),
  or a `Done` checkbox. `Due` (date) and `Assignee` (people) mirror when present.
- Comments ↔ Notion comments, claimed by visible text like Asana
  (`PlanningClient::claimed_comment`). Without comment capabilities, observation
  continues on title/description and delivery says which capability to enable.
- Delete archives the page. Revision = `last_edited_time` (minute resolution).
- OAuth: `PmOAuthSpec` gained per-provider authorize params, redirect port and
  HTTP Basic + JSON token requests; Notion uses its registered port 19223
  (`NOTION_CLIENT_ID`/`NOTION_CLIENT_SECRET` in Doppler).

## Provider edits

Asana and Notion have no team feed, so each Task observation also runs the exact
read through `planning_delivery::ingest_task`, Linear's acceptance path: renames,
description edits and completion made in the provider update the Task. A
concurrent unsent local edit follows the existing policy: the provider's value is
adopted and the local one stays as an "Adopted planning provider change" receipt.

## Linear-only (unchanged)

Team claims, Initiatives/Waves, team delta acquisition, reteam, attachments,
follow-up relations, project content documents, chapter sweeps.

## Remaining

- Notion live comments: the Loopflow integration lacks Read/Insert comments
  capabilities (live 403); Jack enables them in Notion's integration settings.

- Projects: Loopflow Project ↔ Asana mirror (section or project). Until then a
  Task in a Project awaits its Project mapping in an Asana repository.
- Inbound acquisition of tasks created in Asana (team delta equivalent).
- Rename shared observation types off the `Linear` prefix; wire field `linear_id`
  → provider-neutral (touches Swift DTO mirrors).

## Demo (for Jack)

Notion: scratch database "LOO-520 Notion planning demo"
(`3fff8f99-3d81-8141-bd3d-f63d878a7c74`) under the Waves page. Same steps as below
with `lf account connect notion` and
`planning: {provider: notion, database: "3fff8f99-3d81-8141-bd3d-f63d878a7c74"}`.

Asana:

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

## What the deleted providers taught (Jack asked, 2026-10-19)

- `43a1f85ce3^:…/lfd/pm/asana_html.rs`: ported as `asana::rich_text`. Its parser
  joined paragraph lines with spaces and collapsed whitespace on read-back, which
  merged paragraphs; Asana keeps newlines, so the port keeps them. Added `<pre>`
  for fenced code and `<blockquote>`, both verified live.
- `43a1f85ce3^:…/lfd/pm/asana.rs`: pagination (`limit=100` + `next_page.offset`)
  is ported for stories. Its in-client 429 retry is not: Asana returns
  `RateLimited`, which delivery already defers, like Linear. Workspace/team
  resolution, priority and working-branch custom fields and `claim_item` were
  Wave/claim machinery that now stays Linear-only.
- Old OAuth: PKCE + client secret, scope `default` (`ASANA_OAUTH_SCOPE` override),
  redirect `urn:ietf:wg:oauth:2.0:oob` with `complete_oauth_code_paste`. The same
  Doppler client ID still refreshes the July grant, so the app is the old one and
  OOB is its registered redirect; the loopback URL likely needs adding (questions.md).
- Notion (`1c925c0c18`, `63b39a6cd2^:…/notion.rs`, `notion_blocks.rs`): blocks
  conversion ported line-faithfully (its paragraph joining was lossy, like Asana's);
  pagination, status/checkbox completion, Basic-auth OAuth on port 19223 reused.
  Its Waves bootstrap, priority select and "In Progress" claim were Wave/claim
  machinery, left out. Its data model (`PmItem`, `PmProvider`) is not used.
- `fffbada8c9` cut the `PmProvider` trait + `PmProviderKind`: an open trait whose
  main second implementations were in-memory test mocks, removed when Asana became
  the sole provider. `PlanningClient` is a closed enum over concrete clients with
  shared observations; tests use HTTP stand-ins, not trait mocks.

## Deleted

`linear_observe::connected` is gone; shared paths now call `planning_client::connected`.
A duplicate `PlanningProvider` enum was removed too. No earlier Asana code remained
after 43a1f85ce3.

## Checks

- `cargo test --lib -- asana notion planning_client provider_auth::tests` (LF_* unset): 96 passed.
  Live: Asana rich text stored exactly; Notion client create/body/edit/status/archive
  passed (links gain a `/` after the host once). clippy clean. Broader suites → gate. `cargo clippy --all-targets -D warnings`: clean. Broader suites → gate.

## Decision 2026-10-20: every provider names a team

Jack Heart: a Linear team maps to an Asana team, not a project, and the three
provider schemas should be as similar as possible. Accepted shape:

```yaml
planning:
  provider: linear | asana | notion
  team: <id or URL>
```

- `linear.team` moves into `planning.team`, with no fallback. `lf repo connect` writes the team for every provider.
- Asana: the team. Every Task lives in the team project named after the repository. A Task in a Wave Project is also added to that Project, since Asana lets a task belong to several projects. Wave Projects become Asana projects, and sections are dropped. Waves become Portfolios where the plan allows them.
- Notion (Jack chose this option): a page that holds a Tasks database and a Projects database, linked by a `Project` relation, like Notion's Projects & Tasks template. Loopflow finds or creates both.
- Remaining: rewrite the config and scope for all three, the Asana team model, and the Notion page model. Update docs/planning.md to match.
