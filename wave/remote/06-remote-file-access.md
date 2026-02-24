# 06: Remote File Access

One-click "Open in Cursor" per wave, pointing at the remote worktree via SSH.

## What exists after this

Every wave with a worktree shows an "Open in Cursor" button. Clicking it opens Cursor connected to the remote machine at the wave's worktree path. No manual SSH config per wave. Works the same as local — one click.

## Context

Concerto already has "Open in IDE" buttons per wave (`WaveDetailPanel.swift` lines 357-369). They pass `worktreePath` to `cursor <path>`. The wave API already returns `local_worktree` with the remote path.

Cursor supports `--remote ssh-remote+<host> <path>` to open a remote folder via its built-in Remote SSH. VS Code has the same flag. JetBrains Gateway uses a different mechanism.

## Carryover from Phase 05 scope cut

Phase 05 adds only correctness-critical capability gating for remote mode. The following are intentionally deferred here:

- Remote editor launch variants (`cursor/code --remote ...`)
- Remote terminal launch (`ssh -t host 'cd path && exec $SHELL -l'`)
- Replacing hidden local-only actions with remote alternatives (`Copy Path`, `Copy SSH Command`)
- Wider capability matrix polish across secondary UI surfaces

## Adjustments from Phase 01 hardening

Remote execution is headless and timeout-driven. Fork flows now work in Docker (01E shipped). This phase should assume:

- No interactive fallback: remote-only actions must honor capability gating from Phase 05.
- All flows (including forks) are available remotely — no fork capability gating needed.
- Error copy should distinguish unsupported remote action vs execution timeout vs SSH/editor launch failure.

## Implementation

### Remote editor launch

```swift
// TerminalLauncher.swift — add remote variants

func openInIDE(_ ide: IDEApp, at path: URL, remote host: String? = nil) throws {
    switch ide {
    case .cursor:
        try openCursor(path: path, remoteHost: host)
    case .vscode:
        try openVSCode(path: path, remoteHost: host)
    case .zed:
        try openZed(path: path, remoteHost: host)
    }
}

private func openCursor(path: URL, remoteHost: String?) throws {
    let cursorPath = try findExecutable("cursor")

    if let host = remoteHost {
        // Remote: cursor --remote ssh-remote+host /path
        try run(cursorPath, args: [
            "--remote", "ssh-remote+\(host)",
            path.path
        ])
    } else {
        // Local: cursor /path (existing behavior)
        try run(cursorPath, args: [path.path])
    }
}
```

VS Code uses the same `--remote ssh-remote+host path` syntax. Zed has its own remote protocol — check current support.

### Wire up to wave detail

```swift
// WaveDetailPanel.swift — pass remote host when connected remotely

private func openInIDE(path: String) {
    do {
        let remoteHost = repoState.connection.isRemote
            ? repoState.connection.host
            : nil
        try terminalLauncher.openInIDE(
            ideApp,
            at: URL(fileURLWithPath: path),
            remote: remoteHost
        )
    } catch {
        actionError = "Failed to open \(ideApp.displayName): \(error.localizedDescription)"
    }
}
```

### SSH config requirement

Cursor Remote SSH uses the system SSH config. The user needs an entry for the remote host:

```
# ~/.ssh/config (already set up in Phase 04)
Host lfd-dev
  HostName <elastic-ip>
  User lfd
  IdentityFile ~/.ssh/your-key
```

The `ServerConnection.host` should match an SSH config host name. For dev, this is the same host from Phase 04. Document this requirement — don't try to auto-generate SSH configs.

### Terminal launch (remote)

"Open in Terminal" should SSH into the worktree directory:

```swift
private func openTerminalRemote(host: String, path: String) throws {
    // Open terminal app with: ssh -t host 'cd /path && $SHELL'
    let script = "ssh -t \(host) 'cd \(path) && exec $SHELL -l'"
    try terminalLauncher.launchTerminal(terminalApp, with: script)
}
```

### Finder / Reveal

"Reveal in Finder" doesn't make sense for remote files. Hide it when connected remotely. Replace with "Copy Path" or "Copy SSH Command".

## Editor support matrix

| Editor | Remote command | Status |
|--------|---------------|--------|
| Cursor | `cursor --remote ssh-remote+host /path` | Works today |
| VS Code | `code --remote ssh-remote+host /path` | Works today |
| Zed | `zed ssh://host/path` (check current syntax) | Verify |
| JetBrains | Gateway CLI or URL scheme | Investigate |

Don't block on full editor support. Cursor + VS Code covers the primary use case.

## Constraints

- **SSH config must exist**: User must have SSH access configured to the remote host. We don't manage SSH keys.
- **One host per connection**: The remote host for file access is the same as the lfd host. Multi-host (lfd on one machine, files on another) is future work.
- **Worktree paths are remote paths**: `/home/lfd/repos/repo.wave-name`, not local paths. The wave API already returns these.
- **Headless remote invariants apply**: Don't offer actions that depend on interactive daemon prompts.

## Try it

1. Connect Concerto to remote lfd (Phase 05 path).
2. Open the same wave in Cursor and VS Code via remote commands.
3. Launch remote terminal into worktree and run `pwd`.
4. Verify local-only actions are hidden/disabled with explicit reason text.
5. Repeat on local connection to confirm no regression.

## Open questions

- For unsupported editors (for example Zed/JetBrains before remote support is wired), do we hide actions or show disabled actions with guidance?
- Should "Copy SSH Command" include a repo-relative shortcut, or only full `ssh -t host 'cd path && ...'` for clarity?

## Done when

- "Open in Cursor" works for remote waves (opens Cursor with Remote SSH)
- "Open in VS Code" works for remote waves (same mechanism)
- "Open in Terminal" SSHs into the worktree directory
- "Reveal in Finder" hidden for remote connections, replaced with "Copy Path"
- Remote capability-gated actions fail clearly (unsupported vs timeout vs launch error)
- Local waves still open editors locally (no regression)
