# Project conversion review — 2026-10-10

Operator finding on the in-progress implementation, not a new product decision.
The accepted [deletion audit](work-model-reduction.md#5-delete-the-internal-markdown-storage-round-trip)
requires authored prose preservation. Recheck after the current implementation cut;
remove this note when repaired and represented in the normal behavior tests.

`pm/mod.rs::project_prose` currently drops every trimmed `workflow:` or `flow:`
line, including an unrelated fenced example:

````markdown
## Example
```yaml
workflow: review
```
````

It also discards all bytes in a managed section rather than just fields recognized
by extraction. Conversion must preserve unrelated prose, fenced examples and
unknown content. A retained raw description or boundary-aware extraction can keep
that evidence; do not invent a second active Project representation. Prove the
populated migration and Linear read/edit round trip retain these bytes alongside
actual managed KRs, targets and workflow selection. The current migration test
covers ordinary Notes prose, but does not cover this counterexample.

No product tests were run for this static review. Implementation remains the only
source writer; this note supplies feedback to its next context refresh.
