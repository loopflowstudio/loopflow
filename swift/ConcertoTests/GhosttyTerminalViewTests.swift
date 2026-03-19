import Testing
@testable import Concerto

@Suite("Ghostty terminal command")
struct GhosttyTerminalViewTests {
    @Test("wraps environment assignments with env before the shell command")
    func wrapsEnvironmentAssignmentsWithEnv() {
        let command = buildGhosttyShellCommand(
            argv: ["/bin/zsh", "-lc", "echo hi"],
            env: ["RLM_DEPTH": "1"]
        )

        #expect(command == "env RLM_DEPTH='1' '/bin/zsh' '-lc' 'echo hi'")
    }

    @Test("returns the raw command when no environment is provided")
    func returnsRawCommandWithoutEnvironment() {
        let command = buildGhosttyShellCommand(
            argv: ["/bin/zsh", "-lc", "echo hi"],
            env: [:]
        )

        #expect(command == "'/bin/zsh' '-lc' 'echo hi'")
    }

    @Test("returns nil when there is no command to run")
    func returnsNilWithoutCommand() {
        #expect(buildGhosttyShellCommand(argv: [], env: ["RLM_DEPTH": "1"]) == nil)
    }
}
