// Service for loading goals from .lf/goals/*.md files.

import Foundation

public struct GoalService: @unchecked Sendable {
    public init() {}
    public func loadGoals(from repoURL: URL) -> [Goal] {
        let goalsDir = repoURL.appendingPathComponent(".lf/goals")
        let fm = FileManager.default

        guard fm.fileExists(atPath: goalsDir.path) else {
            return []
        }

        guard let files = try? fm.contentsOfDirectory(at: goalsDir, includingPropertiesForKeys: nil) else {
            return []
        }

        return files
            .filter { $0.pathExtension == "md" }
            .compactMap { parseGoalFile($0) }
            .sorted { $0.name < $1.name }
    }

    private func parseGoalFile(_ url: URL) -> Goal? {
        guard let content = try? String(contentsOf: url, encoding: .utf8) else {
            return nil
        }

        let name = url.deletingPathExtension().lastPathComponent

        return Goal(
            id: name,
            name: name,
            content: content.trimmingCharacters(in: .whitespacesAndNewlines),
            path: url
        )
    }
}
