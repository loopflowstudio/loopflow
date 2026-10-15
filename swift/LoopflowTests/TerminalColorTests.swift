#if os(macOS) && canImport(GhosttyKit)
import Foundation
import Testing
@testable import LoopflowMac

@Suite("Terminal color")
struct TerminalColorTests {
    @Test("Contrast is an overrideable theme default")
    @MainActor
    func contrastDefault() {
        #expect(GhosttyManager.loopflowConfig.contains("minimum-contrast = 3"))
        #expect(!GhosttyManager.embeddedConfig.contains("minimum-contrast"))
    }

    @Test("Production Metal colors render offscreen",
          .enabled(if: ProcessInfo.processInfo.environment["LOOPFLOW_GHOSTTY_SOURCE"] != nil,
                   "ghostty-build runs the shader check; set LOOPFLOW_GHOSTTY_SOURCE to run it here"))
    func productionShader() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/swift")
        process.arguments = [root.appendingPathComponent("scripts/ghostty_shader_check.swift").path,
                             try #require(ProcessInfo.processInfo.environment["LOOPFLOW_GHOSTTY_SOURCE"])]
        try process.run()
        process.waitUntilExit()
        #expect(process.terminationStatus == 0)
    }
}
#endif
