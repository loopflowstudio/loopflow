// Validate and normalize explicit repository opening paths.

import Foundation

struct RepositoryPath {
    /// Collapse a Git working-tree root to its main worktree.
    ///
    /// Non-repositories and plain subdirectories return nil. A linked worktree
    /// resolves to the checkout that owns the shared `.git` directory.
    func mainRepository(_ url: URL) -> URL? {
        guard isDirectory(url),
              let topLevel = git(["rev-parse", "--show-toplevel"], at: url),
              canonical(topLevel) == canonical(url.normalizedFilePath),
              let commonDir = git(
                  ["rev-parse", "--path-format=absolute", "--git-common-dir"],
                  at: url
              )
        else {
            return nil
        }
        let main = URL(fileURLWithPath: commonDir)
            .deletingLastPathComponent()
            .standardizedFileURL
        guard isDirectory(main),
              let mainTopLevel = git(["rev-parse", "--show-toplevel"], at: main),
              canonical(mainTopLevel) == canonical(main.normalizedFilePath)
        else {
            return nil
        }
        return main
    }

    private func git(_ args: [String], at url: URL) -> String? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", url.normalizedFilePath] + args
        let output = Pipe()
        process.standardOutput = output
        process.standardError = Pipe()

        do {
            try process.run()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else { return nil }
        } catch {
            return nil
        }

        let data = output.fileHandleForReading.readDataToEndOfFile()
        guard let raw = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines),
            !raw.isEmpty
        else {
            return nil
        }
        return raw
    }

    private func canonical(_ path: String) -> String {
        URL(fileURLWithPath: path).resolvingSymlinksInPath().standardizedFileURL.path
    }

    private func isDirectory(_ url: URL) -> Bool {
        (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true
    }
}
