---
requires: a change on a branch or a security-relevant area to inspect
produces: concrete security findings with severity, exploit path and fix, or a clean report
action_style: procedural
---
Review the change for security mistakes before it ships.

Read the diff against its base, then follow every touched path through its
callers, inputs, auth checks, outbound calls, logs and error paths. Do not
review from the diff alone: most security bugs live in the assumption a caller
or neighbor makes.

Focus on what applies to this change and skip the rest:

- **Auth and access control:** can an unauthenticated or wrong caller reach
  this endpoint, command, socket, file or action? Are checks enforced on the
  real owner, not only in the UI or client?
- **Credentials and secrets:** where do tokens, keys, cookies or environment
  variables enter, travel or land? Can any raw secret reach logs, errors,
  command arguments, crash reports, git history or a less-trusted host?
- **Untrusted input:** shell, SQL, paths, URLs, templates, serialization and
  IPC. Trace untrusted bytes to every sink; reject or escape at the real use.
- **Outbound data and telemetry:** what leaves the Machine or service, to whom,
  and could prompts, file contents, paths, secrets or user data leak into it?
- **Deployment, install and permissions:** file modes, symlinks, temp files,
  downloaded binaries, signing/verification, privilege changes and default
  exposure.

**Report only concrete findings.** For each one, give the exact file and line,
the attack or leak path, and the smallest fix that closes it. Skip theoretical
hardening that has no real path here.

Write findings to `scratch/security-review.md`, ordered worst first. When
nothing turns up, say what you traced and why it holds. Don't change code
unless asked.
