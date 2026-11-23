#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
import os
@testable import Loopflow
@testable import LoopflowMac

@Suite("New Task", .serialized)
@MainActor
struct NewTaskTests {
    private func fixtureArray(_ name: String) throws -> [[String: Any]] {
        try #require(JSONSerialization.jsonObject(with: loadFixtureData(name)) as? [[String: Any]])
    }

    private func json(_ value: Any) throws -> String {
        String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
    }

    @Test("The sheet's fields map to one lf task create command and Rust's default read")
    func queriesPassFieldsThrough() async throws {
        let receipts = try fixtureArray("task_create_receipt.json")
        let receipt = try json(receipts[0])
        let repository = try json(fixtureArray("wave_graph.json")[1])
        let calls = OSAllocatedUnfairLock<[[String]]>(initialState: [])
        let query = RegistryQuery { args, _ in
            calls.withLock { $0.append(args) }
            return args.starts(with: ["task"]) ? receipt : repository
        }
        let created = try await query.createTask(request: "-v is ignored", wave: "product", graph: "auto", run: true, cwd: "/repo")
        #expect(created.receipt.task.identifier == "LOO-1")
        #expect(created.receipt.graph == "code")
        #expect(created.session?.id == created.receipt.sessionId)
        #expect(created.session?.openArgv.isEmpty == false)
        // Until the Task's reading arrives, its row says where it stands.
        #expect(created.row.setupPosition == "Starting code")
        let shown = try await query.waveGraph(wave: nil, cwd: "/repo")
        #expect(shown.wave == nil)
        #expect(shown.default.name == "code")
        #expect(shown.default.source == "builtin")
        let submitted = calls.withLock { $0 }
        #expect(UUID(uuidString: submitted[0][3]) != nil)
        #expect([Array(submitted[0].prefix(2)) + Array(submitted[0].dropFirst(4)), submitted[1]] == [
            ["task", "create", "--wave", "product", "--graph", "auto", "--run", "--json", "--", "-v is ignored"],
            ["wave", "graph", "show", "--no-wave", "--json"],
        ])
        // A headless `-b` run answers with no conversation.
        let batch = try RegistryQuery.decode(TaskCreated.self, from: json(receipts[1]))
        #expect(batch.receipt.sessionId == nil && batch.session == nil)
        // Every Wave and the repository decode, including a null Wave.
        _ = try JSONDecoder().decode([WaveGraph].self, from: loadFixtureData("wave_graph.json"))
    }

    private func toggle(_ name: String, in sheet: NewTaskSheet) throws -> InspectableView<ViewType.Toggle> {
        try sheet.inspect().find(ViewType.Toggle.self, where: { toggle in
            (try? toggle.find(textWhere: { text, _ in text == name || text.hasPrefix("\(name), ") })) != nil
        })
    }

    private func catalogWithCode() throws -> String {
        var code = try #require(JSONSerialization.jsonObject(with: loadFixtureData("graph_composition.json")) as? [String: Any])
        code["name"] = "code"
        return try json(fixtureArray("graph_catalog.json") + [code])
    }

    private func model(catalog: OSAllocatedUnfairLock<String>,
                       write: @escaping @Sendable (ServedWrite) async throws -> String) async throws -> WorkModel {
        let planning = String(decoding: try loadFixtureData("roadmap_snapshot.json"), as: UTF8.self)
        let model = WorkModel(query: RegistryQuery(write: write) { args, _ in
            if args.starts(with: ["plan", "list"]) { return planning }
            if args.starts(with: ["graph", "list"]) { return catalog.withLock { $0 } }
            if args.starts(with: ["wave", "graph", "show"]) {
                return args.contains("--no-wave")
                    ? #"{"wave_id":null,"wave":null,"default":{"name":"code","source":"repository"}}"#
                    : #"{"wave_id":"wave-1","wave":"product","default":{"name":"feature","source":"repository"}}"#
            }
            throw RegistryQueryError("No fixture for \(args)")
        }, repoPath: "/src/loopflow")
        await model.refreshPlanning()
        let product = try #require(model.visibleRoadmaps.first?.wave)
        await model.loadGraphCatalog()
        await model.loadGraphDefault(wave: nil)
        await model.loadGraphDefault(wave: product)
        model.navigation.palette = .newTask
        return model
    }

    @Test("Start follows the scope or pins a named workflow, even when the names match",
          arguments: [true, false])
    func startChoice(explicit: Bool) async throws {
        let admitted = try json(fixtureArray("task_create_receipt.json")[0])
        let model = try await model(catalog: .init(initialState: catalogWithCode())) { change in
            if case .taskCreate(_, _, _, let request, let wave, let graph, nil, true) = change {
                #expect(request == "a brief" && wave == (explicit ? "product" : nil))
                #expect(graph == (explicit ? "code" : nil))
            } else { Issue.record("Start sent \(change)") }
            return admitted
        }
        let product = try #require(model.visibleRoadmaps.first?.wave)
        model.navigation.sidebarWaveID = product.id
        let sheet = NewTaskSheet(model: model)
        let view = try sheet.inspect()
        _ = try view.find(text: "Workflow for Start")
        _ = try view.find(text: "Choose a workflow when you start.")
        #expect(try view.find(button: "Start with feature ⌘↩").isDisabled())
        #expect(try view.find(button: "File to backlog").isDisabled())
        _ = try view.find(text: "You take part at demo.")
        try view.find(ViewType.TextField.self).setInput("a brief")
        try toggle("feature", in: sheet).tap()
        _ = try sheet.inspect().find(text: "chosen for this start")
        try toggle("code", in: sheet).tap()
        _ = try sheet.inspect().find(text: "Runs on its own.")
        try sheet.inspect().find(button: model.repositoryName).tap()
        _ = try sheet.inspect().find(text: "chosen for this start")
        #expect(try toggle("code", in: sheet).isOn())
        try toggle("Use default", in: sheet).tap()
        _ = try sheet.inspect().find(text: "loopflow's default")
        #expect(try toggle("Use default", in: sheet).isOn())
        try sheet.inspect().find(button: product.displayName).tap()
        _ = try sheet.inspect().find(button: "Start with feature ⌘↩")
        if explicit {
            try toggle("code", in: sheet).tap()
        } else {
            try sheet.inspect().find(button: model.repositoryName).tap()
        }
        try sheet.inspect().find(button: "Start with code ⌘↩").tap()
        #expect(model.navigation.palette == nil)
        for await opened in Observations({ model.selection?.kind == .task }) where opened { break }
        let task = try #require(model.navigation.selectedTaskEvidence?.task)
        let bar = TaskWorkflowBar(model: model, task: task, work: .loading)
        _ = try bar.inspect().find(text: "Starting code")
    }

    @Test("File keeps a refused draft and selects its unstarted row even after its Start workflow disappears")
    func fileRefusalAndRetry() async throws {
        let snapshot = try #require(JSONSerialization.jsonObject(with: loadFixtureData("roadmap_snapshot.json")) as? [String: Any])
        let tasks = try #require(snapshot["tasks"] as? [String: Any])
        let rows = try #require(tasks["items"] as? [[String: Any]])
        let filedRow = try #require(rows.first { $0["lifecycle"] as? String == "unstarted" })
        let row = try RegistryQuery.decode(RoadmapTask.self, from: json(filedRow))
        let filed = try json([
            "task": ["id": row.id, "identifier": row.task.identifier, "title": row.task.name],
            "graph": NSNull(), "session_id": NSNull(), "milestone": "filed", "session": NSNull(), "row": filedRow,
        ])
        let catalog = OSAllocatedUnfairLock(initialState: try catalogWithCode())
        let attempts = OSAllocatedUnfairLock<[ServedWrite]>(initialState: [])
        let model = try await model(catalog: catalog) { change in
            let first = attempts.withLock { $0.append(change); return $0.count == 1 }
            if first { throw ServerError(code: "unavailable", message: "Wave unavailable", data: .null) }
            return filed
        }
        let product = try #require(model.visibleRoadmaps.first?.wave)
        let sheet = NewTaskSheet(model: model)
        try sheet.inspect().find(button: product.displayName).tap()
        try sheet.inspect().find(ViewType.TextField.self).setInput(row.task.name)
        try toggle("code", in: sheet).tap()
        let remaining = String(decoding: try loadFixtureData("graph_catalog.json"), as: UTF8.self)
        catalog.withLock { $0 = remaining }
        await model.loadGraphCatalog(force: true)
        let destination = WorkNavigation.Board.wave(product.id)
        model.setHidden(.backlog, true, in: destination)
        model.setZoomed(.tiles, in: destination)
        model.navigation.backlogs[destination] = BacklogState(selection: ["other"], filter: .mine, query: "old")
        await sheet.file()
        let refused = try sheet.inspect()
        #expect(model.navigation.palette == .newTask)
        _ = try refused.find(text: "Not filed.")
        _ = try refused.find(text: "Wave unavailable")
        #expect(try refused.find(ViewType.TextField.self).input() == row.task.name)
        _ = try refused.find(button: "Start with code ⌘↩")
        try refused.find(button: "File to backlog").tap()
        for await closed in Observations({ model.navigation.palette == nil }) where closed { break }
        #expect(model.navigation.waveView == destination)
        #expect(!model.isHidden(.backlog, in: destination))
        #expect(model.zoomedPart(in: destination) == nil)
        let submissions = attempts.withLock { $0 }
        #expect(submissions.count == 2)
        for change in submissions {
            guard case .taskCreate(_, _, _, let request, let wave, nil, nil, false) = change else {
                Issue.record("File sent \(change)")
                return
            }
            #expect(wave == product.name && request == row.task.name)
        }
        let state = try #require(model.navigation.backlogs[destination])
        #expect(state.selection == [row.id] && state.filter == .all && state.query.isEmpty)
        #expect(state.notice == .did("Filed \(row.task.identifier)"))
        let brief = try BacklogView(model: model, view: destination).inspect().find(BacklogBrief.self).actualView()
        #expect(brief.task.id == row.id)
        let resource = CreationRecovery.Presentation.task(row.id)
        #expect(!CreationRecovery.shared.requests(for: resource).isEmpty)
        let pane = ImageRenderer(content: brief)
        defer { withExtendedLifetime(pane) {} }
        _ = try #require(pane.cgImage)
        for await retired in Observations({ CreationRecovery.shared.requests(for: resource).isEmpty }) where retired { break }
        model.openBacklog(brief.task)
        #expect(model.selection == .task(id: row.id))
        let offer = try TaskStartOffer(model: model, task: row).inspect()
        _ = try offer.find(textWhere: { text, _ in text.contains("It will run feature") && text.contains("You take part at demo.") })
    }
}
#endif
