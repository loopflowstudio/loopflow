INSERT INTO pr_landings (id,repo,pr_number,worktree,branch,requested_head_sha,
 observed_head_sha,state,generation,blocked_reason,created_at,updated_at,
 ci_pending_head,ci_pending_since,ci_pending_attempt,ci_timeout_reruns)
VALUES ('slow','owner/repo',1,'/tmp/checkout','feature','requested','observed',
 'blocked',3,'CI timeout rerun allowance exhausted; inspect the pending checks',1,2,
 'observed',1,'old-attempt',1),
 ('blocked','owner/repo',2,'/tmp/other','other','requested','observed',
 'blocked',3,'credential revoked',1,2,NULL,NULL,NULL,0);
