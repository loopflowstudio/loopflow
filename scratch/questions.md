# Open questions (LOO-633)

- A promoted hotfix keeps `InstallSource::Published` on its selection and becomes the
  retained fallback, like any promoted build. Ordinary launches route only to Published
  selections (`installed_cli`), and the receipt validator ties a Published selection to
  its fallback, so a separate source kind would need a new routing rule. Real provenance
  is in `source_revision`, `source_identity` and the new `build_label`. Assumed
  acceptable; renaming `Published` → e.g. `Installed` is a separate change.
- `Behind` (a newer release migrated the store) refuses an unreleased build, following
  the Task's "knows the store's frontier exactly". Published rollbacks into `Behind` still
  promote.
- Every unreleased draft-free build is labeled `hotfix …`, including main-tip dev builds;
  the label describes what promotion would accept, not where the build came from.
