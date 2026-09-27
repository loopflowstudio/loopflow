# Linear and GitHub in newcomer setup

## Feedback

Jack said: "I think we can go so far as to not require linear at all and just
have it be an optional sharing mechanism. I think we might want to assume/rerquie
github for now".

Asked whether GitHub should connect before opening a project or only at first
publication, Jack replied: "i would say make sure theyre signed in asap".

## Design changes

- Linear is optional sharing in the intended product, including local planning
  and Tasks. This goes beyond skipping Linear for a direct conversation.
- GitHub sign-in is required during initial setup, before **Open project…**.
  Verify existing sign-in and skip unnecessary repeat login. Do not postpone
  connection until publication. The demo and acceptance criteria now include
  GitHub and acquiring `gh` on a clean Mac without a package manager.
- Existing first-task work (LOO-316) overlaps the local Task requirement. This
  review does not reassign work or claim its underlying model already works.

## Evidence

- [Current design](a-newcomer-gets-from-download.md).
- [Init review](init-setup-review.md).
- [Init skill](../rust/loopflow/src/engine/builtins/ops/skill/init.md) currently
  describes Linear-backed Tasks and planning; only direct skills explicitly
  proceed without it.
- [Provider auth](../rust/loopflow/src/provider_auth/mod.rs), `GhAuthBroker`,
  invokes `gh auth login`. A clean-account setup that requires GitHub must also
  supply this executable. No account connection was attempted.

## Unresolved questions and next useful action

Jack chose to avoid an init conversation for newcomer setup; use app actions
for account connection. Specify how `gh` is installed without a package manager
in the installer implementation. Define ownership of the broader local planning change before claiming
Linear is optional across the product. Jack confirmed DMG-first installation;
see [first-launch review](dmg-first-launch-review.md). No Flow navigation
decision is recorded.
