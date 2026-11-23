#if os(macOS) && canImport(GhosttyKit)
import AppKit
import GhosttyKit
import Loopflow
import Testing
@testable import LoopflowMac

@MainActor struct AgyShiftQA {
    @Test func realAgyDraftAcrossReattachment() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        try #require(manager.state == .ready)
        try await withPreparedSession(harness: "agy", provider: #"""
        #!/bin/sh
        if [ "$1" = "--version" ]; then exec /Users/jack/.local/bin/agy --version; fi
        echo $$ > "$HOME/agy-qa.pid"
        printf '%s:%s' "$TERM" "$COLORTERM" > "$HOME/agy-qa.terminal"
        exec env HOME=/Users/jack /Users/jack/.local/bin/agy --settings "{\"trustedWorkspaces\":[\"$PWD\",\"$(pwd -P)\"]}"
        """#) { lf, _, record in
            let terminal = TerminalIdentity.session(record.id, machineId: fixtureMachineId)
            let pool = GhosttySurfacePool()
            defer {
                if let s = try? String(contentsOf: lf.cwd.appendingPathComponent("agy-qa.pid"), encoding: .utf8),
                   let pid = Int32(s.trimmingCharacters(in: .whitespacesAndNewlines)) { kill(pid, SIGTERM) }
            }
            var draft = ""
            var replay = record
            for round in 1...2 {
                let view = pool.view(for: terminal)
                view.frame.size = CGSize(width: 1000, height: 600)
                view.workingDirectory = lf.cwd.path
                view.command = buildGhosttyShellCommand(argv: replay.openArgv, env: lf.environment)
                view.createSurface(manager: manager)
                defer { view.destroySurface() }
                let output = round == 1 ? "Antigravity CLI" : draft
                try await waitForSession(view) { view.connectionAcceptsInput && terminalViewport(view).contains(output) }
                let flags = NSEvent.ModifierFlags.shift
                for (c,k,m): (String,UInt16,NSEvent.ModifierFlags) in [("q",12,[]),("?",44,flags),("A",0,flags),("x",7,[])] {
                    view.keyDown(with: try terminalKeyEvent(c,k,m))
                    view.keyUp(with: try terminalKeyEvent(c,k,m,type:.keyUp))
                }
                draft += "q?Ax"
                try await waitForSession(view) { terminalViewport(view).contains(draft) }
                print("agy draft round \(round): \(draft)")
                if round == 1 {
                    await pool.release(terminal)?.value
                    let data = try await lf.run(["session","resume",record.id,"--prepare","--json"])
                    replay = try JSONDecoder().decode(SessionRecord.self, from: Data(data.utf8))
                }
            }
            let capabilities = try String(contentsOf: lf.cwd.appendingPathComponent("agy-qa.terminal"), encoding: .utf8)
            print("agy terminal settings: \(capabilities)")
        }
    }
}
#endif
