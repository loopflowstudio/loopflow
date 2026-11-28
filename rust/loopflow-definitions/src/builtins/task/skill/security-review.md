---
requires: a plan, design or branch diff to review for security
produces: concrete security findings, recommended safeguards and vulnerability tests, or a clean report
action_style: procedural
---
Review a plan, design or branch diff for security mistakes before building or shipping.

- **On a plan or design** (`scratch/plan.md` or a proposed change before code is
  written): trace the proposed flows, callers, inputs, secrets and outbound
  data against the existing codebase to flag concrete design risks to settle
  before building.
- **On a diff:** read the diff against its base, then follow every touched path
  through its callers, inputs, auth checks, outbound calls, logs and error
  paths. Do not review from the diff alone: most security bugs live in the
  assumption a caller or neighbor makes.

Focus on what applies here and skip the rest:

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

**Report concrete findings, patterns and tests:**

- **Findings** (worst first): skip theoretical hardening with no real path
  here. On a plan, state the design risk, how it fails and how to settle it
  before building. On a diff, give the exact `file:line`, the attack or leak
  path, and the smallest fix that closes it.
- **Proven patterns and safeguards:** especially on plans and designs, name
  well-established patterns or practical safeguards that fit this system (e.g.
  least-privilege defaults, token scoping, atomic writes, strict parsers,
  server-side enforcement).
- **How to test and probe it:** name concrete checks that prove the safeguards
  hold—negative auth cases, injection or abuse inputs, permission checks, and
  leak checks on logs, errors or outbound payloads.

Write the report to `scratch/security-review.md`. When no issues turn up, say
what you traced and why it holds alongside any tests worth keeping. Don't
change code unless asked.
