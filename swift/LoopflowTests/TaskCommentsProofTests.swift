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

@Suite("Task Comments native proof", .serialized)
@MainActor
struct TaskCommentsProofTests {
    @Test("Collapsed count, thread, failure and a late read stay with their own Task beside a live Session")
    func commentsFollowTheirTask() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: repo)
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

        var session = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(
            renameFixtureRecord("design", title: "review-design",
                                work: .task(id: "ts_review00000000000000000000000000"))
        )) as? [String: Any])
        session["state"] = "active"
        session["actions"] = sessionActionFixture(kind: "interactive", state: "active")
        session["terminal_ids"] = [shells[0]]
        session["open_argv"] = ["must-not-launch"]
        let source = try CommentSource(session: JSONSerialization.data(withJSONObject: [session]))
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
        let draft = "comments-proof-draft"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        func find(_ id: String) throws -> InspectableView<ViewType.ClassifiedView> {
            try view.inspect().find(viewWithAccessibilityIdentifier: id)
        }
        func label(_ id: String) throws -> String { try find(id).accessibilityLabel().string() }
        func waitFor(_ condition: () async throws -> Bool) async throws {
            for _ in 0..<30 where !(try await condition()) { try await settle(window) }
        }

        // Task A: the count is read on selection, the thread stays collapsed.
        await source.reply("issue-review", with: .thread(["c-bot", "c-1", "c-2"]))
        model.select(.task(id: "issue-review"))
        try await waitFor { model.comments(for: "issue-review").value != nil }
        try await settle(window)
        #expect(try label("task-comments-toggle") == "Comments, 3, collapsed")
        #expect((try? find("task-comments-thread")) == nil)
        #expect(await source.reads.first == ["pm", "task", "comments", "--id", "issue-review", "--wave", "product", "--json"])

        // Expanding rereads and shows actual authorship, dates and Markdown.
        try find("task-comments-toggle").button().tap()
        try await waitFor { (try? find("task-comment-c-1")) != nil && !model.commentsInFlight.contains("issue-review") }
        #expect(try label("task-comments-toggle") == "Comments, 3, expanded")
        let first = try find("task-comment-c-1")
        #expect(try first.find(text: "Maya").string() == "Maya")
        #expect((try? first.find(text: "nested")) != nil)
        #expect(try find("task-comment-c-bot").find(text: "Integration").string() == "Integration")
        #expect(try find("task-comment-c-bot").find(text: "Date unavailable").string() == "Date unavailable")
        #expect(try find("task-comment-c-2").find(text: "Unnamed person").string() == "Unnamed person")
        try captureIfRequested(window, name: "task-comments-expanded")

        // A failed refresh keeps the thread and says it may be out of date.
        await source.reply("issue-review", with: .failure)
        // Collapsing and expanding again rereads.
        try find("task-comments-toggle").button().tap()
        try await settle(window)
        try find("task-comments-toggle").button().tap()
        try await waitFor { model.comments(for: "issue-review").errorMessage != nil }
        try await settle(window)
        #expect(try find("task-comments-stale").text().string() == "May be out of date")
        #expect(try label("task-comments-toggle") == "Comments, 3, expanded")
        _ = try find("task-comment-c-1")
        #expect(try find("task-comments-status").text().string().contains("Linear is unavailable"))

        // A late read of A cannot publish into B, and expansion is per Task.
        await source.reply("issue-review", with: .held(["c-1"]))
        try find("task-comments-retry").button().tap()
        try await waitFor { await source.isHolding }
        await source.reply("issue-available", with: .thread([]))
        model.select(.task(id: "issue-available"))
        try await waitFor { model.comments(for: "issue-available").value != nil }
        try await settle(window)
        #expect(try label("task-comments-toggle") == "Comments, 0, collapsed")
        await source.release()
        try await waitFor { !model.commentsInFlight.contains("issue-review") }
        try await settle(window)
        #expect(model.comments(for: "issue-review").value?.comments.map(\.id) == ["c-1"])
        #expect(model.comments(for: "issue-available").value?.comments.isEmpty == true)
        #expect(try label("task-comments-toggle") == "Comments, 0, collapsed")
        try find("task-comments-toggle").button().tap()
        try await waitFor { (try? find("task-comments-empty")) != nil }
        #expect(try find("task-comments-empty").text().string() == "No comments.")

        await source.reply("issue-review", with: .thread(["c-1"]))
        model.select(.task(id: "issue-review"))
        try await waitFor { (try? find("task-comment-c-1")) != nil }
        #expect(try label("task-comments-toggle") == "Comments, 1, expanded")
        #expect((try? find("task-comments-stale")) == nil)
        #expect(await source.mutations.isEmpty)

        // The Session and its companion survived every Comments interaction.
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

/// Shared planning reads plus per-issue comment replies. Any other operation,
/// including every PM mutation, is recorded and fails loudly.
private actor CommentSource {
    enum Reply { case thread([String]), held([String]), failure }

    private let roadmap: String
    private let session: String
    private let comments: [[String: Any]]
    private var replies: [String: Reply] = [:]
    private var hold: CheckedContinuation<Void, Never>?
    private(set) var reads: [[String]] = []
    private(set) var mutations: [[String]] = []
    var isHolding: Bool { hold != nil }

    init(session: Data) throws {
        roadmap = String(decoding: try fixture("roadmap_snapshot.json"), as: UTF8.self)
        self.session = String(decoding: session, as: UTF8.self)
        let thread = try JSONSerialization.jsonObject(with: fixture("task_comments.json")) as? [String: Any]
        comments = thread?["comments"] as? [[String: Any]] ?? []
    }

    func reply(_ issue: String, with reply: Reply) { replies[issue] = reply }
    func release() { hold?.resume(); hold = nil }

    func respond(_ args: [String]) async throws -> String {
        switch (args.first, args.dropFirst().first) {
        case ("roadmap", _): return roadmap
        case ("ls", _): return "[]"
        case ("activity", _): return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
        case ("session", "list"): return session
        case ("flow", "list"): return "[]"
        case ("pm", "task") where args.dropFirst(2).first == "comments":
            reads.append(args)
            let issue = args[args.firstIndex(of: "--id")! + 1]
            let ids: [String]
            switch replies[issue] {
            case .thread(let chosen): ids = chosen
            case .held(let chosen):
                await withCheckedContinuation { hold = $0 }
                ids = chosen
            case .failure, nil:
                throw RegistryQueryError("Linear is unavailable in this fixture")
            }
            let thread: [String: Any] = [
                "identifier": issue,
                "comments": comments.filter { ids.contains($0["id"] as? String ?? "") },
            ]
            return String(decoding: try JSONSerialization.data(withJSONObject: thread), as: UTF8.self)
        default:
            mutations.append(args)
            throw RegistryQueryError("Unexpected Comments proof operation: \(args)")
        }
    }
}
#endif
