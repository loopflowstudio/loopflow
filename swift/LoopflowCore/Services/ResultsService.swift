// Service for capturing git state and computing session results.

import Foundation

public struct ResultsService: @unchecked Sendable {
    public init() {}

    public func captureBaseline(stepRunId: String, worktree: URL) async -> StepRunBaseline {
        let headSHA = await getHeadSHA(in: worktree)
        let dirtyFiles = await getDirtyFiles(in: worktree)

        return StepRunBaseline(
            stepRunId: stepRunId,
            worktree: worktree.path(),
            headSHA: headSHA,
            dirtyFiles: dirtyFiles,
            timestamp: Date()
        )
    }

    public func computeResults(
        baseline: StepRunBaseline,
        step: String,
        status: StepRunResultStatus,
        startedAt: Date,
        endedAt: Date
    ) async -> StepRunResult {
        let worktree = URL(fileURLWithPath: baseline.worktree)
        let currentSHA = await getHeadSHA(in: worktree)
        let newCommits = await getCommitMessages(from: baseline.headSHA, to: currentSHA, in: worktree)
        let filesChanged = await getChangedFiles(from: baseline.headSHA, in: worktree)

        return StepRunResult(
            id: baseline.stepRunId,
            step: step,
            worktree: baseline.worktree,
            status: status,
            startedAt: startedAt,
            endedAt: endedAt,
            filesChanged: filesChanged,
            newCommits: newCommits,
            baselineSHA: baseline.headSHA,
            currentSHA: currentSHA
        )
    }

    public func loadDiffPreview(for file: FileChange, baselineSHA: String, in worktree: URL) async -> String? {
        // Get first 30 lines of diff for this specific file
        let args = ["diff", "\(baselineSHA)..HEAD", "--", file.path]
        guard let output = try? await runGit(args, in: worktree) else { return nil }

        let lines = output.components(separatedBy: "\n")
        let preview = lines.prefix(30).joined(separator: "\n")
        return preview.isEmpty ? nil : preview
    }

    // MARK: - Private

    private func getHeadSHA(in worktree: URL) async -> String {
        guard let output = try? await runGit(["rev-parse", "HEAD"], in: worktree) else {
            return ""
        }
        return output.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func getDirtyFiles(in worktree: URL) async -> [String] {
        guard let output = try? await runGit(["status", "--porcelain"], in: worktree) else {
            return []
        }
        return output.components(separatedBy: "\n")
            .filter { !$0.isEmpty }
            .compactMap { line in
                // Format: "XY filename" or "XY original -> renamed"
                let trimmed = line.trimmingCharacters(in: .whitespaces)
                guard trimmed.count > 3 else { return nil }
                return String(trimmed.dropFirst(3))
            }
    }

    private func getCommitMessages(from: String, to: String, in worktree: URL) async -> [String] {
        guard !from.isEmpty, !to.isEmpty, from != to else { return [] }

        guard let output = try? await runGit(
            ["log", "\(from)..\(to)", "--format=%s", "--reverse"],
            in: worktree
        ) else {
            return []
        }

        return output.components(separatedBy: "\n")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }

    private func getChangedFiles(from baselineSHA: String, in worktree: URL) async -> [FileChange] {
        // Get diff stat from baseline to current HEAD
        // Also include any uncommitted changes
        guard let output = try? await runGit(
            ["diff", "\(baselineSHA)..HEAD", "--numstat"],
            in: worktree
        ) else {
            return []
        }

        var files: [FileChange] = []

        for line in output.components(separatedBy: "\n") {
            let parts = line.split(separator: "\t")
            guard parts.count >= 3 else { continue }

            let addedStr = String(parts[0])
            let removedStr = String(parts[1])
            let path = String(parts[2])

            // Binary files show "-" for line counts
            let added = Int(addedStr) ?? 0
            let removed = Int(removedStr) ?? 0

            let kind: FileChangeKind
            if removed == 0 && added > 0 {
                kind = .added
            } else if added == 0 && removed > 0 {
                kind = .deleted
            } else {
                kind = .modified
            }

            files.append(FileChange(
                path: path,
                kind: kind,
                linesAdded: added,
                linesRemoved: removed
            ))
        }

        return files
    }

    private func runGit(_ args: [String], in directory: URL) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            let pipe = Pipe()

            process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
            process.arguments = args
            process.currentDirectoryURL = directory
            process.standardOutput = pipe
            process.standardError = FileHandle.nullDevice

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8) ?? ""

                if process.terminationStatus == 0 {
                    continuation.resume(returning: output)
                } else {
                    continuation.resume(throwing: NSError(domain: "git", code: Int(process.terminationStatus)))
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }
}
