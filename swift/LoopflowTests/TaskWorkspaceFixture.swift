#if os(macOS)
import Foundation
import Testing

/// Native proofs place their Task sessions and companion panes in this checkout.
func placingTaskWorktrees(in data: Data, at path: String) throws -> Data {
    var snapshot = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    var waves = try #require(snapshot["waves"] as? [[String: Any]])
    for wi in waves.indices {
        var tasks = try #require(waves[wi]["tasks"] as? [String: Any])
        guard var items = tasks["items"] as? [[String: Any]] else { continue }
        for ti in items.indices {
            var reference = try #require(items[ti]["reference"] as? [String: Any])
            guard var workspace = reference["workspace"] as? [String: Any] else { continue }
            workspace["worktree"] = path
            reference["workspace"] = workspace
            items[ti]["reference"] = reference
        }
        tasks["items"] = items
        waves[wi]["tasks"] = tasks
    }
    snapshot["waves"] = waves
    return try JSONSerialization.data(withJSONObject: snapshot)
}
#endif
