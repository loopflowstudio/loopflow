// LaunchJournal — timings of real launches and their `lf` reads, kept in the
// Machine so a report needs no trace and no staged benchmark.
//
// Two bounded NDJSON files under `<Machine>/desktop-cache/timings/`: `launches`
// holds one line per launch milestone, `reads` one line per `lf` read and per
// workspace refresh. Lines carry durations, verbs and the app version; never
// arguments, output or error text. Each append is one write on a utility queue.
// Report: scripts/benchmarks/desktop-performance/timings.py.
//
// A launch also records how it ended: one `exit` line naming a quit, an
// `exit()` and its caller, a signal or an uncaught exception. `lf doctor` reads
// it; a launch with no `exit` line ended by SIGKILL, a crash the handlers could
// not see, or power loss, and doctor asks the system log.

import AppKit
import Foundation
import QuartzCore
import Synchronization

final class LaunchJournal: @unchecked Sendable {
    /// Each milestone is written once per launch; later windows add nothing.
    enum Milestone: String {
        /// The saved workspace was looked up; `cache` says hit or miss.
        case restored
        /// The first window's content was committed to the render server.
        /// Not on-glass presentation.
        case firstFrame = "first_frame"
        /// Outline rows were observed; `source` says saved or fresh.
        case usable
        /// Every part of the workspace was read by this launch.
        case fresh
        /// A refresh failed before the workspace was fresh; `part` names it.
        case refreshFailed = "refresh_failed"
    }

    /// A file past this size keeps its newer half.
    static let maxBytes = 256 * 1024

    private let directory: URL
    private let queue = DispatchQueue(label: "studio.loopflow.launch-journal", qos: .utility)
    private var launch: String?
    private var app = ""
    private var marked: Set<Milestone> = []
    private var sizes: [String: Int] = [:]
    private var tally = Tally()
    /// Set by the first exit record; every later path into `recordExit` writes nothing.
    /// Allocated once so a signal handler reaches it without retaining the journal.
    private let exited: UnsafeMutablePointer<Atomic<Bool>>

    init(directory: URL) {
        self.directory = directory
        exited = .allocate(capacity: 1)
        exited.initialize(to: Atomic(false))
    }

    deinit {
        // Only `home` installs signal handlers, and it lives for the process.
        exited.deinitialize(count: 1)
        exited.deallocate()
    }

    /// The journal of the Machine this process's `lf` reads resolve to. It writes
    /// nothing until the app calls `begin`, so tests and fixture runs stay silent.
    static let home = LaunchJournal(
        directory: WorkCache.homeDirectory.appendingPathComponent("timings", isDirectory: true))

    static var appVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "dev"
    }

    /// Start this process's launch record. Release and Dev bundles share the
    /// journal; `bundle` and `pid` tell their launches apart.
    func begin(app: String = LaunchJournal.appVersion, preMainMs: Double? = Perf.millisecondsSinceProcessStart(),
               bundle: String = Bundle.main.bundleIdentifier ?? "unbundled",
               revision: String = DesktopLog.sourceRevision) {
        let at = ISO8601DateFormatter().string(from: Date())
        let os = ProcessInfo.processInfo.operatingSystemVersion
        queue.async { [self] in
            guard launch == nil else { return }
            let id = UUID().uuidString
            launch = id
            self.app = app
            var line: [String: Any] = [
                "event": "launch", "launch": id, "app": app, "at": at, "pid": Int(getpid()), "bundle": bundle,
                "rev": revision, "os": "\(os.majorVersion).\(os.minorVersion).\(os.patchVersion)",
            ]
            if let preMainMs { line["ms"] = Self.rounded(preMainMs) }
            append(line, to: "launches")
        }
    }

    /// Milliseconds are measured from kernel process start, so they include
    /// everything before `main`.
    func mark(_ milestone: Milestone, _ detail: [String: String] = [:],
              ms: Double? = Perf.millisecondsSinceProcessStart()) {
        queue.async { [self] in write(milestone, detail, ms: ms) }
    }

    private func write(_ milestone: Milestone, _ detail: [String: String], ms: Double?) {
        guard let launch, let ms, marked.insert(milestone).inserted else { return }
        var line: [String: Any] = ["event": milestone.rawValue, "launch": launch, "app": app, "ms": Self.rounded(ms)]
        for (key, value) in detail { line[key] = value }
        append(line, to: "launches")
    }

    /// Mark once the current Core Animation transaction has been committed.
    @MainActor
    func markAfterCommit(_ milestone: Milestone) {
        CATransaction.setCompletionBlock { [self] in mark(milestone) }
    }

    /// One `lf` read. Only the subcommand words are kept, never flags or values.
    func read(_ args: [String], ms: Double, ok: Bool) {
        let verb = Self.verb(args)
        queue.async { [self] in
            record("read", "verb", verb, ms: ms, ok: ok)
            tally.reads[verb, default: 0] += 1
            if !ok { tally.failed += 1 }
        }
    }

    /// A read cancelled while its `lf` still ran, which Desktop then SIGKILLed.
    func killed(_ args: [String]) {
        let verb = Self.verb(args)
        queue.async { [self] in tally.killed[verb, default: 0] += 1 }
    }

    /// `lf`'s subcommand words, never flags or values.
    static func verb(_ args: [String]) -> String {
        args.prefix(2).filter { !$0.hasPrefix("-") }.joined(separator: " ")
    }

    /// Reads since the last heartbeat, by verb.
    struct Tally: Equatable {
        var reads: [String: Int] = [:]
        var failed = 0
        var killed: [String: Int] = [:]
    }

    /// The reads since the last call, which starts the next window.
    func takeTally() -> Tally {
        queue.sync {
            defer { tally = Tally() }
            return tally
        }
    }

    /// One workspace refresh of `part`. A failure before the launch is fresh
    /// is also that launch's `refresh_failed`.
    func refreshed(_ part: String, ms: Double, ok: Bool) {
        let sinceStart = Perf.millisecondsSinceProcessStart()
        queue.async { [self] in
            record("refresh", "part", part, ms: ms, ok: ok)
            if !ok, !marked.contains(.fresh) { write(.refreshFailed, ["part": part], ms: sinceStart) }
        }
    }

    /// Wait for queued lines; tests only.
    func flush() { queue.sync {} }

    // MARK: - Exit

    /// Record how this launch ended, once; later calls write nothing. Written
    /// before returning, because the process is going away.
    func recordExit(_ reason: String, _ detail: [String: String] = [:]) {
        guard exited.pointee.compareExchange(expected: false, desired: true, ordering: .sequentiallyConsistent)
            .exchanged else { return }
        let at = ISO8601DateFormatter().string(from: Date())
        queue.sync {
            guard let launch else { return }
            var line: [String: Any] = ["event": "exit", "launch": launch, "app": app, "at": at, "reason": reason]
            for (key, value) in detail { line[key] = value }
            append(line, to: "launches")
        }
    }

    /// Record the launch's end on every path the process can observe: quit,
    /// `exit()`, catchable signals and uncaught exceptions. Call once, after `begin`.
    static func recordExits() {
        let journal = home
        journal.queue.async {
            guard let launch = journal.launch else { return }
            ExitSignals.install(path: journal.directory.appendingPathComponent("launches.ndjson").path,
                                flag: journal.exited,
                                fields: ["event": "exit", "launch": launch, "app": journal.app, "reason": "signal"])
        }
        NotificationCenter.default.addObserver(forName: NSApplication.willTerminateNotification, object: nil,
                                               queue: nil) { _ in
            home.recordExit("quit", quitSender(NSAppleEventManager.shared().currentAppleEvent))
        }
        atexit { LaunchJournal.home.recordAtExit() }
        previousExceptionHandler = NSGetUncaughtExceptionHandler()
        NSSetUncaughtExceptionHandler { exception in
            LaunchJournal.home.recordExit("exception", ["name": exception.name.rawValue,
                                                        "message": String((exception.reason ?? "").prefix(500))])
            DesktopLog.home.writeNow(.error, "uncaught exception", [
                "name": exception.name.rawValue, "stack": exception.callStackSymbols.joined(separator: " | "),
            ])
            LaunchJournal.previousExceptionHandler?(exception)
        }
    }

    nonisolated(unsafe) private static var previousExceptionHandler: (@convention(c) (NSException) -> Void)?

    private func recordAtExit() {
        guard !exited.pointee.load(ordering: .sequentiallyConsistent) else { return }
        let stack = Thread.callStackSymbols
        recordExit("exit", Self.exitCaller(stack).map { ["caller": $0] } ?? [:])
        DesktopLog.home.writeNow(.warn, "exit() called", ["stack": stack.joined(separator: " | ")])
    }

    /// The first frame that called `exit`, outside the system libraries:
    /// `LoopflowMac 0x0000000101f00010 ghostty_app_tick + 1220`. The address stays
    /// because a stripped frame names only the nearest symbol; `atos` resolves it.
    static func exitCaller(_ stack: [String]) -> String? {
        let frames = stack.map { $0.split(separator: " ", omittingEmptySubsequences: true).map(String.init) }
        guard let exit = frames.lastIndex(where: { $0.count > 3 && $0[1].hasPrefix("libsystem") && $0[3] == "exit" })
        else { return nil }
        let system = ["libsystem", "libdyld", "dyld"]
        guard let caller = frames[(exit + 1)...].first(where: { frame in
            frame.count > 3 && !system.contains(where: { frame[1].hasPrefix($0) })
        }) else { return nil }
        return caller[1...].joined(separator: " ")
    }

    /// Who asked a quit Apple event to end the app, and why; empty for ⌘Q.
    static func quitSender(_ event: NSAppleEventDescriptor?) -> [String: String] {
        guard let event, event.eventClass == fourCharCode("aevt"), event.eventID == fourCharCode("quit")
        else { return [:] }
        var detail: [String: String] = [:]
        if let pid = event.attributeDescriptor(forKeyword: fourCharCode("spid"))?.int32Value, pid > 0 {
            detail["sender_pid"] = String(pid)
            detail["sender"] = NSRunningApplication(processIdentifier: pid)?.localizedName
        }
        let reasons = ["shut": "shutdown", "rest": "restart", "logo": "logout", "rlgo": "logout"]
        if let why = event.attributeDescriptor(forKeyword: fourCharCode("why?"))?.enumCodeValue,
           let reason = reasons[string(fourCharCode: why)] {
            detail["quit_reason"] = reason
        }
        return detail
    }

    private static func fourCharCode(_ code: String) -> UInt32 {
        code.utf8.reduce(0) { $0 << 8 | UInt32($1) }
    }

    private static func string(fourCharCode code: UInt32) -> String {
        String(decoding: [24, 16, 8, 0].map { UInt8((code >> $0) & 0xFF) }, as: UTF8.self)
    }

    private func record(_ event: String, _ key: String, _ value: String, ms: Double, ok: Bool) {
        guard launch != nil else { return }
        append(["event": event, key: value, "ms": Self.rounded(ms), "ok": ok, "app": app], to: "reads")
    }

    private static func rounded(_ ms: Double) -> Double { (ms * 10).rounded() / 10 }

    private func append(_ line: [String: Any], to name: String) {
        guard var data = try? JSONSerialization.data(withJSONObject: line, options: [.sortedKeys]) else { return }
        data.append(0x0A)
        let url = directory.appendingPathComponent("\(name).ndjson", isDirectory: false)
        if sizes[name] == nil {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            if !FileManager.default.fileExists(atPath: url.path) {
                FileManager.default.createFile(atPath: url.path, contents: nil)
            }
            sizes[name] = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int) ?? 0
        }
        // A journal that cannot be written loses timings, nothing else.
        guard let handle = try? FileHandle(forWritingTo: url) else { return }
        defer { try? handle.close() }
        guard (try? handle.seekToEnd()) != nil, (try? handle.write(contentsOf: data)) != nil else { return }
        sizes[name, default: 0] += data.count
        if sizes[name, default: 0] > Self.maxBytes { trim(url, name: name) }
    }

    private func trim(_ url: URL, name: String) {
        guard let data = try? Data(contentsOf: url) else { return }
        let tail = data.suffix(Self.maxBytes / 2)
        // Start at a whole line.
        let kept = tail.firstIndex(of: 0x0A).map { Data(tail[tail.index(after: $0)...]) } ?? Data()
        guard (try? kept.write(to: url, options: .atomic)) != nil else { return }
        sizes[name] = kept.count
    }
}
