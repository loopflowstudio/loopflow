// Central app state using Observable macro (macOS 15+).

import Foundation
import SwiftUI

@MainActor
@Observable
final class AppState {
    var currentRepo: URL?
    var config: LoopflowConfig?
    var worktrees: [Worktree] = []
    var prompts: [PromptCard] = []

    // Prompt launcher state
    var selectedPrompt: PromptCard?
    var promptArgs: String = ""
    var includeDiff: Bool = true
    var selectedContextFolders: Set<URL> = []
    var runMode: RunMode = .auto
    var estimatedTokens: Int = 0

    // Sidebar state
    var selectedWorktree: Worktree?

    // Loading state
    var isLoading: Bool = false
    var errorMessage: String?

    // Services
    private let worktreeService = WorktreeService()
    private let configLoader = ConfigLoader()
    private let promptService = PromptService()

    func openRepo(_ url: URL) async {
        currentRepo = url
        isLoading = true
        errorMessage = nil

        do {
            config = try configLoader.load(from: url)
            await refreshWorktrees()
            prompts = try promptService.loadPrompts(from: url, config: config)
        } catch {
            errorMessage = error.localizedDescription
        }

        isLoading = false
    }

    func refreshWorktrees() async {
        guard let repo = currentRepo else { return }

        do {
            worktrees = try await worktreeService.list(in: repo)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func createWorktree(name: String, baseBranch: String? = nil) async throws {
        guard let repo = currentRepo else { return }

        try await worktreeService.create(name: name, in: repo, baseBranch: baseBranch)
        await refreshWorktrees()
    }

    func deleteWorktree(_ worktree: Worktree) async throws {
        guard let repo = currentRepo else { return }

        try await worktreeService.remove(name: worktree.branch, in: repo)
        await refreshWorktrees()
    }

    func estimateTokens() async {
        guard let repo = currentRepo else { return }

        let tokenService = TokenEstimator()
        estimatedTokens = await tokenService.estimate(
            prompt: selectedPrompt?.name,
            args: promptArgs,
            context: Array(selectedContextFolders),
            includeDiff: includeDiff,
            in: repo
        )
    }

    func buildCommand() -> String {
        var parts = ["lf"]

        if let prompt = selectedPrompt {
            parts.append(prompt.name)
        } else if !promptArgs.isEmpty {
            parts.append(":")
        }

        if !promptArgs.isEmpty {
            parts.append(promptArgs)
        }

        if runMode == .interactive {
            parts.append("-i")
        } else {
            parts.append("-a")
        }

        for folder in selectedContextFolders {
            if let path = folder.path(percentEncoded: false).components(separatedBy: currentRepo?.path() ?? "").last {
                parts.append("-x")
                parts.append(path.hasPrefix("/") ? String(path.dropFirst()) : path)
            }
        }

        return parts.joined(separator: " ")
    }
}
