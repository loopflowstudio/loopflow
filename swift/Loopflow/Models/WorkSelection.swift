import Foundation

/// Authored identity and expression, evaluated only by the shared Rust Work reader.
public struct WorkSelection: Codable, Equatable, Sendable {
    public let id: String
    public let expression: Expression

    public indirect enum Expression: Codable, Equatable, Sendable {
        case repository
        case wave(String)
        case work(WorkWorkspaceKey)
        case compose([WorkSelection])
        case query(String)
        case filter(Filter)
        case intersect([WorkSelection])

        private enum Keys: String, CodingKey { case kind, wave, work, items, query, filter }
        private enum Kind: String, Codable { case repository, wave, work, compose, query, filter, intersect }
        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: Keys.self)
            switch try c.decode(Kind.self, forKey: .kind) {
            case .repository: self = .repository
            case .wave: self = .wave(try c.decode(String.self, forKey: .wave))
            case .work: self = .work(try c.decode(WorkWorkspaceKey.self, forKey: .work))
            case .compose: self = .compose(try c.decode([WorkSelection].self, forKey: .items))
            case .query: self = .query(try c.decode(String.self, forKey: .query))
            case .filter: self = .filter(try c.decode(Filter.self, forKey: .filter))
            case .intersect: self = .intersect(try c.decode([WorkSelection].self, forKey: .items))
            }
        }
        public func encode(to encoder: Encoder) throws {
            var c = encoder.container(keyedBy: Keys.self)
            switch self {
            case .repository: try c.encode(Kind.repository, forKey: .kind)
            case .wave(let value):
                try c.encode(Kind.wave, forKey: .kind); try c.encode(value, forKey: .wave)
            case .work(let value):
                try c.encode(Kind.work, forKey: .kind); try c.encode(value, forKey: .work)
            case .compose(let value):
                try c.encode(Kind.compose, forKey: .kind); try c.encode(value, forKey: .items)
            case .query(let value):
                try c.encode(Kind.query, forKey: .kind); try c.encode(value, forKey: .query)
            case .filter(let value):
                try c.encode(Kind.filter, forKey: .kind); try c.encode(value, forKey: .filter)
            case .intersect(let value):
                try c.encode(Kind.intersect, forKey: .kind); try c.encode(value, forKey: .items)
            }
        }
    }

    public enum Filter: Codable, Equatable, Sendable {
        case wave(String), task(String), kind(WorkWorkspaceKey.Kind), state(WorkSelectionState)
        private enum Keys: String, CodingKey { case field, value }
        private enum Field: String, Codable { case wave, task, kind, state }
        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: Keys.self)
            switch try c.decode(Field.self, forKey: .field) {
            case .wave: self = .wave(try c.decode(String.self, forKey: .value))
            case .task: self = .task(try c.decode(String.self, forKey: .value))
            case .kind: self = .kind(try c.decode(WorkWorkspaceKey.Kind.self, forKey: .value))
            case .state: self = .state(try c.decode(WorkSelectionState.self, forKey: .value))
            }
        }
        public func encode(to encoder: Encoder) throws {
            var c = encoder.container(keyedBy: Keys.self)
            switch self {
            case .wave(let value): try c.encode(Field.wave, forKey: .field); try c.encode(value, forKey: .value)
            case .task(let value): try c.encode(Field.task, forKey: .field); try c.encode(value, forKey: .value)
            case .kind(let value): try c.encode(Field.kind, forKey: .field); try c.encode(value, forKey: .value)
            case .state(let value): try c.encode(Field.state, forKey: .field); try c.encode(value, forKey: .value)
            }
        }
    }
}

public enum WorkSelectionState: String, Codable, Sendable {
    case open, available, busy, blocked, unstarted, idle, active, moving, shipping, deferred, backlog, done
}

public struct WorkMemberRef: Codable, Hashable, Sendable {
    public let source: [String]
    public let work: WorkWorkspaceKey
}

public struct WorkSelectionResolution: Codable, Equatable, Sendable {
    public let attention: [WorkWorkspaceKey]
    public var members: [WorkMemberRef]
    public let unavailable: [String]
}

/// Occurrence order; the slot layout owns geometry and members fill it in order.
public struct WorkArrangement: Codable, Equatable, Sendable {
    public enum Mode: String, Codable, Sendable { case auto, touched }
    public let order: [WorkMemberRef]
    public let hidden: [WorkMemberRef]
    public let mode: Mode
    public init(order: [WorkMemberRef], hidden: [WorkMemberRef], mode: Mode) {
        self.order = order; self.hidden = hidden; self.mode = mode
    }
}

public struct WorkArrangementProjection: Codable, Equatable, Sendable {
    public struct Suppressed: Codable, Equatable, Sendable {
        public let member: WorkMemberRef
        public let winner: WorkMemberRef
    }
    public let visible: [WorkMemberRef]
    public let suppressed: [Suppressed]
}

/// A Dashboard page's fixed slots, owned by Rust. Members fill slots by number,
/// in order, and overflow to further pages.
public struct DashboardSlotLayout: Codable, Equatable, Sendable {
    public indirect enum Node: Codable, Equatable, Sendable {
        case slot(Int)
        /// `.vertical` places children side by side; `.horizontal` stacks them.
        case split(SplitAxis, [Double], [Node])
        private enum Keys: String, CodingKey { case kind, slot, axis, weights, children }
        private enum Kind: String, Codable { case slot, split }
        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: Keys.self)
            switch try c.decode(Kind.self, forKey: .kind) {
            case .slot: self = .slot(try c.decode(Int.self, forKey: .slot))
            case .split: self = .split(try c.decode(SplitAxis.self, forKey: .axis),
                try c.decode([Double].self, forKey: .weights), try c.decode([Node].self, forKey: .children))
            }
        }
        public func encode(to encoder: Encoder) throws {
            var c = encoder.container(keyedBy: Keys.self)
            switch self {
            case .slot(let slot): try c.encode(Kind.slot, forKey: .kind); try c.encode(slot, forKey: .slot)
            case .split(let axis, let weights, let children):
                try c.encode(Kind.split, forKey: .kind); try c.encode(axis, forKey: .axis)
                try c.encode(weights, forKey: .weights); try c.encode(children, forKey: .children)
            }
        }
    }
    public let root: Node
    public init(root: Node) { self.root = root }
}

public struct Dashboard: Codable, Equatable, Sendable {
    public let repository: String
    public let id: String
    public let revision: Int64
    public let name: String?
    public let pinned: Bool
    /// Nil keeps the default slot layout.
    public let slots: DashboardSlotLayout?
    public let selection: WorkSelection
    public let arrangement: WorkArrangement
}

/// A plan carries observations and intent. It makes no native readiness claim.
public struct WorkViewPlan: Codable, Equatable, Sendable {
    public let name: String?
    public let pinned: Bool
    public struct Location: Codable, Equatable, Sendable {
        public let machineID: String
        public let cwd: String
        public init(machineID: String, cwd: String) { self.machineID = machineID; self.cwd = cwd }
        enum CodingKeys: String, CodingKey { case machineID = "machine_id", cwd }
    }
    public struct Content: Codable, Equatable, Sendable {
        public let paneRevision: Int64?
        public let face: WorkPaneIntent?
        public let work: WorkWorkspaceKey
        public let title: String
        public let states: [WorkSelectionState]
        public let primarySession: String?
        public let location: Location?
        public let argv: [String]?
        public let unavailable: [String]
        enum CodingKeys: String, CodingKey {
            case work, title, states, location, argv, unavailable, face, paneRevision = "pane_revision"
            case primarySession = "primary_session"
        }
    }
    public let dashboardID: String?
    public let repository: String
    public let selection: WorkSelection
    public let savedRevision: Int64?
    public var resolution: WorkSelectionResolution
    public let arrangement: WorkArrangement
    public let projection: WorkArrangementProjection
    /// The page geometry visible members fill, in projection order.
    public let slots: DashboardSlotLayout
    public let contents: [Content]
    enum CodingKeys: String, CodingKey {
        case repository, selection, resolution, arrangement, projection, slots, contents, name, pinned
        case dashboardID = "dashboard_id", savedRevision = "saved_revision"
    }
}

/// Portable intent only. The native host creates fresh incarnations when restoring it.
public struct WorkPaneLayout: Codable, Equatable, Sendable {
    public let work: WorkWorkspaceKey
    public let revision: Int64
    public let tree: WorkPaneTree
    public let focused: String?
    public let zoomed: String?
    public let hidden: [String]
    public let drafts: [WorkFileDraft]
    public init(work: WorkWorkspaceKey, revision: Int64, tree: WorkPaneTree, focused: String?, zoomed: String?, hidden: [String], drafts: [WorkFileDraft]) {
        self.work = work; self.revision = revision; self.tree = tree; self.focused = focused
        self.zoomed = zoomed; self.hidden = hidden; self.drafts = drafts
    }
}

public indirect enum WorkPaneTree: Codable, Equatable, Sendable {
    case pane(id: String, content: WorkPaneIntent)
    case split(axis: SplitAxis, ratio: Double, first: Self, second: Self)
    private enum Keys: String, CodingKey { case kind, id, content, axis, ratio, first, second }
    private enum Kind: String, Codable { case pane, split }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        switch try c.decode(Kind.self, forKey: .kind) {
        case .pane: self = .pane(id: try c.decode(String.self, forKey: .id), content: try c.decode(WorkPaneIntent.self, forKey: .content))
        case .split: self = .split(axis: try c.decode(SplitAxis.self, forKey: .axis), ratio: try c.decode(Double.self, forKey: .ratio), first: try c.decode(Self.self, forKey: .first), second: try c.decode(Self.self, forKey: .second))
        }
    }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case .pane(let id, let content):
            try c.encode(Kind.pane, forKey: .kind); try c.encode(id, forKey: .id); try c.encode(content, forKey: .content)
        case .split(let axis, let ratio, let first, let second):
            try c.encode(Kind.split, forKey: .kind); try c.encode(axis, forKey: .axis); try c.encode(ratio, forKey: .ratio)
            try c.encode(first, forKey: .first); try c.encode(second, forKey: .second)
        }
    }
}

public enum WorkPaneIntent: Codable, Equatable, Sendable {
    case session(id: String?, location: WorkViewPlan.Location?)
    case shell(tabs: [WorkShellTab], selected: String?)
    case browser(location: WorkViewPlan.Location?, tabs: [WorkBrowserTab], selected: String?)
    private enum Keys: String, CodingKey { case kind, sessionID = "session_id", location, tabs, selected }
    private enum Kind: String, Codable { case session, shell, browser }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        switch try c.decode(Kind.self, forKey: .kind) {
        case .session: self = .session(id: try c.decode(String?.self, forKey: .sessionID),
                                       location: try c.decode(WorkViewPlan.Location?.self, forKey: .location))
        case .shell: self = .shell(tabs: try c.decode([WorkShellTab].self, forKey: .tabs),
                                   selected: try c.decode(String?.self, forKey: .selected))
        case .browser: self = .browser(location: try c.decode(WorkViewPlan.Location?.self, forKey: .location),
                                       tabs: try c.decode([WorkBrowserTab].self, forKey: .tabs),
                                       selected: try c.decode(String?.self, forKey: .selected))
        }
    }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case .session(let id, let location):
            try c.encode(Kind.session, forKey: .kind); try c.encode(id, forKey: .sessionID); try c.encode(location, forKey: .location)
        case .shell(let tabs, let selected):
            try c.encode(Kind.shell, forKey: .kind); try c.encode(tabs, forKey: .tabs); try c.encode(selected, forKey: .selected)
        case .browser(let location, let tabs, let selected):
            try c.encode(Kind.browser, forKey: .kind); try c.encode(location, forKey: .location)
            try c.encode(tabs, forKey: .tabs); try c.encode(selected, forKey: .selected)
        }
    }
}

public enum WorkBrowserTab: Codable, Equatable, Sendable, Identifiable {
    case files(id: String, tabs: [WorkFileTab], selected: String?)
    case graphLog(id: String)
    case url(id: String, url: String)
    public var id: String {
        switch self {
        case .files(let id, _, _), .graphLog(let id), .url(let id, _): id
        }
    }
    private enum Keys: String, CodingKey { case kind, id, tabs, selected, url }
    private enum Kind: String, Codable { case files, url; case graphLog = "graph_log" }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        let id = try c.decode(String.self, forKey: .id)
        switch try c.decode(Kind.self, forKey: .kind) {
        case .files: self = .files(id: id, tabs: try c.decode([WorkFileTab].self, forKey: .tabs),
                                   selected: try c.decode(String?.self, forKey: .selected))
        case .graphLog: self = .graphLog(id: id)
        case .url: self = .url(id: id, url: try c.decode(String.self, forKey: .url))
        }
    }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case .files(let id, let tabs, let selected):
            try c.encode(Kind.files, forKey: .kind); try c.encode(id, forKey: .id)
            try c.encode(tabs, forKey: .tabs); try c.encode(selected, forKey: .selected)
        case .graphLog(let id): try c.encode(Kind.graphLog, forKey: .kind); try c.encode(id, forKey: .id)
        case .url(let id, let url):
            try c.encode(Kind.url, forKey: .kind); try c.encode(id, forKey: .id); try c.encode(url, forKey: .url)
        }
    }
}

public struct WorkFileDraft: Codable, Equatable, Sendable {
    public let location: WorkViewPlan.Location
    public let path: String
    public let baseRevision: String
    public let baseContent: String
    public let text: String
    public init(location: WorkViewPlan.Location, path: String, baseRevision: String, baseContent: String, text: String) {
        self.location = location; self.path = path; self.baseRevision = baseRevision; self.baseContent = baseContent; self.text = text
    }
    enum CodingKeys: String, CodingKey {
        case location, path, text, baseRevision = "base_revision", baseContent = "base_content"
    }
}

public struct WorkShellTab: Codable, Equatable, Sendable, Identifiable {
    public let id: String
    public let location: WorkViewPlan.Location?
    public let lfProcessID: String?
    public init(id: String, location: WorkViewPlan.Location?, lfProcessID: String?) {
        self.id = id; self.location = location; self.lfProcessID = lfProcessID
    }
    enum CodingKeys: String, CodingKey { case id, location, lfProcessID = "lf_process_id" }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(id, forKey: .id); try c.encode(location, forKey: .location)
        try c.encode(lfProcessID, forKey: .lfProcessID)
    }
}

public struct WorkFileTab: Codable, Equatable, Sendable, Identifiable {
    public let id: String
    public let path: String
    public init(id: String, path: String) {
        self.id = id; self.path = path
    }
}
