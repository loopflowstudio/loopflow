# LOO-913 verification choices

The native-auth fixture returned “is a Run (finished); `lf resume` takes only Sessions.”
Headless launch and interactive Session resume are checked separately. This keeps
the existing Run/Session contract while checking native authentication in each path.

The design retains existing agent-pin semantics: a bare `claude` pins the harness
but can try its other models; `claude:opus` also pins the model. Docs now state
that distinction instead of promising that every bare harness pin stops.

A forwarded file the destination already has is kept, not replaced; the launch
says so. Replacing would silently sign a Machine out of a newer login of its own.
