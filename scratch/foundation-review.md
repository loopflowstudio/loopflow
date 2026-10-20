# Foundations review (October 16, historical)

Full findings, responses and checks: `f492be7f19:scratch/foundation-review.md`
(earlier: `refs/loopflow/evidence/loo-497-before-compress:foundation-review.md`).
Settled: the path/plan-ID mismatch was disproved (`stateUsesDurableRepositoryIdentity`);
failed saved-query reads retain intent and complete reads prune
(`nested_query_failure_retains_prior_members_but_confirmed_departure_prunes`);
validation covers only the edited selection and its references; fixtures that set
`LF_HOME`/`LF_BIN` take the shared environment lock.
