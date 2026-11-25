/// The actions offered by the Task workspace on both Mac and iPhone.
public struct TaskActions: Equatable {
    public enum Pause: Equatable, Sendable {
        case pause, unpause, unpauseWithoutRunning
    }

    public let control: WorkflowControl?
    public let pause: Pause?
    public let pauseRefusal: String?
    public let runRefusal: String?
    public let auto: Auto?
    public let completeReady: Bool
    public let steps: [String]

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

    public init(work: TaskOverview?, row: RoadmapTask, execution: TaskExecution?, outline: WorkflowOutline? = nil) {
        let graph = work?.graph
        let held = row.lifecycle == .paused
        let on = execution.map { $0.auto?.state == .on } ?? row.selectionStates.contains(.auto)
        auto = on ? .on : row.isCompleted ? nil : .off
        runRefusal = row.runControl.unavailable
        pauseRefusal = row.runControl.pausing ? runRefusal : nil
        completeReady = row.lifecycle == .done || row.pr?.phase == .merged || graph?.control.action == .done

        if row.isCompleted {
            control = WorkflowControl(action: .done, label: row.lifecycle == .abandoned ? "Abandoned" : "Done")
        } else if let graph {
            // A pending lifecycle edit can reach the row before its new Graph reading.
            let graphHeld = graph.control.action == .unpause || graph.control.action == .resume
            control = graphHeld == held ? graph.control : nil
        } else if work == nil {
            control = nil
        } else if held {
            control = WorkflowControl(action: .resume, label: "Paused: start")
        } else if completeReady {
            control = WorkflowControl(action: .done, label: "Done")
        } else {
            control = outline?.start ?? WorkflowControl(action: .run, label: "Run")
        }
        pause = held ? .unpause : row.lifecycle.isWorking ? .pause : nil

        var seen = Set<String>()
        let stages = graph?.steps ?? outline?.steps ?? []
        let interactive = graph?.interactiveSteps ?? outline?.interactiveSteps ?? []
        let choices = graph == nil && outline == nil ? [] : ["start"] + stages.map(\.name) + interactive.map(\.name)
        steps = choices.filter { seen.insert($0).inserted }
    }

    public var autoRefusal: String? { auto == .on ? nil : runRefusal }

    public var primaryRefusal: String? {
        control?.action == .run ? runRefusal : pauseRefusal
    }
}
