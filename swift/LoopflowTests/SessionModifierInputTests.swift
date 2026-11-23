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
        print "\e[>1u";
        open my $report, '>', "$ENV{HOME}/input.tmp" or die $!;
        for my $round (1..2) {
            print "ready-$round\r\n";
            alarm 20;
            my $keys = '';
            while (1) {
                sysread(STDIN, my $byte, 1) == 1 or die 'PTY closed';
                last if $byte eq 'x';
                $keys .= $byte;
                die 'missing delimiter' if length($keys) > 128;
            }
            alarm 0;
            my $hex = unpack('H*', $keys);
            print "bytes-$round=$hex\r\n";
            print {$report} "$hex\n";
        }
        close $report or die $!;
        rename "$ENV{HOME}/input.tmp", "$ENV{HOME}/input.hex" or die $!;
        """#) { lf, _, initial in
            let pool = GhosttySurfacePool()
            var record = initial
            for round in 1...2 {
                let terminal = TerminalIdentity.session(record.id, machineId: fixtureMachineId)
                let view = pool.view(for: terminal)
                view.frame.size = CGSize(width: 800, height: 480)
                view.workingDirectory = lf.cwd.path
                view.command = buildGhosttyShellCommand(argv: record.openArgv, env: lf.environment)
                view.createSurface(manager: manager)
                defer { view.destroySurface() }
                _ = try #require(view.surface)

                try await waitForLine("ready-\(round)", in: view)
                for flags: NSEvent.ModifierFlags in [[], .shift, .control] {
                    try send("\r", keyCode: 36, flags: flags, to: view)
                }
                // An ordinary x delimits the entire batch, including any duplicate final chord.
                try send("x", keyCode: 7, to: view)
                if round == 1 {
                    try await waitForLine("ready-2", in: view)
                    await pool.release(terminal)?.value
                    let prepared = try await lf.run(["session", "resume", record.id, "--prepare", "--json"])
                    record = try JSONDecoder().decode(SessionRecord.self, from: Data(prepared.utf8))
                } else {
                    let report = lf.cwd.appendingPathComponent("input.hex")
                    let deadline = ContinuousClock.now + .seconds(20)
                    while !FileManager.default.fileExists(atPath: report.path) {
                        try #require(ContinuousClock.now < deadline, "Provider did not finish the second batch")
                        try await Task.sleep(for: .milliseconds(20))
                    }
                    let expected = "0d1b5b31333b32751b5b31333b3575\n"
                    #expect(try String(contentsOf: report, encoding: .utf8) == expected + expected)
                }
            }
        }
    }

    private func send(_ text: String, keyCode: UInt16, flags: NSEvent.ModifierFlags = [],
                      to view: GhosttyMetalView) throws {
        let event = try #require(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags,
            timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: 0, context: nil,
            characters: text, charactersIgnoringModifiers: text, isARepeat: false, keyCode: keyCode))
        view.keyDown(with: event)
    }

    private func waitForLine(_ line: String, in view: GhosttyMetalView) async throws {
        let deadline = ContinuousClock.now + .seconds(20)
        while true {
            GhosttyManager.shared.tick()
            let screen = try view.readText(region: .screen, maxBytes: 16384)
            if case .available(let text, _) = screen, view.connectionAcceptsInput,
               text.components(separatedBy: "\n").contains(line) { return }
            try #require(ContinuousClock.now < deadline && !view.childExited, "Waiting for \(line) in \(screen)")
            try await Task.sleep(for: .milliseconds(20))
        }
    }
}
#endif
