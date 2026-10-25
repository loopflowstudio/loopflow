import Foundation

/// A reading of retained UI state, not another layout or Work owner.
public struct DesktopInspection: Codable, Sendable, Equatable {
    public let observedAt: Int64
    public let windows: [DesktopWindowInspection]

    public let openings: [DesktopOpening]

    public init(observedAt: Int64, windows: [DesktopWindowInspection], openings: [DesktopOpening]) {
        self.observedAt = observedAt
        self.windows = windows
        self.openings = openings
    }
    enum CodingKeys: String, CodingKey { case observedAt = "observed_at", windows, openings }
}

public struct DesktopOpening: Codable, Sendable, Equatable {
    public enum Status: String, Codable, Sendable { case opening, usable, failed }
    public let url: String
    public let status: Status
    public let reason: String?

    public init(url: String, status: Status, reason: String?) {
        self.url = url; self.status = status; self.reason = reason
    }
}

public struct DesktopWindowInspection: Codable, Sendable, Equatable {
    public let repository: String
    public let window: String
    public let path: String?
    public let destination: DesktopDestination
    public let history: DesktopHistory
    public let reading: String
    public let reason: String?
    public let task: DesktopTaskInspection?
    public let session: DesktopSessionInspection?
    public let supportedOperations: [String]
    public let workspaces: [DesktopWorkspaceInspection]
    public let view: WorkViewPlan?
    /// Work shown in this window, in slot fill order.
    public let openWork: [WorkWorkspaceKey]
    public let opening: DesktopOpening?

    public init(repository: String, window: String, path: String?, destination: DesktopDestination, history: DesktopHistory,
                reading: String, reason: String?, task: DesktopTaskInspection?,
                session: DesktopSessionInspection?, supportedOperations: [String], workspaces: [DesktopWorkspaceInspection], openWork: [WorkWorkspaceKey], view: WorkViewPlan?, opening: DesktopOpening?) {
        self.repository = repository; self.window = window; self.path = path
        self.destination = destination; self.history = history
        self.reading = reading; self.reason = reason; self.task = task
        self.session = session; self.supportedOperations = supportedOperations; self.workspaces = workspaces; self.openWork = openWork; self.view = view; self.opening = opening
    }
    enum CodingKeys: String, CodingKey {
        case repository, window, path, destination, history, reading, reason, task, session, workspaces, view, opening
        case openWork = "open_work"
        case supportedOperations = "supported_operations"
    }
}

/// What a window shows, as its sidebar, crumbs and tile clicks select it.
public enum DesktopDestination: Codable, Sendable, Equatable {
    /// Full Work; the repository conversation has a `repository` key.
    case work(WorkWorkspaceKey)
    case dashboard(id: String)
    case workingTasks
    /// An unsaved query; the window's `view` carries its plan.
    case view(selection: WorkSelection)
    /// A Dashboard member entered as full Work; back returns to its Dashboard.
    case zoomed(from: DesktopBoard, work: WorkWorkspaceKey)

    private enum CodingKeys: String, CodingKey { case kind, work, id, selection, from }
    private enum Kind: String, Codable { case work, dashboard, view, zoomed; case workingTasks = "working_tasks" }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .work: self = .work(try values.decode(WorkWorkspaceKey.self, forKey: .work))
        case .dashboard: self = .dashboard(id: try values.decode(String.self, forKey: .id))
        case .workingTasks: self = .workingTasks
        case .view: self = .view(selection: try values.decode(WorkSelection.self, forKey: .selection))
        case .zoomed: self = .zoomed(from: try values.decode(DesktopBoard.self, forKey: .from),
                                     work: try values.decode(WorkWorkspaceKey.self, forKey: .work))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .work(let work):
            try values.encode(Kind.work, forKey: .kind)
            try values.encode(work, forKey: .work)
        case .dashboard(let id):
            try values.encode(Kind.dashboard, forKey: .kind)
            try values.encode(id, forKey: .id)
        case .workingTasks: try values.encode(Kind.workingTasks, forKey: .kind)
        case .view(let selection):
            try values.encode(Kind.view, forKey: .kind)
            try values.encode(selection, forKey: .selection)
        case .zoomed(let from, let work):
            try values.encode(Kind.zoomed, forKey: .kind)
            try values.encode(from, forKey: .from)
            try values.encode(work, forKey: .work)
        }
    }
}

/// `{"dashboard": id}` or `"working_tasks"`, matching Rust's external tagging.
public enum DesktopBoard: Codable, Sendable, Equatable {
    case dashboard(String)
    case workingTasks

    private enum CodingKeys: String, CodingKey { case dashboard }

    public init(from decoder: Decoder) throws {
        if let name = try? decoder.singleValueContainer().decode(String.self) {
            guard name == "working_tasks" else {
                throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "Unknown board \(name)"))
            }
            self = .workingTasks
        } else {
            self = .dashboard(try decoder.container(keyedBy: CodingKeys.self).decode(String.self, forKey: .dashboard))
        }
    }

    public func encode(to encoder: Encoder) throws {
        switch self {
        case .dashboard(let id):
            var values = encoder.container(keyedBy: CodingKeys.self)
            try values.encode(id, forKey: .dashboard)
        case .workingTasks:
            var value = encoder.singleValueContainer()
            try value.encode("working_tasks")
        }
    }
}

/// Destinations back and forward reach, as the window's chevrons do.
public struct DesktopHistory: Codable, Sendable, Equatable {
    public let back: Int
    public let forward: Int
    public init(back: Int, forward: Int) { self.back = back; self.forward = forward }
}

/// Window navigation through the selection the window's own controls use.
public struct DesktopNavigationCommand: Codable, Sendable, Equatable {
    public let repository: String
    public let window: String
    public let action: DesktopNavigationAction

    public init(repository: String, window: String, action: DesktopNavigationAction) {
        self.repository = repository; self.window = window; self.action = action
    }
}

public enum DesktopNavigationAction: Codable, Sendable, Equatable {
    case workingTasks, back, forward, next, previous
    /// Enter a member of the shown Dashboard as full Work, as a tile click does.
    case enter(work: WorkWorkspaceKey)
    /// A Working Task by its 1-based slot, as ⌘1–9 select it.
    case slot(Int)

    private enum CodingKeys: String, CodingKey { case kind, work, slot }
    private enum Kind: String, Codable { case enter, back, forward, slot, next, previous; case workingTasks = "working_tasks" }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .workingTasks: self = .workingTasks
        case .enter: self = .enter(work: try values.decode(WorkWorkspaceKey.self, forKey: .work))
        case .back: self = .back
        case .forward: self = .forward
        case .slot: self = .slot(try values.decode(Int.self, forKey: .slot))
        case .next: self = .next
        case .previous: self = .previous
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        let kind: Kind
        switch self {
        case .workingTasks: kind = .workingTasks
        case .enter(let work):
            kind = .enter
            try values.encode(work, forKey: .work)
        case .back: kind = .back
        case .forward: kind = .forward
        case .slot(let slot):
            kind = .slot
            try values.encode(slot, forKey: .slot)
        case .next: kind = .next
        case .previous: kind = .previous
        }
        try values.encode(kind, forKey: .kind)
    }
}

/// The destination after navigating; a following inspection reports the same.
public struct DesktopNavigationReceipt: Codable, Sendable, Equatable {
    public let repository: String
    public let window: String
    public let destination: DesktopDestination
    public let history: DesktopHistory
    public let observedAt: Int64

    public init(repository: String, window: String, destination: DesktopDestination, history: DesktopHistory, observedAt: Int64) {
        self.repository = repository; self.window = window; self.destination = destination
        self.history = history; self.observedAt = observedAt
    }
    enum CodingKeys: String, CodingKey {
        case repository, window, destination, history
        case observedAt = "observed_at"
    }
}

/// A stale or missing target: nothing changed. Apple events report it with
/// `scriptErrorNumber`, which `lf desktop` maps to exit status 3.
public struct DesktopTargetError: LocalizedError, Sendable {
    public static let scriptErrorNumber = 1003
    public let message: String
    public init(_ message: String) { self.message = message }
    public var errorDescription: String? { message }
}

/// Source readings are separate from the UI snapshot time and from each other.
/// Unavailable/saved evidence retains identity and dates, but never legal actions.
public struct DesktopTaskInspection: Codable, Sendable, Equatable {
    public let id: String
    public let reading: String
    public let reason: String?
    public let roadmapGeneratedAt: String?
    public let conditionObservedAt: String?
    public let actions: TaskActionModel?
    public let runControl: TaskRunControl?

    public init(id: String, reading: String, reason: String?, roadmapGeneratedAt: String?,
                conditionObservedAt: String?, actions: TaskActionModel?, runControl: TaskRunControl?) {
        self.id = id; self.reading = reading; self.reason = reason
        self.roadmapGeneratedAt = roadmapGeneratedAt; self.conditionObservedAt = conditionObservedAt
        self.actions = actions; self.runControl = runControl
    }
    enum CodingKeys: String, CodingKey {
        case id, reading, reason, actions
        case roadmapGeneratedAt = "roadmap_generated_at", conditionObservedAt = "condition_observed_at"
        case runControl = "run_control"
    }
}

public struct DesktopSessionInspection: Codable, Sendable, Equatable {
    public let id: String
    public let machineId: String?
    public let reading: String
    public let reason: String?
    /// The Session reader currently supplies no observation timestamp. Do not
    /// substitute UI inspection time or the provider's generation for freshness.
    public let observedAt: String?
    public let actions: [SessionAction]?

    public init(id: String, machineId: String?, reading: String, reason: String?,
                observedAt: String?, actions: [SessionAction]?) {
        self.id = id; self.machineId = machineId; self.reading = reading; self.reason = reason
        self.observedAt = observedAt; self.actions = actions
    }
    enum CodingKeys: String, CodingKey {
        case id, reading, reason, actions
        case machineId = "machine_id", observedAt = "observed_at"
    }
}

public struct DesktopWorkspaceInspection: Codable, Sendable, Equatable {
    public let work: WorkWorkspaceKey
    public let title: String
    public let isOpen: Bool
    public let selected: Bool
    public let layout: DesktopLayoutInspection
    public let focusedPane: String
    public let zoomedPane: String?
    public let hiddenPanes: [String]

    public init(work: WorkWorkspaceKey, title: String, isOpen: Bool, selected: Bool,
                layout: DesktopLayoutInspection, focusedPane: String, zoomedPane: String?, hiddenPanes: [String]) {
        self.work = work; self.title = title; self.isOpen = isOpen; self.selected = selected; self.layout = layout
        self.focusedPane = focusedPane; self.zoomedPane = zoomedPane; self.hiddenPanes = hiddenPanes
    }
    enum CodingKeys: String, CodingKey {
        case work, title, selected, layout
        case isOpen = "is_open", focusedPane = "focused_pane", zoomedPane = "zoomed_pane", hiddenPanes = "hidden_panes"
    }
}

/// Children retain first/second order; a pane has no children. Values are copied
/// from MultiplexerStore at inspection, never written back as layout authority.
public struct DesktopShellInspection: Codable, Sendable, Equatable {
    public let id: String
    public let incarnation: String
    public let surface: String?
    public let location: WorkspaceIdentity?
}

public struct DesktopLayoutInspection: Codable, Sendable, Equatable {
    public let pane: String?
    public let incarnation: String?
    public let surface: String?
    public let content: String?
    public let subject: String?
    public let axis: String?
    public let ratio: Double?
    public let children: [DesktopLayoutInspection]
    public let location: WorkspaceIdentity?
    public let tabs: [DesktopShellInspection]
    public let selectedTab: String?

    enum CodingKeys: String, CodingKey {
        case pane, incarnation, surface, content, subject, axis, ratio, children, location, tabs
        case selectedTab = "selected_tab"
    }

    public init(_ layout: LayoutNode, surface: (PaneContent) -> String?) {
        switch layout {
        case .leaf(let state):
            if case .shell(let shells, let selected) = state.content {
                tabs = shells.map { tab in
                    let content = PaneContent.shell(tabs: [tab], selected: tab.id)
                    return DesktopShellInspection(id: tab.id, incarnation: tab.incarnation, surface: surface(content), location: tab.location)
                }
                selectedTab = selected
            } else { tabs = []; selectedTab = nil }
            location = state.content.location
            pane = state.id; incarnation = state.incarnation; self.surface = surface(state.content); axis = nil; ratio = nil; children = []
            switch state.content {
            case .shell: content = "shell"; subject = nil
            case .session(let id, _, _): content = "session"; subject = id
            case .browser(let id, _, _, _, _): content = "browser"; subject = id
            }
        case .split(let splitAxis, let first, let second, let splitRatio):
            tabs = []; selectedTab = nil
            location = nil
            pane = nil; incarnation = nil; self.surface = nil; content = nil; subject = nil; axis = splitAxis.rawValue; ratio = splitRatio
            children = [Self(first, surface: surface), Self(second, surface: surface)]
        }
    }
}

/// An exact retained view, never a selector that follows current focus.
/// A read additionally requires the native surface incarnation. Neither grants input authority.
public struct DesktopPaneTarget: Codable, Sendable, Equatable {
    public let repository: String
    public let window: String
    public let work: WorkWorkspaceKey
    public let pane: String
    public let incarnation: String
    public let tab: String?

    public init(repository: String, window: String, work: WorkWorkspaceKey,
                pane: String, incarnation: String, tab: String?) {
        self.repository = repository; self.window = window; self.work = work; self.pane = pane; self.incarnation = incarnation; self.tab = tab
    }
    enum CodingKeys: String, CodingKey {
        case repository, window, work, pane, incarnation, tab
    }
    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(repository, forKey: .repository)
        try values.encode(window, forKey: .window)
        try values.encode(work, forKey: .work)
        try values.encode(pane, forKey: .pane)
        try values.encode(incarnation, forKey: .incarnation)
        try values.encode(tab, forKey: .tab)
    }
}

/// The affected pane, including a newly allocated or reused companion. This
/// acknowledges the view operation, not terminal or provider readiness.
public struct DesktopPaneReceipt: Codable, Sendable, Equatable {
    public let repository: String
    public let window: String
    public let work: WorkWorkspaceKey
    public let pane: String
    public let observedAt: Int64
    public let tab: String?

    public init(target: DesktopPaneTarget, pane: String, tab: String?, observedAt: Int64) {
        repository = target.repository; window = target.window
        work = target.work
        self.pane = pane; self.tab = tab; self.observedAt = observedAt
    }
    enum CodingKeys: String, CodingKey {
        case repository, window, work, pane, tab
        case observedAt = "observed_at"
    }
}

public struct DesktopPaneCommand: Codable, Sendable, Equatable {
    public let target: DesktopPaneTarget
    public let action: DesktopPaneAction

    public init(target: DesktopPaneTarget, action: DesktopPaneAction) {
        self.target = target; self.action = action
    }
}

public enum DesktopPaneAction: Codable, Sendable, Equatable {
    case hide, restore, focus, shell
    case text(surface: String, text: String)
    case key(surface: String, key: DesktopKey)
    case files(task: String), graphLog(task: String)
    case move(destination: DesktopPaneTarget, axis: SplitAxis)
    case resize(toward: DesktopPaneTarget, ratio: Double)
    case zoom(enabled: Bool)

    enum CodingKeys: String, CodingKey { case kind, axis, destination, toward, ratio, enabled, task, surface, text, key }
    private enum Kind: String, Codable { case hide, restore, focus, move, resize, zoom, shell, files, text, key; case graphLog = "graph_log" }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .text: self = .text(surface: try values.decode(String.self, forKey: .surface),
                                 text: try values.decode(String.self, forKey: .text))
        case .key: self = .key(surface: try values.decode(String.self, forKey: .surface),
                               key: try values.decode(DesktopKey.self, forKey: .key))
        case .hide: self = .hide
        case .restore: self = .restore
        case .focus: self = .focus
        case .shell: self = .shell
        case .files: self = .files(task: try values.decode(String.self, forKey: .task))
        case .graphLog: self = .graphLog(task: try values.decode(String.self, forKey: .task))
        case .move: self = .move(destination: try values.decode(DesktopPaneTarget.self, forKey: .destination),
                                axis: try values.decode(SplitAxis.self, forKey: .axis))
        case .resize: self = .resize(toward: try values.decode(DesktopPaneTarget.self, forKey: .toward),
                                    ratio: try values.decode(Double.self, forKey: .ratio))
        case .zoom: self = .zoom(enabled: try values.decode(Bool.self, forKey: .enabled))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        let kind: Kind
        switch self {
        case .text(let surface, let text):
            kind = .text
            try values.encode(surface, forKey: .surface)
            try values.encode(text, forKey: .text)
        case .key(let surface, let key):
            kind = .key
            try values.encode(surface, forKey: .surface)
            try values.encode(key, forKey: .key)
        case .hide: kind = .hide
        case .restore: kind = .restore
        case .focus: kind = .focus
        case .shell: kind = .shell
        case .files(let task):
            kind = .files
            try values.encode(task, forKey: .task)
        case .graphLog(let task):
            kind = .graphLog
            try values.encode(task, forKey: .task)
        case .move(let destination, let axis):
            kind = .move
            try values.encode(destination, forKey: .destination)
            try values.encode(axis, forKey: .axis)
        case .resize(let toward, let ratio):
            kind = .resize
            try values.encode(toward, forKey: .toward)
            try values.encode(ratio, forKey: .ratio)
        case .zoom(let enabled):
            kind = .zoom
            try values.encode(enabled, forKey: .enabled)
        }
        try values.encode(kind, forKey: .kind)
    }
}

public enum DesktopKey: String, Codable, Sendable, CaseIterable {
    case enter, tab, escape, backspace, delete, left, right, up, down, home, end
}

/// A bounded observation of one native surface, independent of current focus.
public struct DesktopTextRequest: Codable, Sendable, Equatable {
    public let target: DesktopPaneTarget
    public let surface: String
    public let region: DesktopTextRegion
    public let maxBytes: Int

    public init(target: DesktopPaneTarget, surface: String, region: DesktopTextRegion, maxBytes: Int) {
        self.target = target; self.surface = surface; self.region = region; self.maxBytes = maxBytes
    }
    enum CodingKeys: String, CodingKey { case target, surface, region; case maxBytes = "max_bytes" }
}

public enum DesktopTextRegion: String, Codable, Sendable { case screen, scrollback, selection }

public struct DesktopTextReading: Codable, Sendable, Equatable {
    public let request: DesktopTextRequest
    public let observedAt: Int64
    public let hidden: Bool
    public let result: DesktopTextResult

    public init(request: DesktopTextRequest, observedAt: Int64, hidden: Bool, result: DesktopTextResult) {
        self.request = request; self.observedAt = observedAt; self.hidden = hidden; self.result = result
    }
    enum CodingKeys: String, CodingKey { case request, hidden, result; case observedAt = "observed_at" }
}

public enum DesktopTextUnavailable: String, Codable, Sendable {
    case missingSurface = "missing_surface"
    case notTerminal = "not_terminal"
}

/// Empty text is a successful observation, never a missing surface.
public enum DesktopTextResult: Codable, Sendable, Equatable {
    case available(text: String, truncated: Bool)
    case unavailable(reason: DesktopTextUnavailable)

    private enum CodingKeys: String, CodingKey { case status, text, truncated, reason }
    private enum Status: String, Codable { case available, unavailable }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Status.self, forKey: .status) {
        case .available:
            self = .available(text: try values.decode(String.self, forKey: .text),
                              truncated: try values.decode(Bool.self, forKey: .truncated))
        case .unavailable:
            self = .unavailable(reason: try values.decode(DesktopTextUnavailable.self, forKey: .reason))
        }
    }
    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .available(let text, let truncated):
            try values.encode(Status.available, forKey: .status)
            try values.encode(text, forKey: .text)
            try values.encode(truncated, forKey: .truncated)
        case .unavailable(let reason):
            try values.encode(Status.unavailable, forKey: .status)
            try values.encode(reason, forKey: .reason)
        }
    }
}
