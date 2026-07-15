# 5 Whys: Claude authorization could not complete

## The Problem

`lf auth connect claude --account primary` opened the correct authorization
page, but the Claude CLI could not receive the one-time code returned by that
page, so the managed account login waited forever.

## Chain

Dead login → child stdin was disabled → auth handles modeled output only → the
callback shape was inferred from its URL → tests stopped at URL discovery.

**Problem**: Authorizing in the browser did not complete the managed Claude
account login.
↳ *A live run showed the provider process remained alive after authorization.*

**Why 1**: Anthropic's current CLI asks for the browser's one-time handoff code,
but Loopflow spawned it with stdin set to `/dev/null`.
↳ *Running the installed `claude auth login` directly showed `Paste code here
if prompted >` after the authorization URL.*

**Why 2**: `AuthFlowHandle` owned only a response and completion monitor; it had
no typed way to complete an interactive provider command.
↳ *Loopflow could observe the URL and process exit, but could not participate in
the step between them.*

**Why 3**: The first callback fix treated the absence of a device `user_code`
in the authorization URL as proof that no user input was required.
↳ *The one-time handoff code is produced after authorization, so it cannot
appear in the initial URL.*

**Why 4**: Parser tests proved URL extraction and cancellation, but no test
modeled a provider command that reads a code before writing credentials.
↳ *The test boundary ended at auth startup rather than successful completion.*

**Why 5 (Root)**: Command-backed OAuth was modeled as output parsing plus
waiting instead of a provider-specific conversation with explicit input and
completion requirements.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 1 | Will Anthropic replace the handoff-code flow with a loopback callback? | Low |
| Why 2 | Which non-CLI surfaces need a first-class code-submission endpoint? | Medium |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Give the Claude auth child a private stdin pipe and forward its one-time code | This blocked login |
| Structural | Represent authorization-code input on `AuthFlowHandle` | Output-only command assumptions |
| Systemic | Test a full fake provider conversation: URL, submitted code, successful exit | Future flows that start but cannot complete |

## Changes to Implement

- [x] Add explicit authorization-code input ownership to command auth handles.
- [x] Prompt once in `lf auth connect --account` only when the provider requires it.
- [x] Add a completion test that proves the child receives the code without logging it.
- [ ] Rebuild the compatibility binary and complete real Claude account setup.
