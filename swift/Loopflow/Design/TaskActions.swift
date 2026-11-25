/// What a Task offers from where it stands, on the Mac's Workflow bar and in
/// the phone's "Task actions" menu: the step that moves it and its auto
/// setting. Rust owns the position and whether a run can start; both surfaces
/// draw this and decide nothing themselves.
public struct TaskActions: Equatable {
    /// `lf task run` under the name where the Task stands gives it.
    public struct Run: Equatable {
        /// "Start" or "Run": the button's word and the Notice's.
        public let label: String
        /// The command the button stands for.
        public let help: String
        public var arrival: String? = nil
    }

    /// The Task's auto setting, named in the Notice by the action it offers.
    public enum Auto: Equatable {
        case on, off

        /// The button that changes it.
        public var offer: String { self == .on ? "Stop Auto-Ship" : "Auto-Ship" }

        public func help(issue: String) -> String {
            self == .on
                ? "lf task auto off \(issue): the run waits at its next interactive step for you, and auto-merge is disarmed"
                : "lf task auto on \(issue): run every remaining step and land it"
        }
    }

    /// Nil while a run carries the Task, at its Graph's end, or before its reading arrives.
    public let run: Run?
    /// Nil on a completed Task with auto off: nothing is left to ship.
    public let auto: Auto?
    /// Why Rust would refuse a run now. It disables `run` and "Auto-Ship", never "Stop Auto-Ship".
    public let runRefusal: String?

    /// `work` is the Task's reading, nil until one arrives; `execution` is the
    /// same reading's auto mark, and the row's state stands in until then.
    public init(work: TaskOverview?, row: RoadmapTask, execution: TaskExecution?) {
        let issue = row.task.identifier
        if let graph = work?.graph {
            run = graph.runAction(issue: issue)
        } else if work != nil {
            // A reading with no Graph: the first run takes one up.
            run = Run(label: "Start", help: "lf task run \(issue)")
        } else {
            run = nil
        }
        let on = execution.map { $0.auto?.state == .on } ?? row.selectionStates.contains(.auto)
        auto = on ? .on : row.isCompleted ? nil : .off
        runRefusal = row.runControl.unavailable
    }

    /// Why `auto`'s button is disabled; turning auto off is never refused here.
    public var autoRefusal: String? { auto == .on ? nil : runRefusal }

    /// Why an offered action is disabled, said once for both; nil when none is.
    public var refusal: String? { run != nil || auto == .off ? runRefusal : nil }
}

public extension TaskGraph {
    /// What `lf task run` does from where the Task stands, named for it; `nil`
    /// while a run carries the Task or once it is at `end`.
    func runAction(issue: String) -> TaskActions.Run? {
        let help = "lf task run \(issue)"
        switch position {
        case .at(TaskGraph.start): return .init(label: "Start", help: help)
        case .at(TaskGraph.end): return nil
        case .breakpoint(_, let arrival):
            return .init(label: "Continue", help: "\(help) --at-arrival \(arrival)", arrival: arrival)
        case .at(let step): return .init(label: "Run", help: "\(help): continue after \(step)")
        case .run(_, _, _, true): return nil
        case .run(_, let at, _, false): return .init(label: "Run", help: "\(help): run again at \(at)")
        }
    }
}
