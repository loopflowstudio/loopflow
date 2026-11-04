import Foundation
import Testing
@testable import LoopflowMac

@Suite(.serialized)
@MainActor
struct MachineConnectionTests {
    @Test func recoveryPreservesReadingAndRequiresAcknowledgedInput() async throws {
        let connection = MachineConnection()
        defer { connection.stop() }
        var visible: MachineConnectionReading?
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
            let data = try JSONSerialization.data(withJSONObject: [
                "generation": 2, "state": state, "input": input, "message": message as Any? ?? NSNull()
            ])
            try data.write(to: URL(fileURLWithPath: connection.path), options: .atomic)
            let deadline = ContinuousClock.now + .seconds(2)
            while visible?.state.rawValue != state || visible?.input != input {
                #expect(ContinuousClock.now < deadline)
                if ContinuousClock.now >= deadline { return }
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(visible?.acceptsInput == (state == "connected" && input))
            if state == "unavailable" { #expect(visible?.title == message) }
        }
        connection.retry()
        #expect(FileManager.default.fileExists(atPath: connection.directory.appendingPathComponent("status.retry").path))
        connection.stop()
        #expect(!FileManager.default.fileExists(atPath: connection.directory.path))
    }
}
