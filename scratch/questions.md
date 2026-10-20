# LOO-520 open questions and assumptions

- **Asana OAuth redirect.** `lf account connect asana` uses the same loopback
  redirect as Linear (`http://localhost:19222/oauth/callback`). The Asana app
  behind `ASANA_CLIENT_ID` was last used with the out-of-band code-paste redirect.
  A headless probe could not confirm the loopback URL is registered, because
  Asana shows its login page before checking the redirect. If connect fails with
  a redirect error, add that URL to the app's redirect URLs in the Asana
  developer console.
- **Asana scope is one repository project** (`planning: {provider: asana, project}`).
  Assumed over a workspace or team because Asana tasks need a home, and a project
  is where people already look.
- **Loopflow Projects are not mirrored to Asana yet.** Project acceptance is built
  on Linear Initiatives and Teams (`pm_projects`, `pm_wave_sync`, `attach_in` keyed
  by caller IDs). The candidate mapping is Project ↔ Asana section of the
  repository project, but sections carry no `external` ID. Until then, a Task in
  a Wave Project waits with "Task export awaits its Project mapping", and
  `planning_client::PlanningClient::linear` reports that Asana does not mirror
  Projects. Wave-less Tasks mirror fully.
- **Comment identity in Asana is its visible text.** Stories carry no caller ID,
  so Loopflow strips its hidden `<!-- loopflow-… -->` markers before posting.
  Delivery and read-back match on that visible text, which means two identical
  Loopflow comments on one task collapse into one Asana comment.
- **Canceled Tasks close the Asana task.** Asana only has open and completed.
- **Asana tasks created in Asana are not acquired** into `lf task list` yet.
  Linear's team-delta acquisition has no Asana counterpart. Cold lookup reports
  this instead of guessing.
- **Remote-machine credential transfer** (`machine_credentials.rs`) covers Linear
  only.
- **Notion** (follow-up Task): it fits the same `PlanningClient` variant. Tasks map
  to database rows, the Loopflow UUID goes in a row property queried by an exact
  filter, and comments use the Comments API.
- **Naming debt.** Shared observation types (`LinearIssue`, `LinearComment`,
  `LinearError`), `Task.linear_id` and the wire field `linear_id` now carry Asana
  values too. Renaming the wire field touches the Swift DTO mirrors and fixtures.
  Left for a focused rename.
