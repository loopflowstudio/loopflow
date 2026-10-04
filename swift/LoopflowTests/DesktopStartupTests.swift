#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// Opt-in, display-free startup measurement over captured `lf` reads and their
/// recorded latency. The endpoint is the model holding outline content, not a
/// rendered frame. Run through scripts/benchmarks/desktop-performance/startup.py.
@Suite("Desktop startup measurements", .serialized)
@MainActor
struct DesktopStartupTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["LF_DESKTOP_STARTUP_OUTPUT"] != nil))
    func measureStartup() async throws {
        let environment = ProcessInfo.processInfo.environment
        let output = URL(fileURLWithPath: try #require(environment["LF_DESKTOP_STARTUP_OUTPUT"]))
        let capture = try StartupCapture(directory: URL(fileURLWithPath: try #require(environment["LF_DESKTOP_STARTUP_CAPTURE"])))
        let samples = Int(environment["LF_DESKTOP_STARTUP_SAMPLES"] ?? "") ?? 5
        FileManager.default.createFile(atPath: output.path, contents: nil)
        let journal = try FileHandle(forWritingTo: output)
        defer { try? journal.close() }

        for scenario in ["uncached", "saved", "saved_offline"] {
            for attempt in 0..<samples {
                let directory = FileManager.default.temporaryDirectory.appendingPathComponent("startup-\(UUID().uuidString)")
                defer { try? FileManager.default.removeItem(at: directory) }
                if scenario != "uncached" {
                    // The previous launch: every read succeeded and was saved.
                    let saving = WorkspaceCache(directory: directory)
                    let previous = PodiumModel(query: capture.query(latency: false, offline: false, reads: ReadCounts()),
                                               repoPath: capture.repo, cache: saving)
                    await previous.refresh()
                    previous.confirmHome(capture.homeId)
                    saving.flush()
                }

                let reads = ReadCounts()
                let query = capture.query(latency: true, offline: scenario == "saved_offline", reads: reads)
                let clock = ContinuousClock()
                let start = clock.now
                let model = PodiumModel(query: query, repoPath: capture.repo, cache: WorkspaceCache(directory: directory))
                let initMs = milliseconds(clock.now - start)
                let usableFrom = model.roadmap.value != nil && model.sessions.value != nil ? "saved" : "read"
                let loop = Task { await model.keepWorkspaceCurrent() }
                var usableMs: Double?
                var settledMs: Double?
                let deadline = start + .seconds(capture.totalSeconds * 2 + 30)
                while clock.now < deadline, settledMs == nil {
                    if usableMs == nil, model.roadmap.value != nil, model.sessions.value != nil {
                        usableMs = milliseconds(clock.now - start)
                    }
                    switch model.workspaceStatus {
                    case .current, .failed: settledMs = milliseconds(clock.now - start)
                    case .loading, .updating: try await Task.sleep(for: .milliseconds(1))
                    }
                }
                loop.cancel()
                await loop.value
                let status: String = switch model.workspaceStatus {
                case .loading: "loading"
                case .updating: "updating"
                case .current: "current"
                case .failed: "failed"
                }
                let record: [String: Any] = [
                    "scenario": scenario, "attempt": attempt, "init_main_thread_ms": initMs,
                    "usable_ms": usableMs ?? NSNull(), "usable_from": usableFrom,
                    "settled_ms": settledMs ?? NSNull(), "status": status,
                    "content_kept": model.roadmap.value != nil && model.sessions.value != nil,
                    "reads_until_settled": reads.byVerb,
                ]
                journal.write(try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys]))
                journal.write(Data("\n".utf8))
            }
        }
    }

    private func milliseconds(_ duration: Duration) -> Double {
        Double(duration.components.seconds) * 1000 + Double(duration.components.attoseconds) / 1e15
    }
}

private final class ReadCounts: @unchecked Sendable {
    private let lock = NSLock()
    private var counts: [String: Int] = [:]
    func count(_ verb: String) { lock.lock(); counts[verb, default: 0] += 1; lock.unlock() }
    var byVerb: [String: Int] { lock.lock(); defer { lock.unlock() }; return counts }
}

/// One capture of the startup reads: `manifest.json` names the repository, the
/// Home and each read's wire file and wall time.
private struct StartupCapture: Sendable {
    struct Read: Decodable, Sendable {
        let verb: String
        let file: String
        let ms: Double
    }
    struct Manifest: Decodable {
        let repo: String
        let homeId: String
        let reads: [Read]
    }

    let repo: String
    let homeId: String
    let totalSeconds: Double
    private let texts: [String: [(text: String, ms: Double)]]
    /// The captured Session page each cursor leads to.
    private let pageAfter: [String: Int]

    init(directory: URL) throws {
        let manifest = try JSONDecoder().decode(Manifest.self, from: Data(contentsOf: directory.appendingPathComponent("manifest.json")))
        repo = manifest.repo
        homeId = manifest.homeId
        totalSeconds = manifest.reads.map(\.ms).reduce(0, +) / 1000
        var texts: [String: [(String, Double)]] = [:]
        for read in manifest.reads {
            let text = try String(contentsOf: directory.appendingPathComponent(read.file), encoding: .utf8)
            texts[read.verb, default: []].append((text, read.ms))
        }
        self.texts = texts
        var pageAfter: [String: Int] = [:]
        for (index, page) in (texts["session"] ?? []).enumerated() {
            if let next = try RegistryQuery.decode(SessionPage.self, from: page.0).next { pageAfter[next] = index + 1 }
        }
        self.pageAfter = pageAfter
    }

    /// Replays captured text after its recorded wall time; a verb that was not captured fails.
    func query(latency: Bool, offline: Bool, reads: ReadCounts) -> RegistryQuery {
        RegistryQuery { args, _ in
            let verb = args.first ?? ""
            reads.count(verb)
            guard let captured = texts[verb] else { throw RegistryQueryError("not captured: \(verb)") }
            let after = args.firstIndex(of: "--after").map { args[$0 + 1] }
            guard let page = after.map({ pageAfter[$0] }) ?? 0 else { throw RegistryQueryError("unknown cursor") }
            if latency { try await Task.sleep(for: .milliseconds(Int(offline ? 50 : captured[page].ms))) }
            if offline { throw RegistryQueryError("offline") }
            return captured[page].text
        }
    }
}
#endif
