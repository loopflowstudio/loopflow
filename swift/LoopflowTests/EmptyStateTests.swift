#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
import os
@testable import Loopflow
@testable import LoopflowMac

@Suite("Empty and failed states", .serialized)
@MainActor
struct EmptyStateTests {
    private func fixture(_ name: String) throws -> Data {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        return try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/\(name)"))
    }

    @Test("A repository with no Work and no conversation says so, offers New Task and opens its conversation")
    func emptyRepositoryOffersNewTask() throws {
        let full = try fixture("roadmap_snapshot.json")
        var wire = try #require(JSONSerialization.jsonObject(with: full) as? [String: Any])
        wire["waves"] = []
        wire["tasks"] = ["state": "ok", "items": [], "truncated": false]
        let empty = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("An empty plan reads nothing") },
                              repoPath: "/src/loopflow")
        // As in the Dev app, a conversation waits for a person to open it.
        let registry = SessionsWorkspaceRegistry(localMachineId: "fixture", opensUnvisitedWork: false)
        defer { registry.prepareForQuit() }
        model.bindWorkspaces(registry, repository: "repo")
        let workspace = registry.workspace(for: .init(repository: "repo", kind: .repository, id: "repo"),
                                           title: "repo", location: nil)
        // Nothing is empty before the plan is read.
        #expect(!model.repositoryIsEmpty(workspace))
        model.applyFixture(roadmap: .available(empty), waves: .available([]), workActivity: .loading, repos: [])
        #expect(model.repositoryIsEmpty(workspace))

        var opened = 0
        let view = try NoPlannedWork(model: model) { opened += 1 }.inspect()
        _ = try view.find(text: "No planned Work in this repository")
        _ = try view.find(text: "Say what you want built, or talk it through first.")
        #expect(model.navigation.palette == nil)
        try view.find(button: "New Task ⌘N").tap()
        #expect(model.navigation.palette == .newTask)
        try view.find(button: "Open the repository conversation").tap()
        #expect(opened == 1)

        // A conversation that failed, is opening or is attached brings the page back, and so does a Task.
        workspace.conversationError = "Machine unavailable"
        #expect(!model.repositoryIsEmpty(workspace))
        workspace.conversationError = nil
        workspace.visit()
        #expect(model.conversationIsOpening(workspace) && !model.repositoryIsEmpty(workspace))
        workspace.conversationClosed = true
        #expect(model.repositoryIsEmpty(workspace))
        let planned = try JSONDecoder().decode(RoadmapSnapshot.self, from: full)
        model.applyFixture(roadmap: .available(planned), waves: .available([]), workActivity: .loading, repos: [])
        #expect(!model.repositoryIsEmpty(workspace))
    }

    @Test("A repository that did not open shows the Machine's message and retries")
    func failedRepositoryKeepsRetry() throws {
        let reason = "fatal: not a git repository (or any of the parent directories): .git"
        var retries = 0
        let view = try RepositoryWorkspaceContent(workspace: nil, error: reason, retry: { retries += 1 }) { _ in
            EmptyView()
        }.inspect()
        _ = try view.find(text: "This repository did not open")
        _ = try view.find(text: reason)
        try view.find(button: "Retry").tap()
        #expect(retries == 1)
    }

    @Test("An unstarted Task's pane names the workflow Start will run, in the sheet's words, and starts it")
    func unstartedTaskSaysWhatStartRuns() async throws {
        let roadmap = try #require(JSONSerialization.jsonObject(with: fixture("roadmap_snapshot.json")) as? [String: Any])
        let items = try #require((roadmap["tasks"] as? [String: Any])?["items"] as? [[String: Any]])
        var row = try #require(items.first { ($0["task"] as? [String: Any])?["id"] as? String == "issue-review" })
        row["graph_source"] = "repository"
        let task = try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
        #expect(task.awaitsStart)
        let catalog = String(decoding: try fixture("graph_catalog.json"), as: UTF8.self)
        let runs = OSAllocatedUnfairLock<[[String]]>(initialState: [])
        let model = WorkModel(query: RegistryQuery { args, _ in
            if args.starts(with: ["graph", "list"]) { return catalog }
            if args.starts(with: ["task", "run"]) { runs.withLock { $0.append(args) } }
            return "{}"
        }, repoPath: "/src/loopflow")

        // Before the catalog arrives the pane still says which workflow and whose choice.
        let waiting = try TaskStartOffer(model: model, task: task).inspect()
        _ = try waiting.find(text: "Not started")
        _ = try waiting.find(text: "It will run feature, loopflow's default.")

        await model.loadGraphCatalog()
        let offer = try TaskStartOffer(model: model, task: task).inspect()
        _ = try offer.find(text: "It will run feature, loopflow's default. You take part at demo.")
        _ = try offer.find(viewWithAccessibilityIdentifier: "task-start-with-issue-review")
        let start = try offer.find(viewWithAccessibilityIdentifier: "task-start-issue-review").button()
        #expect(!start.isDisabled())
        try start.tap()
        for _ in 0..<200 where runs.withLock({ $0.isEmpty }) { try await Task.sleep(for: .milliseconds(5)) }
        #expect(runs.withLock { $0 } == [["task", "run", task.task.identifier, "--json"]])
    }

    @Test("A failure beside a repository that stays open says what failed, shows the message and retries")
    func failureBesideRetainedRepositoryKeepsRetry() throws {
        let reason = "lf: the Machine did not answer"
        var retries = 0
        let workspace = RepositoryWorkspace(id: "loopflow", path: "/src/loopflow")
        let view = try RepositoryWorkspaceContent(workspace: workspace, error: reason, retry: { retries += 1 }) { _ in
            Text("retained")
        }.inspect()
        _ = try view.find(text: "retained")
        _ = try view.find(text: "The latest read of this repository failed.")
        _ = try view.find(text: reason)
        try view.find(button: "Retry").tap()
        #expect(retries == 1)
    }

    @Test("A dashboard tile with nothing to press keeps its title on one line")
    func compactStateWithoutActionsKeepsItsTitle() throws {
        let title = try EmptyState(title: "Work unavailable").environment(\.compactPanes, true)
            .inspect().find(text: "Work unavailable")
        #expect(try title.lineLimit() == 1)
        // With a button the slot may drop the title, so the button is what must be there.
        let pressed = try EmptyState(title: "Not started") { Button("Start") {} }.environment(\.compactPanes, true).inspect()
        _ = try pressed.find(button: "Start")
    }

    @Test("A workflow's owner reads the same in the sheet and on a filed Task")
    func workflowChoiceNamesItsOwner() {
        let model = WorkModel(query: RegistryQuery { _, _ in "{}" }, repoPath: "/src/loopflow")
        #expect(model.workflowChoice(source: "repository", wave: nil) == "loopflow's default")
        #expect(model.workflowChoice(source: "personal", wave: nil) == "your default")
        #expect(model.workflowChoice(source: "builtin", wave: nil) == "the default")
        #expect(model.workflowChoice(source: nil, wave: nil) == "for this Task only")
    }
}
#endif
