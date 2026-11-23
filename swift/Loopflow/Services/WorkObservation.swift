import Foundation

/// What a window shows, kept current by one served subscription per part: the reads
/// `lf … --json --subscribe` prints, each internally consistent.
public struct WorkObservation: Sendable {
    public let frames: AsyncThrowingStream<WorkFrame, any Error>
    /// Requests are delivered in the order they are made.
    public let request: @Sendable (WorkRequest) -> Void
    /// Returns only after every owned subscription has exited.
    public let cancel: @Sendable () async -> Void

    public init(
        frames: AsyncThrowingStream<WorkFrame, any Error>,
        request: @escaping @Sendable (WorkRequest) -> Void,
        cancel: @escaping @Sendable () async -> Void
    ) {
        self.frames = frames
        self.request = request
        self.cancel = cancel
    }
}

/// The Work whose activity is shown: `lf history`'s filters.
public struct WorkActivityScope: Equatable, Sendable {
    public let wave: String?
    public let project: String?
    public let task: String?

    public init(wave: String?, project: String?, task: String?) {
        self.wave = wave
        self.project = project
        self.task = task
    }
}

/// What a window shows; it selects which subscriptions run and with what arguments.
public struct WorkScope: Equatable, Sendable {
    public let repo: String?
    public let headless: Bool
    /// Task identifier whose work is shown.
    public let task: String?
    public let activity: WorkActivityScope?
    /// The Wave whose started Tasks the sidebar lists; nil lists the repository's.
    public let taskListWave: String?

    public init(repo: String?, headless: Bool, task: String?, activity: WorkActivityScope?, taskListWave: String?) {
        self.repo = repo
        self.headless = headless
        self.task = task
        self.activity = activity
        self.taskListWave = taskListWave
    }
}

/// Every request carries an id; a frame's `answers` names the newest one its
/// reading began after.
public enum WorkRequest: Equatable, Sendable {
    case scope(id: Int, WorkScope)
    case refresh(id: Int)
    /// Confirm delivery after a command's commit: each named part's subscription, or
    /// every running subscription when `replay` is empty, answers before the
    /// checkpoint frame follows.
    case checkpoint(id: Int, replay: [String])

    public var id: Int {
        switch self {
        case .scope(let id, _), .refresh(let id), .checkpoint(let id, _): id
        }
    }
}

/// One subscription a window runs, and the read behind it.
public enum WorkPart: String, CaseIterable, Sendable {
    case task, sessions, planning, selections
    case taskList = "task_list"
    case planHome = "plan_home"
    case workActivity = "work_activity"

    /// The read this part subscriptions for `scope`; nil when the scope shows no such part.
    public func read(for scope: WorkScope) -> ServedRead? {
        switch self {
        case .planning: return .allPlans
        case .selections: return scope.repo.map { .desktopViewList(directory: $0) }
        case .taskList:
            return scope.repo.map { .desktopViewGet(directory: $0, expression: TaskList.started.expression(wave: scope.taskListWave)) }
        case .planHome: return scope.repo.map { .repoTracker(directory: $0) }
        case .sessions: return scope.repo.map { .sessionList(directory: $0, headless: scope.headless, after: nil, limit: nil, task: nil) }
        case .task:
            // Without a repository the Task resolves Machine-wide, from no checkout.
            return scope.task.map { .taskGet(directory: scope.repo ?? "/", issue: $0) }
        case .workActivity:
            return scope.activity.map {
                .sessionHistory(since: "7d", limit: 50, wave: $0.wave, project: $0.project, task: $0.task)
            }
        }
    }
}

/// One part's reading as a window applies it, decoded from its subscription's frame.
public struct WorkFrame: Sendable {
    /// `lf wave show --all`; Waves are the plan's own.
    public struct Planning: Sendable {
        public let roadmap: RoadmapSnapshot
        public let waves: [WaveSnapshot]
    }

    public struct PlanHomeReading: Sendable {
        public let repo: String
        public let plan: RepositoryPlan
    }

    public struct Sessions: Sendable {
        public let repo: String
        public let includesHeadless: Bool
        public let entries: [SessionRecord]
    }

    /// `lf task status`: a Task's work, each of its Graph processes in the
    /// order of `work.graphProcesses`, comments, and its recorded execution.
    public struct TaskPart: Sendable {
        public let task: String
        /// The Task's row, current even after the Task leaves the plan.
        public let row: RoadmapTask?
        public let work: TaskOverview
        public let graphProcesses: [GraphProcessDetail]
        public let comments: TaskComments
        public let execution: TaskExecution?
    }

    public struct WorkActivityPart: Sendable {
        public let scope: WorkActivityScope
        public let snapshot: WorkActivitySnapshot
    }

    /// A `nil` body is a reading that failed; `unavailable` says why.
    public enum Content: Sendable {
        case planning(Planning?)
        case selections([WorkViewPlan]?)
        /// The sidebar's started Tasks, as the phone lists them.
        case taskList(WorkViewPlan?)
        case planHome(PlanHomeReading?)
        case sessions(Sessions?)
        case task(TaskPart?)
        case workActivity(WorkActivityPart?)
        case checkpoint

        public var part: String {
            switch self {
            case .planning: WorkPart.planning.rawValue
            case .selections: WorkPart.selections.rawValue
            case .taskList: WorkPart.taskList.rawValue
            case .planHome: WorkPart.planHome.rawValue
            case .sessions: WorkPart.sessions.rawValue
            case .task: WorkPart.task.rawValue
            case .workActivity: WorkPart.workActivity.rawValue
            case .checkpoint: "checkpoint"
            }
        }
    }

    /// Orders every frame of one observation.
    public let sequence: Int
    public let answers: Int?
    public let unavailable: String?
    public let content: Content
    /// The revisions the Machine read this frame at; nil without a store, and
    /// for frames the window made itself.
    public let readAt: Revisions?
    /// Wire text of the parts a saved workspace keeps, exactly as a one-shot
    /// read would have printed them.
    public let wire: Wire?
    /// When the observation handed this frame to its window.
    public let received = ContinuousClock.now

    public struct Wire: Sendable {
        public let roadmap: String?
        public let waves: String?
        public let sessionPage: String?
        /// A Task list frame's whole line, which `SavedReadings` keeps.
        public let taskList: Data?
    }

    public init(sequence: Int, answers: Int?, unavailable: String?, content: Content, wire: Wire?, readAt: Revisions? = nil) {
        self.sequence = sequence
        self.answers = answers
        self.unavailable = unavailable
        self.content = content
        self.wire = wire
        self.readAt = readAt
    }

    /// The same reading answering a later request.
    func answering(_ answers: Int, sequence: Int) -> WorkFrame {
        WorkFrame(sequence: sequence, answers: answers, unavailable: unavailable, content: content, wire: wire, readAt: readAt)
    }

    public struct TaskStatusReading: Decodable {
        public let overview: TaskOverview?
        public let graphProcesses: [GraphProcessDetail]
        public let comments: TaskComments?
        public let execution: TaskExecution?
        public let row: RoadmapTask?

        enum CodingKeys: String, CodingKey {
            case overview, comments, execution, row
            case graphProcesses = "graph_processes"
        }
    }

    /// Decode one line of `part`'s subscription, read for `scope`.
    public static func decode(part: WorkPart, scope: WorkScope, sequence: Int, line: Data) throws -> WorkFrame {
        func envelope<Body: Decodable>(_: Body.Type) throws -> SubscriptionFrame<Body> {
            try JSONDecoder().decode(SubscriptionFrame<Body>.self, from: line)
        }
        func frame<Body>(_ envelope: SubscriptionFrame<Body>, _ content: Content, unavailable: String? = nil,
                         wire: Wire? = nil) -> WorkFrame {
            WorkFrame(sequence: sequence, answers: envelope.answers, unavailable: envelope.unavailable ?? unavailable,
                      content: content, wire: wire, readAt: envelope.readAt)
        }
        func text(_ value: Any?) -> String? {
            guard let value, let data = try? JSONSerialization.data(withJSONObject: value) else { return nil }
            return String(data: data, encoding: .utf8)
        }
        func body() throws -> Any? {
            (try JSONSerialization.jsonObject(with: line) as? [String: Any])?["body"]
        }
        switch part {
        case .planning:
            let read = try envelope(RoadmapSnapshot.self)
            guard let roadmap = read.body else { return frame(read, .planning(nil)) }
            let waves = roadmap.waves.map(\.wave).sorted { ($0.repo, $0.name) < ($1.repo, $1.name) }
            let raw = try body()
            let rows = ((raw as? [String: Any])?["waves"] as? [[String: Any]] ?? [])
                .compactMap { $0["wave"] as? [String: Any] }
                .sorted { ($0["repo"] as? String ?? "", $0["name"] as? String ?? "")
                    < ($1["repo"] as? String ?? "", $1["name"] as? String ?? "") }
            return frame(read, .planning(Planning(roadmap: roadmap, waves: waves)),
                         wire: Wire(roadmap: text(raw), waves: text(rows), sessionPage: nil, taskList: nil))
        case .selections:
            let read = try envelope([WorkViewPlan].self)
            return frame(read, .selections(read.body))
        case .taskList:
            let read = try envelope(WorkViewPlan.self)
            return frame(read, .taskList(read.body),
                         wire: Wire(roadmap: nil, waves: nil, sessionPage: nil, taskList: read.body == nil ? nil : line))
        case .planHome:
            let read = try envelope(RepositoryPlan.self)
            return frame(read, .planHome(read.body.map {
                PlanHomeReading(repo: scope.repo ?? "", plan: $0)
            }))
        case .sessions:
            let read = try envelope([SessionRecord].self)
            let entries = try body() as? [Any]
            return frame(read, .sessions(read.body.map {
                Sessions(repo: scope.repo ?? "", includesHeadless: scope.headless, entries: $0)
            }), wire: Wire(roadmap: nil, waves: nil,
                           sessionPage: entries.flatMap { text(["entries": $0, "next": NSNull()] as [String: Any]) },
                           taskList: nil))
        case .task:
            let read = try envelope(TaskStatusReading.self)
            guard let task = scope.task, let status = read.body else { return frame(read, .task(nil)) }
            guard let work = status.overview, let comments = status.comments else {
                return frame(read, .task(nil), unavailable: "Task \(task) is not registered on this Machine")
            }
            return frame(read, .task(TaskPart(task: task, row: status.row, work: work,
                                              graphProcesses: status.graphProcesses,
                                              comments: comments, execution: status.execution)))
        case .workActivity:
            let read = try envelope(WorkActivitySnapshot.self)
            return frame(read, .workActivity(zip(scope.activity, read.body).map {
                WorkActivityPart(scope: $0, snapshot: $1)
            }))
        }
    }
}

extension WorkFrame {
    /// `part`'s failed reading for a `line` whose body this build can't decode;
    /// throws when the line is not a reading at all.
    static func unread(part: WorkPart, sequence: Int, line: Data, reason: String) throws -> WorkFrame {
        struct Unread: Decodable { init(from decoder: any Decoder) {} }
        let read = try JSONDecoder().decode(SubscriptionFrame<Unread>.self, from: line)
        let content: Content = switch part {
        case .planning: .planning(nil)
        case .selections: .selections(nil)
        case .taskList: .taskList(nil)
        case .planHome: .planHome(nil)
        case .sessions: .sessions(nil)
        case .task: .task(nil)
        case .workActivity: .workActivity(nil)
        }
        return WorkFrame(sequence: sequence, answers: read.answers, unavailable: reason, content: content,
                         wire: nil, readAt: read.readAt)
    }
}

extension DecodingError {
    /// Where decoding stopped, for the log and the Notice: "(missing owned_by_you at body.tasks.0)".
    var place: String {
        // An array position names itself "Index 0"; the path says 0.
        func path(_ context: Context) -> String {
            if context.codingPath.isEmpty { return "root" }
            return context.codingPath.map { $0.intValue.map(String.init) ?? $0.stringValue }.joined(separator: ".")
        }
        return switch self {
        case .keyNotFound(let key, let context): "(missing \(key.stringValue) at \(path(context)))"
        case .valueNotFound(_, let context), .typeMismatch(_, let context), .dataCorrupted(let context):
            "(unexpected value at \(path(context)))"
        @unknown default: "(unreadable)"
        }
    }
}

private func zip<A, B>(_ a: A?, _ b: B?) -> (A, B)? {
    guard let a, let b else { return nil }
    return (a, b)
}

/// Every field is required; an absent one is a parse error, not a nil.
public struct SubscriptionFrame<Body: Decodable>: Decodable {
    public let answers: Int?
    public let unavailable: String?
    public let body: Body?
    public let readAt: Revisions?

    enum CodingKeys: String, CodingKey { case answers, unavailable, body, readAt = "read_at" }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        answers = try container.decode(Int?.self, forKey: .answers)
        unavailable = try container.decode(String?.self, forKey: .unavailable)
        body = try container.decode(Body?.self, forKey: .body)
        readAt = try container.decode(Revisions?.self, forKey: .readAt)
    }
}

/// One served subscription speaking `lf <read> --json --subscribe`'s newline-delimited JSON.
public struct LineSubscription: Sendable {
    public let lines: AsyncThrowingStream<Data, any Error>
    /// Queue one newline-terminated request line.
    public let send: @Sendable (Data) -> Void
    /// Ask for a reading begun after this request, even when its body is unchanged.
    public func checkpoint(_ id: Int) { send(Data("{\"id\":\(id)}\n".utf8)) }

    /// Returns only after the subscription has ended.
    public let cancel: @Sendable () async -> Void
    /// What to say of a line this build can't decode, when the transport knows
    /// which server sent it: `ServerLink.unreadable`.
    public let unreadable: @Sendable () -> String?

    public init(lines: AsyncThrowingStream<Data, any Error>, send: @escaping @Sendable (Data) -> Void,
                cancel: @escaping @Sendable () async -> Void, unreadable: @escaping @Sendable () -> String?) {
        self.lines = lines
        self.send = send
        self.cancel = cancel
        self.unreadable = unreadable
    }
}

/// Runs the subscriptions a window's scope selects and merges their frames into one
/// observation. A scope change restarts only the subscriptions whose read changed;
/// the others answer it from their last frame without reading again.
public final class WorkSubscriptions: @unchecked Sendable {
    /// Serves `read` as `lf <read> --subscribe` prints it.
    public typealias Launch = @Sendable (_ read: ServedRead) throws -> LineSubscription

    private struct Running {
        let generation: Int
        let read: ServedRead
        let subscription: LineSubscription
        let reader: Task<Void, Never>
        /// The request it was started after: every reading it makes is newer.
        let launched: Int
        /// The newest request its arguments answer.
        var floor: Int
        var last: WorkFrame?
    }

    private let launch: Launch
    private let lock = NSLock()
    private let continuation: AsyncThrowingStream<WorkFrame, any Error>.Continuation
    private var running: [WorkPart: Running] = [:]
    private var scope = WorkScope(repo: nil, headless: false, task: nil, activity: nil, taskListWave: nil)
    private var sequence = 0
    private var generation = 0
    /// Checkpoints waiting on these parts' answers.
    private var checkpoints: [(id: Int, waiting: Set<WorkPart>, failed: [String])] = []
    private var finished = false

    public static func observe(launch: @escaping Launch) throws -> WorkObservation {
        let (frames, continuation) = AsyncThrowingStream<WorkFrame, any Error>.makeStream()
        let subscriptions = WorkSubscriptions(launch: launch, continuation: continuation)
        try subscriptions.lock.withLock { try subscriptions.select(id: 0) }
        return WorkObservation(frames: frames, request: { subscriptions.handle($0) }, cancel: { await subscriptions.stop() })
    }

    private init(launch: @escaping Launch, continuation: AsyncThrowingStream<WorkFrame, any Error>.Continuation) {
        self.launch = launch
        self.continuation = continuation
    }

    private func handle(_ request: WorkRequest) {
        lock.withLock {
            guard !finished else { return }
            do {
                switch request {
                case .scope(let id, let next):
                    scope = next
                    try select(id: id)
                case .refresh(let id):
                    for running in running.values { running.subscription.checkpoint(id) }
                case .checkpoint(let id, let replay):
                    let parts = replay.isEmpty ? Set(running.keys)
                        : Set(replay.compactMap(WorkPart.init(rawValue:))).intersection(running.keys)
                    for part in parts { running[part]?.subscription.checkpoint(id) }
                    checkpoints.append((id, parts, []))
                    settleCheckpoints()
                }
            } catch { fail(error) }
        }
    }

    /// Start, stop or keep each part's subscription for the current scope. A kept
    /// subscription answers `id` with its last reading.
    private func select(id: Int) throws {
        var stopped: [Running] = []
        for part in WorkPart.allCases {
            let read = part.read(for: scope)
            if var current = running[part], current.read == read {
                current.floor = id
                running[part] = current
                if let last = current.last { yield(part, last) }
                continue
            }
            if let current = running.removeValue(forKey: part) { stopped.append(current) }
            guard let read else { continue }
            let subscription = try launch(read)
            generation += 1
            let generation = generation
            let reader = Task { [weak self] in
                do {
                    for try await line in subscription.lines { self?.receive(part, generation, line: line) }
                    self?.ended(part, generation, error: nil)
                } catch {
                    self?.ended(part, generation, error: error)
                }
            }
            running[part] = Running(generation: generation, read: read, subscription: subscription,
                                    reader: reader, launched: id, floor: id)
        }
        if !stopped.isEmpty {
            Task { for running in stopped { running.reader.cancel(); await running.subscription.cancel() } }
        }
        // A part that stopped no longer holds a checkpoint back.
        settleCheckpoints()
    }

    private func receive(_ part: WorkPart, _ generation: Int, line: Data) {
        lock.withLock {
            guard !finished, var current = running[part], current.generation == generation else { return }
            do {
                sequence += 1
                let decoded: WorkFrame
                do {
                    decoded = try WorkFrame.decode(part: part, scope: scope, sequence: sequence, line: line)
                } catch let error as DecodingError {
                    // Another build's reading fails this part alone, and says so;
                    // the link is up and the other parts stay current.
                    guard let reason = current.subscription.unreadable() else { throw error }
                    decoded = try WorkFrame.unread(part: part, sequence: sequence, line: line, reason: "\(reason) \(error.place)")
                }
                let frame = decoded.answering(max(decoded.answers ?? 0, current.floor), sequence: sequence)
                current.last = frame
                running[part] = current
                continuation.yield(frame)
                // Only a reading that began after the request confirms it.
                let read = max(decoded.answers ?? 0, current.launched)
                for index in checkpoints.indices where checkpoints[index].id <= read
                    && checkpoints[index].waiting.remove(part) != nil {
                    if let reason = frame.unavailable { checkpoints[index].failed.append("\(part.rawValue): \(reason)") }
                }
                settleCheckpoints()
            } catch {
                fail(RegistryQueryError("Invalid \(part.rawValue) subscription frame: \(error)"))
            }
        }
    }

    private func ended(_ part: WorkPart, _ generation: Int, error: (any Error)?) {
        lock.withLock {
            // A subscription that ends on its own ends the observation; the window reopens it.
            guard !finished, running[part]?.generation == generation else { return }
            fail(error ?? RegistryQueryError("The \(part.rawValue) subscription ended"))
        }
    }

    private func yield(_ part: WorkPart, _ last: WorkFrame) {
        sequence += 1
        continuation.yield(last.answering(max(last.answers ?? 0, running[part]?.floor ?? 0), sequence: sequence))
    }

    /// A checkpoint follows once every subscription it waits on has answered; a
    /// subscription that stopped no longer shows anything to confirm.
    private func settleCheckpoints() {
        for index in checkpoints.indices {
            checkpoints[index].waiting = checkpoints[index].waiting.filter { running[$0] != nil }
        }
        while let first = checkpoints.first, first.waiting.isEmpty {
            checkpoints.removeFirst()
            sequence += 1
            let reason = first.failed.isEmpty ? nil
                : "Work confirmation unavailable: \(first.failed.joined(separator: ", "))"
            continuation.yield(WorkFrame(sequence: sequence, answers: first.id,
                                         unavailable: reason, content: .checkpoint, wire: nil))
        }
    }

    private func fail(_ error: any Error) {
        guard !finished else { return }
        finished = true
        continuation.finish(throwing: error)
    }

    private func stop() async {
        let subscriptions: [Running] = lock.withLock {
            finished = true
            continuation.finish()
            defer { running = [:] }
            return Array(running.values)
        }
        for running in subscriptions {
            running.reader.cancel()
            await running.subscription.cancel()
        }
    }
}
