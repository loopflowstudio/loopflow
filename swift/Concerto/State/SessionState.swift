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

    struct ActiveSession {
        let id: String
        let waveId: String?
        let step: String
        let worktree: String
        let startedAt: Date
        var output: [OutputLine] = []
        var status: String?

        var isRunning: Bool {
            status == nil || status == "running" || status == "waiting"
        }
    }

    // MARK: - Session Lifecycle

    func startSession(id: String, waveId: String?, step: String, worktree: String) {
        activeSessions[id] = ActiveSession(
            id: id,
            waveId: waveId,
            step: step,
            worktree: worktree,
            startedAt: Date(),
            status: "running"
        )
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
        activeSessions[id]?.status = status
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

}
