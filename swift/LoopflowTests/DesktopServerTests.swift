#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Desktop socket API", .serialized)
@MainActor
struct DesktopServerTests {
    @Test("A client reads state, gets typed errors, and receives the first change frame")
    func servesStateErrorsAndEvents() async throws {
        let path = "/tmp/lfds-\(UUID().uuidString.prefix(8))/run/desktop/test.sock"
        let server = try DesktopServer(path: path,
            inspect: { DesktopInspection(observedAt: 7, windows: [], openings: []) },
            perform: { _ in throw DesktopTargetError("No such pane; nothing changed.") },
            readText: { _ in throw DesktopArgumentError("bad") })
        defer { _ = server }
        var info = stat()
        #expect(stat(path, &info) == 0 && info.st_mode & 0o777 == 0o600)

        let lines = try await Task.detached {
            try SocketClient(path: path).exchange([
                #"{"v":1,"id":1,"method":"inspect","params":{}}"#,
                #"{"v":1,"id":2,"method":"perform","params":{"repository":"r","window":"w","pane":null,"action":"back","args":{}}}"#,
                #"{"v":9,"id":3,"method":"inspect","params":{}}"#,
                #"{"v":1,"id":4,"method":"events","params":{}}"#,
            ], expecting: 5)
        }.value
        let replies = try lines.map { try JSONSerialization.jsonObject(with: Data($0.utf8)) as! [String: Any] }
        #expect((replies[0]["result"] as? [String: Any])?["observed_at"] as? Int == 7)
        #expect((replies[1]["error"] as? [String: Any])?["code"] as? String == "target")
        #expect((replies[2]["error"] as? [String: Any])?["code"] as? String == "version")
        #expect(replies[3]["id"] as? Int == 4)
        let change = try JSONDecoder().decode([String: DesktopChange].self, from: Data(lines[4].utf8))
        #expect(change["event"] == DesktopChange(observedAt: 7, windows: [], closed: []))
    }
}

/// A blocking client, as `lf desktop` is; run it off the main actor the server answers on.
private struct SocketClient {
    let fd: Int32

    init(path: String) throws {
        fd = socket(AF_UNIX, SOCK_STREAM, 0)
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: path.utf8) }
        let connected = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard connected == 0 else { throw RegistryQueryError("connect failed") }
    }

    func exchange(_ requests: [String], expecting count: Int) throws -> [String] {
        defer { close(fd) }
        let payload = requests.map { $0 + "\n" }.joined()
        _ = payload.withCString { write(fd, $0, strlen($0)) }
        var received = Data()
        var buffer = [UInt8](repeating: 0, count: 65536)
        while received.filter({ $0 == 10 }).count < count {
            let read = Darwin.read(fd, &buffer, buffer.count)
            guard read > 0 else { break }
            received.append(contentsOf: buffer.prefix(read))
        }
        return String(decoding: received, as: UTF8.self).split(separator: "\n").map(String.init)
    }
}
#endif
