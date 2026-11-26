---
extends: builtin
---
## In this repository

Check these Loopflow areas whenever the plan or diff touches them, and name
concrete negative tests or leak checks for each affected area:

- **Secrets and Doppler:** raw secrets must come from Doppler (`doppler run`),
  never dotfiles, hardcoded values, logs, error strings, or terminal output.
- **Remote execution (Modal, Daytona, SSH):** inspect every credential, env
  var, token or SSH agent forwarded into a remote container or sandbox; forward
  only what the run needs and confirm nothing persists in images, snapshots or
  logs.
- **Telemetry and outbound requests:** check every field leaving the Machine
  (events, crash/usage reports, remote sync) so prompts, file paths, repo
  contents and credentials cannot leak.
- **Server, mobile and Tailscale auth:** verify caller identity, socket/HTTP
  permissions and token checks on served operations, pairing and remote Task
  commands.
- **Install, update and release publishing:** check binary download/verify,
  atomic replacement, entry-gate permissions, and the split between
  credential-free CI builds and the release publisher (signing, notarization,
  secrets).
