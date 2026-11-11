import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Test func sessionRecoveryControlKeepsUncertaintyVisible() throws {
    let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/session.json")
    let session = try JSONDecoder().decode(SessionRecord.self, from: Data(contentsOf: fixture))
    let recovery = try #require(session.recovery)
    var requested = false
    let waiting = SessionRecoveryView(recovery: recovery) { requested = true }
    #expect(try waiting.inspect().find(text: recovery.reason).string() == recovery.reason)
    try waiting.inspect().find(button: "Recover affected sessions").tap()
    #expect(requested)
    var payload = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(recovery)) as? [String: Any])
    payload["state"] = "uncertain"
    payload["request"] = "unconfirmed-attempt"
    let uncertain = try JSONDecoder().decode(SessionRecovery.self, from: JSONSerialization.data(withJSONObject: payload))
    let view = SessionRecoveryView(recovery: uncertain) {}
    #expect(try view.inspect().find(text: uncertain.state.label).string() == uncertain.state.label)
    #expect(throws: (any Error).self) { try view.inspect().find(button: "Recover affected sessions") }
}
