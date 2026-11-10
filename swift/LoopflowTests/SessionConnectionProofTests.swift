#if os(macOS) && canImport(GhosttyKit)
import AppKit
import Foundation
import GhosttyKit
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite(.requiresDisplay, .serialized)
@MainActor
struct SessionConnectionProofTests {
    @Test("A mounted Session keeps its screen and keyboard input across server loss")
    func nativeOpeningAndRecovery() async throws {
        _ = NSApplication.shared
        let root = URL(fileURLWithPath: "/tmp").appendingPathComponent("lf-session-proof-\(UUID().uuidString)")
        let fm = FileManager.default
        try fm.createDirectory(at: root.appendingPathComponent("bin"), withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: root) }
        let binary = try BuiltLf.binary("lf")
        let provider = root.appendingPathComponent("bin/claude")
        try #"""
        #!/bin/sh
        if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
        printf '%s\n' "$$" >> "$HOME/launches"
        stty echo < /dev/tty
        printf 'session-proof-ready\n'
        while IFS= read -r line < /dev/tty; do
          printf '%s\n' "$line" >> "$HOME/answers"
          printf 'received:%s\n' "$line"
        done
        """#.write(to: provider, atomically: true, encoding: .utf8)
        let entry = root.appendingPathComponent("bin/lf")
        try "#!/bin/sh\nif [ \"$1\" = install ] && [ \"$2\" = server ]; then exit 0; fi\nexec \(shellEscape(binary)) \"$@\"\n"
            .write(to: entry, atomically: true, encoding: .utf8)
        for file in [provider, entry] { try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: file.path) }
        let lf = try BuiltLf(home: root, overriding: ["PATH": root.appendingPathComponent("bin").path + ":/usr/bin:/bin:/usr/sbin:/sbin", "LF_BIN": entry.path])
        for args in [["init", "-b", "main"], ["config", "user.name", "Fixture"],
                     ["config", "user.email", "fixture@example.invalid"], ["commit", "--allow-empty", "-m", "Fixture"]] {
            _ = try await lf.run(args, executable: "/usr/bin/git")
        }
        let output = root.appendingPathComponent("owner-output")
        let errors = root.appendingPathComponent("owner-errors")
        try Data().write(to: output); try Data().write(to: errors)
        let stdout = try FileHandle(forWritingTo: output)
        let stderr = try FileHandle(forWritingTo: errors)
        defer { try? stdout.close(); try? stderr.close() }
        let owner = lf.process(["session", "serve", "--terminal", "--", binary, "-i", "-a", "claude", ":", "proof"])
        owner.standardInput = FileHandle.nullDevice
        owner.standardOutput = stdout; owner.standardError = stderr
        owner.terminationHandler = { _ in }
        try owner.run()
        defer { if owner.isRunning { owner.terminate() } }
        try await wait { fm.fileExists(atPath: root.appendingPathComponent("launches").path) }
        let ownerID = try String(contentsOf: output, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        let inventory = try await lf.run(["session", "list", "--json"])
        let object = try JSONSerialization.jsonObject(with: Data(inventory.utf8))
        let records = (object as? [[String: Any]]) ?? ((object as? [String: Any])?["entries"] as? [[String: Any]]) ?? []
        let session = try #require(records.first?["id"] as? String)
        let server = LocalServerFixture(lf)
        _ = try await server.status()
        defer { Task { await server.stop() } }
        let manager = GhosttyManager.shared
        manager.initialize()
        try #require(manager.state == .ready)
        let view = GhosttyMetalView(terminal: .session(session, machineId: "fixture"))
        view.workingDirectory = root.path
        view.command = buildGhosttyShellCommand(argv: [binary, "session", "attach", ownerID, "--session", session], env: lf.environment)
        let mount = GhosttyTerminalMount(terminal: view)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 480),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = mount
        defer { view.destroySurface(); window.contentView = nil }
        mount.layoutSubtreeIfNeeded()
        view.createSurface(manager: manager)
        let surface = try #require(view.surface)
        try #require(window.makeFirstResponder(view))
        try await wait { view.connectionAcceptsInput && text(view).contains("session-proof-ready") }
        let frame = view.frame
        try type("before\r", into: view, window: window)
        try await wait { text(view).contains("received:before") }
        await server.stop()
        try await wait { !view.connectionAcceptsInput }
        #expect(text(view).contains("received:before"))
        #expect(view.frame == frame)
        try type("discarded\r", into: view, window: window)
        _ = try await server.status()
        try await wait { view.connectionAcceptsInput }
        #expect(view.surface == surface)
        #expect(view.frame == frame)
        try type("after\r", into: view, window: window)
        try await wait { text(view).contains("received:after") }
        #expect(try String(contentsOf: root.appendingPathComponent("answers"), encoding: .utf8) == "before\nafter\n")
        #expect(try String(contentsOf: root.appendingPathComponent("launches"), encoding: .utf8).split(separator: "\n").count == 1)
        await server.stop()
    }

    private func type(_ text: String, into view: GhosttyMetalView, window: NSWindow) throws {
        for character in text {
            let value = String(character)
            view.keyDown(with: try #require(NSEvent.keyEvent(with: .keyDown, location: .zero,
                modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber, context: nil, characters: value,
                charactersIgnoringModifiers: value, isARepeat: false, keyCode: character == "\r" ? 36 : 0)))
        }
    }

    private func text(_ view: GhosttyMetalView) -> String {
        if case .available(let text, _) = try? view.readText(region: .screen, maxBytes: 65536) { return text }
        return ""
    }

    private func wait(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(20)
        while !condition() {
            try #require(ContinuousClock.now < deadline)
            GhosttyManager.shared.tick()
            try await Task.sleep(for: .milliseconds(20))
        }
    }
}
#endif
