import Foundation

func sessionActionFixture(state: String) -> [[String: Any]] {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
    let data = try! Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/session_actions.json"))
    let cases = try! JSONSerialization.jsonObject(with: data) as! [[String: Any]]
    return cases.first { $0["state"] as? String == state }!["actions"] as! [[String: Any]]
}

func sessionActionFixtureJSON(state: String) -> String {
    String(decoding: try! JSONSerialization.data(withJSONObject: sessionActionFixture(state: state)), as: UTF8.self)
}

/// A Task with no conversation yet, whose explanation offers explicit Start.
func taskConversationStartOfferFixture() -> String {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
    let data = try! Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/session_resume_explanations.json"))
    var report = (try! JSONSerialization.jsonObject(with: data) as! [[String: Any]])[0]
    report["state"] = NSNull()
    report["actions"] = [["kind": "open", "label": "Start conversation", "help": "No saved provider history; start this conversation explicitly", "unavailable_reason": NSNull()]]
    report["impediments"] = [String]()
    report["unavailable"] = [String]()
    return String(decoding: try! JSONSerialization.data(withJSONObject: report), as: UTF8.self)
}
