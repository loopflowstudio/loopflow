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

    /// Collapse a candidate repo directory to its main worktree. A linked worktree
    /// resolves to the repo that owns the shared `.git` dir; a main repo (or any
    /// non-git path) resolves to itself. Concerto targets the main repo, never a
    /// worktree.
    func resolveMainWorktree(_ url: URL) -> URL {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = [
            "git", "-C", url.normalizedFilePath,
            "rev-parse", "--path-format=absolute", "--git-common-dir",
        ]
        let output = Pipe()
        process.standardOutput = output
        process.standardError = Pipe()

        do {
            try process.run()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else { return url.standardizedFileURL }
        } catch {
            return url.standardizedFileURL
        }

        let data = output.fileHandleForReading.readDataToEndOfFile()
        guard let raw = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines),
            !raw.isEmpty
        else {
            return url.standardizedFileURL
        }

        // `--git-common-dir` points at the shared `<main-repo>/.git`; its parent
        // is the main repo root.
        return URL(fileURLWithPath: raw).deletingLastPathComponent().standardizedFileURL
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

