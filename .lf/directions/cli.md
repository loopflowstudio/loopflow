# Work-bound invocation

Work selectors supply attribution, context and placement to direct skills and
flows. Keep that separate from `lf task run` owning the managed Task Flow.
Every direct flow skill receives fresh scratch and the same exact Work subject;
shared contributions leave automatic checkpointing to their caller.

Bare names prefer skills. Explicit skill/flow verbs resolve the requested kind.
The existing flow loader already accepts a skill as a one-step invocation;
do not add a wrapper YAML file or a name-specific dispatch exception for it.
Human review belongs on the authored workflow occurrence that needs acceptance.
