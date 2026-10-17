ALTER TABLE pr_landings DROP COLUMN ci_pending_head;
ALTER TABLE pr_landings DROP COLUMN ci_pending_since;
ALTER TABLE pr_landings DROP COLUMN ci_pending_attempt;
ALTER TABLE pr_landings DROP COLUMN ci_timeout_reruns;

-- Resume observation of retained intent held only by the retired clock policy.
UPDATE pr_landings SET state = 'watching', blocked_reason = NULL
WHERE state = 'blocked' AND blocked_reason IN (
    'CI timeout rerun allowance exhausted; inspect the pending checks',
    'ci-fix blocked: repair startup acknowledgement timed out; reservation retained',
    'ci-fix blocked: landing ci-fix exceeded 10 seconds'
);
