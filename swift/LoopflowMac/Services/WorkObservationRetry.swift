import Loopflow

/// Retry the whole observation without letting one working part hide a failing one.
struct WorkObservationRetry {
    private var delay = Duration.seconds(1)
    private var healthySince: ContinuousClock.Instant?
    private var unavailable: Set<WorkPart> = []

    mutating func receive(_ frame: WorkFrame, scope: WorkScope, at now: ContinuousClock.Instant) {
        guard let part = WorkPart(rawValue: frame.content.part), part.read(for: scope) != nil else { return }
        resetAfterHealthyPeriod(at: now)
        unavailable = unavailable.filter { $0.read(for: scope) != nil }
        let hasReading: Bool
        switch frame.content {
        case .planning(let body): hasReading = body != nil
        case .selections(let body): hasReading = body != nil
        case .taskList(let body): hasReading = body != nil
        case .peerPlanning(let body): hasReading = body != nil
        case .sessions(let body): hasReading = body != nil
        case .task(let body): hasReading = body != nil
        case .workActivity(let body): hasReading = body != nil
        case .activity(let body): hasReading = body != nil
        case .checkpoint: return
        }
        if hasReading && frame.unavailable == nil {
            unavailable.remove(part)
        } else {
            unavailable.insert(part)
        }
        if unavailable.isEmpty {
            if healthySince == nil { healthySince = now }
        } else {
            healthySince = nil
        }
    }

    mutating func failed(at now: ContinuousClock.Instant) -> Duration {
        // Quiet subscriptions are healthy too; no frame needs to arrive at the deadline.
        resetAfterHealthyPeriod(at: now)
        healthySince = nil
        unavailable = []
        let wait = delay
        delay = min(delay * 2, .seconds(30))
        return wait
    }

    private mutating func resetAfterHealthyPeriod(at now: ContinuousClock.Instant) {
        if let healthySince, now - healthySince >= .seconds(30) { delay = .seconds(1) }
    }
}
