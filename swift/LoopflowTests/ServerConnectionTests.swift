#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import LoopflowMac

@Suite(.serialized)
@MainActor
struct ServerConnectionTests {
    @Test(arguments: [
        nil,
        "Session terminal replaced; reattach by Session.",
        "Session terminal retired; resume when needed.",
    ] as [String?])
    func orderlyCloseShowsItsReasonWithoutOfferingRecovery(message: String?) async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        var relay: Relay? = try Relay(connection)
        var bytes = try JSONSerialization.data(withJSONObject: [
            "generation": 1, "state": "ended", "native_ready": false,
            "message": message as Any? ?? NSNull()
        ])
        bytes.append(10)
        try relay?.write(bytes)
        try await wait { connection.reading.state == .ended }
        #expect(connection.reading.title == (message ?? "Session ended."))
        #expect(!connection.reading.acceptsInput)
        #expect(!connection.canRetry)
        connection.retry()
        #expect(try relay?.pendingAction() == nil)
        relay = nil
        try await Task.sleep(for: .milliseconds(60))
        #expect(connection.reading.state == .ended)
    }

    @Test func openingWaitsForNativeReadinessAndRecoversInPlace() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        #expect(connection.reading.title == nil)
        #expect(!connection.reading.acceptsInput)
        let relay = try Relay(connection)
        try relay.publish(generation: 1, state: "connected", nativeReady: false)
        try await wait { connection.reading.state == .connected }
        #expect(!connection.reading.acceptsInput)
        #expect(connection.reading.title == nil)
        #expect(connection.reading.attachment == .opening)
        try relay.publish(generation: 1, state: "connected")
        try await wait { connection.reading.acceptsInput }
        #expect(connection.reading.title == nil)
        #expect(connection.reading.attachment == .usable)
        try relay.publish(generation: 2, state: "reconnecting")
        try await wait { connection.reading.state == .reconnecting }
        #expect(connection.reading.title == "Reconnecting… Input paused; last screen may be stale.")
        #expect(!connection.reading.acceptsInput)
        try relay.publish(generation: 2, state: "connected")
        try await wait { connection.reading.acceptsInput }
        #expect(connection.reading.title == nil)
        #expect(connection.reading.attachment == .usable)
    }

    @Test func resumedRelayPayloadKeepsHeaderHiddenAcrossNativeReadiness() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        let relay = try Relay(connection)
        let transition: [String: Any] = [
            "kind": "resumed", "from_provider_session": "thread", "to_provider_session": "thread",
            "from_agent_process": "44444444-4444-4444-8444-444444444444",
            "to_agent_process": "55555555-5555-4555-8555-555555555555",
            "original_running": NSNull(), "seed": NSNull(), "observed_at": 1794100000,
        ]
        for ready in [false, true] {
            var bytes = try JSONSerialization.data(withJSONObject: [
                "generation": 1, "state": "connected", "native_ready": ready,
                "message": NSNull(), "transition": transition,
            ])
            bytes.append(10)
            try relay.write(bytes)
            try await wait { connection.reading.state == .connected && connection.reading.nativeReady == ready }
            #expect(connection.reading.title == nil)
            #expect(connection.reading.acceptsInput == ready)
            #expect(connection.reading.attachment == (ready ? .usable : .opening))
        }
    }

    @Test func brokenStatusStreamPausesInputAndFreshStreamRestoresIt() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        var relay: Relay? = try Relay(connection)
        try relay?.publish(generation: 1, state: "connected")
        try await wait { connection.reading.acceptsInput }
        relay = nil
        try await wait { connection.reading.state == .unavailable }
        #expect(!connection.reading.acceptsInput)
        #expect(connection.reading.title == "Connection status unavailable. Input paused.")
        #expect(!connection.canRetry)
        relay = try Relay(connection)
        try relay?.publish(generation: 2, state: "connected")
        try await wait { connection.reading.acceptsInput }
        #expect(connection.reading.generation == 2)
    }

    @Test func fragmentedReadingsWaitAndMalformedReadingsRevokeInput() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        let relay = try Relay(connection)
        let bytes = try Relay.reading(generation: 1, state: "connected")
        try relay.write(bytes.prefix(10))
        try await Task.sleep(for: .milliseconds(60))
        #expect(!connection.reading.acceptsInput)
        try relay.write(bytes.dropFirst(10))
        try await wait { connection.reading.acceptsInput }
        try relay.write(Data("{\n".utf8))
        try await wait { connection.reading.state == .unavailable }
        #expect(!connection.reading.acceptsInput)
    }

    @Test func retryIsDeliberateAndNeverReplayed() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        let relay = try Relay(connection)
        try relay.publish(generation: 3, state: "unavailable")
        try await wait { connection.reading.generation == 3 }
        #expect(connection.reading.title == "Session unavailable. Input paused; last screen may be stale.")
        #expect(connection.canRetry)
        connection.retry()
        #expect(try relay.action() as? [String: String] == ["kind": "retry"])
        try relay.publish(generation: 3, state: "connected")
        try await wait { connection.reading.acceptsInput }
        connection.retry()
        #expect(try relay.pendingAction() == nil)
    }

    @Test func closingSurfaceDropsStatusAndIgnoresQueuedReadings() async throws {
        var connection: ServerConnection? = ServerConnection()
        let directory = try #require(connection?.directory)
        var updates = 0
        try connection?.start { _ in updates += 1 }
        let relay = try Relay(try #require(connection))
        try relay.publish(generation: 1, state: "connected")
        connection?.stop()
        connection = nil
        try await Task.sleep(for: .milliseconds(60))
        #expect(updates == 1)
        #expect(!FileManager.default.fileExists(atPath: directory.path))
    }

    private func wait(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(2)
        while !condition() {
            try #require(ContinuousClock.now < deadline)
            try await Task.sleep(for: .milliseconds(10))
        }
    }

    private final class Relay {
        let fd: Int32
        @MainActor init(_ connection: ServerConnection) throws {
            fd = socket(AF_UNIX, SOCK_STREAM, 0)
            var address = sockaddr_un()
            address.sun_family = sa_family_t(AF_UNIX)
            withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: connection.path.utf8) }
            let result = withUnsafePointer(to: &address) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                    Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
                }
            }
            #expect(result == 0)
            var enabled: Int32 = 1
            setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &enabled, socklen_t(MemoryLayout<Int32>.size))
            _ = fcntl(fd, F_SETFL, O_NONBLOCK)
        }
        deinit { close(fd) }

        static func reading(generation: UInt64, state: String, nativeReady: Bool = true) throws -> Data {
            var bytes = try JSONSerialization.data(withJSONObject: ["generation": generation, "state": state,
                "native_ready": nativeReady, "message": NSNull()])
            bytes.append(10)
            return bytes
        }
        func publish(generation: UInt64, state: String, nativeReady: Bool = true) throws {
            try write(Self.reading(generation: generation, state: state, nativeReady: nativeReady))
        }
        func write(_ bytes: Data) throws {
            let sent = bytes.withUnsafeBytes { Darwin.send(fd, $0.baseAddress, $0.count, 0) }
            try #require(sent == bytes.count)
        }
        func pendingAction() throws -> [String: Any]? {
            var bytes = [UInt8](repeating: 0, count: 4096)
            let count = Darwin.read(fd, &bytes, bytes.count)
            if count < 0 && errno == EAGAIN { return nil }
            try #require(count > 0)
            return try JSONSerialization.jsonObject(with: Data(bytes.prefix(count))) as? [String: Any]
        }
        func action() throws -> [String: Any] { try #require(try pendingAction()) }
    }
}
#endif
