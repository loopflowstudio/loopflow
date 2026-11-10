#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import LoopflowMac

@Suite(.serialized)
@MainActor
struct ServerConnectionTests {
    @Test func openingWaitsForNativeReadinessAndRecoversInPlace() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        #expect(connection.reading.title == "Opening… Input paused.")
        #expect(!connection.reading.acceptsInput)
        let relay = try Relay(connection)
        try relay.publish(generation: 1, state: "connected", input: true, nativeReady: false)
        try await wait { connection.reading.state == .connected }
        #expect(!connection.reading.acceptsInput)
        #expect(connection.reading.title == ServerConnectionReading.openingTitle)
        try relay.publish(generation: 1, state: "connected", input: true)
        try await wait { connection.reading.acceptsInput }
        #expect(connection.reading.title == nil)
        try relay.publish(generation: 2, state: "reconnecting", input: false)
        try await wait { connection.reading.state == .reconnecting }
        #expect(!connection.reading.acceptsInput)
        try relay.publish(generation: 2, state: "connected", input: false)
        try await wait { connection.reading.canMoveHere }
        #expect(connection.reading.title == "Read-only. Move here to type.")
    }

    @Test func providerTransitionSurvivesTheStatusStreamAndWaitsForNativeReadiness() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        let relay = try Relay(connection)
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/provider_transition.json")
        let transition = try JSONSerialization.jsonObject(with: Data(contentsOf: fixture))
        var reading: [String: Any] = ["generation": 1, "state": "connected", "input": true,
                                    "native_ready": false, "message": NSNull(), "transition": transition]
        var bytes = try JSONSerialization.data(withJSONObject: reading)
        bytes.append(10)
        try relay.write(bytes)
        try await wait { connection.reading.transition != nil }
        #expect(!connection.reading.acceptsInput)
        #expect(connection.reading.title == ServerConnectionReading.openingTitle)
        reading["native_ready"] = true
        bytes = try JSONSerialization.data(withJSONObject: reading)
        bytes.append(10)
        try relay.write(bytes)
        try await wait { connection.reading.acceptsInput }
        #expect(connection.reading.title == "Forked from earlier; original was running at fork")
    }

    @Test func brokenStatusStreamPausesInputAndFreshStreamRestoresIt() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        var relay: Relay? = try Relay(connection)
        try relay?.publish(generation: 1, state: "connected", input: true)
        try await wait { connection.reading.acceptsInput }
        relay = nil
        try await wait { connection.reading.state == .unavailable }
        #expect(!connection.reading.acceptsInput)
        #expect(!connection.canRetry)
        relay = try Relay(connection)
        try relay?.publish(generation: 2, state: "connected", input: true)
        try await wait { connection.reading.acceptsInput }
        #expect(connection.reading.generation == 2)
    }

    @Test func fragmentedReadingsWaitAndMalformedReadingsRevokeInput() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        let relay = try Relay(connection)
        let bytes = try Relay.reading(generation: 1, state: "connected", input: true)
        try relay.write(bytes.prefix(10))
        try await Task.sleep(for: .milliseconds(60))
        #expect(!connection.reading.acceptsInput)
        try relay.write(bytes.dropFirst(10))
        try await wait { connection.reading.acceptsInput }
        try relay.write(Data("{\n".utf8))
        try await wait { connection.reading.state == .unavailable }
        #expect(!connection.reading.acceptsInput)
    }

    @Test func moveAndRetryAreDeliberateAndNeverReplayed() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        var relay: Relay? = try Relay(connection)
        try relay?.publish(generation: 2, state: "connected", input: false)
        try await wait { connection.reading.canMoveHere }
        connection.moveHere()
        #expect(connection.reading.state == .moving)
        #expect(!connection.reading.acceptsInput)
        #expect(try relay?.action()["generation"] as? Int == 2)
        try relay?.publish(generation: 2, state: "connected", input: true)
        try await Task.sleep(for: .milliseconds(60))
        #expect(connection.reading.state == .moving)
        relay = nil
        try await wait { connection.reading.state == .unavailable }
        relay = try Relay(connection)
        try relay?.publish(generation: 3, state: "unavailable", input: false)
        try await wait { connection.reading.generation == 3 }
        connection.retry()
        #expect(try relay?.action()["kind"] as? String == "retry")
        try relay?.publish(generation: 3, state: "connected", input: false)
        try await wait { connection.reading.canMoveHere }
        #expect(try relay?.pendingAction() == nil)
        connection.moveHere()
        #expect(try relay?.action()["generation"] as? Int == 3)
        try relay?.publish(generation: 4, state: "connected", input: true)
        try await wait { connection.reading.acceptsInput }
    }

    @Test func closingSurfaceDropsStatusAndIgnoresQueuedReadings() async throws {
        var connection: ServerConnection? = ServerConnection()
        let directory = try #require(connection?.directory)
        var updates = 0
        try connection?.start { _ in updates += 1 }
        let relay = try Relay(try #require(connection))
        try relay.publish(generation: 1, state: "connected", input: true)
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

        static func reading(generation: UInt64, state: String, input: Bool, nativeReady: Bool = true) throws -> Data {
            var bytes = try JSONSerialization.data(withJSONObject: ["generation": generation, "state": state,
                "input": input, "native_ready": nativeReady, "message": NSNull()])
            bytes.append(10)
            return bytes
        }
        func publish(generation: UInt64, state: String, input: Bool, nativeReady: Bool = true) throws {
            try write(Self.reading(generation: generation, state: state, input: input, nativeReady: nativeReady))
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
