#if os(macOS)
import Foundation
import Testing

/// Native proofs place their Task sessions and companion panes in this checkout.
func placingTaskWorktrees(in data: Data, at path: String) throws -> Data {
    var snapshot = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    var tasks = try #require(snapshot["tasks"] as? [String: Any])
    if var items = tasks["items"] as? [[String: Any]] {
        for ti in items.indices {
            var reference = try #require(items[ti]["reference"] as? [String: Any])
            guard var workspace = reference["workspace"] as? [String: Any] else { continue }
            workspace["worktree"] = path
            reference["workspace"] = workspace
            items[ti]["reference"] = reference
        }
        tasks["items"] = items
    }
    snapshot["tasks"] = tasks
    return try JSONSerialization.data(withJSONObject: snapshot)
}
#endif
