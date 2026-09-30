# v0.12.26

v0.12.26 makes it easier to inspect what Loopflow will run and which managed login it will use. Local command, skill, and flow discovery explains available invocations before execution, while account identity checks catch mismatched logins before connecting or routing Codex work. Upgrade for clearer command selection and more reliable managed-account status, with two changes required for existing scripts and custom flows.

## Find the invocation before starting work

Discovery now exposes command ownership and definition sources without launching agents, connecting accounts, or fetching definitions. Selection follows the same rules in CLI execution and flow composition.

- Use `lf list`, `lf help NAME`, and `lf help --all` to find definitions, inspect their sources, and see usable invocations.
- Unique shortcuts resolve to canonical commands: `lf land` selects `lf pr land`. Ambiguous names report the available canonical choices; commands retain precedence over definitions.
- `lf run NAME` runs a definition. Untyped lookup prefers flows over same-named skills; `lf skill NAME` and `lf flow NAME` select only the requested kind. A malformed flow reports an error instead of silently falling back to a skill.
- Saved execution retains captured instructions and the existing saved-plan encoding for resume.

## Keep managed work on the intended login

A configured email could previously appear verified while its credential belonged to another login. Managed-account checks now compare observed identity before connecting, accepting verified usage, or routing Codex work, and provide recovery commands when identities disagree.

- Codex identity checks compare the reported email with its native credential and per-user identity. Claude connect/import and explicit verification use the provider profile’s email and UUID.
- Duplicate managed logins and misleading email changes are rejected. Different people sharing a workspace remain valid, and staged reconnect keeps the installed credential while authorization waits.
- Observed identity and plan persist without resetting configuration or cooldowns. Temporary verification outages preserve cached credential health; provider rejection marks credentials missing.
- Automatic Codex routing prefers observed Pro plans over Plus among healthy candidates while preserving explicit preferences and Session pins.
- `lf auth status codex` stays read-only and shows observed login and recovery guidance. Add `--verify` to request current identity and usage. Usage displays as used/left, and expired windows display as unknown.

## Operational notes

- **Script and flow migration:** replace `lf catalog` with `lf list`, and replace authored YAML `op:` steps with `cmd:`. Bundled flows are migrated. Review same-named skills and flows because untyped lookup now selects the flow.
- **Account status scope:** state remains local to each Home. Ordinary status does not refresh provider observations; JSON retains dated usage observations, so consumers must check reset timestamps. Shared account state, current status by default, sole browser ownership, and Claude cached identity/routing remain follow-up work. Native provider credential writers do not participate in Loopflow’s installation lock.
- **Validation limits:** account identity proofs use synthetic fixtures; live OAuth and deployed acceptance remain unverified. The supplied account-change record reports passing isolated Rust and website suites, formatting, Clippy, architecture, migration-history, and fresh-Home JSON checks. An earlier gate failed after inherited executable selection launched real Claude; that group was stopped and isolated checks subsequently passed. `TESTING.md` records the required isolation.
- **Documentation previews:** proposed command-owner moves and overview commands are labeled designs, not shipped functionality.

## Small changes

- The new `pr-review` skill produces HTML code walkthroughs.
- CLI documentation separates the workflow guide from the command reference.
- External skills use consistent frontmatter parsing when fetched and when read from cache.
