// Service for interacting with `wt` CLI for worktree operations.

import Foundation

struct WorktreeService {
    enum WorktreeError: LocalizedError {
        case commandFailed(String)
        case parseError(String)
        case notFound
        case wtNotInstalled

        var errorDescription: String? {
            switch self {
            case .commandFailed(let msg): return "Worktree command failed: \(msg)"
            case .parseError(let msg): return "Failed to parse worktree data: \(msg)"
            case .notFound: return "Worktree not found"
            case .wtNotInstalled: return "wt not found. Install from https://github.com/anthropics/worktrunk"
            }
        }
    }

    private func findWt() -> URL? {
        // Use login shell to resolve PATH properly
        let process = Process()
        let pipe = Pipe()
        process.executableURL = URL(fileURLWithPath: "/bin/zsh")
        process.arguments = ["-l", "-c", "which wt"]
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            process.waitUntilExit()
            if process.terminationStatus == 0 {
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                if let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !path.isEmpty {
                    return URL(fileURLWithPath: path)
                }
            }
        } catch {
            // Fall through to return nil
        }
        return nil
    }

    func list(in repoURL: URL) async throws -> [Worktree] {
        let output = try await run(["-C", repoURL.path(), "list", "--format", "json", "--full"], in: repoURL)

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
        var args = ["-C", repoURL.path(), "switch", "--create", name]
        if let base = baseBranch {
            args.append(contentsOf: ["--base", base])
        }

        _ = try await run(args, in: repoURL)
    }

    func remove(name: String, in repoURL: URL) async throws {
        _ = try await run(["-C", repoURL.path(), "remove", name], in: repoURL)
    }

    func createPR(in worktreePath: URL, title: String? = nil) async throws {
        _ = try await runLfpr(["create"] + (title.map { ["-t", $0] } ?? []), in: worktreePath)
    }

    func landPR(in worktreePath: URL) async throws {
        _ = try await runLfpr(["land"], in: worktreePath)
    }

    private func findLfpr() -> URL? {
        let process = Process()
        let pipe = Pipe()
        process.executableURL = URL(fileURLWithPath: "/bin/zsh")
        process.arguments = ["-l", "-c", "which lfpr"]
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            process.waitUntilExit()
            if process.terminationStatus == 0 {
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                if let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !path.isEmpty {
                    return URL(fileURLWithPath: path)
                }
            }
        } catch {
            // Fall through
        }
        return nil
    }

    private func runLfpr(_ args: [String], in directory: URL) async throws -> String {
        guard let lfprURL = findLfpr() else {
            throw WorktreeError.commandFailed("lfpr not found. Install loopflow.")
        }

        return try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            let pipe = Pipe()

            process.executableURL = lfprURL
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

    private func run(_ args: [String], in directory: URL) async throws -> String {
        guard let wtURL = findWt() else {
            throw WorktreeError.wtNotInstalled
        }

        return try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            let pipe = Pipe()

            process.executableURL = wtURL
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
