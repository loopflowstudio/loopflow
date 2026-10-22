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
