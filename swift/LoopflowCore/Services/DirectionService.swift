// Service for loading directions from .lf/directions/*.md files.

import Foundation

public struct DirectionService: @unchecked Sendable {
    public init() {}
    public func loadDirections(from repoURL: URL) -> [Direction] {
        let directionsDir = repoURL.appendingPathComponent(".lf/directions")
        let fm = FileManager.default

        guard fm.fileExists(atPath: directionsDir.path) else {
            return []
        }

        guard let files = try? fm.contentsOfDirectory(at: directionsDir, includingPropertiesForKeys: nil) else {
            return []
        }

        return files
            .filter { $0.pathExtension == "md" }
            .compactMap { parseDirectionFile($0) }
            .sorted { $0.name < $1.name }
    }

    private func parseDirectionFile(_ url: URL) -> Direction? {
        guard let content = try? String(contentsOf: url, encoding: .utf8) else {
            return nil
        }

        let name = url.deletingPathExtension().lastPathComponent

        return Direction(
            id: name,
            name: name,
            content: content.trimmingCharacters(in: .whitespacesAndNewlines),
            path: url
        )
    }
}
