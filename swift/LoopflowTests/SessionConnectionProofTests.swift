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
        try await withPreparedSession(provider: #"""
        #!/bin/bash
        if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
        printf '%s\n' "$$" >> "$HOME/launches"
        stty echo < /dev/tty
        printf 'session-proof-ready\n'
        while IFS= read -r -t 20 line < /dev/tty; do
          [ "$line" = quit ] && exit 0
          printf '%s\n' "$line" >> "$HOME/answers"
          printf 'received:%s\n' "$line"
        done
        """#) { lf, server, record in
            let manager = GhosttyManager.shared
            manager.initialize()
            try #require(manager.state == .ready)
            let view = GhosttyMetalView(terminal: .session(record.id, machineId: fixtureMachineId))
            view.workingDirectory = lf.cwd.path
            view.command = buildGhosttyShellCommand(argv: record.openArgv, env: lf.environment)
            let mount = GhosttyTerminalMount(terminal: view)
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 480),
                                  styleMask: [.titled], backing: .buffered, defer: false)
            window.contentView = mount
            defer { view.destroySurface(); window.contentView = nil }
            mount.layoutSubtreeIfNeeded()
            view.createSurface(manager: manager)
            let surface = try #require(view.surface)
            try #require(window.makeFirstResponder(view))
            try await wait(in: view) { view.connectionAcceptsInput && text(view).contains("session-proof-ready") }
            let frame = view.frame
            try type("before\r", into: view, window: window)
            try await wait(in: view) { text(view).contains("received:before") }
            await server.stop()
            try await wait(in: view) { !view.connectionAcceptsInput }
            #expect(text(view).contains("received:before"))
            #expect(view.frame == frame)
            try type("discarded\r", into: view, window: window)
            _ = try await server.status()
            try await wait(in: view) { view.connectionAcceptsInput }
            #expect(view.surface == surface)
            #expect(view.frame == frame)
            try type("after\r", into: view, window: window)
            try await wait(in: view) { text(view).contains("received:after") }
            #expect(try String(contentsOf: lf.cwd.appendingPathComponent("answers"), encoding: .utf8) == "before\nafter\n")
            #expect(try String(contentsOf: lf.cwd.appendingPathComponent("launches"), encoding: .utf8).split(separator: "\n").count == 1)
            try type("quit\r", into: view, window: window)
            try await wait(in: view) { view.childExited || ghostty_surface_process_exited(surface) }
        }
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

    private func wait(in view: GhosttyMetalView, at location: SourceLocation = #_sourceLocation,
                      until condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(20)
        while !condition() {
            try #require(ContinuousClock.now < deadline,
                         "Connection \(String(describing: view.connectionReading)), screen \(text(view))",
                         sourceLocation: location)
            GhosttyManager.shared.tick()
            try await Task.sleep(for: .milliseconds(20))
        }
    }
}

/// The same prepared Session a Desktop pane opens, on a private Machine with a stand-in provider.
@MainActor
func withPreparedSession(provider providerScript: String,
                         run: (BuiltLf, LocalServerFixture, SessionRecord) async throws -> Void) async throws {
    let root = URL(fileURLWithPath: "/tmp").appendingPathComponent("lf-session-proof-\(UUID().uuidString)")
    let home = root.appendingPathComponent("home")
    let fm = FileManager.default
    try fm.createDirectory(at: home.appendingPathComponent("bin"), withIntermediateDirectories: true)
    defer { try? fm.removeItem(at: root) }
    let binary = try BuiltLf.binary("lf")
    let provider = home.appendingPathComponent("bin/claude")
    try providerScript.write(to: provider, atomically: true, encoding: .utf8)
    let entry = home.appendingPathComponent("bin/lf")
    try "#!/bin/sh\nif [ \"$1\" = install ] && [ \"$2\" = server ]; then exit 0; fi\nexec \(shellEscape(binary)) \"$@\"\n"
        .write(to: entry, atomically: true, encoding: .utf8)
    for file in [provider, entry] { try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: file.path) }
    let lf = try BuiltLf(home: home, overriding: ["PATH": home.appendingPathComponent("bin").path + ":/usr/bin:/bin:/usr/sbin:/sbin", "LF_BIN": entry.path])
    for args in [["init", "-b", "main"], ["config", "user.name", "Fixture"],
                 ["config", "user.email", "fixture@example.invalid"], ["commit", "--allow-empty", "-m", "Fixture"]] {
        _ = try await lf.run(args, executable: "/usr/bin/git")
    }
    // Preparing the Session finishes first-open migrations before the server readiness check.
    let prepared = try await lf.run(["session", "resume", "--prepare", "--json", "-a", "claude"])
    let record = try JSONDecoder().decode(SessionRecord.self, from: Data(prepared.utf8))
    let server = LocalServerFixture(lf)
    do {
        _ = try await server.status()
        try await run(lf, server, record)
    } catch {
        await server.stop()
        throw error
    }
    await server.stop()
}
#endif
