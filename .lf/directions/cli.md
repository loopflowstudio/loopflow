# Work-bound invocation

Work selectors supply attribution, context and placement to direct skills and
flows. Keep that separate from `lf task run` owning the managed Task Flow.
Every direct flow skill receives fresh scratch and the same exact Work subject;
shared contributions leave automatic checkpointing to their caller.

Work resolution is read-only: Run attribution and filtered observation reuse it.
Record a registered Task's existing Started event at actual Run capture, not
while resolving context or publishing an unopened human review. Keep the
transactional chapter check at launch; never derive execution from inspection.

Bare names prefer skills. Explicit skill/flow verbs resolve the requested kind.
The existing flow loader already accepts a skill as a one-step invocation;
do not add a wrapper YAML file or a name-specific dispatch exception for it.
Human review belongs on the authored workflow occurrence that needs acceptance.

# Large provider inputs

Recursive Task scratch can exceed OS argv limits. Claude batch launch sends the
complete task prompt through text stdin backed by an anonymous file; system
instructions use the existing context file. Keep captured and streaming output
on the same input path. Do not truncate scratch, move it into argv, or add a
second launcher to fit a large contribution.
