#if os(macOS) && canImport(GhosttyKit)
import AppKit
import Foundation
import GhosttyKit
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

private let fixtureRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent()
    .appendingPathComponent("tests/fixtures/dto")

private func fixture(_ name: String) throws -> Data {
    try Data(contentsOf: fixtureRoot.appendingPathComponent(name))
}

@Suite("Task recent Runs and Session rows native proof", .requiresDisplay, .serialized)
@MainActor
struct TaskRunsProofTests {
    @Test("Recent Runs load on demand for their own Task; Session rows show recorded provider and summary")
    func runsFollowTheirTask() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry(localHomeId: fixtureHomeId)
        let workspace = registry.workspace(for: fixtureWorkspace(repo))
        var terminals: [GhosttyMetalView] = []
        for _ in 0..<2 {
            workspace.multiplexer.newShell()
            let pane = workspace.multiplexer.focusedPaneId
            let terminal = registry.surfaces.view(for: .shell(pane))
            terminal.frame = CGRect(x: 0, y: 0, width: 400, height: 350)
            terminal.workingDirectory = NSTemporaryDirectory()
            terminal.command = buildWorkspaceShellCommand(id: pane, argv: ["/bin/cat"], env: [:])
            terminal.createSurface(manager: GhosttyManager.shared)
            terminals.append(terminal)
        }
        defer { for terminal in terminals { registry.surfaces.release(terminal.terminal) } }
        let surfaces = try terminals.map { try #require($0.surface) }
        let shells = workspace.multiplexer.layout.allPanes.map(\.id)
        let layout = workspace.multiplexer.layout

        let work = WorkReference.task(id: "ts_review00000000000000000000000000")
        var design = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(
            renameFixtureRecord("design", title: "review-design", work: work)
        )) as? [String: Any])
        design["state"] = "waiting"
        design["actions"] = sessionActionFixture(kind: "conversation", state: "waiting")
        design["terminal_ids"] = [shells[0]]
        design["open_argv"] = ["must-not-launch"]
        design["provider"] = "claude"
        design["ready_summary"] = "Design revised; ready for your review."
        var notes = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(
            renameFixtureRecord("notes", title: "notes", work: work)
        )) as? [String: Any])
        notes["provider"] = NSNull()
        notes["open_argv"] = ["must-not-launch"]
        let source = try RunSource(session: JSONSerialization.data(withJSONObject: [design, notes]))
        let query = RegistryQuery { args, _ in try await source.respond(args) }
        let model = PodiumModel(query: query, repoPath: repo)
        await model.refresh()
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1500, height: 900),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        try await settle(window)
        let draft = "runs-proof-draft"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        func find(_ id: String) throws -> InspectableView<ViewType.ClassifiedView> {
            try view.inspect().find(viewWithAccessibilityIdentifier: id)
        }
        func label(_ id: String) throws -> String { try find(id).accessibilityLabel().string() }
        func waitFor(_ condition: () async throws -> Bool) async throws {
            for _ in 0..<30 {
                if try await condition() { return }
                try await settle(window)
            }
        }

        // Task A overview: Session rows carry only what their Runs recorded,
        // and showing the Task reads no Run history.
        model.select(.task(id: "issue-review"))
        try await waitFor { (try? find("task-session-design")) != nil }
        try await settle(window)
        #expect(try find("task-session-provider-design").text().string() == "claude")
        #expect(try find("task-session-summary-design").text().string() == "Design revised; ready for your review.")
        _ = try find("task-session-notes")
        #expect((try? find("task-session-provider-notes")) == nil)
        #expect((try? find("task-session-summary-notes")) == nil)
        #expect(try label("task-runs-toggle") == "Recent runs, collapsed")
        #expect(await source.reads.isEmpty)

        // Expanding reads that Task's exact identifier through the shared reader.
        await source.reply("W2-131", with: .runs(["run_a1", "run_a2"]))
        try find("task-runs-toggle").button().tap()
        try await waitFor { (try? find("task-run-run_a2:12")) != nil && !model.recentRuns.inFlight.contains("issue-review") }
        #expect(await source.reads == [["usage", "--days", "0", "--task", "W2-131", "--json"]])
        #expect(try label("task-runs-toggle") == "Recent runs, expanded")
        _ = try find("task-run-run_a1:12")
        #expect((try? find("task-run-run_a2:12").find(text: "Unknown")) != nil)
        try captureIfRequested(window, name: "task-runs-expanded")

        // A failed refresh keeps the last Runs and says they may be out of date.
        await source.reply("W2-131", with: .failure)
        try find("task-runs-toggle").button().tap()
        try await settle(window)
        try find("task-runs-toggle").button().tap()
        try await waitFor { model.recentRuns["issue-review"].errorMessage != nil }
        try await settle(window)
        #expect(try find("task-runs-stale").text().string() == "May be out of date")
        _ = try find("task-run-run_a1:12")
        #expect(try find("task-runs-status").text().string().contains("Session history unavailable"))

        // A late read of A cannot publish into B; B reads only when expanded.
        await source.reply("W2-131", with: .held(["run_a3"]))
        try find("task-runs-retry").button().tap()
        try await waitFor { await source.isHolding }
        let readsBeforeB = await source.reads.count
        model.select(.task(id: "issue-available"))
        try await waitFor { (try? find("task-runs-toggle")) != nil }
        try await settle(window)
        #expect(try label("task-runs-toggle") == "Recent runs, collapsed")
        #expect(await source.reads.count == readsBeforeB)
        await source.reply("W2-156", with: .runs([]))
        try find("task-runs-toggle").button().tap()
        try await waitFor { (try? find("task-runs-empty")) != nil }
        #expect(try find("task-runs-empty").text().string() == "No Session history recorded for this Task.")
        await source.release()
        try await waitFor { !model.recentRuns.inFlight.contains("issue-review") }
        try await settle(window)
        #expect(model.recentRuns["issue-review"].value?.map(\.id) == ["run_a3:12"])
        #expect(model.recentRuns["issue-available"].value?.isEmpty == true)
        #expect((try? find("task-run-run_a3:12")) == nil)
        #expect(await source.mutations.isEmpty)

        // The Session and its companion survived every interaction.
        model.navigation.content = .terminals
        model.navigation.selectedSessionId = "design"
        workspace.multiplexer.setFocusedPane(shells[0])
        try await settle(window)
        #expect(workspace.multiplexer.layout == layout)
        for (index, surface) in surfaces.enumerated() {
            #expect(terminals[index].surface == surface)
            let reply = index == 0 ? draft : "companion-responds"
            let input = index == 0 ? "\n" : reply + "\n"
            input.withCString { ghostty_surface_text(surface, $0, UInt(input.utf8.count)) }
            let deadline = ContinuousClock.now + .seconds(3)
            while terminalText(surface).components(separatedBy: reply).count < 3, ContinuousClock.now < deadline {
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(terminalText(surface).components(separatedBy: reply).count == 3)
        }
    }

    private func settle(_ window: NSWindow) async throws {
        window.contentView?.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
    }

    private func captureIfRequested(_ window: NSWindow, name: String) throws {
        guard let directory = ProcessInfo.processInfo.environment["LOOPFLOW_FLOW_CAPTURE_DIR"],
              let content = window.contentView,
              let bitmap = content.bitmapImageRepForCachingDisplay(in: content.bounds) else { return }
        content.cacheDisplay(in: content.bounds, to: bitmap)
        let png = try #require(bitmap.representation(using: .png, properties: [:]))
        try png.write(to: URL(fileURLWithPath: directory).appendingPathComponent("\(name).png"))
    }

    private func terminalText(_ surface: ghostty_surface_t) -> String {
        var text = ghostty_text_s()
        let selection = ghostty_selection_s(
            top_left: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
            bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0),
            rectangle: false
        )
        guard ghostty_surface_read_text(surface, selection, &text) else { return "" }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text else { return "" }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }
}

/// Shared planning reads plus per-Task `lf runs` replies. Any other
/// operation — including preparing or starting a Task — fails loudly.
private actor RunSource {
    enum Reply { case runs([String]), held([String]), failure }

    private let roadmap: String
    private let session: String
    private let template: [String: Any]
    private var replies: [String: Reply] = [:]
    private var hold: CheckedContinuation<Void, Never>?
    private(set) var reads: [[String]] = []
    private(set) var mutations: [[String]] = []
    var isHolding: Bool { hold != nil }

    init(session: Data) throws {
        roadmap = String(decoding: try fixture("roadmap_snapshot.json"), as: UTF8.self)
        self.session = String(decoding: session, as: UTF8.self)
        let detail = try JSONSerialization.jsonObject(with: fixture("wave_detail.json")) as? [String: Any]
        let runs = detail?["runs"] as? [String: Any]
        template = (runs?["items"] as? [[String: Any]])?.first ?? [:]
    }

    func reply(_ task: String, with reply: Reply) { replies[task] = reply }
    func release() { hold?.resume(); hold = nil }

    func respond(_ args: [String]) async throws -> String {
        switch (args.first, args.dropFirst().first) {
        case ("roadmap", _): return roadmap
        case ("wave", "list"): return "[]"
        case ("activity", _): return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
        case ("session", "list"): return #"{"entries":\#(session),"next":null}"#
        case ("flow", "list"): return "[]"
        case ("task", "comment"):
            return #"{"identifier":"fixture","comments":[]}"#
        case ("usage", "--days"):
            reads.append(args)
            let task = args[4]
            let ids: [String]
            switch replies[task] {
            case .runs(let chosen): ids = chosen
            case .held(let chosen):
                await withCheckedContinuation { hold = $0 }
                ids = chosen
            case .failure, nil:
                throw RegistryQueryError("Session history unavailable in this fixture")
            }
            let runs = ids.enumerated().map { index, id -> [String: Any] in
                var run = template
                run["artifact_key"] = id; run["session_id"] = id
                run["task_identifier"] = task
                if index == 1 { run["recorded_outcome"] = NSNull(); run["recorded_at"] = NSNull() }
                return run
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: runs), as: UTF8.self)
        default:
            mutations.append(args)
            throw RegistryQueryError("Unexpected recent-Runs proof operation: \(args)")
        }
    }
}
#endif
