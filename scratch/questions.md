# Assumptions

- For LOO-217, “landing” means a managed Task PR auto-merge request created by
  `lf pr arm` or `lf pr land` in a repository that declares the schema-4
  structured pre-land protocol. Direct non-Task landings, `lf pr submit`,
  historical rows, and repositories without that protocol are explicit
  unsupported populations rather than silently eligible landings.
- The receipt covers the local pre-integration tree. A later hosted CI repair
  advances the same landing head but does not replace its original pre-land
  receipt; CI evidence remains owned by the landing's CI incidents.
