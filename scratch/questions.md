# LOO-520 open questions and assumptions

- **Asana OAuth redirect.** `lf account connect asana` uses the same loopback
  redirect as Linear (`http://localhost:19222/oauth/callback`). The app behind
  `ASANA_CLIENT_ID` is the one the deleted code-paste flow used (it still refreshes
  that July grant), and its registered redirect was `urn:ietf:wg:oauth:2.0:oob`.
  Expect to add the loopback URL to its redirect URLs in the Asana developer
  console once. Not restoring code paste: one redirect for every planning provider.
- **Asana team project is named after the repository** (origin repo name, else
  the main checkout directory). Several projects with that name in the team is an
  error naming the fix, not a guess.
- **Asana Project identity is a visible notes footer** (`Loopflow ID: <uuid>`).
  Asana projects accept no `external` ID and custom fields are paid-only.
- **Notion: one Tasks and one Projects database per team page**, found by title
  (case-insensitive). Several repositories pointing at one page share them; no
  per-repository property yet.
- **Wave scope for Asana/Notion is the team** (all Waves share it as their
  "initiative"); Projects are confirmed through the local Project, never by
  scanning Waves.
- **Comment identity in Asana is its visible text.** Stories carry no caller ID,
  so Loopflow strips its hidden `<!-- loopflow-… -->` markers before posting.
  Delivery and read-back match on that visible text, which means two identical
  Loopflow comments on one task collapse into one Asana comment.
- **Canceled Tasks close the Asana task.** Asana only has open and completed.
- **Remote-machine credential transfer** (`machine_credentials.rs`) covers Linear
  only.
- **Notion adapts the database's schema** by adding `Loopflow ID` (and `Status`
  when absent) rather than refusing a database without them. `Due`/`Assignee`
  are only used when present; a Task with a due date or assignee in a database
  without them fails export with a message naming the property.
- **Notion revision has minute resolution.** Two edits in one minute share a
  revision; content comparison still sees the change.
- **Notion API version 2022-06-28.** Newer versions query data sources instead of
  databases; staying put keeps the database ID as the scope.
- **Naming debt.** Shared observation types (`LinearIssue`, `LinearComment`,
  `LinearError`), `Task.linear_id` and the wire field `linear_id` now carry Asana
  values too. Renaming the wire field touches the Swift DTO mirrors and fixtures.
  Left for a focused rename.
