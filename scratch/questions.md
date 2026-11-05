# LOO-728 assumptions

Jack Heart's November 4 independent-main direction permits preserving parent
ancestry as recovery history while removing parent behavior from the PR tree.
The simpler safe choice is an owned fork-based merge, preserving the existing
branch and PR, rather than rewriting or deleting source history.

Explicit `--file` terminal additions depend on LOO-498 and remain in the saved
source. Main has no such file-opening flag; its existing explicit Work-opening
capabilities remain unchanged. Desktop Files is sufficient for accepted scope.

Recovery decision, November 4: when the recorded fork is missing, managed sync
uses HEAD's common ancestor with the recorded parent's remote branch. Here Git
confirms df3b5e810d3841a19bc5911c332fb42f23193d46. Planning remains read-only;
only successful owned sync records placement. Missing parent history still
reports Git's error rather than inventing a comparison base.

No Wave-memory edits are authorized. The previously proposed correction to
infrastructure memory (Desktop-only rather than current-host automation) remains
for its owner, preserved at
`ea0e0781b22f092b6a0169802997ff7a3628c65c:scratch/questions.md`.

Headless demo decision, November 4: apply the repository's existing optional
hosted-diagnostic policy to this independent-main candidate. The configured Task
command still fails at the known installed-schema boundary; this is recorded as
failed evidence, not waived into a pass. Fresh candidate-owned fallback and
headless Files checks support advancing to gate without importing parent schema
changes. No new product choice or human approval is inferred; see docs-first-demo.md.
