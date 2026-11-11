#if os(macOS)
import Darwin
import Foundation
import Testing
import Loopflow

@Suite("Uninherited pipes")
struct UninheritedPipeTests {
    /// A terminal pane forks from Desktop with every inheritable descriptor. An
    /// observer reading until end of input must still see it once Desktop closes
    /// its end, while the pane runs on.
    @Test func paneForkedMeanwhileDoesNotHoldTheWriter() throws {
        let pipe = Pipe.uninherited()
        var pane: pid_t = 0
        let argv: [UnsafeMutablePointer<CChar>?] = [strdup("/bin/sleep"), strdup("30"), nil]
        defer { argv.forEach { free($0) } }
        #expect(posix_spawn(&pane, "/bin/sleep", nil, nil, argv, environ) == 0)
        defer {
            kill(pane, SIGTERM)
            var status: Int32 = 0
            waitpid(pane, &status, 0)
        }
        try pipe.fileHandleForWriting.close()
        var fd = pollfd(fd: pipe.fileHandleForReading.fileDescriptor, events: Int16(POLLIN), revents: 0)
        #expect(poll(&fd, 1, 2_000) == 1)
        var byte: UInt8 = 0
        #expect(read(fd.fd, &byte, 1) == 0)
    }
}
#endif
