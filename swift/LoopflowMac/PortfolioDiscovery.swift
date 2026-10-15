import Foundation
import Loopflow

enum PortfolioDiscovery {
    nonisolated static func resolveLaunchRepo(_ initialPath: String) -> String? {
        let scanner = RepositoryPath()
        let candidate = URL(fileURLWithPath: initialPath)
        guard let mainURL = scanner.mainRepository(candidate) else { return nil }
        return mainURL.normalizedFilePath
    }

    static func authoredWaves(in repos: [PortfolioRepo]) async -> [String: [String]] {
        await Task.detached {
            var result: [String: [String]] = [:]
            for repo in repos {
                result[repo.path] = authoredWaves(inRepo: repo.path)
            }
            return result
        }.value
    }

    nonisolated static func authoredWaves(inRepo repoPath: String) -> [String] {
        let waveDir = URL(fileURLWithPath: repoPath).appendingPathComponent(".lf/wave", isDirectory: true)
        let fileManager = FileManager.default
        guard let children = try? fileManager.contentsOfDirectory(
            at: waveDir,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }
        return children
            .filter { url in
                var isDirectory = ObjCBool(false)
                let goal = url.appendingPathComponent("GOAL.md")
                return fileManager.fileExists(atPath: goal.path, isDirectory: &isDirectory)
                    && !isDirectory.boolValue
            }
            .map(\.lastPathComponent)
            .sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
    }
}
