import Foundation

/// Seconds on the platform event clock, including time queued before dispatch.
public struct InteractionInput: Sendable, Equatable {
    public enum Source: String, Codable, Sendable { case appKitEvent, uiKitEvent, processStart }
    public let uptime: TimeInterval
    public let source: Source

    public init(uptime: TimeInterval, source: Source) {
        self.uptime = uptime
        self.source = source
    }
}

/// Attribution follows structured Tasks; callers carry it into detached work explicitly.
public enum InteractionTrace {
    @TaskLocal public static var input: InteractionInput?
    @TaskLocal public static var current: Interaction?
}

/// Work is counted rather than inferred from elapsed time. Missing SQL receipts stay nil.
public final class InteractionWork: @unchecked Sendable {
    public struct Counts: Codable, Sendable, Equatable {
        public var cliReads: Int
        public var servedReads: Int
        public var servedWrites: Int
        public var projections: Int
        public var sqlStatements: UInt64?
        public var sqlRows: UInt64?
        public var sqlComplete: Bool
    }

    private let lock = NSLock()
    private var counts = Counts(cliReads: 0, servedReads: 0, servedWrites: 0, projections: 0, sqlStatements: 0, sqlRows: 0, sqlComplete: true)
    private var missingSQL = false
    private var sealed = false
    private var pendingRequests = 0
    private var receipts = Set<String>()

    public init() {}

    @discardableResult
    public func cliRead() -> Bool {
        lock.withLock {
            guard !sealed else { return false }
            counts.cliReads += 1
            pendingRequests += 1
            return true
        }
    }
    public func servedRead() {
        lock.withLock {
            guard !sealed else { return }
            counts.servedReads += 1
            pendingRequests += 1
        }
    }
    public func servedWrite() {
        lock.withLock {
            guard !sealed else { return }
            counts.servedWrites += 1
            pendingRequests += 1
        }
    }
    public func sqlUnavailable() {
        lock.withLock { if !sealed { missingSQL = true; counts.sqlStatements = nil; counts.sqlRows = nil } }
    }
    public func project() { lock.withLock { if !sealed { counts.projections += 1 } } }
    public var isSealed: Bool { lock.withLock { sealed } }
    public func seal() { lock.withLock { sealed = true } }
    public func requestFinished() { lock.withLock { if !sealed { pendingRequests -= 1 } } }
    public func sql(statements: UInt64, rows: UInt64, receipt: String? = nil) {
        lock.withLock {
            guard !missingSQL, !sealed else { return }
            if let receipt, !receipts.insert(receipt).inserted { return }
            counts.sqlStatements = (counts.sqlStatements ?? 0) + statements
            counts.sqlRows = (counts.sqlRows ?? 0) + rows
        }
    }
    public var snapshot: Counts {
        lock.withLock {
            var result = counts
            result.sqlComplete = !missingSQL && pendingRequests == 0
            if !result.sqlComplete { result.sqlStatements = nil; result.sqlRows = nil }
            return result
        }
    }
}

/// One input, its local acknowledgment, usable content, and terminal outcome.
/// App-state observations remain distinguishable from presented frames.
@MainActor
public final class Interaction {
    public enum Phase: String, Codable, Sendable { case input, acknowledged, usable, outcome }
    public enum Evidence: String, Codable, Sendable {
        case inputEvent = "input_event"
        case processStart = "process_start"
        case appState = "app_state"
        case renderCommit = "render_commit"
        case presentedFrame = "presented_frame"
        case capturedFrame = "captured_frame"
        case unavailable
    }
    public enum Outcome: String, Codable, Sendable {
        case completed, failed, superseded, cancelled, unmeasured
    }
    public struct Context: Codable, Sendable, Equatable {
        public let thermalState: Int
        public let liveSessions: Int?
        public let repository: String?
        /// `local` or `remote`: whether the Work runs on this device's Machine.
        public let machine: String?
        public init(thermalState: Int, liveSessions: Int?, repository: String? = nil, machine: String? = nil) {
            self.thermalState = thermalState
            self.liveSessions = liveSessions
            self.repository = repository
            self.machine = machine
        }
    }
    public struct Record: Encodable, Sendable {
        public let schema: Int
        public let observation: UUID
        public let name: String
        public let scenario: String
        public let phase: Phase
        public let evidence: Evidence
        public let inputSource: String?
        public let ms: Double?
        public let at: TimeInterval
        public let outcome: Outcome?
        public let context: Context
        public let work: InteractionWork.Counts
    }

    public nonisolated let id: UUID
    public nonisolated let work = InteractionWork()
    public let name: String
    public let scenario: String
    private let input: InteractionInput?
    private var context: Context
    private let sink: @Sendable (Record) -> Void
    private var terminal = false
    private var acknowledgedAt: TimeInterval?
    private var usableAt: TimeInterval?
    private var acknowledgmentPresented = false
    private var usabilityPresented = false
    private var visualObservation: Task<Void, Never>?

    public init(id: UUID = UUID(), name: String, scenario: String, input: InteractionInput?, context: Context,
                sink: @escaping @Sendable (Record) -> Void) {
        self.id = id
        self.name = name
        self.scenario = scenario
        self.input = input
        self.context = context
        self.sink = sink
        let evidence: Evidence = input == nil ? .unavailable : input?.source == .processStart ? .processStart : .inputEvent
        emit(.input, uptime: input?.uptime, evidence: evidence, outcome: nil)
    }

    /// Launch learns its repository only after the restored window is identified.
    public func identifyLaunch(repository: String?, liveSessions: Int?) {
        guard name == "open_app", !terminal else { return }
        context = Context(thermalState: context.thermalState, liveSessions: liveSessions, repository: repository,
                          machine: context.machine)
    }

    public func acknowledged(at uptime: TimeInterval, evidence: Evidence) {
        guard !terminal, valid(uptime), !acknowledgmentPresented else { return }
        acknowledgedAt = uptime
        acknowledgmentPresented = evidence == .presentedFrame || evidence == .capturedFrame
        emit(.acknowledged, uptime: uptime, evidence: evidence, outcome: nil)
    }

    public func usable(at uptime: TimeInterval, evidence: Evidence) {
        guard !terminal, valid(uptime), !usabilityPresented else { return }
        usableAt = uptime
        usabilityPresented = evidence == .presentedFrame || evidence == .capturedFrame
        emit(.usable, uptime: uptime, evidence: evidence, outcome: nil)
    }

    public var isFinished: Bool { terminal }
    public func waitForObservation() async { await visualObservation?.value }

    /// Serialize the visual oracles so a fast usable result cannot overtake
    /// acknowledgment. Failed capture never upgrades a model observation.
    public func observe(_ phase: Phase, capture: @escaping @MainActor () async -> TimeInterval?) {
        guard !terminal, phase == .acknowledged || phase == .usable else { return }
        guard let input, input.source != .processStart || name == "open_app" else {
            if phase == .usable { finish(.unmeasured) }
            return
        }
        let previous = visualObservation
        visualObservation = Task { [self] in
            await previous?.value
            guard !terminal else { return }
            if let uptime = await capture(), !terminal {
                if phase == .acknowledged { acknowledged(at: uptime, evidence: .capturedFrame) }
                else { usable(at: uptime, evidence: .capturedFrame) }
            }
            if phase == .usable { finish(.completed) }
        }
    }

    public func finish(_ requested: Outcome) {
        guard !terminal else { return }
        terminal = true
        work.seal()
        let ordered: Bool
        if let acknowledgedAt, let usableAt { ordered = acknowledgedAt <= usableAt }
        else { ordered = false }
        let complete = input != nil && (input?.source != .processStart || name == "open_app")
            && acknowledgmentPresented && usabilityPresented && ordered
        let outcome: Outcome = requested == .completed && !complete ? .unmeasured : requested
        emit(.outcome, uptime: ProcessInfo.processInfo.systemUptime, evidence: .unavailable, outcome: outcome)
    }

    private func valid(_ uptime: TimeInterval) -> Bool {
        uptime.isFinite && input.map { uptime >= $0.uptime } != false
    }

    private func emit(_ phase: Phase, uptime: TimeInterval?, evidence: Evidence, outcome: Outcome?) {
        let ms = input.flatMap { start in uptime.map { ($0 - start.uptime) * 1000 } }
        sink(Record(schema: 1, observation: id, name: name, scenario: scenario, phase: phase,
                    evidence: evidence, inputSource: input?.source.rawValue, ms: ms,
                    at: Date().timeIntervalSince1970, outcome: outcome, context: context, work: work.snapshot))
    }
}

/// Local, bounded daily records, shared by Desktop and phone. Failures never affect input.
public final class InteractionJournal: @unchecked Sendable {
    public static let maximumDailyBytes = 16 * 1024 * 1024
    public static let retainedDays = 14
    private let directory: URL
    private let queue = DispatchQueue(label: "studio.loopflow.interactions", qos: .utility)
    private let identity: [String: String]
    private var currentDay: String?
    private var bytes = 0

    public init(directory: URL, identity: [String: String]) {
        self.directory = directory
        self.identity = identity
    }

    public func append(_ record: Interaction.Record) { append(record, at: record.at) }

    private struct Stall: Encodable, Sendable {
        let event = "main_thread_stall"
        let at: TimeInterval
        let ms: Double
    }

    public func stalled(milliseconds: Double) {
        let now = Date().timeIntervalSince1970
        append(Stall(at: now, ms: milliseconds), at: now)
    }

    private func append<Record: Encodable & Sendable>(_ record: Record, at: TimeInterval) {
        queue.async { [self] in
            guard let encoded = try? JSONEncoder().encode(record),
                  var row = try? JSONSerialization.jsonObject(with: encoded) as? [String: Any]
            else { return }
            row["build"] = identity
            let day = ISO8601DateFormatter().string(from: Date(timeIntervalSince1970: at)).prefix(10)
            let filename = "\(day).ndjson"
            let url = directory.appendingPathComponent(filename)
            if currentDay != filename {
                try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                let files = ((try? FileManager.default.contentsOfDirectory(at: directory,
                    includingPropertiesForKeys: nil)) ?? []).filter { $0.pathExtension == "ndjson" }.sorted { $0.lastPathComponent > $1.lastPathComponent }
                for old in files where old.lastPathComponent < filename && !files.prefix(Self.retainedDays - 1).contains(old) {
                    try? FileManager.default.removeItem(at: old)
                }
                bytes = ((try? FileManager.default.attributesOfItem(atPath: url.path)[.size]) as? Int) ?? 0
                currentDay = filename
            }
            guard var data = try? JSONSerialization.data(withJSONObject: row, options: [.sortedKeys]) else { return }
            data.append(0x0a)
            if bytes + data.count > Self.maximumDailyBytes {
                // Keep the prefix; a missing outcome exposes the retention gap.
                return
            }
            if !FileManager.default.fileExists(atPath: url.path) { FileManager.default.createFile(atPath: url.path, contents: nil) }
            guard let file = try? FileHandle(forWritingTo: url) else { return }
            defer { try? file.close() }
            do {
                try file.seekToEnd()
                try file.write(contentsOf: data)
                bytes += data.count
            } catch { return }
        }
    }

    public func flush() { queue.sync {} }
}
