/// The actions offered by the Task workspace on both Mac and iPhone.
public struct TaskActions: Equatable {
    public enum Pause: Equatable, Sendable {
        case pause, unpause
    }

    public let control: WorkflowControl?
    public let pause: Pause?
    public let runRefusal: String?
    public let primaryRefusal: String?
    public let auto: Auto?
    public let completeReady: Bool
    public let steps: [String]
    public let breakpoints: Set<String>
    public let breakpointArrival: String?
    public let breakpointBlocker: String?
    /// The Graph's top-level steps as the workflow track draws them.
    public let track: [TaskGraph.Step]
    /// Where the control's label stands in the track: the step it replaces,
    /// or `track.count` past the last one at the end.
    public let chip: Int
    /// Rust's sentence for where the Task stands, or where you take part.
    public let sentence: String?

    /// The Task's auto setting.
    public enum Auto: Equatable {
        case on, off

        public func help(issue: String) -> String {
            self == .on
                ? "lf task auto off \(issue): the run waits at its next breakpoint for you, and auto-merge is disarmed"
                : "lf task auto on \(issue): run every remaining step and land it"
        }
    }

    /// `autoPending` is the switch's change the Machine has not shown yet.
    public init(work: TaskOverview?, row: RoadmapTask, execution: TaskExecution?, outline: WorkflowOutline? = nil,
                autoPending: Bool? = nil) {
        let graph = work?.graph
        let held = row.lifecycle == .paused
        let on = autoPending ?? execution.map { $0.auto?.state == .on } ?? row.selectionStates.contains(.auto)
        auto = on ? .on : row.isCompleted ? nil : .off
        runRefusal = row.runControl.unavailable
        completeReady = row.lifecycle == .done || row.pr?.phase == .merged || graph?.control.action == .done

        if row.isCompleted {
            control = WorkflowControl(action: .done, label: row.lifecycle == .abandoned ? "Abandoned" : "Done")
        } else if let graph {
            // A pending lifecycle edit can reach the row before its new Graph reading.
            let graphHeld = graph.control.action == .pausing || graph.control.action == .unpause || graph.control.action == .resume
            control = graphHeld == held ? graph.control : nil
        } else if work == nil {
            control = nil
        } else if held {
            control = WorkflowControl(action: .resume, label: "Paused: \(outline?.steps.first?.name ?? "start")")
        } else if completeReady {
            control = WorkflowControl(action: .done, label: "Done")
        } else {
            control = outline?.start ?? WorkflowControl(action: .run, label: "Run")
        }
        primaryRefusal = (control?.action == .run || control?.action == .continue || row.runControl.pausing) ? runRefusal : nil
        if let graph, case .breakpoint(_, let arrival, let blocked) = graph.position {
            breakpointArrival = arrival
            breakpointBlocker = blocked && !held ? graph.control.label : nil
        } else {
            breakpointArrival = nil
            breakpointBlocker = nil
        }
        breakpoints = Set(graph?.breakpoints ?? outline?.breakpoints ?? [])
        pause = held ? .unpause : row.lifecycle.isWorking ? .pause : nil

        if let graph {
            track = graph.steps
            let ended = graph.position == .at(step: TaskGraph.end) || control?.action == .done
            chip = graph.currentStep ?? (ended ? graph.steps.count : 0)
            sentence = graph.sentence
        } else {
            track = outline?.steps ?? []
            chip = control?.action == .done ? track.count : 0
            sentence = outline?.sentence
        }

        // The same track supplies Start from before and after Graph take-up.
        var excluded = breakpoints
        excluded.insert(TaskGraph.start)
        steps = graph == nil && outline == nil ? [] : [TaskGraph.start] + track.map(\.name).filter {
            excluded.insert($0).inserted
        }
    }

    public var autoRefusal: String? { auto == .on ? nil : runRefusal }
}
