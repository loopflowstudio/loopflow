# Shared provider homes, multiple credentials

Draft plan — 2026-10-02. Jack Heart authorized pursuing this design and settled the product decisions quoted below. Mechanisms marked “chosen” are implementation choices made in planning and remain open to his correction. Slices 1 (Codex), 2 (Claude) and 4 (docs) are built on this branch and both fixtures pass; slice 3 remains, gated on research, and one finding under Risk findings bears on the demo. Delivery is not yet requested. Placement is unresolved; no Wave supplied.

## Problem

Loopflow launches Codex and Claude in a private per-account home, so a conversation started through Loopflow is invisible to the provider's own CLI. Jack: “codex resume doesnt work because our session was in a different codex”. He still needs several stored credentials per provider: “we do need to be able to store multiple credentials”, and accepts the consequence of sharing: “I am ok with it meaning the active user in my manual codex jumps around”.

## Outcome

Loopflow conversations run in the provider's ordinary home by default. Jack's own `codex` and `claude` see the same history and settings as Loopflow, and one command changes which stored account that home is signed in as. A conversation that must stay on one account runs isolated instead.

## The demo

With Codex accounts A and B stored and A active:

```bash
lf -m codex : "say hi"          # runs as A in ~/.codex
codex resume <native-id>        # plain Codex finds that conversation
lf account codex use B          # from now on
lf -m codex : "say hi"          # runs as B; plain `codex` is B too
lf account codex use A --now    # everyone now; A still works after its token rotated
lf --account codex=B --isolate -m codex : "say hi"   # stays on B in its own home
```

Then the same with Claude. Manual resume never inherits Loopflow process or Flow authority.

## Accepted decisions (Jack Heart, 2026-10-02)

1. **One owner of account management.** “1 launch path doing account management is good”, “For codex and claude (and opencode ...)”. Each provider has one place that selects an account and supplies its credential.
2. **One active account at a time.** “I think eventually two accounts at time is desirable -- it is not essential right now”.
3. **Shared model for every provider.** “i think Claude kinda forces our hand on this model though”. A switch moves the provider, not one launch.
4. **Two metaphors.** “i think we should try to do as much as we can through these metaphors. i do imagine needing to like move all my open workers to a new account. I think maybe if isolated, dont move, keep it on and use its own backup plan”. Shared agents move together; isolated agents stay and use their own fallback.
5. **Isolated is the current model, demoted.** “something like the current model can still work if we ever do really need like persistently on a specific account agents”; “maybe we dont delete, we just demote the capability”; “there is an extra flag … i meant we would add it”; “--isolate?”.
6. **Isolation is the only pinning mechanism** (“yes”), following “just one API thats like "enterprise mode" basically rather than have a bunch of one-off specifi mechanisms”. Codex external-token auth and Claude's environment token are dropped.
7. **Isolation is set both per launch and as a standing default** (“both”), with `--shared` as the per-launch override.
8. **One command owns the switch, at two strengths.** “theres gradation between "start using this for the future" and "migrate everyone now" and something should do both those tings and be the only thing that does those things”. The strengths are intents: “the exact details and how good we are at each is irrelevant to the intent expressed”. That “from now on” also moves running Claude agents is “fine, its just a detail”.
9. **“Everyone now” moves Codex agents immediately.** “so move all shared to X that also migrates the codex agents immediately”.
10. **Launch wrapper.** “some sort of decorator/handler that basically locked the active account and launched [didnt wait] the lf and then return”, “does nothing if the right account is already active”, “or if no account was specified”.
11. **Old history is not migrated.** “Dont bother migrating”.
12. **`lf session` accepts provider session IDs.** “I want to take this opportunity to make sure the lf session command all accept the session IDs from the providers”; “so if codex resume X works than so should lf session resume X or whatever it is”.

## Runtime ownership (design discussion, 2026-10-03)

**Accepted behavior — Jack Heart:** ordinary interactive exit closes the live
runtime by default. If Desktop or `lf session` takes over the conversation, the
old interface's exit must leave it running under its new owner. Jack also
proposed closing after batch invocation exit and after interactive Flow steps
whose exit advances the Flow. The close-on-exit path is now implemented; remaining limits and proof are below.

The durable conversation, a provider turn, the current driver, and the engine
have separate lifetimes. Closing the runtime preserves saved history; it does
not by itself mean completing a Task or settling a Flow. A handoff transfers
control of the same live conversation rather than completing it.

**Implementation:** `session_record::finish_session_driver` closes the recorded
Codex app-server through a callback inside `SqliteStore::finish_session_driver`'s
existing driver transaction. That same transaction records exit and clears the
live endpoint, preserving thread identity. Transfer makes old-driver exit stale;
if close wins, connect falls through to saved-history resume. Client replacement
claims before stopping old clients. Batch completion, connected UI exit and
cooperative interruption use this path; harness teardown only drops its connection.
There is no surface-specific retention flag or second ownership record.

**Limits:** engine shutdown checks native loaded threads and preserves separately
started conversations with a close error; provider subagents belong to their
parent's runtime. Shutdown verifies PID/start and group ownership, sends SIGTERM,
then SIGKILL if needed. Shared-engine thread release remains unresolved. SIGKILL
cannot run cleanup; existing recovery still handles a surviving engine. Rendered
Desktop/TUI continuity and closing during a side-effecting tool remain unproven.

**Proof:** the native shared-home fixture now resumes after batch exit without
manual engine cleanup. The native public-connect fixture covers headless-to-UI
handoff, stale-client write exclusion, replacement while a sibling runs, and
current-owner UI exit closing a single-conversation engine. Clients are controlled
protocol fixtures, not rendered Desktop/TUI. Store tests prove current-owner
shutdown and stale-owner preservation. Provider-history resumability remains
distinct from Loopflow's refusal to connect some completed Sessions.

## Risk findings

**Codex holds its login for the life of the process; a fresh process takes the native one.** Probe, synthetic credentials in a temporary home, Codex 0.160.0: an app-server started under A still reported A's token after the native `auth.json` was replaced with B's, including after a refresh request; a second process started afterward reported B; the native file still belonged to B. The binary also carries “Skipping auth reload due to account id mismatch” and “…you have since logged out or signed in to another account.” Consequences: activation cannot be clobbered by a running Codex agent; a running shared Codex agent does not move by itself and must be resumed to move.

**Changing the account in `auth.json` breaks one turn of a running Codex.** Reproduced 2026-10-04 without Loopflow, Codex 0.160.0, synthetic logins and a local provider (`scratch/codex_auth_switch_repro.py <fixture> <runs> <variants>`): an app-server holds a turn at the provider while its home's `auth.json` changes, six runs per case.

| Change while a turn is in flight | Turn outcome |
|---|---|
| none; same bytes rewritten; same account with a rotated refresh token | completed, 36 of 36 |
| a different account, by atomic rename or in place, released at once | completed, 12 of 12 |
| a different account, released 0.5 s later | failed 6 of 12: “Fatal error: application network permission was revoked” |
| file deleted | failed 5 of 12, same error or “application network policy is unavailable” |

The damage is one turn, not the engine: when the in-flight turn survives, the next turn on that thread usually fails instead (8 of 12), and when it fails, the next completes. An idle engine switched and left for two seconds ran two turns cleanly, 12 of 12. So Codex notices the identity change within about a second and drops something network-related that the next request then trips over. This is why the shared-home fixture's “from now on leaves a running shared agent alone” step now fails intermittently, and it corrects the finding above: a running Codex keeps its login, but a turn in flight across the switch is not safe. Built: a headless run classifies that error as `AgentFailure::AccountSwitched` and the existing transient-retry loop resumes the same conversation after two seconds, on a fresh engine that reads the new login. Holding the switch until turns end was not built: a Codex turn can run for an hour, the login file is shared by every agent, and a held switch would block all new turns behind the longest one. Still to do: an interactive conversation's turn fails visibly and the person resends. Decided by Jack Heart, 2026-10-04, tracked as LOO-374: do not interrupt a turn in flight (“either is fine. slight preference for dont interrupt”), and avoid the failure rather than retry it if the same mechanism makes that clean (“avoid it if its clean with the above, but whatever is fine”). One mechanism then serves this and “everyone now”: hold the change, or each agent's move, until its turn ends. Waiting alone is not enough, since the next turn usually fails instead; the agent must be resumed or given a moment first.

**Codex's native store is a file here.** `~/.codex/config.toml` sets no `cli_auth_credentials_store` and `~/.codex/auth.json` exists. Activation writes that file. A native config that selects the keyring is an error state below, not a second writer.

**Claude re-reads its login.** Claude Code 2.1.288 caches the Keychain read for 30 seconds and at refresh adopts a stored token that differs from its own; no identity check appears there. Read from the binary, not exercised. A running shared Claude agent follows the native login.

**A Claude started after a switch accepts the installed login.** Proven with Claude Code 2.1.288, synthetic logins and a local endpoint: after `security add-generic-password -U` wrote the hashed item `Claude Code-credentials-<8 hex of sha256(path)>` for a temporary config directory, the next `claude -p` sent that login's bearer; the same for `.credentials.json`. Where both exist on macOS the Keychain item wins. The default home's unsuffixed item was not written by any check.

**A Claude login names no person.** Unlike Codex's, it carries no identity, so “which account is active” is answered offline only while the native login still shares a token with a stored profile. After Claude refreshes it, Loopflow asks Claude's profile endpoint once, under the credential lock, and copies the login into that account's profile (`observe_active_account`). That request refreshes an expired native login in place, as Claude would. If it cannot be asked, routing sees no active account and a switch keeps the login as `accounts/claude/native-<digest>.credentials.json`.

**The lock does not need a readiness signal.** The earlier worry was a second launch swapping the credential before a just-started process read it. Chosen instead: install the credential by atomic rename, and hold the lock across activate and spawn; at “from now on” either account is then a correct outcome for a concurrent launch, and no per-launch-site signal is required. Built that far. Holding it across recording the launch as a shared agent too, so a concurrent “everyone now” sees and resumes the new agent, waits on slice 3's record of running shared agents.

**Plain Codex cannot resume a conversation whose Loopflow engine is still alive.** Observed with Codex 0.160.0 through `app-server` (`thread/resume` from a second process): “thread … already has an active writer”. The base retained that engine after a finished headless run, blocking plain resume; while it lives, `lf session connect <native-id>` is the way in. Whether the Codex terminal `codex resume` is refused the same way was not exercised. Normal owner exit now closes the engine; transfer preserves it. The native fixture no longer manually stops finished batch engines before plain resume.

**An isolated launch on the active account shares its token lineage.** Activation copies one login into the native home, so the native copy and the stored profile descend from the same refresh token. Probes now read the native home for the active account, but an `--isolate` launch on that same account still runs on the profile copy. If Codex rotates refresh tokens, the two can invalidate each other. Unproven either way; no live credential was exercised.

**Identity is already observable.** `provider_account/identity.rs` derives an account's identity from the credential in a home (`codex_identity_from_home`, `check_current_identity`). Pointing it at the native home answers “which account is active” without a remembered choice.

## Alternatives considered

- **Per-engine external tokens for Codex, environment token for Claude.** Gives side-by-side accounts in one home. Rejected by decision 6: two provider-specific mechanisms, an experimental API, a ten-second refresh callback, and no refresh at all for Claude.
- **Symlinking the session store between homes.** Shares history but federates live SQLite files across homes. Forbidden below.
- **Swapping the credential with no saved-back copy.** Loses rotated refresh tokens, leaving stored profiles dead after one switch. Activation saves back first.

## Target architecture

**Before.** Selecting an account means launching in that account's home. `ProviderAccountRoute::apply` sets `CODEX_HOME` / `CLAUDE_CONFIG_DIR`; five sites call it (`harness/codex.rs`, `harness/claude.rs` twice, `lf/commands/util.rs`, `engine/agent.rs`); three sites add `cli_auth_credentials_store="file"`; `ensure_account_home_at` mirrors settings. `select_provider_account` may choose a different account for every launch.

**After.**

- **Stored profile:** the existing per-account directory. Holds one credential for login, refresh and quota probes. Also the runtime home of an isolated conversation.
- **Native home:** the provider default, or the caller's explicit `CODEX_HOME` / `CLAUDE_CONFIG_DIR`. Resolved once at admission from the launch's own environment so a nested launch never mistakes an inherited isolated home for the native one.
- **Active account:** whichever stored account the native credential identifies. Source of truth is the native credential; nothing remembers it separately.
- **Activation** (`provider_account/activation.rs`): under the provider credential lock, save the native credential back to the stored profile with the same identity; if it matches none, store it as a new profile; install the selected profile's credential by atomic rename (Codex `auth.json`; Claude Keychain item on macOS, `.credentials.json` elsewhere). Already active: no-op.
- **Launch wrapper** (`ProviderAccountRoute::launch_as`, chosen name): no account named, or the named account already active → spawn. Otherwise lock, activate, spawn, record, unlock, return the child without waiting. Built without “record”: the lock is held across activate and spawn only, and recording the launch as a running shared agent arrives with slice 3. Isolated → today's `apply` path unchanged. All five sites call it and nothing else touches account environment.
- **Selection:** in shared mode routing returns the active account while it is eligible. When it is strained or unavailable, routing calls the switch at “everyone now” toward the next eligible account, since every shared agent is on the failing one; until slice 3 it switches at “from now on”. In isolated mode selection is unchanged from today.
- **Switch command** (`lf account <provider> use <account> [--now]`, named by Jack Heart 2026-10-03: “it should be lf account codex use jack@loopflow.studio”): the only writer of a shared account change. Bare is “from now on” and is built; `--now` is “everyone now”, which also resumes each running shared Codex agent under the new account, keeping its conversation and native ID, and arrives with slice 3.
- **Two independent choices per launch** (Jack Heart, 2026-10-03/04: “why do we need so many ways, not just 2?”; “I prefer it as only an environment variable”; remote is “orthogonal to shared vs isoalted. I could want to do remote with either”). `RouteHome` is where the conversation runs: `Shared` or `Isolated`. `AccountLogin` is who holds the login: `Stored` (this Home's profile), `Replayed` (a recorded Run's catalog, read-only, records nothing) or `Lent` (an `lf ssh` origin's access token, environment only, never written on the target). A lent account runs shared in the target's ordinary home or isolated in `accounts/<provider>/forwarded-<account>`.
- **Modes:** `--isolate` and `--shared` are bare global flags beside `--account` / `--only-account`, mutually exclusive. They travel in `LF_ACCOUNT_ISOLATION` beside `LF_ACCOUNT_SELECTION`, and a Flow invocation captures the mode in an optional `isolate` field beside its captured accounts. Chosen default key: `isolate: true` in `.lf/config.yaml`; config loading already merges `~/.lf/config.yaml` under the repository file, so a Home-level default works too. Resolution: flag, then config, then shared.

**Source of truth and records.** The native credential decides the active account. One draft migration (`shared_provider_homes`) adds: a switch log `provider_account_switches` (provider, account, time, strength, cause: person or exhaustion), and `isolated` on `provider_session_accounts` so resume returns to the home the conversation started in. Rows that predate the change are isolated. Usage attribution reads the switch log and provider-reported identity; a launch's named account is not evidence of what a shared agent later used. Historical usage is never rewritten.

## Provider session IDs in `lf session`

Every `lf session` subcommand that takes an ID (`connect`, `history`, `complete`, `rename`, `bind`) accepts the provider's own session ID wherever it accepts a Loopflow one. The rule Jack set: if `codex resume X` works, `lf session connect X` works; the same for a Claude session ID. It is one half of a pair, in his words “along codex resume X just working for shared lf launches”: a provider ID is good in both tools. A shared Loopflow launch is resumable by the plain provider CLI with that ID, and the same ID opens the conversation through `lf session`.

Built. `find_session` (`ops/human_session.rs`) and `session_by_id`, which `history` and `bind` use, fall through to the provider form: the Session whose recorded provider conversation is that ID (`ops/human_session/provider_conversation.rs`). Two Sessions recording one ID are reported as ambiguous, naming both.

Accepted — Jack Heart, 2026-10-02 (“i dont see why not”): the ID also resolves for a conversation Loopflow never started, since shared homes make those resumable by the provider. Connecting to one creates its Session then, attributed to no Task. An ID that matches conversations in more than one provider or home is reported as ambiguous with the candidates named, never guessed. An ID the provider cannot resume is “not found”.

As built: only `connect` admits; the other subcommands report an unconnected provider conversation as not found. Homes searched are each provider's native home and every stored account's own; a conversation found only in an account's home is recorded as isolated to that account, so it opens there. Only UUID-shaped IDs are searched for, so a Codex thread name does not resolve.

## Affected surfaces

`lf` global flags and their forwarding over `lf ssh`; `lf account` (new `use`; `route` explains the active account and mode); `lf session connect` / resume, which must return an isolated conversation to its home and a shared one to the native home; quota probes in `subscription.rs`, which keep reading stored profiles; SSH account leases, unchanged in this plan; `docs/` subscription, security and environment pages; Desktop only through existing account status readers. No wire DTO changes are planned; if account status gains a mode or active field, it is required-or-Optional with a fixture.

## Absent and error states

- No account named and none stored: launch on the native login, exactly as an unmanaged launch today.
- Native credential missing: activation installs the selected profile; nothing to save back.
- Native credential belongs to no stored account: stored as a new profile before anything is installed. Activation never discards a credential it cannot place.
- Selected profile has no usable credential: the switch fails before touching the native store and names the reconnect command.
- Native Codex config selects the keyring: the switch fails with that reason; it does not write a file Codex will not read.
- Lock contention: the launch waits for the lock rather than failing; a switch in progress is short.
- A running shared Codex agent the move could not reach: reported per agent; it fails at its next refresh with a recoverable account error and resumes under the active account.
- Conversations that predate this change: opened against the home they were captured in.

## Exclusions

Migrating old per-account history. Side-by-side accounts in one home. A pinned provider or `lf` executable (what “pinned binary” covers beyond the account's own home is open). opencode account selection; when added it enters through activation. Changing SSH lease forwarding. Wider `lf auth` CLI redesign.

## Demote, and what goes

Kept, reached only by isolated launches: `ProviderAccountRoute::apply`'s home redirection, the `cli_auth_credentials_store="file"` override, `ensure_account_home_at` mirroring, and their tests, which become isolated-mode coverage (`native_routes_select_independent_provider_homes`, the `CODEX_HOME` cases in `agent_tests.rs` and `flow_tests.rs`).

Gone: the unconditional call to that path at the five sites; `uses_native_home` as a launch-site question; per-launch account choice in shared mode.

Must survive: stored profiles and `acquire_managed_login_lock`, identity checks, quota observation and strain ordering, session account records, SSH leases, clearing ambient provider credentials on isolated launches.

## Forbidden outcomes

Session-store symlinks or a history synchronizer between homes. A second place that changes the shared account. A provider-specific option on the switch command. Credential environment variables on shared launches. A remembered “active account” that can disagree with the native credential. Rewritten historical usage. API billing substituted for subscription auth.

## Internal slices

One Task, complete end state above. Each slice leaves the tree working.

1. **Codex shared by default — built.** Activation (`provider_account/activation.rs`), `launch_as` at all five sites, shared-mode selection, `lf account <provider> use` at “from now on”, `--isolate` / `--shared` / `isolate:`, the session mode record, the migration draft, provider session IDs in `lf session`, and the gate fixture. Also:
   - Quota probes, reset redemption, launch readiness and lease credentials read the native home for the active account (`activation::credential_home`).
   - A shared conversation resumes under whichever account is active; the account it began under is not a pin. Before this, reconnecting one switched the native home back.
   - A launch naming several accounts stays on the active one it includes; naming only another account moves to it.
   - `lf account route --json` reports `isolated` and `active_account` (fixture `auth_routes.json`).
   - Switch-log reader: `provider_account_switched_since`, read by `ProviderAccountRoute::used_account`. Rate-limit and credential-health observations from a shared agent go to the account the log says it moved to, only for providers whose running agents follow the native login (`running_agents_follow_native_login`: Claude). For Codex the launch's account stays correct until a process is resumed. No per-account usage report exists to redirect; these observations are the account attribution there is.
2. **Claude shared by default — built.** One reader and writer for a Claude home's login (`provider_auth::read_claude_login` / `write_claude_login`: Keychain for a native home on macOS, written through `security -i` on standard input; the file for stored profiles, other platforms and unit tests). Activation, `active_account`, `credential_home` and `identity::same_login` take the provider. Routing, live status and activation place a refreshed login through `observe_active_account`. Quota polls, identity checks and token preparation read the native store for the active account. Integration test support points `CLAUDE_CONFIG_DIR` at a temporary directory.
3. **“Everyone now” for Codex — mostly unnecessary, remainder not built.** Headless agents already move without it: a run is one engine, closed on exit, so the next launch reads the new login, and a turn that fails across the switch is resumed on the new account. What remains is an interactive conversation that stays open, whose engine keeps the old login until it is closed and reopened. Moving it means replacing an engine under a live interface between turns (`claim_session_driver` with `replace_provider`, then `thread/resume`), which was not built because it cannot be exercised headlessly. Original scope: Resume each running shared Codex agent under the new account; exhaustion switches use it (today a strained active account makes routing activate the next one at “from now on”, logged with cause `exhaustion`). Needs a record of running shared agents, which slice 1 does not keep: the credential lock is held across activate and spawn only.
4. **Docs — built.** `docs/subscriptions.md` and `docs/security.md` (where logins live, what a provider child's environment carries, `LF_ACCOUNT_ISOLATION`); `TESTING.md` names the Claude fixture. There is no separate environment page.

Owed besides slice 3, from “Done when”: a Loopflow-started Claude conversation found by plain `claude --resume`, and a Claude conversation ID opening through `lf session`. Both are built on the shared path and neither has been exercised.

## Remaining questions

- **Slice 3 timing — settled (Jack Heart, 2026-10-04):** a mid-turn agent is moved when its turn ends, not interrupted. The research below is no longer a gate; its remaining questions are detail.
- **Research before slice 3 (Jack):** “would have to research what aspects of the experience most impact the seamlessness of having your sessions interrupted”. For an attached terminal session and a headless worker separately: what the person sees; whether unsent input, scrollback and the attached client survive; whether an in-flight tool call can run twice; gap length; whether the agent notices; what history shows. Sets whether a mid-turn agent is interrupted at once or moved when its turn ends.
- **Config levels:** repository and Home `config.yaml` both work; whether a Wave-level default is also wanted is open.
- **“Pinned binary”:** only the account's own home, or more.
- **Lent Codex logins — built, unexercised end to end.** The Codex harness signs its engine in with `account/login/start` type `chatgptAuthTokens` (`ProviderAccountRoute::engine_login`) and answers `account/chatgptAuthTokens/refresh` through a new broker `Renew` operation; `CODEX_ACCESS_TOKEN` is no longer set. Synthetic probe, Codex 0.160.0: the login is accepted only with the `experimentalApi` capability, is held in memory, writes no `auth.json`, and a fresh process sees no login. OpenAI's schema labels this login “[UNSTABLE] FOR OPENAI INTERNAL USE ONLY - DO NOT USE”. Launches that start Codex directly (`lf/commands/util.rs`, `engine/agent.rs`) refuse a lent Codex account. No real token or `lf ssh` session was used; attached terminal clients on such an engine are untested.
- **Lent conversations and resume:** the origin's session pin carries no shared/isolated mode, so a lent isolated conversation resumed under a shared default looks in the wrong home.
- **Resume across accounts:** a conversation started under A continuing under B is assumed to work within one workspace; across workspaces or plans is unverified.

## Done when

Jack starts a Codex conversation through Loopflow and finds it with plain `codex resume`; switches the shared account with one command and both Loopflow and plain `codex` follow; switches back and the first account still works; and an isolated conversation stays on its account throughout. The same holds for Claude.

Gate, headless:

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test -p loopflow provider_account` and `cargo test -p loopflow --test agent_tests --test auth_tests --test flow_tests`: pass.
- `uv run --script tests/e2e/codex_connect.py --codex "$(command -v codex)" --lf target/debug/lf --output <dir> --shared-provider-home`: exits 0. With temporary homes and the fixture's synthetic provider it shows: a Loopflow conversation listed and resumed by plain Codex with no home override, immediately after batch exit closes its engine; A → B → A with A's rotated refresh token intact in its profile; an unknown native login preserved as a new profile; a launch naming no account, or the active one, changing nothing; no shared launch setting a provider home or credential variable; “from now on” leaving a running shared agent's engine running; an `--isolate` launch and a launch under `isolate: true` each running in their account's own home and recorded as isolated, and a conversation living in an account's home reopening there after a switch; `--shared` under an isolated default running in the native home; and `lf session connect`, `history` and `rename` with a native ID reaching the same Session as its Loopflow ID, for a conversation Loopflow started and for one started by plain Codex.
- Owed to slice 3: “everyone now” leaving a running shared agent on B with its native ID unchanged.
- Not shown by the fixture: a Loopflow-started isolated conversation reopening in its home. A finished headless conversation's Session is complete and `connect` refuses it under either ID, and a terminal conversation cannot run headless. `isolated_launch_and_its_conversation_stay_in_the_account_home` covers that routing.

- `uv run --script tests/e2e/claude_shared_home.py --claude "&#36;(command -v claude)" --lf target/debug/lf --output <dir>`: exits 0. With a temporary config directory, synthetic logins and a local endpoint it shows: plain Claude sending A, then B, then A's login across switches; a switch to the active account changing nothing; a shared launch, naming no account or the active one, getting no account home or credential variable and logging no switch; a launch naming B moving plain Claude to B; and an `--isolate` launch running in its account's home without touching the native login.
- Not shown by the Claude fixture: a rotated login placed by asking Claude, and an unknown login kept as a new profile, since both need Claude's real profile endpoint. `claude_switching_away_and_back_keeps_a_login_the_provider_rotated` and `an_unknown_native_claude_login_is_kept_as_a_new_profile` cover them against a local endpoint. Claude conversation IDs in `lf session` and plain `claude --resume` of a Loopflow conversation were not exercised.

Claude's Keychain path is macOS-only and the fixture uses it there, the file elsewhere; neither fixture runs in CI today.

## Evidence

Check, owner exit: `cargo fmt` and `cargo clippy --all-targets -- -D warnings` clean; Session event tests 12 passed, Exec ownership 11 passed/2 ignored, lifecycle 18 passed/1 ignored before stopping a Keychain-blocked fixture; the repaired fixture and captured-isolation retry then passed separately. Both native Codex fixtures (`--shared-provider-home`, `--launch --public-connect`) passed with synthetic credentials; rendered Desktop/TUI and Claude resume remain unverified.

Review repair: direct Flow retry now forwards captured isolation. Its fixture simulates an empty Keychain and uses an isolated profile, avoiding a macOS prompt; the first retry exposed the missing propagation.

Check, realign at `5ccba23fb`: named functions, tests, the five `launch_as` sites, both fixtures and the docs read against the tree — all present; nothing rerun, the tree is unchanged since the compress check below.

Check, compress: `cargo test -p loopflow --lib -- provider_account` — 49 passed; `cargo clippy -p loopflow --all-targets -- -D warnings` — clean. No behavior change; both fixtures and the integration suites stay owed to gate.

Check, slice 2: `cargo clippy -p loopflow --all-targets -- -D warnings` — clean; `cargo test -p loopflow --lib -- provider_account subscription identity account_status profile session_launch provider_auth` — 208 passed; `uv run --script tests/e2e/claude_shared_home.py …` against Claude Code 2.1.288 on macOS (Keychain store) — exits 0. `agent_tests` and `auth_tests` passed before the last Claude edits; `flow_tests` and the Codex fixture were not rerun; owed to gate. A full `--lib` run inside a Task Session fails two `lf::commands::run` tests on the ambient Flow environment; they pass with it removed.

Check, at `46eaa2bd4` merged with `main` v0.12.31: `cargo test -p loopflow --lib -- provider_conversation human_session provider_account subscription profile` — 100 passed; `uv run --script tests/e2e/codex_connect.py … --shared-provider-home` against Codex 0.160.0 — exits 0. Clippy, `agent_tests`, `auth_tests` last passed before that routing refactor and merge, `flow_tests` before the earlier routing change; all four owed to gate. `store::migrations::tests::remove_ask_preserves_conversations_and_history` failed with and without this branch's draft when last run; not caused here.

Check: Codex credential probe with synthetic credentials in a temporary home (start under A, replace native with B, query auth status on the running and a fresh process) — running process kept A, fresh process took B, native file unchanged; no live credential read or changed.

Public-connect now uses `--mode batch` and writes long-running command diagnostics to files so an undrained pipe cannot block startup. Its proof has been run. Older Flow fixture modes still pass the rejected `-b`; they remain outside this lifecycle proof. `docs/lf-reference.md` was edited by hand: no command that emits the Clap catalog for `scripts/generate_cli_reference.py` was found.

Observed on Jack's machine, 2026-10-02: Codex access-token lifetime 240 hours (lifetime only); about 3,011 Codex rollouts and 315 Claude transcripts in per-account homes against 3,595 and 58 native. Retained but unused: Codex 0.160.0's schema includes external-token login and refresh; [Codex app-server documentation](https://learn.chatgpt.com/docs/app-server); [Claude environment documentation](https://code.claude.com/docs/en/env-vars).
