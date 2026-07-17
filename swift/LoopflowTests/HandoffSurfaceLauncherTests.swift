#if os(macOS)
import Testing

@testable import Loopflow
@testable import LoopflowMac

/// Launch-level proof that an "attach" surface actually carries the exact
/// provider-session-bearing command *and environment* the store handed back —
/// and that a surface which cannot carry it is never dressed up as attach.
///
/// The pure model decides reach; these tests pin the *action* each reach
/// performs, which is where a dishonest "attach" would hide.
@Suite("Handoff surface launch actions")
struct HandoffSurfaceLauncherTests {
    // A shared attach command that names a specific provider Session — the thing
    // an honest attach must run and a folder-open cannot.
    private let argv = ["claude", "--resume", "sess_abc123", "--cwd", "/src/repo"]

    @Test("the Warp attach config runs the exact provider-session-bearing argv")
    func warpConfigCarriesTheSessionCommand() {
        let name = HandoffSurfaceLauncher.warpLaunchConfigName(sessionId: "ih_1")
        let yaml = HandoffSurfaceLauncher.warpLaunchConfigYAML(
            name: name,
            cwd: "/src/repo",
            argv: argv
        )

        // The launch runs the whole argv as one command — the session id rides
        // through, so Warp attaches the same Session rather than a fresh shell.
        let expectedCommand = "'claude' '--resume' 'sess_abc123' '--cwd' '/src/repo'"
        #expect(yaml.contains("exec: \"\(expectedCommand)\""))
        #expect(yaml.contains("cwd: \"/src/repo\""))
        #expect(yaml.contains("name: \(name)"))
    }

    @Test("the Warp attach config preserves the descriptor environment")
    func warpConfigPreservesEnvironment() {
        let yaml = HandoffSurfaceLauncher.warpLaunchConfigYAML(
            name: "n",
            cwd: "/src/repo",
            argv: argv,
            environment: ["LF_WAVE_ID": "w_42", "TERM": "xterm-256color"]
        )
        // The environment rides an `env KEY=VALUE …` prefix, sorted for
        // determinism, ahead of the exact argv — so Warp attaches with the same
        // environment the embedded terminal would inherit, not a bare shell's.
        let expected =
            "'env' 'LF_WAVE_ID=w_42' 'TERM=xterm-256color' "
            + "'claude' '--resume' 'sess_abc123' '--cwd' '/src/repo'"
        #expect(yaml.contains("exec: \"\(expected)\""))
    }

    @Test("every argv token survives quoting into the Warp command")
    func warpConfigQuotesEveryToken() {
        let yaml = HandoffSurfaceLauncher.warpLaunchConfigYAML(
            name: "n",
            cwd: "/src/repo",
            argv: argv
        )
        for token in argv {
            #expect(yaml.contains("'\(token)'"))
        }
    }

    @Test("an argument with a quote cannot break out of the command")
    func warpConfigEscapesQuotes() {
        let yaml = HandoffSurfaceLauncher.warpLaunchConfigYAML(
            name: "n",
            cwd: "/tmp",
            argv: ["echo", "it's fine"]
        )
        // The apostrophe is closed, escaped, and reopened — never left bare. The
        // shell backslash is itself escaped for the YAML double-quoted scalar, so
        // Warp decodes `\\` back to `\` and the shell sees `'it'\''s fine'`.
        #expect(yaml.contains(#"'it'\\''s fine'"#))
    }

    @Test("an IDE without Claude or a known session is worktree-only")
    func ideWithoutClaudeOrSessionIsWorktreeOnly() {
        // With both IDEs installed and a proven local workspace, but a non-Claude
        // provider: reach refuses to promote them to attach.
        let nonClaude = HandoffSurfaceCapability(
            installedApps: [.vscode, .cursor],
            workspaceProven: true,
            warpCommandBearing: false,
            isRemoteHome: false,
            providerIsClaude: false,
            providerSessionKnown: true
        )
        #expect(nonClaude.reach(.vscode) == .worktreeOnly)
        #expect(nonClaude.reach(.cursor) == .worktreeOnly)

        // Claude but no known session id: still worktree-only.
        let noSession = HandoffSurfaceCapability(
            installedApps: [.vscode, .cursor],
            workspaceProven: true,
            warpCommandBearing: false,
            isRemoteHome: false,
            providerIsClaude: true,
            providerSessionKnown: false
        )
        #expect(noSession.reach(.vscode) == .worktreeOnly)
        #expect(noSession.reach(.cursor) == .worktreeOnly)

        // Claude + known session id: IDEs attach.
        let claudeWithSession = HandoffSurfaceCapability(
            installedApps: [.vscode, .cursor],
            workspaceProven: true,
            warpCommandBearing: false,
            isRemoteHome: false,
            providerIsClaude: true,
            providerSessionKnown: true
        )
        #expect(claudeWithSession.reach(.vscode) == .attach)
        #expect(claudeWithSession.reach(.cursor) == .attach)
    }

    @Test("descriptor host classifies local versus remote Home")
    func remoteHomeDetection() {
        #expect(!HandoffSurfaceLauncher.isRemoteHome("localhost"))
        #expect(!HandoffSurfaceLauncher.isRemoteHome("127.0.0.1"))
        // The attach DTO emits a bare hostname for a remote Home.
        #expect(HandoffSurfaceLauncher.isRemoteHome("mini"))
        #expect(HandoffSurfaceLauncher.isRemoteHome("mini.local.net"))
        #expect(HandoffSurfaceLauncher.isRemoteHome("mini.local.net:2222"))
        #expect(HandoffSurfaceLauncher.isRemoteHome("ssh://jack@mini"))
    }

    @Test("a remote Home is never locally workspace-proven")
    func remoteHomeIsNeverLocallyProven() {
        // Even pointed at a directory that exists on this machine, a remote Home
        // yields no local workspace proof, so IDEs stay unavailable.
        let capability = HandoffSurfaceLauncher.capability(
            host: "ssh://jack@mini",
            cwd: "/tmp",
            provider: "claude",
            providerSessionId: "sess_1"
        )
        #expect(capability.isRemoteHome)
        #expect(!capability.workspaceProven)
        #expect(capability.reach(.vscode) == .unavailable)
        #expect(capability.reach(.cursor) == .unavailable)
    }

    @Test("a local descriptor stays byte-for-byte unchanged")
    func localDescriptorCommandIsExact() {
        let attach = InteractiveHandoffAttach(
            sessionId: "ih_local",
            status: .attached,
            cwd: "/src/repo",
            host: "localhost",
            environment: ["LF_WAVE_ID": "w_42"],
            argv: argv
        )
        let command = HandoffSurfaceLauncher.command(for: attach)
        #expect(command.cwd == attach.cwd)
        #expect(command.argv == attach.argv)
        #expect(command.environment == attach.environment)
    }

    @Test("a remote descriptor executes cwd, environment, and argv on its host")
    func remoteDescriptorUsesSSH() {
        let attach = InteractiveHandoffAttach(
            sessionId: "ih_remote",
            status: .attached,
            cwd: "/remote/repo",
            host: "mini.example:2222",
            environment: ["LF_WAVE_ID": "w_42", "TERM": "xterm-256color"],
            argv: argv
        )
        let command = HandoffSurfaceLauncher.command(for: attach, home: "ssh://jack@mini.example:2222")
        #expect(command.cwd == "/")
        #expect(command.environment.isEmpty)
        #expect(command.argv == [
            "ssh",
            "-p",
            "2222",
            "jack@mini.example",
            "cd '/remote/repo' && exec 'env' 'LF_WAVE_ID=w_42' 'TERM=xterm-256color' "
                + "'claude' '--resume' 'sess_abc123' '--cwd' '/src/repo'",
        ])
    }

    // MARK: - IDE attach: the exact shared command in the IDE's terminal

    @Test("the IDE shell command runs the exact provider-session-bearing argv")
    func ideShellCommandCarriesTheSessionArgv() {
        let command = HandoffSurfaceLauncher.Command(
            cwd: "/src/repo",
            argv: argv,
            environment: [:]
        )
        let shell = HandoffSurfaceLauncher.ideShellCommand(from: command)
        // The command cds to the worktree and execs the exact argv — the session
        // id rides through, so the IDE's terminal attaches the same Session.
        #expect(shell == "cd '/src/repo' && exec 'claude' '--resume' 'sess_abc123' '--cwd' '/src/repo'")
    }

    @Test("the IDE shell command preserves the descriptor environment")
    func ideShellCommandPreservesEnvironment() {
        let command = HandoffSurfaceLauncher.Command(
            cwd: "/src/repo",
            argv: argv,
            environment: ["LF_WAVE_ID": "w_42", "TERM": "xterm-256color"]
        )
        let shell = HandoffSurfaceLauncher.ideShellCommand(from: command)
        // The environment rides an `env KEY=VALUE …` prefix, sorted for
        // determinism, ahead of the exact argv.
        #expect(shell == "cd '/src/repo' && exec 'env' 'LF_WAVE_ID=w_42' 'TERM=xterm-256color' "
            + "'claude' '--resume' 'sess_abc123' '--cwd' '/src/repo'")
    }

    @Test("the IDE AppleScript activates the app and runs the command in a terminal")
    func ideAttachAppleScriptStructure() {
        let script = HandoffSurfaceLauncher.ideAttachAppleScript(
            bundleName: "Cursor",
            shellCommand: "cd '/src/repo' && exec 'claude' '--resume' 'sess_abc123'"
        )
        // The script activates the IDE, opens the command palette, creates a
        // terminal, and types the exact shell command.
        #expect(script.contains("tell application \"Cursor\" to activate"))
        #expect(script.contains("Terminal: Create New Integrated Terminal"))
        #expect(script.contains("cd '/src/repo' && exec 'claude' '--resume' 'sess_abc123'"))
    }

    @Test("the IDE AppleScript escapes double quotes in the shell command")
    func ideAttachAppleScriptEscapesQuotes() {
        let script = HandoffSurfaceLauncher.ideAttachAppleScript(
            bundleName: "VS Code",
            shellCommand: "echo \"hello\""
        )
        // Double quotes are escaped for the AppleScript string literal.
        #expect(script.contains("echo \\\"hello\\\""))
    }

    @Test("Claude with a known session id makes IDE reach attach")
    func claudeSessionKnownMakesIDEAttach() {
        let capability = HandoffSurfaceCapability(
            installedApps: [.vscode, .cursor],
            workspaceProven: true,
            warpCommandBearing: false,
            isRemoteHome: false,
            providerIsClaude: true,
            providerSessionKnown: true
        )
        #expect(capability.reach(.vscode) == .attach)
        #expect(capability.reach(.cursor) == .attach)
        let ideOptions = capability.offeredOptions.filter { $0.surface.isIDE }
        #expect(ideOptions.allSatisfy { $0.reach == .attach })
        #expect(ideOptions.allSatisfy { $0.label == $0.surface.appName })
    }

    @Test("a non-Claude provider keeps IDE at worktree-only")
    func nonClaudeProviderKeepsIDEWorktreeOnly() {
        let capability = HandoffSurfaceCapability(
            installedApps: [.cursor],
            workspaceProven: true,
            warpCommandBearing: false,
            isRemoteHome: false,
            providerIsClaude: false,
            providerSessionKnown: true
        )
        #expect(capability.reach(.cursor) == .worktreeOnly)
        let option = capability.offeredOptions.first { $0.surface == .cursor }
        #expect(option?.label == "Cursor (open worktree)")
    }

    @Test("Claude without a known session id keeps IDE at worktree-only")
    func claudeWithoutSessionKeepsIDEWorktreeOnly() {
        let capability = HandoffSurfaceCapability(
            installedApps: [.cursor],
            workspaceProven: true,
            warpCommandBearing: false,
            isRemoteHome: false,
            providerIsClaude: true,
            providerSessionKnown: false
        )
        #expect(capability.reach(.cursor) == .worktreeOnly)
    }
}
#endif
