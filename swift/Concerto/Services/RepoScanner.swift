// Scan local directories for git repositories.

import Foundation

struct RepoScanner {
    var fileManager: FileManager = .default

    func scanMainWorktrees(in root: URL) -> [URL] {
        guard let children = try? fileManager.contentsOfDirectory(
            at: root,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }

        return children
            .filter { isDirectory($0) }
            .filter(isMainGitWorktree)
            .map(\.standardizedFileURL)
            .sorted { lhs, rhs in
                lhs.lastPathComponent.localizedCaseInsensitiveCompare(rhs.lastPathComponent) == .orderedAscending
            }
    }

    func scanDefaultRoot() -> [URL] {
        let root = fileManager.homeDirectoryForCurrentUser.appendingPathComponent("src", isDirectory: true)
        return scanMainWorktrees(in: root)
    }

    private func isDirectory(_ url: URL) -> Bool {
        (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true
    }

    private func isMainGitWorktree(_ url: URL) -> Bool {
        let gitMarker = url.appendingPathComponent(".git")
        var isDirectory = ObjCBool(false)
        let exists = fileManager.fileExists(atPath: gitMarker.normalizedFilePath, isDirectory: &isDirectory)
        guard exists else { return false }

        if isDirectory.boolValue {
            return true
        }

        guard let content = try? String(contentsOf: gitMarker, encoding: .utf8) else {
            return false
        }

        return !content.contains("/.git/worktrees/")
    }
}
