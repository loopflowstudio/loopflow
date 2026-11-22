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
    private func fixture(_ name: String) throws -> String {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        return try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/\(name)"), encoding: .utf8)
    }

    @Test("The sheet's fields map to one lf task create command and Rust's default read")
    func queriesPassFieldsThrough() async throws {
        let receipts = try #require(JSONSerialization.jsonObject(with: Data(fixture("task_create_receipt.json").utf8)) as? [Any])
        let receipt = String(decoding: try JSONSerialization.data(withJSONObject: receipts[0]), as: UTF8.self)
        let graphs = try #require(JSONSerialization.jsonObject(with: Data(fixture("wave_graph.json").utf8)) as? [Any])
        let repository = String(decoding: try JSONSerialization.data(withJSONObject: graphs[1]), as: UTF8.self)
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
        let batch = try RegistryQuery.decode([TaskCreated].self, from: fixture("task_create_receipt.json"))[1]
        #expect(batch.receipt.sessionId == nil && batch.session == nil)
        // Every Wave and the repository decode, including a null Wave.
        _ = try RegistryQuery.decode([WaveGraph].self, from: fixture("wave_graph.json"))
    }

    @Test("File creates the Task without running it and selects its row in that backlog")
    func fileLeavesTheTaskUnstartedAndSelected() async throws {
        let receipts = try #require(JSONSerialization.jsonObject(with: Data(fixture("task_create_receipt.json").utf8)) as? [Any])
        let receipt = String(decoding: try JSONSerialization.data(withJSONObject: receipts[1]), as: UTF8.self)
        let calls = OSAllocatedUnfairLock<[[String]]>(initialState: [])
        let model = WorkModel(query: RegistryQuery { args, _ in
            guard args.starts(with: ["task", "create"]) else { throw RegistryQueryError("Filing only creates") }
            calls.withLock { $0.append(args) }
            return receipt
        }, repoPath: "/repo")
        model.navigation.backlogs[.repository] = BacklogState(selection: ["other"], filter: .mine, query: "old")
        try await model.fileTask(request: "an idea", wave: nil, graph: nil)
        let submitted = try #require(calls.withLock { $0 }.first)
        #expect(!submitted.contains("--run") && submitted.suffix(2) == ["--", "an idea"])
        let created = try RegistryQuery.decode(TaskCreated.self, from: receipt)
        let state = try #require(model.navigation.backlogs[.repository])
        #expect(state.selection == [created.receipt.task.id] && state.filter == .all && state.query.isEmpty)
        #expect(state.notice == "Filed \(created.receipt.task.identifier)")
        #expect(model.navigation.startingTask == nil)
    }

    @Test("The sheet follows the scope's reported workflow, names whose default it is, and waits for a request")
    func sheetDefaultsToTheReportedWorkflow() async throws {
        let catalog = try fixture("graph_catalog.json")
        let defaultGraph = #"{"wave_id": null, "wave": null, "default": {"name": "feature", "source": "repository"}}"#
        let query = RegistryQuery { args, _ in
            if args.starts(with: ["graph", "list"]) { return catalog }
            if args.starts(with: ["wave", "graph", "show"]) { return defaultGraph }
            throw RegistryQueryError("The sheet only reads the catalog and the default")
        }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        await model.loadGraphCatalog()
        await model.loadGraphDefault(wave: nil)
        let sheet = try NewTaskSheet(model: model).inspect()
        _ = try sheet.find(text: "What do you want?")
        _ = try sheet.find(text: "feature")
        let named = try sheet.find(viewWithAccessibilityIdentifier: "new-task-default").text().string()
        #expect(named == ", loopflow's default")
        // The workflow as the Task shows it later, then Rust's sentence for where you take part.
        let steps = try sheet.find(viewWithAccessibilityIdentifier: "new-task-steps")
        _ = try steps.find(text: "pursue")
        _ = try steps.find(text: "You take part at demo.")
        let start = try sheet.find(viewWithAccessibilityIdentifier: "new-task-create").button()
        #expect(try start.labelView().text().string() == "Start ⌘↩")
        #expect(start.isDisabled())
        #expect(try sheet.find(viewWithAccessibilityIdentifier: "new-task-file").button().isDisabled())
    }

    @Test("Choosing another workflow changes the steps, the take-part sentence and whose choice it is")
    func choosingAWorkflowRedrawsItsLine() async throws {
        var entries = try #require(JSONSerialization.jsonObject(with: Data(fixture("graph_catalog.json").utf8)) as? [[String: Any]])
        var quick = entries[0]
        var workflow = try #require(quick["workflow"] as? [String: Any])
        var steps = try #require(workflow["steps"] as? [[String: Any]])
        steps[0]["name"] = "sketch"
        workflow["steps"] = steps
        workflow["sentence"] = "You take part at sketch."
        quick["name"] = "quick"
        quick["workflow"] = workflow
        entries.append(quick)
        let catalog = String(decoding: try JSONSerialization.data(withJSONObject: entries), as: UTF8.self)
        let defaultGraph = #"{"wave_id": null, "wave": null, "default": {"name": "feature", "source": "repository"}}"#
        let model = WorkModel(query: RegistryQuery { args, _ in
            args.starts(with: ["graph", "list"]) ? catalog : defaultGraph
        }, repoPath: "/src/loopflow")
        await model.loadGraphCatalog()
        await model.loadGraphDefault(wave: nil)
        var graph: String?
        var waveID: String?
        func sentence() throws -> InspectableView<ViewType.ClassifiedView> {
            try NewTaskWorkflow(model: model, waveID: Binding(get: { waveID }, set: { waveID = $0 }),
                                graph: Binding(get: { graph }, set: { graph = $0 })).inspect()
        }
        func item(_ name: String) throws -> InspectableView<ViewType.Toggle> {
            try sentence().find(ViewType.Toggle.self, where: { toggle in
                (try? toggle.find(textWhere: { text, _ in text.hasPrefix("\(name) ") })) != nil
            })
        }
        try item("quick").tap()
        #expect(graph == "quick")
        let other = try sentence()
        _ = try other.find(viewWithAccessibilityIdentifier: "new-task-steps").find(text: "sketch")
        _ = try other.find(text: "You take part at sketch.")
        #expect(try other.find(viewWithAccessibilityIdentifier: "new-task-default").text().string() == ", for this Task only")
        // Picking the scope's default again goes back to following it.
        try item("feature").tap()
        #expect(graph == nil)
        #expect(try sentence().find(viewWithAccessibilityIdentifier: "new-task-default").text().string() == ", loopflow's default")
    }
}
#endif
