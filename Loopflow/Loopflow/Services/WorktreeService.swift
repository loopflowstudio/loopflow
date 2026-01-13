// Service for interacting with `wt` CLI for worktree operations.

import Foundation

struct WorktreeService {
    enum WorktreeError: LocalizedError {
        case commandFailed(String)
        case parseError(String)
        case notFound

        var errorDescription: String? {
            switch self {
            case .commandFailed(let msg): return "Worktree command failed: \(msg)"
            case .parseError(let msg): return "Failed to parse worktree data: \(msg)"
            case .notFound: return "Worktree not found"
            }
        }
    }

    func list(in repoURL: URL) async throws -> [Worktree] {
        let output = try await run(["wt", "-C", repoURL.path(), "list", "--format", "json", "--full"], in: repoURL)

        guard let data = output.data(using: .utf8) else {
            return []
        }

        let decoder = JSONDecoder()
        let items = try decoder.decode([WorktreeJSON].self, from: data)

        // Filter to actual worktrees (not just branches)
        return items
            .filter { $0.kind != "branch" }
            .map { json in
                let hasWorkspace = checkForCodeWorkspace(at: URL(fileURLWithPath: json.path))
                return Worktree(from: json, hasCodeWorkspace: hasWorkspace)
            }
    }

    func create(name: String, in repoURL: URL, baseBranch: String? = nil) async throws {
        var args = ["wt", "-C", repoURL.path(), "switch", "--create", name]
        if let base = baseBranch {
            args.append(contentsOf: ["--base", base])
        }

        _ = try await run(args, in: repoURL)
    }

    func remove(name: String, in repoURL: URL) async throws {
        _ = try await run(["wt", "-C", repoURL.path(), "remove", name], in: repoURL)
    }

    private func run(_ args: [String], in directory: URL) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            let pipe = Pipe()

            process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            process.arguments = args
            process.currentDirectoryURL = directory
            process.standardOutput = pipe
            process.standardError = pipe

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8) ?? ""

                if process.terminationStatus == 0 {
                    continuation.resume(returning: output)
                } else {
                    continuation.resume(throwing: WorktreeError.commandFailed(output))
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }

    private func checkForCodeWorkspace(at url: URL) -> Bool {
        let fm = FileManager.default
        guard let contents = try? fm.contentsOfDirectory(atPath: url.path()) else {
            return false
        }
        return contents.contains { $0.hasSuffix(".code-workspace") }
    }
}
