// Session tracking state - running steps and their output.

import Foundation
import LoopflowCore

struct OutputLine: Identifiable {
    let id = UUID()
    let text: String
    let timestamp: Date
}

@MainActor
@Observable
final class SessionState {
    // Active sessions (step runs in progress)
    var activeSessions: [String: ActiveSession] = [:]

    // Interactive session (one at a time for MVP)
    var interactiveSession: InteractiveSession?

    // UI toggle for streaming log view
    var showResultsLog: Bool = false

    struct ActiveSession {
        let id: String
        let waveId: String?
        let step: String
        let worktree: String
        let startedAt: Date
        var baseline: StepRunBaseline?
        var output: [OutputLine] = []
        var result: StepRunResult?

        var isRunning: Bool {
            result?.status == .running || result == nil
        }
    }

    // Services
    private let resultsService = ResultsService()

    // MARK: - Session Lifecycle

    func startSession(id: String, waveId: String?, step: String, worktree: String) {
        activeSessions[id] = ActiveSession(
            id: id,
            waveId: waveId,
            step: step,
            worktree: worktree,
            startedAt: Date()
        )

        // Capture baseline
        Task {
            let baseline = await resultsService.captureBaseline(
                stepRunId: id,
                worktree: URL(fileURLWithPath: worktree)
            )
            await MainActor.run {
                activeSessions[id]?.baseline = baseline
                activeSessions[id]?.result = StepRunResult.running(
                    stepRunId: id,
                    step: step,
                    worktree: worktree,
                    startedAt: activeSessions[id]?.startedAt ?? Date()
                )
            }
        }
    }

    func appendOutput(sessionId: String, text: String, timestamp: Date) {
        guard activeSessions[sessionId] != nil else { return }

        let line = OutputLine(text: text, timestamp: timestamp)
        activeSessions[sessionId]?.output.append(line)

        // Cap buffer at 1000 lines
        if activeSessions[sessionId]?.output.count ?? 0 > 1000 {
            activeSessions[sessionId]?.output.removeFirst()
        }
    }

    func endSession(id: String, status: String) {
        guard let session = activeSessions[id],
              let baseline = session.baseline else { return }

        let resultStatus: StepRunResultStatus = status == "completed" ? .completed : .error

        Task {
            let result = await resultsService.computeResults(
                baseline: baseline,
                step: session.step,
                status: resultStatus,
                startedAt: session.startedAt,
                endedAt: Date()
            )
            await MainActor.run {
                activeSessions[id]?.result = result
            }
        }
    }

    func clearCompletedSessions() {
        activeSessions = activeSessions.filter { _, session in
            session.isRunning
        }
    }

    // MARK: - Query

    var activeSessionIds: Set<String> {
        Set(activeSessions.filter { $0.value.isRunning }.keys)
    }

    func isWorktreeRunning(_ path: String) -> Bool {
        activeSessions.values.contains { $0.worktree == path && $0.isRunning }
    }

    func output(for waveId: String) -> [OutputLine] {
        activeSessions.values.first { $0.waveId == waveId }?.output ?? []
    }

    /// Most recent output line for a wave (for activity summary display).
    func recentOutput(for waveId: String, maxLength: Int = 60) -> String? {
        guard let session = activeSessions.values.first(where: { $0.waveId == waveId }),
              let lastLine = session.output.last else {
            return nil
        }
        let text = lastLine.text.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.isEmpty { return nil }
        if text.count <= maxLength { return text }
        return String(text.prefix(maxLength - 3)) + "..."
    }

    func currentResult(for worktreePath: String?) -> StepRunResult? {
        let results = activeSessions.values.compactMap { $0.result }
        let filtered: [StepRunResult]

        if let path = worktreePath {
            filtered = results.filter { $0.worktree == path }
        } else {
            filtered = results
        }

        return filtered.sorted { $0.startedAt > $1.startedAt }.first
    }

    // MARK: - Interactive Sessions

    func launchInteractiveSession(waveId: String, step: String, worktreePath: String, prompt: String? = nil) {
        interactiveSession = InteractiveSession(
            waveId: waveId,
            step: step,
            worktreePath: worktreePath,
            prompt: prompt
        )
    }

    func endInteractiveSession() {
        interactiveSession = nil
    }

    func hasActiveSession(for waveId: String) -> Bool {
        interactiveSession?.waveId == waveId
    }

    // MARK: - Diff Preview

    func loadDiffPreview(sessionId: String, fileIndex: Int) async {
        guard var session = activeSessions[sessionId],
              var result = session.result,
              fileIndex < result.filesChanged.count,
              result.filesChanged[fileIndex].diffPreview == nil,
              let baseline = session.baseline else { return }

        let file = result.filesChanged[fileIndex]
        let worktree = URL(fileURLWithPath: result.worktree)

        let preview = await resultsService.loadDiffPreview(
            for: file,
            baselineSHA: baseline.headSHA,
            in: worktree
        )

        result.filesChanged[fileIndex].diffPreview = preview
        session.result = result
        activeSessions[sessionId] = session
    }
}
