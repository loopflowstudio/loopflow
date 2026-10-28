#if os(macOS)
import Foundation
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
        let created = try await query.createTask(request: "-v is ignored", wave: "product", graph: "auto", cwd: "/repo")
        #expect(created.task.identifier == "LOO-1")
        #expect(created.graph == "code")
        #expect(created.sessionId != nil)
        let shown = try await query.waveGraph(wave: nil, cwd: "/repo")
        #expect(shown.wave == nil)
        #expect(shown.default.name == "code")
        #expect(shown.default.source == "default")
        #expect(calls.withLock { $0 } == [
            ["task", "create", "--wave", "product", "--graph", "auto", "--run", "--json", "--", "-v is ignored"],
            ["wave", "graph", "show", "--no-wave", "--json"],
        ])
        // A run that failed after filing reports the Task with no graph or conversation.
        let failed = try RegistryQuery.decode([TaskCreateReceipt].self, from: fixture("task_create_receipt.json"))[1]
        #expect(failed.graph == nil && failed.sessionId == nil)
        // Every Wave and the repository decode, including a null Wave.
        _ = try RegistryQuery.decode([WaveGraph].self, from: fixture("wave_graph.json"))
    }

    @Test("The sheet follows the scope's reported workflow, names whose default it is, and waits for a request")
    func sheetDefaultsToTheReportedWorkflow() async throws {
        let catalog = try fixture("graph_catalog.json")
        let defaultGraph = #"{"wave_id": null, "wave": null, "default": {"name": "feature", "source": "repo"}}"#
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
        #expect(named == "· loopflow's default")
        _ = try sheet.find(viewWithAccessibilityIdentifier: "new-task-steps")
        #expect(try sheet.find(viewWithAccessibilityIdentifier: "new-task-create").button().isDisabled())
    }
}
#endif
