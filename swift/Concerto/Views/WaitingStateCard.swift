// WaitingStateCard - shows why a wave is blocked and provides action to resolve it.

import SwiftUI
import LoopflowCore

struct WaitingStateCard: View {
    let wave: WaveViewModel

    @Environment(\.palette) private var palette

    private let terminalLauncher = TerminalLauncher()

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(spacing: Spacing.sm) {
                Image(systemName: "pause.circle.fill")
                    .foregroundStyle(Color.statusWarning)
                Text("Waiting")
                    .font(.headline)
            }

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

            HStack(spacing: Spacing.sm) {
                Button {
                    openPRList()
                } label: {
                    Label("Review PRs", systemImage: "arrow.up.right.square")
                }
                .buttonStyle(.bordered)
                .accessibilityLabel("Review open pull requests")

                Text("Use the Runs tab to collapse or absorb PRs.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(Spacing.lg)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
    }

    private func openPRList() {
        let ownerRepo =
            wave.worktreePath.flatMap(extractOwnerRepo(from:))
            ?? extractOwnerRepoFromPath(wave.repo)
        guard let ownerRepo,
              let url = URL(string: "https://github.com/\(ownerRepo)/pulls?q=is:open+is:pr+author:@me")
        else {
            return
        }
        terminalLauncher.openURL(url)
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
        if url.contains("github.com") {
            if let colonIndex = url.lastIndex(of: ":"),
               url.hasPrefix("git@") {
                var ownerRepo = String(url[url.index(after: colonIndex)...])
                if ownerRepo.hasSuffix(".git") {
                    ownerRepo = String(ownerRepo.dropLast(4))
                }
                return ownerRepo
            }

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
        let components = repoPath.split(separator: "/")
        guard components.count >= 2 else { return nil }
        let repo = String(components[components.count - 1])
        let owner = String(components[components.count - 2])
        if owner.isEmpty || repo.isEmpty || owner == "src" || owner == "Users" {
            return nil
        }
        return "\(owner)/\(repo)"
    }
}

#Preview {
    let wave = WaveViewModel(
        api: Wave(
            id: "test",
            name: "test-wave",
            repo: "/Users/jack/src/loopflow",
            status: .waiting
        ),
        worktreePath: "/Users/jack/src/loopflow-worktree",
        prLimit: 5,
        waitingReason: .prLimitReached(open: 2, limit: 5)
    )

    return ThemePreview {
        WaitingStateCard(wave: wave)
            .padding()
            .frame(width: 400)
    }
}
