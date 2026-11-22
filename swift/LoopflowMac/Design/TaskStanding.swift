import Loopflow

/// Where a Task stands and whether it needs someone: one status word and, when
/// it does, one line saying why. Every Task row and the title bar print this,
/// so a Task says the same thing about itself wherever it shows.
struct TaskStanding: Equatable {
    /// What decided the word: the first of these that applies.
    enum Source: Equatable {
        /// Its program's report, or a shared Session that is Available.
        case program
        /// Session readings could not be read; there is a reason and no word.
        case unavailable
        /// A blocked or waiting condition, with no program reporting.
        case condition
        case lifecycle
        /// Planning has not answered and no program reports.
        case none
    }

    let source: Source
    let word: String?
    /// Nil prints in the row's secondary ink.
    let tone: WorkTone?
    let help: String?
    /// What the Task needs: its program's ask, else why its condition is not clear.
    let line: String?

    /// A screen with no Session readings passes `.unknown`.
    init(task: RoadmapTask?, status: SessionProgramStatus) {
        let need = task?.need
        let reason = need == nil ? nil : task?.condition.reason
        line = status.ask ?? reason.flatMap { $0.isEmpty ? nil : $0 }
        if let label = status.label {
            (source, word, tone, help) = (.program, label, status.tone, status.help)
        } else if case .unavailable = status {
            (source, word, tone, help) = (.unavailable, nil, nil, status.help)
        } else if let task, let need, need != .unknown {
            (source, word, tone, help) = (.condition, need.title, .warning, task.condition.reason)
        } else if let task {
            let undated = task.lifecycle == .done && task.task.completedAt == nil
            (source, word, tone, help) = (.lifecycle, undated ? "Done, date unavailable" : task.lifecycle.label.capitalized, nil, nil)
        } else {
            (source, word, tone, help) = (.none, nil, nil, nil)
        }
    }
}
