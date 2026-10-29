import Foundation

/// What a window shows, kept current by one served watch per part: the reads
/// `lf … --json --watch` prints, each internally consistent.
public struct WorkObservation: Sendable {
    public let frames: AsyncThrowingStream<WorkFrame, any Error>
    /// Requests are delivered in the order they are made.
    public let request: @Sendable (WorkRequest) -> Void
    /// Returns only after every owned watch has exited.
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

/// What a window shows; it selects which watches run and with what arguments.
public struct WorkScope: Equatable, Sendable {
    public let repo: String?
    public let headless: Bool
    /// Task identifier whose work is shown.
    public let task: String?
    public let activity: WorkActivityScope?

    public init(repo: String?, headless: Bool, task: String?, activity: WorkActivityScope?) {
        self.repo = repo
        self.headless = headless
        self.task = task
        self.activity = activity
    }
}

/// Every request carries an id; a frame's `answers` names the newest one its
/// reading began after.
public enum WorkRequest: Equatable, Sendable {
    case scope(id: Int, WorkScope)
    case refresh(id: Int)
    /// Confirm delivery after a command's commit: each named part's watch, or
    /// every running watch when `replay` is empty, answers before the
    /// checkpoint frame follows.
    case checkpoint(id: Int, replay: [String])

    public var id: Int {
        switch self {
        case .scope(let id, _), .refresh(let id), .checkpoint(let id, _): id
        }
    }
}

/// A read Loopflow serves inside the app, encoded as its Rust `Operation`.
/// Each is an `lf` command, which `lf <argv> --json --watch` prints the same.
public enum WorkOperation: Equatable, Sendable, Encodable {
    /// `lf history --since <since> --limit <limit> [--wave …] [--project …] [--task …]`
    case history(since: String, limit: Int, wave: String?, project: String?, task: String?)
    /// `lf monitor ps`
    case monitorPs
    /// `lf desktop plan --all`
    case desktopPlan
    /// `lf --cwd <directory> desktop plans`
    case desktopPlans(directory: String)
    /// `lf --cwd <directory> desktop sessions [--interactive all]`
    case desktopSessions(directory: String, headless: Bool)
    /// `lf --cwd <directory> planning status`
    case planningStatus(directory: String)
    /// `lf --cwd <directory> desktop task <issue>`
    case desktopTask(directory: String, issue: String)

    public func encode(to encoder: any Encoder) throws {
        let tagged = switch self {
        case .history(let since, let limit, let wave, let project, let task):
            TaggedFields("history", ["since": since, "limit": limit, "wave": wave, "project": project, "task": task])
        case .monitorPs: TaggedFields("monitor_ps")
        case .desktopPlan: TaggedFields("desktop_plan")
        case .desktopPlans(let directory): TaggedFields("desktop_plans", ["directory": directory])
        case .desktopSessions(let directory, let headless):
            TaggedFields("desktop_sessions", ["directory": directory, "headless": headless])
        case .planningStatus(let directory): TaggedFields("planning_status", ["directory": directory])
        case .desktopTask(let directory, let issue):
            TaggedFields("desktop_task", ["directory": directory, "issue": issue])
        }
        try tagged.encode(to: encoder, tag: "operation")
    }
}

/// One watch a window runs, and the read behind it.
public enum WorkPart: String, CaseIterable, Sendable {
    case task, sessions, planning, selections, activity
    case peerPlanning = "peer_planning"
    case workActivity = "work_activity"

    /// The read this part watches for `scope`; nil when the scope shows no such part.
    public func operation(for scope: WorkScope) -> WorkOperation? {
        switch self {
        case .planning: return .desktopPlan
        case .activity: return .monitorPs
        case .selections: return scope.repo.map { .desktopPlans(directory: $0) }
        case .peerPlanning: return scope.repo.map { .planningStatus(directory: $0) }
        case .sessions: return scope.repo.map { .desktopSessions(directory: $0, headless: scope.headless) }
        case .task:
            // Without a repository the Task resolves Machine-wide, from no checkout.
            return scope.task.map { .desktopTask(directory: scope.repo ?? "/", issue: $0) }
        case .workActivity:
            return scope.activity.map {
                .history(since: "7d", limit: 50, wave: $0.wave, project: $0.project, task: $0.task)
            }
        }
    }
}

/// One part's reading as a window applies it, decoded from its watch's frame.
public struct WorkFrame: Sendable {
    /// `lf wave show --all`; Waves are the plan's own.
    public struct Planning: Sendable {
        public let roadmap: RoadmapSnapshot
        public let waves: [WaveSnapshot]
    }

    public struct PeerPlanning: Sendable {
        public let repo: String
        public let destinations: [PeerPlanningStatus]
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
        public let diagnostics: TaskWork?
    }

    public struct WorkActivityPart: Sendable {
        public let scope: WorkActivityScope
        public let snapshot: WorkActivitySnapshot
    }

    /// A `nil` body is a reading that failed; `unavailable` says why.
    public enum Content: Sendable {
        case planning(Planning?)
        case selections([WorkViewPlan]?)
        case peerPlanning(PeerPlanning?)
        case sessions(Sessions?)
        case task(TaskPart?)
        case workActivity(WorkActivityPart?)
        case activity(ActivitySnapshot?)
        case checkpoint

        public var part: String {
            switch self {
            case .planning: WorkPart.planning.rawValue
            case .selections: WorkPart.selections.rawValue
            case .peerPlanning: WorkPart.peerPlanning.rawValue
            case .sessions: WorkPart.sessions.rawValue
            case .task: WorkPart.task.rawValue
            case .workActivity: WorkPart.workActivity.rawValue
            case .activity: WorkPart.activity.rawValue
            case .checkpoint: "checkpoint"
            }
        }
    }

    /// Orders every frame of one observation.
    public let sequence: Int
    public let answers: Int?
    public let unavailable: String?
    public let content: Content
    /// Wire text of the parts a saved workspace keeps, exactly as a one-shot
    /// read would have printed them.
    public let wire: Wire?

    public struct Wire: Sendable {
        public let roadmap: String?
        public let waves: String?
        public let sessionPage: String?
    }

    public init(sequence: Int, answers: Int?, unavailable: String?, content: Content, wire: Wire?) {
        self.sequence = sequence
        self.answers = answers
        self.unavailable = unavailable
        self.content = content
        self.wire = wire
    }

    /// The same reading answering a later request.
    func answering(_ answers: Int, sequence: Int) -> WorkFrame {
        WorkFrame(sequence: sequence, answers: answers, unavailable: unavailable, content: content, wire: wire)
    }

    /// Every field is required; an absent one is a parse error, not a nil.
    private struct Envelope<Body: Decodable>: Decodable {
        let answers: Int?
        let unavailable: String?
        let body: Body?

        enum CodingKeys: String, CodingKey { case answers, unavailable, body }

        init(from decoder: any Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            answers = try container.decode(Int?.self, forKey: .answers)
            unavailable = try container.decode(String?.self, forKey: .unavailable)
            body = try container.decode(Body?.self, forKey: .body)
        }
    }

    private struct TaskStatusReading: Decodable {
        struct Execution: Decodable { let work: TaskWork }
        let overview: TaskOverview?
        let graphProcesses: [GraphProcessDetail]
        let comments: TaskComments?
        let execution: Execution?
        let row: RoadmapTask?

        enum CodingKeys: String, CodingKey {
            case overview, comments, execution, row
            case graphProcesses = "graph_processes"
        }
    }

    private struct PeerPlanningReading: Decodable {
        let destinations: [PeerPlanningStatus]
    }

    /// Decode one line of `part`'s watch, read for `scope`.
    public static func decode(part: WorkPart, scope: WorkScope, sequence: Int, line: Data) throws -> WorkFrame {
        func envelope<Body: Decodable>(_: Body.Type) throws -> Envelope<Body> {
            try JSONDecoder().decode(Envelope<Body>.self, from: line)
        }
        func frame<Body>(_ envelope: Envelope<Body>, _ content: Content, unavailable: String? = nil,
                         wire: Wire? = nil) -> WorkFrame {
            WorkFrame(sequence: sequence, answers: envelope.answers, unavailable: envelope.unavailable ?? unavailable,
                      content: content, wire: wire)
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
                         wire: Wire(roadmap: text(raw), waves: text(rows), sessionPage: nil))
        case .selections:
            let read = try envelope([WorkViewPlan].self)
            return frame(read, .selections(read.body))
        case .peerPlanning:
            let read = try envelope(PeerPlanningReading.self)
            return frame(read, .peerPlanning(read.body.map {
                PeerPlanning(repo: scope.repo ?? "", destinations: $0.destinations)
            }))
        case .sessions:
            let read = try envelope([SessionRecord].self)
            let entries = try body() as? [Any]
            return frame(read, .sessions(read.body.map {
                Sessions(repo: scope.repo ?? "", includesHeadless: scope.headless, entries: $0)
            }), wire: Wire(roadmap: nil, waves: nil,
                           sessionPage: entries.flatMap { text(["entries": $0, "next": NSNull()] as [String: Any]) }))
        case .task:
            let read = try envelope(TaskStatusReading.self)
            guard let task = scope.task, let status = read.body else { return frame(read, .task(nil)) }
            guard let work = status.overview, let comments = status.comments else {
                return frame(read, .task(nil), unavailable: "Task \(task) is not registered on this Machine")
            }
            return frame(read, .task(TaskPart(task: task, row: status.row, work: work,
                                              graphProcesses: status.graphProcesses,
                                              comments: comments, diagnostics: status.execution?.work)))
        case .workActivity:
            let read = try envelope(WorkActivitySnapshot.self)
            return frame(read, .workActivity(zip(scope.activity, read.body).map {
                WorkActivityPart(scope: $0, snapshot: $1)
            }))
        case .activity:
            let read = try envelope(ActivitySnapshot.self)
            return frame(read, .activity(read.body))
        }
    }
}

private func zip<A, B>(_ a: A?, _ b: B?) -> (A, B)? {
    guard let a, let b else { return nil }
    return (a, b)
}

/// One served watch speaking `lf <read> --json --watch`'s newline-delimited JSON.
public struct LineWatch: Sendable {
    public let lines: AsyncThrowingStream<Data, any Error>
    /// Queue one newline-terminated request line.
    public let send: @Sendable (Data) -> Void
    /// Returns only after the watch has ended.
    public let cancel: @Sendable () async -> Void

    public init(lines: AsyncThrowingStream<Data, any Error>, send: @escaping @Sendable (Data) -> Void,
                cancel: @escaping @Sendable () async -> Void) {
        self.lines = lines
        self.send = send
        self.cancel = cancel
    }
}

/// Runs the watches a window's scope selects and merges their frames into one
/// observation. A scope change restarts only the watches whose read changed;
/// the others answer it from their last frame without reading again.
public final class WorkWatches: @unchecked Sendable {
    /// Serves `operation` as `lf <read> --json --watch` prints it.
    public typealias Launch = @Sendable (_ operation: WorkOperation) throws -> LineWatch

    private struct Running {
        let generation: Int
        let operation: WorkOperation
        let watch: LineWatch
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
    private var scope = WorkScope(repo: nil, headless: false, task: nil, activity: nil)
    private var sequence = 0
    private var generation = 0
    /// Checkpoints waiting on these parts' answers.
    private var checkpoints: [(id: Int, waiting: Set<WorkPart>, failed: [String])] = []
    private var finished = false

    public static func observe(launch: @escaping Launch) throws -> WorkObservation {
        let (frames, continuation) = AsyncThrowingStream<WorkFrame, any Error>.makeStream()
        let watches = WorkWatches(launch: launch, continuation: continuation)
        try watches.lock.withLock { try watches.select(id: 0) }
        return WorkObservation(frames: frames, request: { watches.handle($0) }, cancel: { await watches.stop() })
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
                    for watch in running.values { watch.watch.send(Self.confirm(id)) }
                case .checkpoint(let id, let replay):
                    let parts = replay.isEmpty ? Set(running.keys)
                        : Set(replay.compactMap(WorkPart.init(rawValue:))).intersection(running.keys)
                    for part in parts { running[part]?.watch.send(Self.confirm(id)) }
                    checkpoints.append((id, parts, []))
                    settleCheckpoints()
                }
            } catch { fail(error) }
        }
    }

    /// Start, stop or keep each part's watch for the current scope. A kept
    /// watch answers `id` with its last reading.
    private func select(id: Int) throws {
        var stopped: [Running] = []
        for part in WorkPart.allCases {
            let operation = part.operation(for: scope)
            if var current = running[part], current.operation == operation {
                current.floor = id
                running[part] = current
                if let last = current.last { yield(part, last) }
                continue
            }
            if let current = running.removeValue(forKey: part) { stopped.append(current) }
            guard let operation else { continue }
            let watch = try launch(operation)
            generation += 1
            let generation = generation
            let reader = Task { [weak self] in
                do {
                    for try await line in watch.lines { self?.receive(part, generation, line: line) }
                    self?.ended(part, generation, error: nil)
                } catch {
                    self?.ended(part, generation, error: error)
                }
            }
            running[part] = Running(generation: generation, operation: operation, watch: watch,
                                    reader: reader, launched: id, floor: id)
        }
        if !stopped.isEmpty {
            Task { for watch in stopped { watch.reader.cancel(); await watch.watch.cancel() } }
        }
        // A part that stopped no longer holds a checkpoint back.
        settleCheckpoints()
    }

    private func receive(_ part: WorkPart, _ generation: Int, line: Data) {
        lock.withLock {
            guard !finished, var current = running[part], current.generation == generation else { return }
            do {
                sequence += 1
                let decoded = try WorkFrame.decode(part: part, scope: scope, sequence: sequence, line: line)
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
                fail(RegistryQueryError("Invalid \(part.rawValue) watch frame: \(error)"))
            }
        }
    }

    private func ended(_ part: WorkPart, _ generation: Int, error: (any Error)?) {
        lock.withLock {
            // A watch that ends on its own ends the observation; the window reopens it.
            guard !finished, running[part]?.generation == generation else { return }
            fail(error ?? RegistryQueryError("The \(part.rawValue) watch ended"))
        }
    }

    private func yield(_ part: WorkPart, _ last: WorkFrame) {
        sequence += 1
        continuation.yield(last.answering(max(last.answers ?? 0, running[part]?.floor ?? 0), sequence: sequence))
    }

    /// A checkpoint follows once every watch it waits on has answered; a
    /// watch that stopped no longer shows anything to confirm.
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
        let watches: [Running] = lock.withLock {
            finished = true
            continuation.finish()
            defer { running = [:] }
            return Array(running.values)
        }
        for watch in watches {
            watch.reader.cancel()
            await watch.watch.cancel()
        }
    }

    private static func confirm(_ id: Int) -> Data {
        Data("{\"id\":\(id)}\n".utf8)
    }
}
