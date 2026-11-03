# Weekly-limit failure evidence — LOO-714

Source: /Users/jack/src/loopflow/scratch/desktop-exit-2026-11-02.md, Chain C, November 2, 2026. Copied for LOO-714 part (a); existing work and decisions remain authoritative. Proposed remedies below are investigation options.

### Chain C — why the first 5whys run died (`session_record.rs:1914`)

Status: explained from the store; affects every Claude run that hits a weekly limit while
another account is eligible. Not related to Desktop.

1. The run (`lf -b 5whys`, LfProcess `5bf2baa1…`, pid 85481, Session
   `session_8fa0bcae…`) started 21:35:15Z on Claude account `primary` (jackstah@gmail.com),
   provider thread `edd6adc7…`.
2. At 22:01:03Z Claude rejected the turn: `provider_rate_limited: rejected:
   seven_day_overage_included` (session_events 2829383). `primary` was put in cooldown until
   2026-11-04T12:00:05Z with that reason (`provider_accounts`).
3. `_classify_agent_failure` called it `provider rate limit`, a transient failure, not
   `AccountSubscriptionLimit`: `_classify_subscription_limit` matches phrases such as "usage
   limit reached" and none matches `seven_day_overage_included`. On the transient branch the
   retry keeps the account unset and **resumes the same thread**
   (`agent/mod.rs` ~1027: `resume_token = result.agent_session`; lf.log 22:01:04Z
   `resumed=true account_failover=false`).
4. On the retry, `resolve_account_route_blocking` correctly skipped the cooling `primary` and
   chose `loopflow` (jack@loopflow.studio). It also recorded
   `provider_session_accounts: edd6adc7… → loopflow` at 22:01:06Z.
5. `retry_agent_process` then saw `account_changed` (primary → loopflow) together with a
   resume thread, which its contract forbids, and bailed. That bail is the
   `AgentProcess retry must retain its thread unless the selected account changes` error.
   The guard was right. The retry loop and the account router disagreed about whether the
   account was changing: the loop decided from error text, the router from the cooldown it
   had just written.
6. It was systemic. Seven runs that hit the same rejection between 22:00:25Z and 22:01:41Z
   (pids 24956, 40036, 60954, 74398, 30228, 85481, 61016) all failed with this error. Later
   ones (36573, 96865) failed earlier, with "no eligible managed claude account", once both
   accounts were cooling.

Side effect to check: the `edd6adc7 → loopflow` row binds a thread that never ran on
`loopflow`, so a later resume of that Session may route to the wrong account.

Prevention (proposed): make one owner decide. When the provider's rate-limit signal puts the
account in cooldown, the attempt is an account limit: take the failover branch (fresh thread,
`LIMIT_FAILOVER_PROMPT`, `provider_account_id = None`) rather than matching text. Or, per
AGENTS.md's Flexibility rule, have `retry_agent_process` adapt when the router changes the
account (drop the resume and start fresh) rather than refusing. Proof: a fake-claude test
with two accounts whose stream emits `rate_limit_event` `rejected: seven_day_overage_included`
on the first; the run continues on the second account and succeeds, and no
`provider_session_accounts` row binds the old thread to the new account.
