// OutputBuffer - agent output buffering for active runs.

import Foundation
import LoopflowCore

struct OutputLine: Identifiable {
    let id = UUID()
    let text: String
    let timestamp: Date
}

@MainActor
@Observable
final class OutputBuffer {
    // Output keyed by wave_id
    private var waveOutput: [String: [OutputLine]] = [:]

    // Active stream tasks per wave_id (cancel on clear/new stream)
    private var streamTasks: [String: Task<Void, Never>] = [:]
    // Generation counter to prevent stale task cleanup from removing newer tasks
    private var streamGeneration: [String: Int] = [:]

    // Interactive session (one at a time for MVP)
    var interactiveSession: InteractiveSession?

    private let maxLines = 2000
    private var waveService = WaveService()

    func configureConnection(
        _ connection: ServerConnection,
        tokenProvider: (@Sendable () -> String?)? = nil
    ) {
        waveService = WaveService(
            connection: connection,
            tokenProvider: tokenProvider
        )
    }

    // MARK: - Output

    /// Append output from the WebSocket event handler. Skipped if a
    /// streaming connection is active for this wave (it provides the
    /// same lines and handles replay).
    func appendOutput(waveId: String, text: String, timestamp: Date) {
        // If a stream is active for this wave, it's already feeding output.
        if streamTasks[waveId] != nil { return }
        appendLine(waveId: waveId, text: text, timestamp: timestamp)
    }

    private func appendLine(waveId: String, text: String, timestamp: Date) {
        let line = OutputLine(text: text, timestamp: timestamp)
        waveOutput[waveId, default: []].append(line)

        // Cap buffer
        if waveOutput[waveId]?.count ?? 0 > maxLines {
            waveOutput[waveId]?.removeFirst()
        }
    }

    func output(for waveId: String) -> [OutputLine] {
        waveOutput[waveId] ?? []
    }

    func clearOutput(for waveId: String) {
        streamTasks[waveId]?.cancel()
        streamTasks.removeValue(forKey: waveId)
        waveOutput.removeValue(forKey: waveId)
    }

    // MARK: - Stream Loading

    /// Connect to the replay+follow endpoint for a wave. Populates
    /// the buffer with historical output then continues with live lines.
    func startStreaming(waveId: String) {
        // Don't start a second stream for the same wave
        if streamTasks[waveId] != nil { return }

        // Clear existing output so replay doesn't produce duplicates
        waveOutput.removeValue(forKey: waveId)

        let gen = (streamGeneration[waveId] ?? 0) + 1
        streamGeneration[waveId] = gen

        let service = waveService
        streamTasks[waveId] = Task { [weak self] in
            defer {
                // Only clean up if this is still the current generation
                if self?.streamGeneration[waveId] == gen {
                    self?.streamTasks.removeValue(forKey: waveId)
                    self?.streamGeneration.removeValue(forKey: waveId)
                }
            }
            do {
                let stream = service.streamOutput(waveId: waveId)
                for try await line in stream {
                    guard !Task.isCancelled else { break }
                    self?.appendLine(waveId: waveId, text: line, timestamp: Date())
                }
            } catch {
                // Stream ended (connection closed or wave stopped) — normal.
            }
        }
    }

    /// Stop streaming for a wave (e.g. when deselected).
    func stopStreaming(waveId: String) {
        streamTasks[waveId]?.cancel()
        streamTasks.removeValue(forKey: waveId)
    }

    /// Most recent output line for a wave (for activity summary display).
    func recentOutput(for waveId: String, maxLength: Int = 60) -> String? {
        guard let lastLine = waveOutput[waveId]?.last else {
            return nil
        }
        let text = lastLine.text.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.isEmpty { return nil }
        if text.count <= maxLength { return text }
        return String(text.prefix(maxLength - 3)) + "…"
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
