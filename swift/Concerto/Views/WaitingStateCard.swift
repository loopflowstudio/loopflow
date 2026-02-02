// WaitingStateCard - shows why a wave is blocked and provides action to resolve it.

import SwiftUI
import LoopflowCore

struct WaitingStateCard: View {
    let wave: Wave

    @Environment(\.colorScheme) private var colorScheme

    private let terminalLauncher = TerminalLauncher()
    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            // Status line
            HStack(spacing: Spacing.sm) {
                Image(systemName: "pause.circle.fill")
                    .foregroundStyle(.yellow)
                Text("Waiting")
                    .font(.headline)
            }

            // Reason with count
            if let reason = wave.waitingReason {
                Text(reason.description)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
                    .accessibilityLabel(reason.accessibilityDescription)
            } else {
                Text("PR limit reached")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            // Action button
            Button {
                openPRList()
            } label: {
                Label("Review PRs", systemImage: "arrow.up.right.square")
            }
            .buttonStyle(.borderedProminent)
            .tint(.loopflowBurgundy)
            .accessibilityLabel("Review open pull requests")
        }
        .padding(Spacing.lg)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
    }

    private func openPRList() {
        // Construct GitHub PR list URL for this repo
        // wave.repo gives us the repo path - we need to extract owner/repo from git remote
        let repoPath = wave.repo

        // Try to get owner/repo from the worktree's git remote
        if let worktreePath = wave.worktreePath {
            let ownerRepo = extractOwnerRepo(from: worktreePath) ?? extractOwnerRepoFromPath(repoPath)
            if let ownerRepo = ownerRepo,
               let url = URL(string: "https://github.com/\(ownerRepo)/pulls?q=is:open+is:pr+author:@me") {
                terminalLauncher.openURL(url)
            }
        } else {
            // Fallback: try to extract from repo path
            if let ownerRepo = extractOwnerRepoFromPath(repoPath),
               let url = URL(string: "https://github.com/\(ownerRepo)/pulls?q=is:open+is:pr+author:@me") {
                terminalLauncher.openURL(url)
            }
        }
    }

    private func extractOwnerRepo(from worktreePath: String) -> String? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = ["remote", "get-url", "origin"]
        process.currentDirectoryURL = URL(fileURLWithPath: worktreePath)

        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            process.waitUntilExit()

            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            guard let output = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) else {
                return nil
            }

            return parseGitRemoteURL(output)
        } catch {
            return nil
        }
    }

    private func parseGitRemoteURL(_ url: String) -> String? {
        // Parse git@github.com:owner/repo.git or https://github.com/owner/repo.git
        if url.contains("github.com") {
            // SSH format: git@github.com:owner/repo.git
            if let colonIndex = url.lastIndex(of: ":"),
               url.hasPrefix("git@") {
                var ownerRepo = String(url[url.index(after: colonIndex)...])
                if ownerRepo.hasSuffix(".git") {
                    ownerRepo = String(ownerRepo.dropLast(4))
                }
                return ownerRepo
            }

            // HTTPS format: https://github.com/owner/repo.git
            if let range = url.range(of: "github.com/") {
                var ownerRepo = String(url[range.upperBound...])
                if ownerRepo.hasSuffix(".git") {
                    ownerRepo = String(ownerRepo.dropLast(4))
                }
                return ownerRepo
            }
        }
        return nil
    }

    private func extractOwnerRepoFromPath(_ repoPath: String) -> String? {
        // Last resort: assume the folder structure mimics GitHub (e.g., ~/src/owner/repo)
        let components = repoPath.split(separator: "/")
        guard components.count >= 2 else { return nil }
        let repo = String(components[components.count - 1])
        let owner = String(components[components.count - 2])
        // Only return if both look like valid names
        if owner.isEmpty || repo.isEmpty || owner == "src" || owner == "Users" {
            return nil
        }
        return "\(owner)/\(repo)"
    }
}

#Preview {
    let wave = Wave(
        id: "test",
        name: "test-wave",
        repo: "/Users/jack/src/loopflow",
        status: .waiting,
        worktreePath: "/Users/jack/src/loopflow-worktree",
        prLimit: 5,
        waitingReason: .prLimitReached(open: 2, limit: 5)
    )

    return WaitingStateCard(wave: wave)
        .padding()
        .frame(width: 400)
}
