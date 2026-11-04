import Foundation
import Testing
@testable import LoopflowMac

@Suite(.serialized)
@MainActor
struct ServerConnectionTests {
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

    private func publish(_ connection: ServerConnection, generation: UInt64, state: String, input: Bool, message: String? = nil) async throws {
        let data = try JSONSerialization.data(withJSONObject: [
            "generation": generation, "state": state, "input": input, "message": message as Any? ?? NSNull()
        ])
        try data.write(to: URL(fileURLWithPath: connection.path), options: .atomic)
        let deadline = ContinuousClock.now + .seconds(2)
        while connection.reading.generation != generation || connection.reading.state.rawValue != state
                || connection.reading.input != input || connection.reading.message != message {
            try #require(ContinuousClock.now < deadline)
            try await Task.sleep(for: .milliseconds(20))
        }
    }
}
