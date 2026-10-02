# Shared provider homes, multiple credentials

Draft plan — 2026-10-02. Jack Heart authorized pursuing this design and settled the product decisions quoted below. Mechanisms marked “chosen” are implementation choices made in planning and remain open to his correction. Slice 1 is implemented on this branch, less its gate fixture; slices 2–4 remain. Delivery is not yet requested. Placement is unresolved; no Wave supplied.

## Problem

Loopflow launches Codex and Claude in a private per-account home, so a conversation started through Loopflow is invisible to the provider's own CLI. Jack: “codex resume doesnt work because our session was in a different codex”. He still needs several stored credentials per provider: “we do need to be able to store multiple credentials”, and accepts the consequence of sharing: “I am ok with it meaning the active user in my manual codex jumps around”.

## Outcome

Loopflow conversations run in the provider's ordinary home by default. Jack's own `codex` and `claude` see the same history and settings as Loopflow, and one command changes which stored account that home is signed in as. A conversation that must stay on one account runs isolated instead.

## The demo

With Codex accounts A and B stored and A active:

```bash
lf -m codex : "say hi"          # runs as A in ~/.codex
codex resume <native-id>        # plain Codex finds that conversation
lf account use codex B          # from now on
lf -m codex : "say hi"          # runs as B; plain `codex` is B too
lf account use codex A --now    # everyone now; A still works after its token rotated
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

## Risk findings

**Codex holds its login for the life of the process; a fresh process takes the native one.** Probe, synthetic credentials in a temporary home, Codex 0.160.0: an app-server started under A still reported A's token after the native `auth.json` was replaced with B's, including after a refresh request; a second process started afterward reported B; the native file still belonged to B. The binary also carries “Skipping auth reload due to account id mismatch” and “…you have since logged out or signed in to another account.” Consequences: activation cannot be clobbered by a running Codex agent; a running shared Codex agent does not move by itself and must be resumed to move.

**Codex's native store is a file here.** `~/.codex/config.toml` sets no `cli_auth_credentials_store` and `~/.codex/auth.json` exists. Activation writes that file. A native config that selects the keyring is an error state below, not a second writer.

**Claude re-reads its login.** Claude Code 2.1.288 caches the Keychain read for 30 seconds and at refresh adopts a stored token that differs from its own; no identity check appears there. Read from the binary, not exercised. A running shared Claude agent follows the native login.

**Claude's native login is two stores.** On macOS it is the Keychain item `Claude Code-credentials`; a non-default config directory uses `Claude Code-credentials-<8 hex of sha256(path)>`, and ten such items exist for the per-account homes. Loopflow already reads both stores (`read_claude_keychain_credential`, `.credentials.json`) and writes the file atomically (`write_claude_profile_credentials`), but has no Keychain writer. Activation needs one; that a subsequently started Claude accepts an item written by `security add-generic-password -U` is the one mechanism still unproven.

**The lock does not need a readiness signal.** The earlier worry was a second launch swapping the credential before a just-started process read it. Chosen instead: install the credential by atomic rename, and hold the lock across activate, spawn and recording the launch as a shared agent. A concurrent switch then sees the new agent and, at “everyone now”, resumes it; at “from now on” either account is a correct outcome. No per-launch-site signal is required.

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
- **Activation** (`provider_account.rs`, new): under the provider credential lock, save the native credential back to the stored profile with the same identity; if it matches none, store it as a new profile; install the selected profile's credential by atomic rename (Codex `auth.json`; Claude Keychain item on macOS, `.credentials.json` elsewhere). Already active: no-op.
- **Launch wrapper** (`provider_account.rs`, new; chosen name `launch_as`): no account named, or the named account already active → spawn. Otherwise lock, activate, spawn, record, unlock, return the child without waiting. Isolated → today's `apply` path unchanged. All five sites call it and nothing else touches account environment.
- **Selection:** in shared mode routing returns the active account while it is eligible. When it is strained or unavailable, routing calls the switch at “everyone now” toward the next eligible account, since every shared agent is on the failing one. In isolated mode selection is unchanged from today.
- **Switch command** (chosen: `lf account use <provider> <account> [--now]`; the account namespace is `lf account`, there is no `lf auth`): the only writer of a shared account change. Bare is “from now on” and is built; `--now` is “everyone now”, which also resumes each running shared Codex agent under the new account, keeping its conversation and native ID, and arrives with slice 3.
- **Modes:** `--isolate` and `--shared` are bare global flags beside `--account` / `--only-account`, mutually exclusive. They travel in `LF_ACCOUNT_ISOLATION` beside `LF_ACCOUNT_SELECTION`, and a Flow invocation captures the mode in an optional `isolate` field beside its captured accounts. Chosen default key: `isolate: true` in `.lf/config.yaml`; config loading already merges `~/.lf/config.yaml` under the repository file, so a Home-level default works too. Resolution: flag, then config, then shared.

**Source of truth and records.** The native credential decides the active account. One draft migration (`shared_provider_homes`) adds: a switch log `provider_account_switches` (provider, account, time, strength, cause: person or exhaustion), and `isolated` on `provider_session_accounts` so resume returns to the home the conversation started in. Rows that predate the change are isolated. Usage attribution reads the switch log and provider-reported identity; a launch's named account is not evidence of what a shared agent later used. Historical usage is never rewritten.

## Provider session IDs in `lf session`

Every `lf session` subcommand that takes an ID (`connect`, `history`, `complete`, `rename`, `bind`) accepts the provider's own session ID wherever it accepts a Loopflow one. The rule Jack set: if `codex resume X` works, `lf session connect X` works; the same for a Claude session ID. It is one half of a pair, in his words “along codex resume X just working for shared lf launches”: a provider ID is good in both tools. A shared Loopflow launch is resumable by the plain provider CLI with that ID, and the same ID opens the conversation through `lf session`.

Today one resolver, `find_session` in `ops/human_session.rs`, accepts a Session ID or a Run ID. It gains a third form: a provider session ID, looked up through the session's recorded provider thread. One resolver, used by every subcommand; no per-command lookup.

Accepted — Jack Heart, 2026-10-02 (“i dont see why not”): the ID also resolves for a conversation Loopflow never started, since shared homes make those resumable by the provider. Connecting to one creates its Session then, attributed to no Task. An ID that matches conversations in more than one provider or home is reported as ambiguous with the candidates named, never guessed. An ID the provider cannot resume is “not found”.

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

1. **Codex shared by default — built, gate fixture outstanding.** In place: activation (`provider_account/activation.rs`), `launch_as` at all five sites (Claude routes stay isolated until slice 2), shared-mode selection, `lf account use` at “from now on”, `--isolate` / `--shared` / `isolate:`, the session mode record, the migration draft, and the subscription, config and reference docs for them. Left in this slice:
   - `tests/e2e/codex_connect.py --shared-provider-home`, the gate's acceptance fixture, against a real Codex binary. Not started.
   - **Quota probes for the active account.** `subscription.rs` and `lf account` still probe every account through its stored profile. While an account is active its live credential is the native one, and the profile copy is only current as of the last switch. If Codex rotates refresh tokens, a probe that refreshes the stale profile copy and the native process refreshing its own would invalidate each other. Launch readiness (`verify_ready`) already reads the native home for the active account; probes need the same rule before this ships. Unproven either way: no live credential was exercised.
   - Shared launches under a forwarded lease or an explicit `--account` list take the first eligible candidate rather than preferring the active account.
   - `lf account route --json` does not report the mode or active account; the text form does.
2. **Claude shared by default.** Keychain writer and activation for Claude, then drop the `provider != Codex` branch in `launch_isolated`. Starts with the fixture proving a started Claude accepts an activated credential, using a temporary config directory and its hashed Keychain item on macOS and the file elsewhere.
3. **“Everyone now” for Codex.** Gated on the research below. Resume each running shared Codex agent under the new account; exhaustion switches use it. Needs a record of running shared agents, which slice 1 does not keep: the credential lock is held across activate and spawn only.
4. Remaining docs: security and environment pages.

## Remaining questions

- **Research before slice 3 (Jack):** “would have to research what aspects of the experience most impact the seamlessness of having your sessions interrupted”. For an attached terminal session and a headless worker separately: what the person sees; whether unsent input, scrollback and the attached client survive; whether an in-flight tool call can run twice; gap length; whether the agent notices; what history shows. Sets whether a mid-turn agent is interrupted at once or moved when its turn ends.
- **Command name:** `lf account use … [--now]` is a planning choice; Jack has not named it.
- **Config levels:** repository and Home `config.yaml` both work; whether a Wave-level default is also wanted is open.
- **“Pinned binary”:** only the account's own home, or more.
- **SSH-forwarded Codex:** the lease exports `CODEX_ACCESS_TOKEN`, which `provider_auth/mod.rs` records as the wrong credential type. Untouched here; whether a remote Home activates a forwarded credential is open.
- **Resume across accounts:** a conversation started under A continuing under B is assumed to work within one workspace; across workspaces or plans is unverified.

## Done when

Jack starts a Codex conversation through Loopflow and finds it with plain `codex resume`; switches the shared account with one command and both Loopflow and plain `codex` follow; switches back and the first account still works; and an isolated conversation stays on its account throughout. The same holds for Claude.

Gate, headless:

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test -p loopflow provider_account` and `cargo test -p loopflow --test agent_tests --test auth_tests --test flow_tests`: pass.
- `uv run --script tests/e2e/codex_connect.py --codex "$(command -v codex)" --lf target/debug/lf --output <dir> --shared-provider-home`: exits 0. This mode does not exist yet. With temporary homes and the fixture's synthetic provider it must show: a Loopflow conversation listed and resumed by plain Codex with no home override; A → B → A with A's rotated refresh token intact in its profile; an unknown native login preserved as a new profile; a launch naming no account, or the active one, changing nothing; no shared launch setting a provider home or credential variable; “from now on” leaving a running shared agent on A and “everyone now” leaving it on B with its native ID unchanged; an `--isolate` launch and a launch under `isolate: true` each staying in their own home across a switch and resuming there; `--shared` under an isolated default running in the native home. And: `lf session connect <native-id>` and `lf session history <native-id>` reaching the same Session as its Loopflow ID, for a conversation Loopflow started and for one started by plain Codex.

Claude's Keychain path is macOS-only; its fixture runs in the macOS CI job and the file path runs everywhere.

## Evidence

Check: `cargo clippy -p loopflow --all-targets -- -D warnings`, `cargo test -p loopflow --lib provider_account`, and `--test agent_tests --test auth_tests --test flow_tests` — pass. Rerun after compress (one shared-route constructor, `provider_session_isolated` replacing the two-query mode lookup, `launch_as_blocking` running `launch_as` on a scoped thread): pass. `store::migrations::tests::remove_ask_preserves_conversations_and_history` fails with and without this branch's draft; not caused here. The e2e gate fixture is owed by slice 1.

Check: Codex credential probe with synthetic credentials in a temporary home (start under A, replace native with B, query auth status on the running and a fresh process) — running process kept A, fresh process took B, native file unchanged; no live credential read or changed, no implementation tests run.

Observed on Jack's machine, 2026-10-02: Codex access-token lifetime 240 hours (lifetime only); about 3,011 Codex rollouts and 315 Claude transcripts in per-account homes against 3,595 and 58 native. Retained but unused: Codex 0.160.0's schema includes external-token login and refresh; [Codex app-server documentation](https://learn.chatgpt.com/docs/app-server); [Claude environment documentation](https://code.claude.com/docs/en/env-vars).
