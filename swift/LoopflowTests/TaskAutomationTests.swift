import Foundation
import Testing
@testable import Loopflow

struct TaskAutomationTests {
    @Test func coverageDoesNotEraseRepairHold() throws {
        let file = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_automation.json")
        let data = try Data(contentsOf: file)
        let status = try JSONDecoder().decode(AutomationStatus.self, from: data)
        #expect(status.coverage == "overdue")
        #expect(status.tasks[1].enabled == false)
        let roundTrip = try JSONDecoder().decode(AutomationStatus.self, from: JSONEncoder().encode(status))
        #expect(roundTrip.lastError == status.lastError)
        #expect(roundTrip.tasks[0].enabled == true)
        #expect(roundTrip.deliveries[0].prNumber == 42)
    }
}
