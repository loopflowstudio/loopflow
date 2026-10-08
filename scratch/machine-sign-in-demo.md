# Machine sign-in demo — 2026-10-08

Jack Heart subsequently accepted the reported result with “sounds good. advance
the task.” The Task's saved `ship` edge now owns gate and delivery. Earlier
no-landing statements below describe the demo's boundary before that request;
remaining evidence limits are unchanged.

LOO-413 aims to sign an added machine in from a laptop with at most one browser
approval per account and no terminal on the target. Current design:
[Sign a machine in as me](work-on-another-machine-name.md).

## Observed result

**Real Codex transfer and remote launch passed for `jackstah@gmail.com`.** After
Jack Heart approved the new browser prompt, `machine connect MINI codex
jackstah@gmail.com --chrome-profile 'Profile 3'` exited successfully with
`Connected codex on MINI`. The Mini registered one account and retained its
native credential in a private `0600` file inside the source deployment.

Both the original laptop login and Mini's resident login then passed
authenticated requests. The original remained signed in. An account-selected
remote agent launch returned `MINI_LOGIN_OK` and exited zero:

```sh
lf --machine MINI --account codex=jackstah@gmail.com -m codex -b \
  --max-turns 1 --no-loopflow : \
  'Reply exactly MINI_LOGIN_OK. Do not use tools, run commands, read files, or change anything.'
```

These commands run through `uv run python <local-demo>/run_lf.py`, the explicit
source deployment selector described below. A subsequent headless
`-b machine connect MINI codex jackstah@gmail.com` returned zero without another
login prompt, exercising reuse of the existing account. Public Session inventory
with `--interactive false --history` retains the completed remote Run as
`session_da337fbee55e4e269f47dc6391261abe`, on the registered Mini Machine.

The checks used native Codex app-server `account/read` with `refreshToken: true`
and `account/rateLimits/read`, verified the returned email, and required a
successful rate-limit response. These establish authenticated access at the time
of the check. They do not establish that an OAuth refresh exchange occurred or
that both refresh chains remain independent on later days.

Jack Heart authorized this real-account check with “i give you the permission to
do that,” superseding the October 7 fixture-only restriction for this demo. Jack
accepted the proposed Codex account and corrected the target name to `MINI`.
Jack said “approved” after each of two distinct demonstrations: the earlier
laptop-only check and the later real transfer. Each started one browser login
flow; total browser clicks were not independently counted. Jack used no remote
terminal. The later approval completed the actual remote credential installation.

## Source deployment

The installed-version blocker below is superseded for the source demo: Jack
Heart authorized updating the Mini and explicitly permitted building there or
shipping the binary. `cargo build -p loopflow --bin lf` passed. The source build
is now deployed at `/Users/jack/loo-413-demo.VvtZ13/bin/lf` on `mini-heart` with
its own `machine/` directory. The existing runtime and data remain unchanged;
the branch's four unreleased migration drafts were not applied to them.

After `scp` completed successfully, local and remote SHA-256 matched:
`edac83a2c4b0049271ea44e878de154b44c7f5ed835509729157d7371f5a841a`.
An earlier probe ran before transfer completion and returned a partial hash and
exit 255; that discarded probe supplies no acceptance. The completed binary
reports `0.13.9+178abab13.dirty` and initialized remote Machine
`home_bec7f530aaa05dc286c44bab4e592c9a`. The dirty source changes are review notes.
The Mini's `/Users/jack/src/loopflow` directory exists.

The local demo's `run_lf.py` selects its isolated Machine. A scoped
`ssh_transport.py` delegates to real `/usr/bin/ssh`, retaining strict host-key
checks and setting the remote source PATH, `LF_BIN`, `LF_HOME` and token-key
path. It changes deployment context only; provider responses and credential
stdin are real. This is an explicit source deployment, not acceptance of the
default installed command or existing remote Machine. Both peers use identical
bytes. The version warning nevertheless compares the remote build version with
the local package version and incorrectly reports a difference.

Public `machine add mini-heart --label MINI --repo src/loopflow --json` now
succeeds. `--machine MINI machine id` returns the registered identity. Public
`machine connect MINI codex jackstah@gmail.com --chrome-profile 'Profile 3'`
started a fresh native browser login, completed after Jack's second approval,
and installed its credential through SSH stdin. The earlier laptop-only approval
was a separate browser transaction. No credential bytes appeared in output.

### Earlier installed-version failure

The installed laptop and Mini CLIs both report `0.13.9` and lack `machine`
commands. The branch CLI reports `0.13.9+715404677`; it was run with cleared
inherited Loopflow authority, pinned `LF_BIN`, and an isolated `LF_HOME` and
token-key path. Neither installed store was opened by the branch CLI.

Both `mini` and `MINI` failed DNS resolution. Tailscale identifies the online
machine as `mini heart`, with DNS name `mini-heart.tail0eda02.ts.net` and address
`100.96.227.95`. Its short name `mini-heart` and IP already have matching trusted
SSH keys on the laptop. The FQDN lacked a saved host key; that attempt stopped
without accepting one. Using the trusted short name reached the Mini.

`lf -b machine add mini-heart --label MINI --repo src/loopflow --json` failed
because the Mini's installed CLI cannot answer `lf machine id`. A direct SSH
inspection confirmed `lf 0.13.9`, arm64 and `/opt/homebrew/bin/codex`. The proposed
repository path was never saved or validated. No machine was registered and no
credentials were sent. No replacement binary was installed on the Mini.

To exercise the authorized first hand check despite that blocker, the second
login used the branch's public command in the isolated local Machine:

```sh
lf account connect codex jackstah@gmail.com --chrome-profile 'Profile 3'
```

This exercises the shared native login and identity verification path. It does
not exercise `lf machine connect`, SSH credential receipt or a remote agent
launch. The registered new account directory resolves inside the isolated demo
directory; its `auth.json` permissions are `0600`. The initial lexical path check
returned false because macOS resolved `/var` to `/private/var`; canonical path
comparison confirmed isolation. No credential bytes were printed or exported.

## Evidence and next action

The isolated experiment remains at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo-413-live-demo-q57njj1t`.
`check_codex.py` drives native verification and emits only selected non-secret
results. `laptop-before.json`, `laptop-after.json` and `second-login-after.json`
record the earlier laptop-only coexistence check. `laptop-before-transfer.json`
and `laptop-after-transfer.json` record the original laptop login around the
real transfer. Remote `/Users/jack/loo-413-demo.VvtZ13/mini-after-transfer.json`
records the Mini's resident login. All have `passed: true` and
`authenticated_limits_returned: true`. Both new logins remain in their private
experiments; no logout or token revocation was run.
The existing laptop account is
`/Users/jack/.lf/accounts/codex/jackstah-1066ea9c99d1`.

The source deployment now proves registration, fresh Codex login transfer,
immediate coexistence, account-selected remote execution and no-login reuse.
Remaining acceptance: independent refresh on later days, Claude/Linear/GitHub,
automatic connection during a missing-account launch, preservation of unrelated
existing target accounts and SSH access after reboot with locked Keychain.
The explicit token-key path did not exercise migration of a real Keychain key.
Default installed-runtime acceptance remains separate; both standard installs
are still 0.13.9. Jack's source-deployment authorization grants no landing.

The version warning is a concrete review finding: identical checked bytes still
produce “versions differ” because the local package version omits the source
suffix. Correct that comparison before claiming the source-version UX is clean.
Release child memory was inspected for source isolation, transfer completion
and bounded cleanup lessons; unrelated release history was not re-reviewed.

No design changes were agreed. Jack's browser approval is authorization of that
login, not acceptance of the entire Task. The retained source fixture results
remain in the design; gate/CI separately owns the OAuth fixture output-handle
investigation and affected suites. No tests were rerun for these note changes.
PR #1493 remains the designated review artifact; hosted state was not queried.
No landing, Task completion, manual Session closure or Flow restart was performed.

Checks: source build, matched transfer checksum, registration/identity, real
connect, authenticated reads on both machines, remote one-turn launch and
headless reconnect passed; broader gate/CI and the remaining acceptance above
are deferred.
