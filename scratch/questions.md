# Current scope resolution — October 17, 2026

Jack Heart requested “get opencode 2 running.” The revised opencode-2-design.md prioritizes normal startup, resume and execution. The earlier synthetic crash-continuation choice is deferred; no acceptance is inferred. It does not block the normal adapter repair. Preserve prior native evidence under docs/reviews/opencode-2/ and implement the revised design.

Native prompt semantics: OpenCode 2 `--prompt` submits automatically; bare `lf -a opencode` runs its default skill. Preserve that behavior. Quitting a resumed empty composer makes no model request, verified in the real PTY. Automatic crash continuation remains deferred; explicit new input must never retroactively complete lost work.
