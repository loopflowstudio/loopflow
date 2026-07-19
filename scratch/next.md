# Slice 5: finish Project and Task planning truth

## Implement

- Replace launch/snapshot wrappers with `ProjectDefinition` and
  `TaskDirective`.
- Expose `Project.definition`; expose `Task.directive` and `Task.project_id`.
- Delete Task's copied Project name/slug/definition/KRs/snapshot timestamp.
- Make Task SQL select only Task data plus `project_id`; prompt, status,
  roadmap, journal, and diagnostics resolve the parent Project deliberately.
- Update Rust/Swift/JSON/docs/tests without compatibility aliases.

## Preserve

- Stable Project and Task Work ids and Linear bindings.
- Runtime `agent`, `provider`, `provider_session_id`, abandon intent, and
  handoff until the server-topology design chooses their replacement.
- Current schema if existing normalized columns already express this shape; do
  not add a data migration only to rename Rust fields.

## Done when

- [ ] `ProjectLaunchReceipt`, `TaskLaunchReceipt`, Linear snapshot wrappers,
      `launch_context`, and `.launch.project|issue` are absent.
- [ ] Task carries no copied Project PM metadata.
- [ ] Project definition updates affect the next Task prompt/status without a
      Task rewrite.
- [ ] focused store/migration/prompt/status/roadmap tests, fmt, and clippy pass.
