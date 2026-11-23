#if os(macOS) && canImport(GhosttyKit)
import AppKit
import GhosttyKit
import Loopflow
import Testing
@testable import LoopflowMac

@MainActor
struct SessionModifierInputTests {
    @Test("A held Session receives modified Returns once before and after reopening")
    func modifiedReturnAcrossReattachment() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        try #require(manager.state == .ready)

        try await withPreparedSession(provider: #"""
        #!/usr/bin/perl
        use strict;
        use warnings;
        if (($ARGV[0] // '') eq '--version') { print "2.1.302\n"; exit }
        open STDIN, '<', '/dev/tty' or die $!;
        system('stty', 'raw', '-echo') == 0 or die 'stty';
        $| = 1;
        print "\e[>1uready\r\n";
        $/ = 'x';
        for my $round (1..2) {
            alarm 20;
            my $keys = <STDIN>;
            defined($keys) && chomp($keys) or die 'incomplete batch';
            printf "bytes-%d=%s\r\n", $round, unpack('H*', $keys);
        }
        # Keep the result on screen until Desktop acknowledges it.
        <STDIN>;
        """#) { lf, _, initial in
            let pool = GhosttySurfacePool()
            var record = initial
            var ready = "ready"
            for round in 1...2 {
                let terminal = TerminalIdentity.session(record.id, machineId: fixtureMachineId)
                let view = pool.view(for: terminal)
                view.frame.size = CGSize(width: 800, height: 480)
                view.workingDirectory = lf.cwd.path
                view.command = buildGhosttyShellCommand(argv: record.openArgv, env: lf.environment)
                view.createSurface(manager: manager)
                defer { view.destroySurface() }
                let surface = try #require(view.surface)

                try await waitForLine(ready, in: view)
                for flags: NSEvent.ModifierFlags in [[], .shift, .control] {
                    view.keyDown(with: try terminalKeyEvent("\r", 36, flags))
                }
                // An ordinary x delimits the entire batch, including any duplicate final chord.
                view.keyDown(with: try terminalKeyEvent("x", 7))
                ready = "bytes-\(round)=0d1b5b31333b32751b5b31333b3575"
                try await waitForLine(ready, in: view)
                if round == 1 {
                    await pool.release(terminal)?.value
                    let prepared = try await lf.run(["session", "resume", record.id, "--prepare", "--json"])
                    record = try JSONDecoder().decode(SessionRecord.self, from: Data(prepared.utf8))
                } else {
                    view.keyDown(with: try terminalKeyEvent("x", 7))
                    try await waitForSession(view) {
                        view.childExited || ghostty_surface_process_exited(surface)
                    }
                }
            }
        }
    }

    private func waitForLine(_ line: String, in view: GhosttyMetalView) async throws {
        try await waitForSession(view) {
            view.connectionAcceptsInput && terminalViewport(view).split(separator: "\n").contains(Substring(line))
        }
    }
}
#endif
