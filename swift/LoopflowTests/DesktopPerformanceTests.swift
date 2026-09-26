#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
import Vision
#if canImport(GhosttyKit)
import GhosttyKit
#endif
@testable import Loopflow
@testable import LoopflowMac

/// Opt-in native benchmark. The endpoint is captured native pixels + owned PTY input,
/// not compositor presentation. Run through scripts/desktop_performance.py.
@Suite("Desktop experience measurements", .serialized)
@MainActor
struct DesktopPerformanceTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["LF_DESKTOP_PERF_OUTPUT"] != nil))
    func measureExperiences() async throws {
        let environment = ProcessInfo.processInfo.environment
        let output = URL(fileURLWithPath: try #require(environment["LF_DESKTOP_PERF_OUTPUT"]))
        let samples = try #require(Int(environment["LF_DESKTOP_PERF_SAMPLES"] ?? "20"))
        let journal = try PerformanceJournal(url: output)
        let scenarios = ["full", "fold", "expand", "compact", "sessions", "filter", "scroll_refresh",
                         "monitor_active", "monitor_empty", "session_return", "combined_zoom", "combined_restore"]
        try journal.write(["event": "plan", "population_version": "desktop-v1",
                           "populations": ["small": 8, "large": 256], "samples": samples,
                           "scenarios": scenarios, "endpoint": "native_capture_ocr_and_pty_reply",
                           "poll_interval_ms": 5, "frame_hitches": NSNull(),
                           "scroll_refresh": ["window_height": 300, "destination": "end",
                                              "refresh_release": "after_captured_scroll", "updated_title_prefix": "Updated"],
                           "gaps": ["Compositor presentation and frame hitches are not measured.",
                                    "Fixture transport excludes CLI, retained-registry discovery and provider readiness.",
                                    "Actions use SwiftUI controls; OS event delivery latency is excluded.",
                                    "Forced bitmap capture and OCR are intrusive observer costs, recorded separately."]])
#if canImport(GhosttyKit)
        _ = NSApplication.shared
        NSApp.setActivationPolicy(.accessory)
        NSApp.finishLaunching()
        guard NSScreen.main != nil else {
            try journal.write(["event": "setup", "outcome": "unavailable", "reason": "No native screen"])
            throw PerformanceFailure("unavailable", "No native screen")
        }
        GhosttyManager.shared.initialize()
        guard GhosttyManager.shared.state == .ready else {
            try journal.write(["event": "setup", "outcome": "unavailable", "reason": "Ghostty initialization failed"])
            throw PerformanceFailure("unavailable", "Ghostty initialization failed")
        }
        for (population, taskCount) in [("small", 8), ("large", 256)] {
            do {
                try await measure(population: population, taskCount: taskCount, samples: samples, journal: journal)
            } catch {
                try journal.write(["event": "setup_or_journey", "population": population,
                                   "outcome": (error as? PerformanceFailure)?.outcome ?? "failed",
                                   "reason": String(describing: error)])
                throw error
            }
        }
#else
        try journal.write(["event": "setup", "outcome": "unavailable", "reason": "GhosttyKit is absent"])
        throw PerformanceFailure("unavailable", "GhosttyKit is absent")
#endif
    }

#if canImport(GhosttyKit)
    private func measure(population: String, taskCount: Int, samples: Int, journal: PerformanceJournal) async throws {
        let (query, planning) = try populationQuery(taskCount: taskCount)
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        try #require(model.workspace.waves.first?.tasks.count == taskCount)
        try #require(model.sessions.value?.count == taskCount / 2)
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: "/src/loopflow")
        let multiplexer = workspace.multiplexer
        multiplexer.load(sessionId: "perf-session-0")
        let sessionPane = multiplexer.focusedPaneId
        multiplexer.newShell()
        let companionPane = multiplexer.focusedPaneId
        multiplexer.load(sessionId: "perf-session-2")
        let otherPane = multiplexer.focusedPaneId
        let identities: [TerminalIdentity] = [.session("perf-session-0"), .shell(companionPane), .session("perf-session-2")]
        let terminals = identities.map { registry.surfaces.view(for: $0) }
        defer { for identity in identities { registry.surfaces.release(identity) } }
        for terminal in terminals {
            terminal.frame = CGRect(x: 0, y: 0, width: 450, height: 350)
            terminal.workingDirectory = NSTemporaryDirectory()
            terminal.command = buildGhosttyShellCommand(argv: ["/bin/cat"], env: [:])
            terminal.createSurface(manager: GhosttyManager.shared)
        }
        let surfaces = try terminals.map { try #require($0.surface) }
        multiplexer.setFocusedPane(sessionPane)
        model.select(.task(id: "perf-task-0"))
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: "/src/loopflow", workspaces: registry, query: query)
        let window = PerformanceWindow(contentRect: CGRect(x: 0, y: 0, width: 1400, height: 800),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: view)
        window.orderFront(nil)
        defer { window.contentView = nil; window.close() }
        try await wait(window, render: true) { hasRendered(window, "workspace-task-perf-task-0") }
        let navigator = try view.inspect().find(WorkspaceNavigator.self).actualView()
        let picker = try navigator.inspect().find(ViewType.Picker.self)
        let search = try navigator.inspect().find(ViewType.TextField.self)
        let records = try #require(model.sessions.value)
        let first = try #require(records.first { $0.id == "perf-session-0" })
        let other = try #require(records.first { $0.id == "perf-session-2" })
        let openTask = try #require(navigator.onOpenTask)

        for attempt in 0..<samples {
            // Reset outside the measured interval; every attempt starts from the
            // same visible population and retained surfaces, including the first.
            try search.setInput("")
            try picker.select(value: WorkspacePresentation.compact)
            try await wait(window, render: true) { hasRendered(window, "workspace-task-perf-task-0") }
            try await sample("full", population, attempt, journal, window, action: {
                try picker.select(value: WorkspacePresentation.full)
            }, ready: { hasRendered(window, "workspace-wave-wave-1") && hasRendered(window, "workspace-task-perf-task-0") })
            let disclosure = try navigator.inspect().find(viewWithAccessibilityIdentifier: "workspace-disclose-wave-wave-1").button()
            try await sample("fold", population, attempt, journal, window, action: {
                try disclosure.tap()
            }, ready: { hasRendered(window, "workspace-wave-wave-1") && !hasRendered(window, "workspace-task-perf-task-0") })
            try await sample("expand", population, attempt, journal, window, action: {
                try disclosure.tap()
            }, ready: { hasRendered(window, "workspace-task-perf-task-0") && hasRendered(window, "workspace-session-count-perf-task-0") })
            try await sample("compact", population, attempt, journal, window, action: {
                try picker.select(value: WorkspacePresentation.compact)
            }, ready: { hasRendered(window, "workspace-task-perf-task-0") && !hasRendered(window, "workspace-wave-wave-1") })
            try await sample("sessions", population, attempt, journal, window, action: {
                try picker.select(value: WorkspacePresentation.sessions)
            }, ready: { hasRendered(window, "session-row-perf-session-0") && !hasRendered(window, "workspace-task-perf-task-0") })
            try await sample("filter", population, attempt, journal, window, action: {
                try search.setInput("Conversation 002")
            }, ready: { hasRendered(window, "session-row-perf-session-2") && !hasRendered(window, "session-row-perf-session-0") })
            try #require(model.selection == .task(id: "perf-task-0"))
            try search.setInput("")
            try picker.select(value: WorkspacePresentation.compact)
            // Keep the same population; a shorter viewport makes both sizes
            // scrollable. Resize and start the held read outside the interval.
            window.setContentSize(NSSize(width: 1400, height: 300))
            try await wait(window, render: true) { hasRendered(window, "workspace-task-perf-task-0") }
            let scroll = try #require(window.contentView.flatMap { scrollView(in: $0) })
            let document = try #require(scroll.documentView)
            let destination = document.bounds.height - scroll.contentView.bounds.height
            try #require(destination > 0)
            planning.holdNextRead = true
            let refresh = Task { await model.refresh() }
            defer { planning.release() }
            try await wait(window) { planning.pending != nil }
            var scrolledOffset: CGFloat = 0
            var labelsBeforeRefresh: [String] = []
            var initialVerificationMS: Double = 0
            try await sample("scroll_refresh", population, attempt, journal, window, action: {
                scroll.contentView.scroll(to: CGPoint(x: 0, y: destination))
                scroll.reflectScrolledClipView(scroll.contentView)
                // Lazy row measurement can correct the estimated content height.
                // Capture the destination before judging retention across refresh.
                try await wait(window, render: true) {
                    hasRendered(window, "workspace-task-perf-task-\(taskCount - 1)")
                }
                scrolledOffset = scroll.contentView.bounds.minY
                labelsBeforeRefresh = window.outlineText
                initialVerificationMS = milliseconds(window.capturedAt)
                try #require(scrolledOffset > 0 && model.isRefreshing)
                planning.release()
            }, ready: {
                !model.isRefreshing && abs(scroll.contentView.bounds.minY - scrolledOffset) < 1
                    && model.selection == .task(id: "perf-task-0")
                    && model.sessions.value == records
                    && model.workspace.waves.first?.tasks.last?.task.task.id == "perf-task-\(taskCount - 1)"
                    && window.outlineText.contains { $0.contains(String(format: "Updated %03d", taskCount - 1)) }
                    && !window.outlineText.contains { $0.contains("Updated 000") }
            }, observation: {
                ["offset_after_scroll": scrolledOffset, "offset_after_refresh": scroll.contentView.bounds.minY,
                 "refresh_complete": !model.isRefreshing, "visible_labels": window.outlineText,
                 "visible_labels_before_refresh": labelsBeforeRefresh,
                 "initial_capture_verification_ms": initialVerificationMS,
                 "expected_task_id": "perf-task-\(taskCount - 1)"]
            })
            await refresh.value
            await model.refresh()
            window.setContentSize(NSSize(width: 1400, height: 800))
            scroll.contentView.scroll(to: .zero)
            scroll.reflectScrolledClipView(scroll.contentView)
            try await wait(window, render: true) { hasRendered(window, "workspace-task-perf-task-0") }
            navigator.onOpenSession(first)
            try await wait(window) { window.firstResponder === terminals[0] }
            let draft = "draft-\(attempt)"
            try send(window, draft)
            try await wait(window) { terminalText(surfaces[0]).contains(draft) }
            navigator.onOpenSession(other)
            try await wait(window) { window.firstResponder === terminals[2] && multiplexer.focusedPaneId == otherPane }
            openTask(.task(id: "perf-task-0"))
            try await wait(window) { window.firstResponder === terminals[0] }
            let monitorButton = try view.inspect().find(ViewType.Button.self, where: {
                try $0.accessibilityIdentifier() == "task-show-monitor-perf-task-0"
            })
            try await sample("monitor_active", population, attempt, journal, window, action: {
                // This Task already has a retained Session choice; the explicit
                // Monitor control reveals observation alongside it.
                try monitorButton.tap()
            }, ready: {
                !model.isRefreshingActiveRuns && hasRendered(window, "monitor-run-perf-run-0")
                    && !hasRendered(window, "monitor-run-perf-run-2")
                    && !terminals.contains { window.firstResponder === $0 }
                    && multiplexer.focusedPane.content == .monitor(taskId: "perf-task-0")
            })
            let monitorPane = multiplexer.focusedPaneId
            try await sample("monitor_empty", population, attempt, journal, window, action: {
                // A Task without Sessions opens its overview; Monitor is explicit.
                openTask(.task(id: "perf-task-1"))
                try view.inspect().find(ViewType.Button.self, where: {
                    try $0.accessibilityIdentifier() == "task-show-monitor-perf-task-1"
                }).tap()
            }, ready: { !model.isRefreshingActiveRuns && hasRendered(window, "monitor-empty-perf-task-1") })
            try await sample("session_return", population, attempt, journal, window, action: {
                navigator.onOpenSession(first)
            }, ready: {
                guard window.firstResponder === terminals[0], terminals[0].window === window,
                      terminals[0].surface == surfaces[0], terminals[1].surface == surfaces[1],
                      terminals[2].surface == surfaces[2],
                      model.navigation.selectedSessionId == first.id,
                      multiplexer.focusedPane.content == .session(id: first.id) else { return false }
                return hasRendered(window, "breadcrumb-session")
            }, input: {
                try send(window, "\n")
                try await wait(window) { terminalText(surfaces[0]).components(separatedBy: draft).count >= 3 }
                // The companion uses its existing surface and process too.
                let reply = "companion-\(attempt)"
                let text = reply + "\n"
                text.withCString { ghostty_surface_text(surfaces[1], $0, UInt(text.utf8.count)) }
                try await wait(window) { terminalText(surfaces[1]).components(separatedBy: reply).count >= 3 }
            })
            multiplexer.updateRatio(between: sessionPane, and: monitorPane, ratio: 0.5)
            try await sample("combined_zoom", population, attempt, journal, window, action: {
                multiplexer.setFocusedPane(monitorPane)
                multiplexer.toggleZoom(monitorPane)
            }, ready: { hasRendered(window, "monitor-run-perf-run-0") && terminals[0].window == nil })
            try await sample("combined_restore", population, attempt, journal, window, action: {
                multiplexer.toggleZoom(monitorPane)
                multiplexer.updateRatio(between: sessionPane, and: monitorPane, ratio: 0.6)
                navigator.onOpenSession(first)
            }, ready: {
                window.firstResponder === terminals[0] && terminals.allSatisfy { $0.window === window }
                    && hasRendered(window, "monitor-run-perf-run-0")
            })
            guard terminals.enumerated().allSatisfy({ $0.element.surface == surfaces[$0.offset] }),
                  multiplexer.layout.pane(for: companionPane)?.content == .shell,
                  multiplexer.layout.pane(for: otherPane)?.content == .session(id: other.id) else {
                throw PerformanceFailure("failed", "Retained terminal identity or companion changed")
            }
        }
    }

    private func terminalText(_ surface: ghostty_surface_t) -> String {
        var text = ghostty_text_s()
        let selection = ghostty_selection_s(
            top_left: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
            bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0), rectangle: false)
        guard ghostty_surface_read_text(surface, selection, &text) else { return "" }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text else { return "" }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }
#endif

    private func sample(_ scenario: String, _ population: String, _ attempt: Int,
                        _ journal: PerformanceJournal, _ window: PerformanceWindow,
                        action: () async throws -> Void, ready: () -> Bool,
                        observation: () -> [String: Any] = { [:] },
                        input: () async throws -> Void = {}) async throws {
        let metric = ["full", "fold", "expand", "compact", "sessions", "filter", "scroll_refresh"].contains(scenario)
            ? "hierarchy_interaction_ms" : "task_workspace_ready_ms"
        var record: [String: Any] = ["event": "begin", "id": "\(population)-\(scenario)-\(attempt)",
            "metric": metric, "scenario": scenario, "population": population, "attempt": attempt,
            "state": attempt == 0 ? "first_interaction" : "warm"]
        try journal.write(record)
        let start = DispatchTime.now().uptimeNanoseconds
        do {
            try await action()
            try await wait(window, render: true, ready: ready)
            let captured = Double(window.capturedAt - start) / 1_000_000
            let verified = milliseconds(start)
            try await input()
            record["outcome"] = "passed"
            record["capture_ready_ms"] = captured
            record["verification_ms"] = verified - captured
        } catch {
            record["outcome"] = (error as? PerformanceFailure)?.outcome ?? "failed"
            record["reason"] = String(describing: error)
            record["event"] = "end"
            record["duration_ms"] = milliseconds(start)
            record["observation"] = observation()
            try journal.write(record)
            throw error
        }
        record["event"] = "end"
        record["duration_ms"] = milliseconds(start)
        record["observation"] = observation()
        try journal.write(record)
    }

    private func milliseconds(_ start: UInt64) -> Double {
        Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
    }

    private func wait(_ window: PerformanceWindow, render: Bool = false, ready: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(5)
        repeat {
            window.contentView?.layoutSubtreeIfNeeded()
            window.layoutIfNeeded()
            window.displayIfNeeded()
            if render { try window.capture() }
            if ready() { return }
            try await Task.sleep(for: .milliseconds(5))
        } while ContinuousClock.now < deadline
        throw PerformanceFailure("timeout", "Captured native content/input endpoint not reached in 5 seconds")
    }

    private func hasRendered(_ window: PerformanceWindow, _ id: String) -> Bool {
        // Identities are asserted against the shared model/pane owners; these
        // unique fixture labels independently prove the pixels changed too.
        if id == "workspace-wave-wave-1" { return window.outlineText.contains { $0.contains("product") } }
        if let index = Int(id.replacingOccurrences(of: "workspace-task-perf-task-", with: "")) {
            return window.outlineText.contains { $0.contains(String(format: "Task %03d", index)) }
        }
        if let index = Int(id.replacingOccurrences(of: "session-row-perf-session-", with: "")) {
            return window.outlineText.contains { $0.contains(String(format: "Conversation %03d", index)) }
        }
        if id.hasPrefix("monitor-run-") {
            return window.contentText.contains { $0.contains(String(id.dropFirst("monitor-run-".count))) }
        }
        if id.hasPrefix("monitor-empty-") {
            return window.contentText.joined(separator: " ").contains("No active Runs in this observation")
        }
        return false
    }

    private func send(_ window: NSWindow, _ text: String) throws {
        for character in text {
            let event = try #require(NSEvent.keyEvent(
                with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                windowNumber: window.windowNumber, context: nil, characters: String(character),
                charactersIgnoringModifiers: String(character), isARepeat: false,
                keyCode: character == "\n" ? 36 : 0))
            window.sendEvent(event)
        }
    }

    private func scrollView(in view: NSView) -> NSScrollView? {
        if let scroll = view as? NSScrollView { return scroll }
        return view.subviews.lazy.compactMap { scrollView(in: $0) }.first
    }

    private func populationQuery(taskCount: Int) throws -> (RegistryQuery, PerformancePlanning) {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"))
        var snapshot = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var wave = try #require((snapshot["waves"] as? [[String: Any]])?.first)
        var tasks = try #require(wave["tasks"] as? [String: Any])
        let template = try #require((tasks["items"] as? [[String: Any]])?.first)
        tasks["items"] = try (0..<taskCount).map { index in
            var task = template
            var planning = try #require(task["task"] as? [String: Any])
            planning["id"] = "perf-task-\(index)"
            planning["identifier"] = "PERF-\(index)"
            planning["name"] = String(format: "Task %03d", index)
            planning["rank"] = Double(index)
            planning["completed"] = false
            task["task"] = planning
            var reference: [String: Any] = ["issue_url": NSNull(), "workspace": NSNull()]
            if index == 1 {
                reference["workspace"] = ["slug": "benchmark-empty", "branch": "benchmark-empty",
                                          "worktree": NSTemporaryDirectory(), "local_exists": true]
            }
            task["reference"] = reference
            var runtime = try #require(task["runtime"] as? [String: Any])
            runtime["work_id"] = "perf-work-\(index)"
            runtime["started"] = true
            task["runtime"] = runtime
            return task
        }
        wave["tasks"] = tasks
        wave["unavailable_tasks"] = []
        snapshot["waves"] = [wave]
        let roadmap = String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
        let planning = PerformancePlanning(roadmap: roadmap)
        let sessions = try JSONSerialization.data(withJSONObject: stride(from: 0, to: taskCount, by: 2).map { index in
            ["id": "perf-session-\(index)", "run_id": "perf-run-\(index)", "kind": "interactive",
             "work": ["kind": "task", "id": "perf-work-\(index)"],
             "title": String(format: "Conversation %03d", index), "detail": "Benchmark fixture",
             "cwd": "/src/loopflow", "wave_id": "wave-1", "state": "active", "ready_summary": NSNull(), "work_path": NSNull(),
             "actions": sessionActionFixture(kind: "interactive", state: "active"),
             "title_source": "generated", "flow_membership": ["kind": "independent"], "terminal_ids": [], "open_argv": ["must-not-launch"]] as [String: Any]
        })
        let active = try JSONSerialization.data(withJSONObject: [
            "discovery": "ready", "home": "benchmark-fixture", "observed_at": 1790270400, "task": NSNull(), "gaps": [],
            "runs": [0, 2].map { index in
                ["id": "perf-run-\(index)", "work": ["kind": "task", "id": "perf-work-\(index)"],
                 "subjects": [], "label": "Fixture Run \(index)", "harness": "fixture", "model": NSNull(),
                 "repo": "/src/loopflow", "processes": [["pid": index + 1, "provider": "fixture", "state": "waiting"]]] as [String: Any]
            }
        ])
        let sessionJSON = String(decoding: sessions, as: UTF8.self)
        let activeJSON = String(decoding: active, as: UTF8.self)
        let feed = ActiveRunsTestFeed()
        let query = RegistryQuery(watchActiveRuns: { try await feed.open(initial: activeJSON) }) { args, _ in
            switch args.first {
            case "roadmap": return await planning.read()
            case "ls": return "[]"
            case "session" where args.dropFirst().first == "list": return sessionJSON
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Benchmark does not launch providers or mutate planning")
            }
        }
        return (query, planning)
    }
}

/// Hold the fixture's transport response to exercise scrolling while the real
/// Podium reader is refreshing. No product state is changed outside that reader.
@MainActor
private final class PerformancePlanning {
    let roadmap: String
    var holdNextRead = false
    var pending: CheckedContinuation<Void, Never>?

    init(roadmap: String) { self.roadmap = roadmap }

    func read() async -> String {
        guard holdNextRead else { return roadmap }
        holdNextRead = false
        await withCheckedContinuation { pending = $0 }
        return roadmap.replacingOccurrences(of: "Task ", with: "Updated ")
    }

    func release() {
        pending?.resume()
        pending = nil
    }
}

/// A test-owned window observes its actual AppKit bitmap. Recognition verifies
/// visible labels after capture; neither a model change nor onAppear ends a sample.
@MainActor
private final class PerformanceWindow: NSWindow {
    var outlineText: [String] = []
    var contentText: [String] = []
    var capturedAt: UInt64 = 0

    func capture() throws {
        guard let host = contentView,
              let bitmap = host.bitmapImageRepForCachingDisplay(in: host.bounds) else {
            throw PerformanceFailure("unavailable", "Native capture is unavailable")
        }
        host.cacheDisplay(in: host.bounds, to: bitmap)
        guard let image = bitmap.cgImage else {
            throw PerformanceFailure("unavailable", "Native bitmap has no image")
        }
        capturedAt = DispatchTime.now().uptimeNanoseconds
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .fast
        request.recognitionLanguages = ["en-US"]
        request.usesLanguageCorrection = false
        try VNImageRequestHandler(cgImage: image).perform([request])
        let rows = request.results ?? []
        let outlineWidth = 300 / host.bounds.width
        outlineText = rows.filter { $0.boundingBox.midX < outlineWidth }.compactMap { $0.topCandidates(1).first?.string }
        contentText = rows.filter { $0.boundingBox.midX >= outlineWidth }.compactMap { $0.topCandidates(1).first?.string }
    }
}

private struct PerformanceFailure: Error, CustomStringConvertible {
    let outcome: String
    let description: String
    init(_ outcome: String, _ description: String) { self.outcome = outcome; self.description = description }
}

private final class PerformanceJournal {
    private let file: FileHandle
    init(url: URL) throws {
        guard FileManager.default.createFile(atPath: url.path, contents: nil) else {
            throw PerformanceFailure("failed", "Cannot create benchmark journal")
        }
        file = try FileHandle(forWritingTo: url)
    }
    deinit { try? file.close() }
    func write(_ record: [String: Any]) throws {
        var data = try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys])
        data.append(10)
        try file.write(contentsOf: data)
        try file.synchronize()
    }
}
#endif
