import Foundation
import Testing
@testable import Loopflow

/// The first Task of `roadmap_snapshot.json` as the server would send it with
/// these facts changed.
func roadmapTask(condition: String, reason: String, lifecycle: String = "idle",
                 recommended: String? = nil, nextMove: String? = nil, graphHasRun: Bool = true,
                 selectionStates: [String]? = nil, updatedAt: String? = nil, id: String? = nil) throws -> RoadmapTask {
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
    if let updatedAt { try set("runtime", "updated_at", updatedAt) }
    if let id { try set("task", "id", id) }
    row["lifecycle"] = lifecycle
    if let selectionStates { row["selection_states"] = selectionStates }
    if !graphHasRun { row["latest_graph_process"] = NSNull() }
    return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
}
