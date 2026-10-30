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

/// One `lf desktop events` frame: the windows whose reading changed, whole, and
/// the windows that closed. The first frame after subscribing carries every window.
public struct DesktopChange: Codable, Sendable, Equatable {
    public let observedAt: Int64
    public let windows: [DesktopWindowInspection]
    public let closed: [String]

    public init(observedAt: Int64, windows: [DesktopWindowInspection], closed: [String]) {
        self.observedAt = observedAt
        self.windows = windows
        self.closed = closed
    }
    enum CodingKeys: String, CodingKey { case observedAt = "observed_at", windows, closed }
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
    /// The sidebar's Tasks in ⌘1–9 order.
    public let list: [DesktopListEntry]
    /// Every gesture `perform desktop action` accepts.
    public let actions: [DesktopActionDescriptor]
    public let workspaces: [DesktopWorkspaceInspection]
    public let view: WorkViewPlan?
    /// The view's named parts that are neither panes nor tiles, such as a Wave view's board.
    public let viewParts: [DesktopViewPart]
    /// Work shown in this window, in slot fill order.
    public let openWork: [WorkWorkspaceKey]
    public let opening: DesktopOpening?

    public init(repository: String, window: String, path: String?, destination: DesktopDestination, history: DesktopHistory,
                reading: String, reason: String?, task: DesktopTaskInspection?,
                session: DesktopSessionInspection?, list: [DesktopListEntry], actions: [DesktopActionDescriptor],
                workspaces: [DesktopWorkspaceInspection], openWork: [WorkWorkspaceKey], view: WorkViewPlan?,
                viewParts: [DesktopViewPart], opening: DesktopOpening?) {
        self.repository = repository; self.window = window; self.path = path
        self.destination = destination; self.history = history
        self.reading = reading; self.reason = reason; self.task = task
        self.session = session; self.list = list; self.actions = actions; self.workspaces = workspaces; self.openWork = openWork; self.view = view
        self.viewParts = viewParts; self.opening = opening
    }
    enum CodingKeys: String, CodingKey {
        case repository, window, path, destination, history, reading, reason, task, session, list, actions, workspaces, view, opening
        case openWork = "open_work"
        case viewParts = "view_parts"
    }
}

/// A named part of a view that `-p` addresses before tiles and panes. Each kind
/// reports its own state; `actions` names the verbs it accepts.
public struct DesktopViewPart: Codable, Sendable, Equatable {
    public enum Kind: String, Codable, Sendable { case board, session, strip }
    public let name: String
    public let kind: Kind
    /// The Work a session part converses with.
    public let work: WorkWorkspaceKey?
    /// A board's shown section: `backlog`, `working`, `autonomous` or `done`.
    public let section: String?
    /// A board's Done window in days; 0 includes all ages.
    public let doneDays: Int?
    /// A board as rendered text, what `lf desktop capture -p board` prints.
    public let text: String?
    /// A strip's page, from 1, and its page count.
    public let page: Int?
    public let pages: Int?
    /// A strip's tiles on the shown page; positions still count the whole list.
    public let visible: [WorkWorkspaceKey]
    /// Hidden by `lf desktop hide` or its hover button; the view's `Show…` menu lists it.
    public let hidden: Bool
    /// The part's share of its split: the conversation's and board's of the top
    /// band's width, the strip's of the view's height.
    public let ratio: Double
    /// Filling the whole view, as `lf desktop zoom` or its hover button set it.
    public let zoomed: Bool
    public let actions: [String]

    public init(name: String, kind: Kind, work: WorkWorkspaceKey?, section: String?, doneDays: Int?, text: String?,
                page: Int?, pages: Int?, visible: [WorkWorkspaceKey], hidden: Bool, ratio: Double, zoomed: Bool,
                actions: [String]) {
        self.name = name; self.kind = kind; self.work = work; self.section = section; self.doneDays = doneDays
        self.text = text; self.page = page; self.pages = pages; self.visible = visible; self.hidden = hidden
        self.ratio = ratio; self.zoomed = zoomed; self.actions = actions
    }
    enum CodingKeys: String, CodingKey {
        case name, kind, work, section, text, page, pages, visible, hidden, ratio, zoomed, actions
        case doneDays = "done_days"
    }
}

/// One sidebar Task; `position` is what ⌘n and `lf desktop -t n` select.
public struct DesktopListEntry: Codable, Sendable, Equatable {
    public let position: Int
    public let work: WorkWorkspaceKey
    public let identifier: String?
    public let title: String

    public init(position: Int, work: WorkWorkspaceKey, identifier: String?, title: String) {
        self.position = position; self.work = work; self.identifier = identifier; self.title = title
    }
}

/// A registry action as scripts discover it: name, arguments and default keys.
public struct DesktopActionDescriptor: Codable, Sendable, Equatable {
    public let name: String
    public let title: String
    /// `window` actions need no pane; `pane` actions address one (the focused pane for keys).
    public let scope: DesktopActionScope
    public let arguments: [DesktopActionArgument]
    /// Display spellings of the default key bindings, such as `⌘[`.
    public let keys: [String]

    public init(name: String, title: String, scope: DesktopActionScope, arguments: [DesktopActionArgument], keys: [String]) {
        self.name = name; self.title = title; self.scope = scope; self.arguments = arguments; self.keys = keys
    }
}

public enum DesktopActionScope: String, Codable, Sendable { case window, pane }

public struct DesktopActionArgument: Codable, Sendable, Equatable {
    public enum Kind: String, Codable, Sendable { case string, integer, boolean, url, pane, axis, direction, ratio, key }
    public let name: String
    public let kind: Kind
    public let required: Bool

    public init(name: String, kind: Kind, required: Bool) {
        self.name = name; self.kind = kind; self.required = required
    }
}

/// One gesture for an exact window, and for pane actions an exact pane.
/// Arguments are the `k=v` strings an `lf desktop` action passes; the registry parses them.
public struct DesktopActionRequest: Codable, Sendable, Equatable {
    public let repository: String
    public let window: String
    public let pane: DesktopPaneTarget?
    public let action: String
    public let args: [String: String]

    public init(repository: String, window: String, pane: DesktopPaneTarget?, action: String, args: [String: String]) {
        self.repository = repository; self.window = window; self.pane = pane; self.action = action; self.args = args
    }
    enum CodingKeys: String, CodingKey { case repository, window, pane, action, args }
    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(repository, forKey: .repository)
        try values.encode(window, forKey: .window)
        try values.encode(pane, forKey: .pane)
        try values.encode(action, forKey: .action)
        try values.encode(args, forKey: .args)
    }
}

/// What the window reads after the action, as the next `inspect desktop` reports it.
/// This acknowledges the view change, not terminal or provider readiness.
public struct DesktopActionReceipt: Codable, Sendable, Equatable {
    public let repository: String
    public let window: String
    public let action: String
    public let destination: DesktopDestination
    public let history: DesktopHistory
    /// The affected pane and tab, including a newly added shell tab.
    public let pane: String?
    public let tab: String?
    public let workspace: DesktopWorkspaceInspection?
    public let observedAt: Int64
    public let appliesTo: DesktopAppliesTo

    public init(repository: String, window: String, action: String, destination: DesktopDestination, history: DesktopHistory,
                pane: String?, tab: String?, workspace: DesktopWorkspaceInspection?, observedAt: Int64, appliesTo: DesktopAppliesTo) {
        self.repository = repository; self.window = window; self.action = action
        self.destination = destination; self.history = history
        self.pane = pane; self.tab = tab; self.workspace = workspace; self.observedAt = observedAt
        self.appliesTo = appliesTo
    }
    enum CodingKeys: String, CodingKey {
        case repository, window, action, destination, history, pane, tab, workspace
        case observedAt = "observed_at"
        case appliesTo = "applies_to"
    }
    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(repository, forKey: .repository)
        try values.encode(window, forKey: .window)
        try values.encode(action, forKey: .action)
        try values.encode(destination, forKey: .destination)
        try values.encode(history, forKey: .history)
        try values.encode(pane, forKey: .pane)
        try values.encode(tab, forKey: .tab)
        try values.encode(workspace, forKey: .workspace)
        try values.encode(observedAt, forKey: .observedAt)
        try values.encode(appliesTo, forKey: .appliesTo)
    }
}

/// Whether a verb changed its window or a Dashboard's shared record, which every
/// window showing it follows.
public enum DesktopAppliesTo: String, Codable, Sendable, Equatable {
    case window
    case dashboard
}

/// What a window shows, as its sidebar, crumbs and tile clicks select it.
public enum DesktopDestination: Codable, Sendable, Equatable {
    /// Full Work; the repository conversation has a `repository` key.
    case work(WorkWorkspaceKey)
    case dashboard(id: String)
    case workingTasks
    /// An unsaved query; the window's `view` carries its plan.
    case view(selection: WorkSelection)
    /// A tile entered as full Work; back returns to its Dashboard or Wave view.
    case entered(from: DesktopBoard, work: WorkWorkspaceKey)

    private enum CodingKeys: String, CodingKey { case kind, work, id, selection, from }
    private enum Kind: String, Codable { case work, dashboard, view, entered; case workingTasks = "working_tasks" }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .work: self = .work(try values.decode(WorkWorkspaceKey.self, forKey: .work))
        case .dashboard: self = .dashboard(id: try values.decode(String.self, forKey: .id))
        case .workingTasks: self = .workingTasks
        case .view: self = .view(selection: try values.decode(WorkSelection.self, forKey: .selection))
        case .entered: self = .entered(from: try values.decode(DesktopBoard.self, forKey: .from),
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
        case .entered(let from, let work):
            try values.encode(Kind.entered, forKey: .kind)
            try values.encode(from, forKey: .from)
            try values.encode(work, forKey: .work)
        }
    }
}

/// `{"dashboard": id}`, `{"wave": id}`, `"working_tasks"` or `"repository"`, matching Rust's external tagging.
public enum DesktopBoard: Codable, Sendable, Equatable {
    case dashboard(String)
    case workingTasks
    case wave(String)
    /// The repository page, the default Wave's view.
    case repository

    private enum CodingKeys: String, CodingKey { case dashboard, wave }

    public init(from decoder: Decoder) throws {
        if let name = try? decoder.singleValueContainer().decode(String.self) {
            switch name {
            case "working_tasks": self = .workingTasks
            case "repository": self = .repository
            default:
                throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "Unknown board \(name)"))
            }
        } else {
            let values = try decoder.container(keyedBy: CodingKeys.self)
            if let wave = try values.decodeIfPresent(String.self, forKey: .wave) { self = .wave(wave) }
            else { self = .dashboard(try values.decode(String.self, forKey: .dashboard)) }
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
        case .wave(let id):
            var values = encoder.container(keyedBy: CodingKeys.self)
            try values.encode(id, forKey: .wave)
        case .repository:
            var value = encoder.singleValueContainer()
            try value.encode("repository")
        }
    }
}

/// Destinations back and forward reach, as the window's chevrons do.
public struct DesktopHistory: Codable, Sendable, Equatable {
    public let back: Int
    public let forward: Int
    public init(back: Int, forward: Int) { self.back = back; self.forward = forward }
}

/// A stale or missing target: nothing changed. The Desktop socket reports it as
/// error code `target`, which `lf desktop` maps to exit status 3.
public struct DesktopTargetError: LocalizedError, Sendable {
    public let message: String
    public init(_ message: String) { self.message = message }
    public var errorDescription: String? { message }
}

/// An unknown action or a bad action argument: nothing changed. The Desktop
/// socket reports it as error code `argument`, which `lf desktop` maps to exit status 2.
public struct DesktopArgumentError: LocalizedError, Sendable {
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
/// A shell tab carries its own occurrence and surface; a browser tab shares its pane's.
/// A shell surface's commands as its shell integration reports them: lines `run`
/// submitted, commands finished, and the last one's exit status. `lf desktop wait` reads it.
public struct DesktopCommandStatus: Codable, Sendable, Equatable {
    public let submitted: Int
    public let finished: Int
    public let exitStatus: Int?

    public init(submitted: Int, finished: Int, exitStatus: Int?) {
        self.submitted = submitted; self.finished = finished; self.exitStatus = exitStatus
    }
    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(submitted, forKey: .submitted)
        try values.encode(finished, forKey: .finished)
        try values.encode(exitStatus, forKey: .exitStatus)
    }
    enum CodingKeys: String, CodingKey { case submitted, finished; case exitStatus = "exit_status" }
}

public struct DesktopTabInspection: Codable, Sendable, Equatable {
    public enum Kind: String, Codable, Sendable { case shell, files; case graphLog = "graph_log"; case url }
    public let id: String
    public let kind: Kind
    public let title: String
    public let incarnation: String
    public let surface: String?
    public let location: WorkspaceIdentity?
    public let url: String?
    /// Shell tabs with a live surface; `nil` before the surface starts and for browser tabs.
    public let commands: DesktopCommandStatus?

    public init(id: String, kind: Kind, title: String, incarnation: String, surface: String?, location: WorkspaceIdentity?, url: String?,
                commands: DesktopCommandStatus?) {
        self.id = id; self.kind = kind; self.title = title; self.incarnation = incarnation
        self.surface = surface; self.location = location; self.url = url; self.commands = commands
    }
    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(id, forKey: .id)
        try values.encode(kind, forKey: .kind)
        try values.encode(title, forKey: .title)
        try values.encode(incarnation, forKey: .incarnation)
        try values.encode(surface, forKey: .surface)
        try values.encode(location, forKey: .location)
        try values.encode(url, forKey: .url)
        try values.encode(commands, forKey: .commands)
    }
    enum CodingKeys: String, CodingKey { case id, kind, title, incarnation, surface, location, url, commands }
}

public struct DesktopLayoutInspection: Codable, Sendable, Equatable {
    public let pane: String?
    public let incarnation: String?
    public let surface: String?
    /// The designated kind: `session`, `shell` or `browser`.
    public let content: String?
    public let subject: String?
    public let axis: String?
    public let ratio: Double?
    public let children: [DesktopLayoutInspection]
    public let location: WorkspaceIdentity?
    public let tabs: [DesktopTabInspection]
    public let selectedTab: String?

    enum CodingKeys: String, CodingKey {
        case pane, incarnation, surface, content, subject, axis, ratio, children, location, tabs
        case selectedTab = "selected_tab"
    }

    public init(_ layout: LayoutNode, surface: (PaneContent) -> String?, commands: (PaneContent) -> DesktopCommandStatus?) {
        switch layout {
        case .leaf(let state):
            switch state.content {
            case .shell(let shells, let selected):
                tabs = shells.map { tab in
                    let content = PaneContent.shell(tabs: [tab], selected: tab.id)
                    return DesktopTabInspection(id: tab.id, kind: .shell, title: tab.command?.joined(separator: " ") ?? "Shell", incarnation: tab.incarnation,
                        surface: surface(content), location: tab.location, url: nil, commands: commands(content))
                }
                selectedTab = selected
            case .browser(_, let pages, let selected, _, _):
                tabs = pages.map { page in
                    let kind: DesktopTabInspection.Kind
                    let url: String?
                    switch page {
                    case .files: kind = .files; url = nil
                    case .graphLog: kind = .graphLog; url = nil
                    case .url(_, let address): kind = .url; url = address.absoluteString
                    }
                    return DesktopTabInspection(id: page.id, kind: kind, title: page.title, incarnation: state.incarnation,
                        surface: nil, location: state.content.location, url: url, commands: nil)
                }
                selectedTab = selected
            case .session: tabs = []; selectedTab = nil
            }
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
            children = [Self(first, surface: surface, commands: commands), Self(second, surface: surface, commands: commands)]
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
