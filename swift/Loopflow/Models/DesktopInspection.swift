import Foundation

/// A reading of retained UI state, not another layout or Work owner.
public struct DesktopInspection: Codable, Sendable, Equatable {
    public let observedAt: Int64
    public let windows: [DesktopWindowInspection]

    public init(observedAt: Int64, windows: [DesktopWindowInspection]) {
        self.observedAt = observedAt
        self.windows = windows
    }
    enum CodingKeys: String, CodingKey { case observedAt = "observed_at", windows }
}

public struct DesktopWindowInspection: Codable, Sendable, Equatable {
    public let repository: String
    public let window: String
    public let path: String?
    public let selectionKind: String?
    public let selectionId: String?
    public let reading: String
    public let reason: String?
    public let task: DesktopTaskInspection?
    public let session: DesktopSessionInspection?
    public let supportedOperations: [String]
    public let workspaces: [DesktopWorkspaceInspection]
    public let layouts: [DesktopWorktreeInspection]

    public init(repository: String, window: String, path: String?, selectionKind: String?, selectionId: String?,
                reading: String, reason: String?, task: DesktopTaskInspection?,
                session: DesktopSessionInspection?, supportedOperations: [String], workspaces: [DesktopWorkspaceInspection], layouts: [DesktopWorktreeInspection]) {
        self.repository = repository; self.window = window; self.path = path
        self.selectionKind = selectionKind; self.selectionId = selectionId
        self.reading = reading; self.reason = reason; self.task = task
        self.session = session; self.supportedOperations = supportedOperations; self.workspaces = workspaces; self.layouts = layouts
    }
    enum CodingKeys: String, CodingKey {
        case repository, window, path, reading, reason, task, session, workspaces, layouts
        case selectionKind = "selection_kind", selectionId = "selection_id"
        case supportedOperations = "supported_operations"
    }
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
    public let machineId: String
    public let worktree: String
    public let layout: DesktopLayoutInspection
    public let focusedPane: String
    public let zoomedPane: String?
    public let hiddenPanes: [String]

    public init(machineId: String, worktree: String, layout: DesktopLayoutInspection, focusedPane: String,
                zoomedPane: String?, hiddenPanes: [String]) {
        self.machineId = machineId; self.worktree = worktree; self.layout = layout
        self.focusedPane = focusedPane; self.zoomedPane = zoomedPane; self.hiddenPanes = hiddenPanes
    }
    enum CodingKeys: String, CodingKey {
        case worktree, layout
        case machineId = "machine_id", focusedPane = "focused_pane", zoomedPane = "zoomed_pane", hiddenPanes = "hidden_panes"
    }
}

/// Children retain first/second order; a pane has no children. Values are copied
/// from MultiplexerStore at inspection, never written back as layout authority.
public struct DesktopLayoutInspection: Codable, Sendable, Equatable {
    public let pane: String?
    public let incarnation: String?
    public let surface: String?
    public let content: String?
    public let subject: String?
    public let axis: String?
    public let ratio: Double?
    public let children: [DesktopLayoutInspection]

    public init(_ layout: LayoutNode, surface: (PaneState) -> String?) {
        switch layout {
        case .leaf(let state):
            pane = state.id; incarnation = state.incarnation; self.surface = surface(state); axis = nil; ratio = nil; children = []
            switch state.content {
            case .empty: content = "empty"; subject = nil
            case .shell: content = "shell"; subject = nil
            case .session(let id): content = "session"; subject = id
            case .files(let id): content = "files"; subject = id
            case .flowLog(let id): content = "flow_log"; subject = id
            }
        case .split(let splitAxis, let first, let second, let splitRatio):
            pane = nil; incarnation = nil; self.surface = nil; content = nil; subject = nil; axis = splitAxis.rawValue; ratio = splitRatio
            children = [Self(first, surface: surface), Self(second, surface: surface)]
        }
    }
}

public struct DesktopWorktreeInspection: Codable, Sendable, Equatable {
    public let machineId: String
    public let repositoryPath: String
    public let focusedSlot: String
    public let layout: DesktopWorktreeNode

    public init(machineId: String, repositoryPath: String, focusedSlot: String, layout: WorktreeLayout) {
        self.machineId = machineId; self.repositoryPath = repositoryPath
        self.focusedSlot = focusedSlot; self.layout = DesktopWorktreeNode(layout)
    }
    enum CodingKeys: String, CodingKey {
        case machineId = "machine_id", repositoryPath = "repository_path", focusedSlot = "focused_slot", layout
    }
}

public struct DesktopWorktreeNode: Codable, Sendable, Equatable {
    public let slot: String?
    public let machineId: String?
    public let worktree: String?
    public let axis: String?
    public let children: [DesktopWorktreeNode]

    init(_ layout: WorktreeLayout) {
        switch layout {
        case .leaf(let id, let path):
            slot = id; machineId = path?.machineId; worktree = path?.worktree; axis = nil; children = []
        case .split(let splitAxis, let first, let second):
            slot = nil; machineId = nil; worktree = nil; axis = splitAxis.rawValue
            children = [Self(first), Self(second)]
        }
    }
    enum CodingKeys: String, CodingKey { case slot, worktree, axis, children, machineId = "machine_id" }
}

/// An exact retained view, never a selector that follows current focus.
/// A read additionally requires the native surface incarnation. Neither grants input authority.
public struct DesktopPaneTarget: Codable, Sendable, Equatable {
    public let repository: String
    public let window: String
    public let machineId: String
    public let worktree: String
    public let pane: String
    public let incarnation: String

    public init(repository: String, window: String, machineId: String, worktree: String,
                pane: String, incarnation: String) {
        self.repository = repository; self.window = window; self.machineId = machineId
        self.worktree = worktree; self.pane = pane; self.incarnation = incarnation
    }
    enum CodingKeys: String, CodingKey {
        case repository, window, worktree, pane, incarnation
        case machineId = "machine_id"
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
    case files(task: String), flowLog(task: String)
    case split(axis: SplitAxis)
    case move(destination: DesktopPaneTarget, axis: SplitAxis)
    case resize(toward: DesktopPaneTarget, ratio: Double)
    case zoom(enabled: Bool)

    enum CodingKeys: String, CodingKey { case kind, axis, destination, toward, ratio, enabled, task, surface, text, key }
    private enum Kind: String, Codable { case hide, restore, focus, split, move, resize, zoom, shell, files, text, key; case flowLog = "flow_log" }

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
        case .flowLog: self = .flowLog(task: try values.decode(String.self, forKey: .task))
        case .split: self = .split(axis: try values.decode(SplitAxis.self, forKey: .axis))
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
        case .flowLog(let task):
            kind = .flowLog
            try values.encode(task, forKey: .task)
        case .split(let axis):
            kind = .split
            try values.encode(axis, forKey: .axis)
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
    case boundedReaderUnavailable = "bounded_reader_unavailable"
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
