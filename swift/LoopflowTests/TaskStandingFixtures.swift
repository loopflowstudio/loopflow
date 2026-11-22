import Foundation
import Testing
@testable import Loopflow
#if os(macOS)
@testable import LoopflowMac
#endif

/// The first Task of `roadmap_snapshot.json` as the server would send it with
/// these facts changed.
func roadmapTask(condition: String, reason: String, lifecycle: String = "idle",
                 recommended: String? = nil, nextMove: String? = nil, graphHasRun: Bool = true) throws -> RoadmapTask {
    let wire = try #require(JSONSerialization.jsonObject(with: loadFixtureData("roadmap_snapshot.json")) as? [String: Any])
    var row = try #require(((wire["tasks"] as? [String: Any])?["items"] as? [[String: Any]])?.first)
    func set(_ key: String, _ field: String, _ value: String) throws {
        var object = try #require(row[key] as? [String: Any])
        object[field] = value
        row[key] = object
    }
    try set("condition", "state", condition)
    try set("condition", "reason", reason)
    if let recommended { try set("actions", "recommended", recommended) }
    if let nextMove { try set("next_move", "reason", nextMove) }
    row["lifecycle"] = lifecycle
    if !graphHasRun { row["latest_graph_process"] = NSNull() }
    return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
}

#if os(macOS)
/// What a Task's one Session reports.
func programStatus(reporting state: ProgramStatusState, msg: String?) throws -> SessionProgramStatus {
    let report = ProgramStatusReport(state: state, id: nil, kind: nil, progress: nil, app: nil, title: nil, msg: msg)
    var wire = try #require(JSONSerialization.jsonObject(with: loadFixtureData("session.json")) as? [String: Any])
    wire["program_status"] = try JSONSerialization.jsonObject(
        with: JSONEncoder().encode(ProgramStatusRecords(seen: true, records: [report])))
    let session = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wire))
    return SessionProgramStatus(sessions: [session], unavailable: nil)
}
#endif
