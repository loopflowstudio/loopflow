#if os(macOS)
import Foundation
import Testing
@testable import LoopflowMac

@Suite(.serialized)
@MainActor
struct ServerConnectionTests {
    @Test func startReadsStatusPublishedBeforeReturning() throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        var visible: ServerConnectionReading?
        try connection.start {
            visible = $0
            if $0.generation == 0 {
                // Stand in for a relay publishing while observation starts.
                try? writeStatus(connection, generation: 1, state: "connected", input: true)
            }
        }
        #expect(visible?.generation == 1)
        #expect(visible?.acceptsInput == true)
    }

    @Test func atomicReplacementsCoalesceToLatestReading() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        var visible: ServerConnectionReading?
        try connection.start { visible = $0 }
        for generation in 1..<20 {
            try writeStatus(connection, generation: UInt64(generation), state: "reconnecting", input: false)
        }
        try await publish(connection, generation: 20, state: "connected", input: true)
        #expect(visible?.generation == 20)
        #expect(visible?.acceptsInput == true)
        // A later replacement must still be observed on the same connection.
        try await publish(connection, generation: 21, state: "ended", input: false)
        #expect(visible?.state == .ended)
    }

    @Test func missingMalformedAndDuplicateReadingsKeepLastStatus() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        var updates = 0
        try connection.start { _ in updates += 1 }
        try await publish(connection, generation: 1, state: "connected", input: true)
        let status = URL(fileURLWithPath: connection.path)
        try FileManager.default.removeItem(at: status)
        try await Task.sleep(for: .milliseconds(100))
        #expect(connection.reading.acceptsInput)
        try Data("{".utf8).write(to: status, options: .atomic)
        try await Task.sleep(for: .milliseconds(100))
        #expect(connection.reading.acceptsInput)
        try writeStatus(connection, generation: 1, state: "connected", input: true)
        try Data().write(to: connection.directory.appendingPathComponent("status.retry"), options: .atomic)
        try await Task.sleep(for: .milliseconds(100))
        #expect(updates == 2)
        try await publish(connection, generation: 2, state: "unavailable", input: false)
        #expect(updates == 3)
    }

    @Test func stopDiscardsQueuedReadingAndRemovesMailbox() async throws {
        let connection = ServerConnection()
        var updates = 0
        try connection.start { _ in updates += 1 }
        try writeStatus(connection, generation: 1, state: "connected", input: true)
        connection.stop()
        connection.stop()
        try await Task.sleep(for: .milliseconds(100))
        #expect(updates == 1)
        #expect(!FileManager.default.fileExists(atPath: connection.directory.path))
        #expect(!FileManager.default.fileExists(atPath: connection.directory.appendingPathExtension("closed").path))
    }

    @Test nonisolated func releaseOutsideMainActorClosesMailboxWithPendingEvents() async throws {
        var connection: ServerConnection? = await ServerConnection()
        let directory = try #require(await connection?.directory)
        try await connection?.start { _ in }
        try Data(#"{"generation":1,"state":"connected","input":true,"message":null}"#.utf8)
            .write(to: directory.appendingPathComponent("status.json"), options: .atomic)
        connection = nil
        try await Task.sleep(for: .milliseconds(100))
        #expect(!FileManager.default.fileExists(atPath: directory.path))
    }

    @Test func recoveryPreservesReadingAndRequiresAcknowledgedInput() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        var visible: ServerConnectionReading?
        try connection.start { visible = $0 }
        #expect(visible?.acceptsInput == false)
        #expect(visible?.title?.contains("Reconnecting") == true)
        let readings: [(String, Bool, String?)] = [
            ("connected", false, nil),
            ("unavailable", false, "Enable Loopflow in Background Items, then Retry."),
            ("connected", true, nil),
            ("ended", false, nil),
        ]
        for (state, input, message) in readings {
            try await publish(connection, generation: 2, state: state, input: input, message: message)
            #expect(visible?.acceptsInput == (state == "connected" && input))
            if state == "unavailable" {
                #expect(visible?.title == message)
                connection.retry()
                #expect(FileManager.default.fileExists(atPath: connection.directory.appendingPathComponent("status.retry").path))
            }
        }
        connection.stop()
        #expect(!FileManager.default.fileExists(atPath: connection.directory.path))
    }

    @Test func moveHereIsConnectionBoundAndWaitsForAcknowledgement() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        var visible: ServerConnectionReading?
        try connection.start { visible = $0 }
        let action = connection.directory.appendingPathComponent("status.move")
        connection.moveHere()
        #expect(!FileManager.default.fileExists(atPath: action.path))
        try await publish(connection, generation: 2, state: "connected", input: false)
        #expect(visible?.canMoveHere == true)
        #expect(visible?.title == "Read-only. Move here to type.")
        connection.moveHere()
        #expect(visible?.state == .moving)
        #expect(visible?.acceptsInput == false)
        #expect(visible?.canMoveHere == false)
        #expect(try JSONDecoder().decode(UInt64.self, from: Data(contentsOf: action)) == 2)
        try FileManager.default.removeItem(at: action) // Relay consumed the action.
        connection.moveHere()
        #expect(!FileManager.default.fileExists(atPath: action.path))
        // The previous status file remains readable while the relay is busy.
        try await Task.sleep(for: .milliseconds(250))
        #expect(visible?.state == .moving)
        try await publish(connection, generation: 3, state: "moving", input: false)
        #expect(visible?.acceptsInput == false)
        try await publish(connection, generation: 3, state: "connected", input: true)
        #expect(visible?.acceptsInput == true)
        #expect(visible?.title == nil)
    }

    @Test func lostMoveAcknowledgementAndClosureNeverRepeatAction() async throws {
        let connection = ServerConnection()
        defer { connection.stop() }
        try connection.start { _ in }
        try await publish(connection, generation: 2, state: "connected", input: false)
        connection.moveHere()
        let action = connection.directory.appendingPathComponent("status.move")
        try FileManager.default.removeItem(at: action)
        try await publish(connection, generation: 4, state: "reconnecting", input: false)
        try await publish(connection, generation: 4, state: "connected", input: false)
        #expect(connection.reading.canMoveHere)
        #expect(!FileManager.default.fileExists(atPath: action.path))
        connection.moveHere() // A new deliberate gesture is allowed.
        #expect(try JSONDecoder().decode(UInt64.self, from: Data(contentsOf: action)) == 4)
        connection.stop()
        connection.moveHere()
        connection.retry()
        #expect(!FileManager.default.fileExists(atPath: connection.directory.path))
    }

    private func writeStatus(_ connection: ServerConnection, generation: UInt64, state: String, input: Bool, message: String? = nil) throws {
        let data = try JSONSerialization.data(withJSONObject: [
            "generation": generation, "state": state, "input": input, "message": message as Any? ?? NSNull()
        ])
        try data.write(to: URL(fileURLWithPath: connection.path), options: .atomic)
    }

    private func publish(_ connection: ServerConnection, generation: UInt64, state: String, input: Bool, message: String? = nil) async throws {
        try writeStatus(connection, generation: generation, state: state, input: input, message: message)
        let deadline = ContinuousClock.now + .seconds(2)
        while connection.reading.generation != generation || connection.reading.state.rawValue != state
                || connection.reading.input != input || connection.reading.message != message {
            try #require(ContinuousClock.now < deadline)
            try await Task.sleep(for: .milliseconds(20))
        }
    }
}
#endif
