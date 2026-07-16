#if os(macOS)
import Testing

@testable import Loopflow
@testable import LoopflowMac

/// Launch-level proof that an "attach" surface actually carries the exact
/// provider-session-bearing command the store handed back — and that a surface
/// which cannot carry it is never dressed up as attach.
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

    @Test("an IDE bears no session action — it is only ever worktree-only")
    func ideBearsNoSessionAction() {
        // With both IDEs installed and a proven workspace, reach still refuses to
        // promote them to attach: there is no launch that resumes the Session.
        let capability = HandoffSurfaceCapability(
            installedApps: [.vscode, .cursor],
            workspaceProven: true,
            warpCommandBearing: false
        )
        #expect(capability.reach(.vscode) == .worktreeOnly)
        #expect(capability.reach(.cursor) == .worktreeOnly)
        // No offered IDE option ever advertises attach.
        let ideOptions = capability.offeredOptions.filter { $0.surface.isIDE }
        #expect(!ideOptions.isEmpty)
        #expect(ideOptions.allSatisfy { $0.reach == .worktreeOnly })
    }
}
#endif
