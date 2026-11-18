// Perf — os_signpost intervals at the workspace's user-visible boundaries.
//
// One signposter: subsystem "studio.loopflow", category "perf". logd records
// the intervals whether or not Instruments is attached, so `log show --signpost`
// and `xctrace` both read them; nothing leaves the machine. When no interval is
// pending the cost is one dictionary lookup and one os_signpost call.
// Interval definitions: scripts/benchmarks/desktop-performance/README.md.

import AppKit
import Darwin
import Foundation
import Loopflow
import OSLog

enum Perf {
    static let signposter = PerfClock.signposter

    enum Interval: Hashable {
        case coldStart, workspaceCurrent, hierarchyInteraction, taskWorkspaceReady
        case workflowReady, workFrameApply, retainedWorkspaceAction, startTask, completeTask, newConversation, reconnect, openFile, firstWord

        var signpost: StaticString {
            switch self {
            case .coldStart: "cold_start"
            case .workspaceCurrent: "workspace_current"
            case .hierarchyInteraction: "hierarchy_interaction"
            case .taskWorkspaceReady: "task_workspace_ready"
            case .workflowReady: "workflow_ready"
            case .workFrameApply: "work_frame_apply"
            case .retainedWorkspaceAction: "retained_workspace_action"
            case .startTask: "start_task"
            case .completeTask: "complete_task"
            case .newConversation: "new_conversation"
            case .reconnect: "reconnect"
            case .openFile: "open_file"
            case .firstWord: "first_word"
            }
        }
        var recordsInteraction: Bool {
            switch self {
            case .coldStart, .hierarchyInteraction, .taskWorkspaceReady, .retainedWorkspaceAction, .startTask, .completeTask, .newConversation, .reconnect, .openFile, .firstWord: true
            case .workspaceCurrent, .workflowReady, .workFrameApply: false
            }
        }
        func experience(_ scenario: String) -> String {
            switch self {
            case .coldStart: "open_app"
            case .hierarchyInteraction: "find_work"
            case .taskWorkspaceReady: scenario == "session" ? "return_session" : scenario == "task" ? "open_task" : "open_wave"
            case .retainedWorkspaceAction: "workspace_layout"
            case .startTask: "start_task"
            case .completeTask: "complete_task"
            case .newConversation: "new_conversation"
            case .reconnect: "reconnect"
            case .openFile: "open_file"
            case .firstWord: "first_word"
            default: "diagnostic"
            }
        }
    }
    static let coldStart = Interval.coldStart
    static let workspaceCurrent = Interval.workspaceCurrent
    static let hierarchyInteraction = Interval.hierarchyInteraction
    static let taskWorkspaceReady = Interval.taskWorkspaceReady
    static let workflowReady = Interval.workflowReady
    static let workFrameApply = Interval.workFrameApply
    static let retainedWorkspaceAction = Interval.retainedWorkspaceAction

    /// One `lf` subprocess read; the message names the verb.
    static let lf: StaticString = "lf"
    /// One Markdown parse into blocks; the message carries the source length.
    static let markdownParse: StaticString = "markdown_parse"
    /// Terminal key press → the next surface draw after it.
    static let terminalKeyToDraw: StaticString = "terminal_key_to_draw"

    /// Correlates one Task gesture with preparation. It grants no readiness or ownership.
    @TaskLocal static var inputTrace: String = ""

    static func agentInput(_ phase: String, task: String = "", session: String = "", detail: String = "") {
        signposter.emitEvent("agent_input",
            "phase=\(phase, privacy: .public) trace=\(inputTrace, privacy: .public) task=\(task, privacy: .public) session=\(session, privacy: .public) wall=\(Date().timeIntervalSince1970, privacy: .public) \(detail, privacy: .public)")
    }

    private struct Pending {
        let instance = UUID()
        let state: OSSignpostIntervalState
        var id: String
        let interaction: Interaction?
        weak var window: NSWindow?
        var stateObserved = false
        var acknowledgmentObserved = false
        var commandAccepted = false
        var rowRemoved = false
    }

    private struct Key: Hashable {
        let interval: Interval
        let owner: UUID?
    }
    @MainActor private static var pending: [Key: Pending] = [:]
    @MainActor private static var journal: InteractionJournal?

    @MainActor
    static func startRecording() {
        var identity = ["platform": "macOS", "launch": UUID().uuidString]
        identity["app"] = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
        identity["revision"] = Bundle.main.object(forInfoDictionaryKey: "LoopflowSourceRevision") as? String
        journal = InteractionJournal(directory: WorkCache.homeDirectory.appendingPathComponent("timings/interactions"),
                                     identity: identity)
    }

    @MainActor
    static func stalled(milliseconds: Double) { journal?.stalled(milliseconds: milliseconds) }

    @MainActor private static var launchObserved = false

    /// The first restored window acknowledges launch; a focused input surface
    /// supplies usability. The clock starts at process creation, not this call.
    @MainActor
    static func launchVisible(repository: String?, liveSessions: Int?, window: NSWindow?) {
        guard !launchObserved, let window, window.isVisible else { return }
        launchObserved = true
        pending[Key(interval: .coldStart, owner: nil)]?.interaction?.identifyLaunch(repository: repository, liveSessions: liveSessions)
        pending[Key(interval: .coldStart, owner: nil)]?.window = window
        acknowledged(.coldStart, id: "launch")
    }

    @MainActor
    static var awaitsTerminal: Bool {
        pending.contains { [.coldStart, .taskWorkspaceReady, .startTask, .newConversation, .reconnect].contains($0.key.interval) && !$0.value.stateObserved }
    }

    @MainActor
    static func terminalReady(session: String, window: NSWindow, prepareFrame: @escaping @MainActor () -> Void) {
        let matches = pending.filter {
            [.taskWorkspaceReady, .startTask, .newConversation, .reconnect].contains($0.key.interval)
                && $0.value.id == session && $0.value.window === window
                && ($0.key.interval != .reconnect || $0.value.commandAccepted)
        }
        for (key, _) in matches { end(key.interval, id: session, owner: key.owner, prepareFrame: prepareFrame) }
        let launch = Key(interval: .coldStart, owner: nil)
        if pending[launch]?.window === window {
            stateObserved(.coldStart, id: "launch", prepareFrame: prepareFrame)
        }
    }

    @MainActor
    static func reconnected(id: String, owner: UUID) {
        let key = Key(interval: .reconnect, owner: owner)
        guard pending[key]?.id == id else { return }
        pending[key]?.commandAccepted = true
    }

    /// Capture before launching Tasks so their model work retains the initiating event.
    @MainActor
    static func userAction<Result>(_ interval: Interval, scenario: String, id: String,
                                   repository: String? = nil, machine: String? = nil, liveSessions: Int?, owner: UUID,
                                   _ action: () -> Result) -> Result {
        InteractionTrace.$input.withValue(InteractionInput.current) {
            begin(interval, scenario, id: id, repository: repository, machine: machine, liveSessions: liveSessions, owner: owner)
            return InteractionTrace.$current.withValue(pending[Key(interval: interval, owner: owner)]?.interaction, operation: action)
        }
    }

    @MainActor
    static func begin(_ interval: Interval, _ scenario: String, id: String, detail: String = "",
                      repository: String? = nil, machine: String? = nil, liveSessions: Int? = nil, owner: UUID? = nil) {
        let key = Key(interval: interval, owner: owner)
        if let inherited = InteractionTrace.current {
            if inherited.isFinished { return }
            if var existing = pending[key], existing.interaction === inherited {
                existing.id = id
                pending[key] = existing
                return
            }
        }
        if let stale = pending.removeValue(forKey: key) {
            if !stale.stateObserved { signposter.endInterval(interval.signpost, stale.state, "superseded") }
            stale.interaction?.finish(.superseded)
        }
        let state = signposter.beginInterval(interval.signpost, id: signposter.makeSignpostID(),
            "scenario=\(scenario, privacy: .public) id=\(id, privacy: .public) \(detail, privacy: .public)")
        let interaction: Interaction?
        if interval.recordsInteraction, let journal {
            let input: InteractionInput?
            if interval == .coldStart, let ms = PerfClock.millisecondsSinceProcessStart() {
                input = InteractionInput(uptime: ProcessInfo.processInfo.systemUptime - ms / 1000, source: .processStart)
            } else { input = InteractionTrace.input }
            // A request's ID names its interaction, so the Machine's spans for
            // that request join the journal record.
            interaction = Interaction(id: UUID(uuidString: id) ?? UUID(), name: interval.experience(scenario), scenario: scenario, input: input,
                context: Interaction.Context(thermalState: ProcessInfo.processInfo.thermalState.rawValue,
                                             liveSessions: liveSessions, repository: repository, machine: machine),
                sink: journal.append)
        } else { interaction = nil }
        let eventWindow = NSApp?.currentEvent?.window
        let window = eventWindow?.canBecomeMain == true ? eventWindow : NSApp?.mainWindow ?? NSApp?.keyWindow
        pending[key] = Pending(state: state, id: id, interaction: interaction, window: window)
    }

    @MainActor
    static func acknowledged(_ interval: Interval, id: String, owner: UUID? = nil) {
        let key = Key(interval: interval, owner: owner)
        guard let current = pending[key], current.id == id, !current.acknowledgmentObserved else { return }
        pending[key]?.acknowledgmentObserved = true
        current.interaction?.acknowledged(at: ProcessInfo.processInfo.systemUptime, evidence: .appState)
        current.interaction?.observe(.acknowledged) { [weak window = current.window] in
            await InteractionFrame.capture(window: window)
        }
    }

    @MainActor
    static func end(_ interval: Interval, id: String, owner: UUID? = nil, inputReady: Bool = true, prepareFrame: @escaping @MainActor () -> Void = {}) {
        let key = Key(interval: interval, owner: owner)
        guard var current = pending[key], current.id == id, !current.stateObserved else { return }
        current.stateObserved = true
        pending[key] = current
        signposter.endInterval(interval.signpost, current.state, "state_observed")
        current.interaction?.usable(at: ProcessInfo.processInfo.systemUptime, evidence: .appState)
        if inputReady {
            current.interaction?.observe(.usable) { [weak window = current.window] in
                await InteractionFrame.capture(window: window, prepare: prepareFrame)
            }
        } else { current.interaction?.finish(.unmeasured) }
        Task {
            await current.interaction?.waitForObservation()
            if pending[key]?.instance == current.instance { pending.removeValue(forKey: key) }
        }
    }

    @MainActor
    static func retarget(_ interval: Interval, from id: String, to next: String, owner: UUID) {
        let key = Key(interval: interval, owner: owner)
        guard pending[key]?.id == id else { return }
        if let inherited = InteractionTrace.current, pending[key]?.interaction !== inherited { return }
        pending[key]?.id = next
    }

    @MainActor
    static func finish(_ interval: Interval, id: String, owner: UUID, outcome: Interaction.Outcome) {
        let key = Key(interval: interval, owner: owner)
        let inherited = InteractionTrace.current.map { pending[key]?.interaction === $0 } ?? false
        guard let current = pending[key], current.id == id || inherited else { return }
        pending.removeValue(forKey: key)
        if !current.stateObserved { signposter.endInterval(interval.signpost, current.state, "finished") }
        current.interaction?.finish(outcome)
    }

    /// Only an observed removal of this row can finish Complete → row gone.
    @MainActor
    static func rowsChanged(from before: [String], to after: [String], owner: UUID) {
        guard let current = pending[Key(interval: .completeTask, owner: owner)],
              before.contains(current.id), !after.contains(current.id) else { return }
        let key = Key(interval: .completeTask, owner: owner)
        pending[key]?.rowRemoved = true
        if current.commandAccepted { stateObserved(.completeTask, id: current.id, owner: owner) }
    }

    @MainActor
    static func completionAccepted(id: String, owner: UUID) {
        let key = Key(interval: .completeTask, owner: owner)
        guard let current = pending[key], current.id == id else { return }
        pending[key]?.commandAccepted = true
        if current.rowRemoved { stateObserved(.completeTask, id: id, owner: owner) }
    }

    /// Observed view state is diagnostic until a real frame confirms presentation.
    @MainActor
    static func stateObserved(_ interval: Interval, id: String, owner: UUID? = nil, inputReady: Bool = true, prepareFrame: @escaping @MainActor () -> Void = {}) {
        if owner == nil, pending[Key(interval: interval, owner: nil)] == nil {
            // A terminal callback can lack its window scope. Ambiguity stays
            // unmeasured rather than completing another window's interaction.
            let matches = pending.filter { $0.key.interval == interval && $0.value.id == id }
            if matches.count == 1, let key = matches.first?.key { end(interval, id: id, owner: key.owner, inputReady: inputReady, prepareFrame: prepareFrame) }
        } else { end(interval, id: id, owner: owner, inputReady: inputReady, prepareFrame: prepareFrame) }
    }

    @MainActor
    static func isPending(_ interval: Interval, owner: UUID? = nil) -> Bool {
        pending[Key(interval: interval, owner: owner)] != nil
    }

    @MainActor
    static func project<Result>(owner: UUID, _ operation: () -> Result) -> Result {
        let candidates = pending.filter { $0.key.owner == owner && !$0.value.stateObserved }.values.compactMap(\.interaction)
        let work = InteractionTrace.current?.work ?? (candidates.count == 1 ? candidates.first?.work : nil)
        work?.project()
        return operation()
    }

    static func measure<Value>(_ name: StaticString, _ detail: String, _ body: () throws -> Value) rethrows -> Value {
        let state = signposter.beginInterval(name, id: signposter.makeSignpostID(), "\(detail, privacy: .public)")
        defer { signposter.endInterval(name, state) }
        return try body()
    }

    static func measure<Value>(_ name: StaticString, _ detail: String, _ body: () async throws -> Value) async rethrows -> Value {
        let state = signposter.beginInterval(name, id: signposter.makeSignpostID(), "trace=\(inputTrace, privacy: .public) \(detail, privacy: .public)")
        defer { signposter.endInterval(name, state) }
        return try await body()
    }
}
