import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Telemetry codebases")
struct TelemetryCodebaseTests {
    private func run(_ session: String, repo: String?, collected: Bool) throws -> SessionHistory {
        var row = try loadFixture("session_history_summary.json")
        row["session_id"] = session
        row["repo"] = repo ?? NSNull()
        if !collected { row["origin"] = NSNull() }
        return try JSONDecoder().decode(SessionHistory.self, from: JSONSerialization.data(withJSONObject: row))
    }

    @Test("Collected runs keep other Machines' paths out of the picker and stay visible")
    func collectedRunsStayOutOfThePicker() throws {
        let mac = try run("local", repo: "/Users/me/src/manabot", collected: false)
        let other = try run("other", repo: "/Users/me/src/loopflow", collected: false)
        let modalTask = try run("modal-task", repo: "/Users/me/src/manabot", collected: true)
        let bareModal = try run("bare-modal", repo: "/home/lf/src/manabot", collected: true)
        let usage = [mac, other, modalTask, bareModal]

        #expect(usage.localCodebases == ["/Users/me/src/loopflow", "/Users/me/src/manabot"])
        #expect(usage.shown(in: "/Users/me/src/manabot").map(\.sessionId)
            == ["local", "modal-task", "bare-modal"])
        #expect(usage.shown(in: "/Users/me/src/loopflow").map(\.sessionId) == ["other", "bare-modal"])
        // With one codebase there is nothing to choose between.
        #expect([mac, bareModal].shown(in: "/Users/me/src/manabot").count == 2)
    }
}
