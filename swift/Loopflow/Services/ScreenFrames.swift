import Foundation

/// Pending observations consume a newly acquired, host-clocked screen frame.
/// A stale buffer or an unknown timestamp domain can never complete a wait.
@MainActor
public final class ScreenFrames {
    private struct Pending {
        let after: TimeInterval
        let continuation: CheckedContinuation<TimeInterval?, Never>
        let deadline: Task<Void, Never>
    }
    private var pending: [UUID: Pending] = [:]

    public init() {}

    static func observedTime(presented: TimeInterval, acquired: TimeInterval, after: TimeInterval) -> TimeInterval? {
        guard presented.isFinite, acquired.isFinite, presented >= after, presented <= acquired else { return nil }
        return presented
    }

    public func next(after: TimeInterval) async -> TimeInterval? {
        let id = UUID()
        return await withTaskCancellationHandler {
            await withCheckedContinuation { continuation in
                guard !Task.isCancelled else { continuation.resume(returning: nil); return }
                let deadline = Task { [weak self] in
                    do { try await Task.sleep(for: .seconds(15)) } catch { return }
                    self?.finish(id, at: nil)
                }
                pending[id] = Pending(after: after, continuation: continuation, deadline: deadline)
            }
        } onCancel: {
            Task { @MainActor in self.finish(id, at: nil) }
        }
    }

    public func receive(presented: TimeInterval, acquired: TimeInterval) {
        let ready = pending.compactMap { id, wait in
            Self.observedTime(presented: presented, acquired: acquired, after: wait.after).map { (id, $0) }
        }
        for (id, time) in ready { finish(id, at: time) }
    }

    public func stop() { for id in Array(pending.keys) { finish(id, at: nil) } }

    private func finish(_ id: UUID, at time: TimeInterval?) {
        guard let wait = pending.removeValue(forKey: id) else { return }
        wait.deadline.cancel()
        wait.continuation.resume(returning: time)
    }
}
