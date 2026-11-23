#if os(macOS) && canImport(GhosttyKit)
import AppKit
import Loopflow
import Testing
@testable import LoopflowMac

func terminalKeyEvent(_ text: String, _ code: UInt16, _ flags: NSEvent.ModifierFlags = [],
                      type: NSEvent.EventType = .keyDown, unmodified: String? = nil,
                      repeating: Bool = false, windowNumber: Int = 0) throws -> NSEvent {
    try #require(NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags,
        timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: windowNumber, context: nil,
        characters: text, charactersIgnoringModifiers: unmodified ?? text,
        isARepeat: repeating, keyCode: code))
}

// The Session relay keeps its status connection open while echoing terminal input.
let echoingSessionRelay = buildGhosttyShellCommand(argv: ["/bin/sh", "-c", #"""
    { printf '%s\n' '{"generation":1,"state":"connected","native_ready":true,"message":null}'; /bin/cat >/dev/tty; } | /usr/bin/nc -U "$LF_SERVER_STATUS"
    """#], env: [:])

@MainActor
func terminalViewport(_ view: GhosttyMetalView) -> String {
    if case .available(let text, _) = try? view.readText(region: .screen, maxBytes: 65536) { return text }
    return ""
}

@MainActor
func waitForTerminal(_ view: GhosttyMetalView, timeout: Duration = .seconds(20),
                     at location: SourceLocation = #_sourceLocation,
                     until condition: () -> Bool) async throws {
    let deadline = ContinuousClock.now + timeout
    while !condition() {
        try #require(ContinuousClock.now < deadline,
                     "Connection \(String(describing: view.connectionReading)), screen \(terminalViewport(view))",
                     sourceLocation: location)
        GhosttyManager.shared.tick()
        try await Task.sleep(for: .milliseconds(20))
    }
}

@MainActor
func waitForTerminalLine(_ line: String, in view: GhosttyMetalView, timeout: Duration = .seconds(20),
                         at location: SourceLocation = #_sourceLocation) async throws {
    try await waitForTerminal(view, timeout: timeout, at: location) {
        view.connectionAcceptsInput && terminalViewport(view).split(separator: "\n").contains(Substring(line))
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
