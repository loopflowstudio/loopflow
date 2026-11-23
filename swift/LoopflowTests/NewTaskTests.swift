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
        let created = try await query.createTask(request: "-v is ignored", wave: .name("product"), graph: "auto", run: true, cwd: "/repo")
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

    private func model(catalog: OSAllocatedUnfairLock<String> = .init(initialState: "[]"),
                       planning: String? = nil, submissions: CreationSubmissions? = nil,
                       subscribe: (@Sendable () async throws -> WorkObservation)? = nil,
                       write: @escaping @Sendable (ServedWrite) async throws -> String) async throws -> WorkModel {
        let initial = String(decoding: try loadFixtureData("roadmap_snapshot.json"), as: UTF8.self)
        let model = WorkModel(query: RegistryQuery(write: write, submissions: submissions, subscribeWork: subscribe) { args, _ in
            if args.starts(with: ["plan", "list"]), !args.contains("--task") { return planning ?? initial }
            if args.starts(with: ["graph", "list"]) { return catalog.withLock { $0 } }
            if args.starts(with: ["wave", "graph", "show"]) {
                if args.contains("--no-wave") {
                    return #"{"wave_id":null,"wave":null,"default":{"name":"code","source":"repository"}}"#
                }
                guard args.contains("wave-1") else { throw RegistryQueryError("Unknown Wave") }
                return #"{"wave_id":"wave-1","wave":"product","default":{"name":"feature","source":"repository"}}"#
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
                #expect(request == "a brief" && wave == (explicit ? .existing(id: "wave-1") : nil))
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
        for await opened in Observations({ model.selection?.kind == .task }) where opened { break }
        #expect(model.navigation.palette == nil)
        let task = try #require(model.navigation.selectedTaskEvidence?.task)
        let bar = TaskWorkflowBar(model: model, task: task, work: .loading)
        _ = try bar.inspect().find(text: "Starting code")
    }

    @Test("A removed Wave needs an explicit destination before File or Start", arguments: [true, false])
    func removedWave(planningCaughtUp: Bool) async throws {
        let (row, filed, planning, _) = try filing()
        let model = try await model(catalog: .init(initialState: catalogWithCode())) { change in
            if case .taskCreate(_, _, _, _, .existing(let id), _, _, _) = change {
                throw ServerError(code: "not_found", message: "The selected Wave is no longer available in this repository.",
                                  data: .object(["wave_id": .string(id)]))
            }
            guard case .taskCreate(_, _, _, "keep my words", nil, nil, nil, false) = change else {
                throw RegistryQueryError("Unexpected File: \(change)")
            }
            return filed
        }
        let wave = try #require(model.visibleRoadmaps.first?.wave)
        model.navigation.sidebarWaveID = wave.id
        let sheet = NewTaskSheet(model: model)
        try sheet.inspect().find(ViewType.TextField.self).setInput("keep my words")
        try toggle("code", in: sheet).tap()
        var snapshot = try #require(JSONSerialization.jsonObject(with: Data(planning.utf8)) as? [String: Any])
        snapshot["waves"] = []
        let removed = try RegistryQuery.decode(RoadmapSnapshot.self, from: json(snapshot))
        if planningCaughtUp {
            model.applyFixture(roadmap: .available(removed), waves: .available([]), workActivity: .loading, repos: [])
        } else {
            try sheet.inspect().find(button: "File to backlog").tap()
            try await eventually { (try? sheet.inspect().find(text: "Not filed.")) != nil }
        }
        let view = try sheet.inspect()
        _ = try view.find(text: "Wave unavailable")
        #expect(try view.find(button: "File to backlog").isDisabled())
        #expect(try view.find(button: "Start with code ⌘↩").isDisabled())
        #expect(try toggle("code", in: sheet).isOn())
        try toggle("Use default", in: sheet).tap()
        try view.find(button: model.repositoryName).tap()
        #expect(try toggle("Use default", in: sheet).isOn())
        #expect(try sheet.inspect().find(ViewType.TextField.self).input() == "keep my words")
        try sheet.inspect().find(button: "File to backlog").tap()
        try await eventually { model.navigation.palette == nil }
        await CreationRecovery.shared.presented(CreationRecovery.shared.requests(for: .task(row.id)))
    }

    @Test("A refused named Start can be edited and started with the default")
    func correctRefusedStart() async throws {
        let admitted = try json(fixtureArray("task_create_receipt.json")[0])
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let attempts = OSAllocatedUnfairLock<[ServedWrite]>(initialState: [])
        let journal = CreationSubmissions(directory: directory, machine: { "original" }, lookup: { _ in ReadOutput("null") },
            send: { data in
                let payload = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
                if payload["graph"] as? String == "code" {
                    throw ServerError(code: "invalid", message: "Workflow unavailable", data: .object(["graph": .string("code")]))
                }
                return admitted
            })
        let model = try await model(catalog: .init(initialState: catalogWithCode())) { change in
            guard case .taskCreate(_, let id, _, _, _, _, _, _) = change else { throw RegistryQueryError("Expected Start") }
            attempts.withLock { $0.append(change) }
            return try await journal.submit(change, requestID: id)
        }
        let wave = try #require(model.visibleRoadmaps.first?.wave)
        let sheet = NewTaskSheet(model: model)
        try sheet.inspect().find(button: wave.displayName).tap()
        try sheet.inspect().find(ViewType.TextField.self).setInput("draft")
        try toggle("code", in: sheet).tap()
        try sheet.inspect().find(button: "Start with code ⌘↩").tap()
        try await eventually { (try? sheet.inspect().find(text: "Not started.")) != nil }
        let refused = try sheet.inspect()
        #expect(model.navigation.palette == .newTask)
        _ = try refused.find(text: "Workflow unavailable")
        #expect(try refused.find(ViewType.TextField.self).input() == "draft")
        #expect(try toggle("code", in: sheet).isOn())
        #expect(try await journal.pending().isEmpty)

        try refused.find(ViewType.TextField.self).setInput("edited draft")
        try toggle("Use default", in: sheet).tap()
        try sheet.inspect().find(button: "Start with feature ⌘↩").tap()
        try await eventually { model.selection?.kind == .task }
        #expect(model.navigation.palette == nil)
        let writes = attempts.withLock { $0 }
        try #require(writes.count == 2)
        guard case .taskCreate(_, let first, nil, "draft", let originalWave, "code", nil, true) = writes[0],
              case .taskCreate(_, let second, nil, "edited draft", let finalWave, nil, nil, true) = writes[1] else {
            Issue.record("Unexpected Start submissions: \(writes)")
            return
        }
        #expect(originalWave == .existing(id: wave.id) && finalWave == originalWave)
        #expect(first != second)
        #expect(try await journal.pending().map(\.id) == [second])
        await CreationRecovery.shared.presented([second])
    }

    @Test("A lost Start reply closes the sheet and keeps the original submission")
    func lostStartReply() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let server = CreationTestMachine()
        let journal = CreationSubmissions(directory: directory, machine: { "original" },
            lookup: { await server.lookup($0) }, send: { payload in
                _ = try await server.send(payload)
                throw RegistryQueryError("Connection lost after admission")
            })
        let model = try await model { change in
            guard case .taskCreate(_, let id, _, _, _, _, _, _) = change else { throw RegistryQueryError("Expected Start") }
            return try await journal.submit(change, requestID: id)
        }
        let sheet = NewTaskSheet(model: model)
        try sheet.inspect().find(ViewType.TextField.self).setInput("start once")
        try sheet.inspect().find(button: "Start with code ⌘↩").tap()
        try await eventually { model.navigation.palette == nil }
        let saved = try await journal.pending()
        let record = try #require(saved.first)
        #expect(saved.count == 1 && record.destination == "original")
        let recovered = try #require(try await journal.reconcile(record))
        #expect(recovered.requestId == record.id && recovered.taskId == "original")
        #expect(await server.submissions == [record.payload])
        let payload = try #require(JSONSerialization.jsonObject(with: record.payload) as? [String: Any])
        #expect(payload["request_id"] as? String == record.id)
        #expect(payload["request"] as? String == "start once")
    }

    @Test("A lost creation refused on replay reopens as an editable draft across restart",
          arguments: [NewTaskAction.start(graph: "code"), .file])
    func editDraftAfterLostReply(action: NewTaskAction) async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let sends = OSAllocatedUnfairLock<[Data]>(initialState: [])
        let refusal = ServerError(code: "not_found", message: "The selected Wave disappeared.",
                                  data: .object(["wave_id": .string("wave-1")]))
        let admitted = try json(fixtureArray("task_create_receipt.json")[0])
        func journal() -> CreationSubmissions {
            CreationSubmissions(directory: directory, machine: { "original" }, lookup: { _ in ReadOutput("null") },
                send: { payload in
                    let attempt = sends.withLock { $0.append(payload); return $0.count }
                    switch attempt {
                    case 1: throw RegistryQueryError("Connection lost before admission")
                    case 2: throw refusal
                    default: return admitted
                    }
                })
        }
        let first = journal()
        let catalog = OSAllocatedUnfairLock(initialState: try catalogWithCode())
        func window(_ submissions: CreationSubmissions) async throws -> WorkModel {
            try await self.model(catalog: catalog, submissions: submissions) { _ in
                throw RegistryQueryError("Unexpected write")
            }
        }
        let initial = try await window(first)
        let sheet = NewTaskSheet(model: initial)
        try sheet.inspect().find(button: #require(initial.visibleRoadmaps.first?.wave.displayName)).tap()
        try sheet.inspect().find(ViewType.TextField.self).setInput("keep these words")
        try toggle("code", in: sheet).tap()
        try sheet.inspect().find(button: action == .file ? "File to backlog" : "Start with code ⌘↩").tap()
        try await eventually { initial.navigation.palette == nil }
        let original = try #require(await first.pending().first)
        let recovery = CreationRecovery(submissions: first)
        await recovery.refresh()
        #expect(recovery.items.first?.draft?.id == original.id)
        #expect(sends.withLock { $0 } == [original.payload, original.payload])
        if action == .file {
            let command = try #require(JSONSerialization.jsonObject(with: original.payload) as? [String: Any])
            #expect(command["graph"] is NSNull)
            #expect(command["run"] as? Bool == false)
        }

        let cold = journal()
        let restarted = CreationRecovery(submissions: cold)
        let model = try await window(cold)
        model.navigation.palette = nil
        let notices = model.navigation.notices
        if notices.event != nil { notices.dismiss() }
        restarted.report(to: notices, editDraft: model.editDraft)
        await restarted.refresh()
        #expect(model.navigation.palette == nil)
        #expect(notices.current?.action?.title == "Edit draft")
        #expect(notices.current?.text.contains(refusal.message) == true)
        #expect(try await cold.reconcile(original) == nil)
        notices.current?.action?.perform()
        guard case .editDraft(let draft) = model.navigation.palette else {
            Issue.record("Edit draft did not open the refused creation")
            return
        }
        let editing = NewTaskSheet(model: model, recovered: draft, recovery: restarted)
        let view = try editing.inspect()
        #expect(try view.find(ViewType.TextField.self).input() == "keep these words")
        #expect(try toggle("code", in: editing).isOn())
        _ = try view.find(text: "Wave unavailable")
        _ = try view.find(text: refusal.message)
        #expect(try view.find(button: "File to backlog").isDisabled())
        #expect(try view.find(button: "Start with code ⌘↩").isDisabled())
        try view.find(button: model.repositoryName).tap()
        try toggle("Use default", in: editing).tap()
        try view.find(ViewType.TextField.self).setInput("corrected words")
        await restarted.refresh()
        #expect(sends.withLock { $0.count } == 2)
        #expect(try await cold.pending().first?.refusedDraft == draft)
        if action == .file {
            try editing.inspect().find(button: "Discard draft").tap()
            try await eventually { restarted.items.isEmpty }
            #expect(try await cold.pending().isEmpty)
            #expect(sends.withLock { $0.count } == 2)
            return
        }
        try editing.inspect().find(button: "Start with code ⌘↩").tap()
        try await eventually { model.selection?.kind == .task }
        #expect(model.navigation.palette == nil)
        let records = try await cold.pending()
        let replacement = try #require(records.first)
        #expect(records.count == 1 && replacement.id != original.id && replacement.refusedDraft == nil)
        let sent = sends.withLock { $0 }
        #expect(sent == [original.payload, original.payload, replacement.payload])
        let command = try #require(JSONSerialization.jsonObject(with: replacement.payload) as? [String: Any])
        #expect(command["request"] as? String == "corrected words")
        #expect(command["run"] as? Bool == true && command["wave"] is NSNull && command["graph"] is NSNull)
        #expect(command["request_id"] as? String == replacement.id)
        try await cold.discardDraft(original.id)
        #expect(try await cold.pending().map(\.id) == [replacement.id])
        try await cold.presented(replacement.id)
        #expect(try await cold.pending().isEmpty)
        await CreationRecovery.shared.presented([replacement.id])
    }

    enum TaskChange: CaseIterable { case moved, started }

    private func filing(change: TaskChange? = nil) throws -> (row: RoadmapTask, receipt: String, before: String, after: String) {
        var snapshot = try #require(JSONSerialization.jsonObject(with: loadFixtureData("roadmap_snapshot.json")) as? [String: Any])
        var tasks = try #require(snapshot["tasks"] as? [String: Any])
        var rows = try #require(tasks["items"] as? [[String: Any]])
        let index = try #require(rows.firstIndex { $0["lifecycle"] as? String == "unstarted" })
        var filed = rows.remove(at: index)
        let row = try RegistryQuery.decode(RoadmapTask.self, from: json(filed))
        let receipt = try json([
            "task": ["id": row.id, "identifier": row.task.identifier, "title": row.task.name],
            "graph": NSNull(), "session_id": NSNull(), "milestone": "filed", "session": NSNull(), "row": filed,
        ])
        tasks["items"] = rows
        snapshot["tasks"] = tasks
        let before = try json(snapshot)

        var task = try #require(filed["task"] as? [String: Any])
        task["description"] = "Edited after filing"
        switch change {
        case .moved: task["wave_id"] = NSNull()
        case .started:
            filed["section"] = "working"
            filed["lifecycle"] = "active"
        case nil: break
        }
        filed["task"] = task
        rows.insert(filed, at: index)
        tasks["items"] = rows
        snapshot["tasks"] = tasks
        return (row, receipt, before, try json(snapshot))
    }

    @Test("File opens the committed brief before planning catches up")
    func fileRefusalAndRetry() async throws {
        let (row, filed, beforeFile, planned) = try filing()
        let feed = WorkFeed()
        let catalog = OSAllocatedUnfairLock(initialState: try catalogWithCode())
        let attempts = OSAllocatedUnfairLock<[ServedWrite]>(initialState: [])
        let refusal = AsyncStream<Void>.makeStream()
        let model = try await model(catalog: catalog, planning: beforeFile, subscribe: { await feed.open() }) { change in
            if case .waveSync = change { return "{}" }
            let first = attempts.withLock { $0.append(change); return $0.count == 1 }
            if first {
                for await _ in refusal.stream {}
                throw ServerError(code: "unavailable", message: "Wave unavailable", data: .null)
            }
            return filed
        }
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
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
        try sheet.inspect().find(button: "File to backlog").tap()
        #expect(try sheet.inspect().find(button: "Filing…").isDisabled())
        refusal.continuation.finish()
        try await eventually { (try? sheet.inspect().find(text: "Not filed.")) != nil }
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
            #expect(wave == .existing(id: product.id) && request == row.task.name)
        }
        let state = try #require(model.navigation.backlogs[destination])
        #expect(state.selection == [row.id] && state.filter == .all && state.query.isEmpty)
        #expect(state.notice == .did("Filed \(row.task.identifier)"))
        let brief = try BacklogView(model: model, view: destination).inspect().find(BacklogBrief.self).actualView()
        #expect(brief.task.id == row.id)
        #expect(!model.visibleTasks.contains { $0.id == row.id })
        let resource = CreationRecovery.Presentation.task(row.id)
        #expect(!CreationRecovery.shared.requests(for: resource).isEmpty)
        let pane = ImageRenderer(content: brief)
        defer { withExtendedLifetime(pane) {} }
        _ = try #require(pane.cgImage)
        for await retired in Observations({ CreationRecovery.shared.requests(for: resource).isEmpty }) where retired { break }
        let scope = model.workScope
        func deliver(_ snapshot: String, sequence: Int) async throws {
            let line = #"{"answers":null,"unavailable":null,"body":\#(snapshot),"read_at":null}"#
            try await feed.send(WorkFrame.decode(part: .planning, scope: scope, sequence: sequence, line: Data(line.utf8)))
            for await received in Observations({ model.planningSequence }) where received == sequence { break }
        }
        try await deliver(planned, sequence: 1)
        let caughtUp = model.backlog(destination)
        #expect(caughtUp.ids.filter { $0 == row.id }.count == 1)
        #expect(caughtUp.opened?.task.description == "Edited after filing")
        try await deliver(beforeFile, sequence: 2)
        #expect(!model.backlog(destination).ids.contains(row.id))
        try await deliver(planned, sequence: 3)
        model.openBacklog(try #require(model.backlog(destination).opened))
        #expect(model.selection == .task(id: row.id))
        let offer = try TaskStartOffer(model: model, task: row).inspect()
        _ = try offer.find(textWhere: { text, _ in text.contains("It will run feature") && text.contains("You take part at demo.") })
    }

    @Test("A File reply follows the newer Task's backlog membership", arguments: TaskChange.allCases)
    func planningBeforeFileReply(change: TaskChange) async throws {
        let (row, receipt, before, after) = try filing(change: change)
        let current = try RegistryQuery.decode(RoadmapSnapshot.self, from: after)
        let sent = AsyncStream<Void>.makeStream()
        let reply = AsyncStream<Void>.makeStream()
        let model = try await model(planning: before) { _ in
            sent.continuation.finish()
            for await _ in reply.stream {}
            return receipt
        }
        let wave = try #require(model.visibleRoadmaps.first?.wave)
        let filing = Task { try await model.submitTask(.file, request: row.task.name, wave: wave) }
        for await _ in sent.stream {}
        model.applyFixture(roadmap: .available(current), waves: .available([]), workActivity: .loading, repos: [])
        reply.continuation.finish()
        try await filing.value
        #expect(!model.backlog(.wave(wave.id)).ids.contains(row.id))
        let backlog = model.backlog(.repository)
        switch change {
        case .moved:
            #expect(model.navigation.waveView == .repository)
            #expect(backlog.opened?.task.description == "Edited after filing")
        case .started:
            #expect(!backlog.ids.contains(row.id))
        }
        await CreationRecovery.shared.presented(CreationRecovery.shared.requests(for: .task(row.id)))
    }

    @Test("A complete Task lookup can remove the filed brief while planning is paused")
    func exactLookupRemovesFiledBrief() async throws {
        let (row, receipt, before, _) = try filing()
        let empty = #"{"generated_at":"now","waves":[],"tasks":{"state":"ok","items":[],"truncated":false}}"#
        let partial = OSAllocatedUnfairLock(initialState: true)
        let model = WorkModel(query: RegistryQuery(write: { _ in receipt }) { args, _ in
            if args.contains("--task") {
                return partial.withLock { $0 } ? empty.replacingOccurrences(of: "false", with: "true") : empty
            }
            return before
        }, repoPath: row.task.repo)
        await model.refreshPlanning()
        try await model.submitTask(.file, request: row.task.name, wave: nil)
        let wave = WorkNavigation.Board.wave(try #require(row.task.waveId))
        #expect(model.navigation.waveView == wave)
        #expect(model.backlog(wave).opened?.id == row.id)
        await model.refreshFiledRow(in: wave)
        #expect(model.backlog(wave).opened?.id == row.id)
        partial.withLock { $0 = false }
        let pane = ImageRenderer(content: BacklogView(model: model, view: wave).frame(width: 900, height: 600))
        defer { withExtendedLifetime(pane) {} }
        _ = try #require(pane.cgImage)
        try await eventually { !model.backlog(wave).ids.contains(row.id) }
        await CreationRecovery.shared.presented(CreationRecovery.shared.requests(for: .task(row.id)))
    }

    enum Departure: CaseIterable { case selection, repository, machine }

    @Test("Leaving the filed selection releases its receipt row", arguments: Departure.allCases)
    func leaveFiledBrief(departure: Departure) async throws {
        let (row, receipt, before, _) = try filing()
        let model = try await model(planning: before) { _ in receipt }
        model.confirmMachine("original")
        try await model.submitTask(.file, request: row.task.name, wave: nil)
        let view = try #require(model.navigation.waveView)
        #expect(model.backlog(view).opened?.id == row.id)
        switch departure {
        case .selection:
            model.navigation.backlogs[view]?.select(row.id)
            model.navigation.backlogs[view]?.select(row.id)
        case .repository:
            model.setRepoPath("/another")
            model.setRepoPath(row.task.repo)
        case .machine:
            model.confirmMachine("another")
        }
        #expect(!model.backlog(view).ids.contains(row.id))
        await CreationRecovery.shared.presented(CreationRecovery.shared.requests(for: .task(row.id)))
    }

    @Test("Leaving during File keeps the new destination and the saved submission")
    func leaveBeforeFileReply() async throws {
        let (row, receipt, before, _) = try filing()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let sent = AsyncStream<Void>.makeStream()
        let reply = AsyncStream<Void>.makeStream()
        let journal = CreationSubmissions(directory: directory, machine: { "original" },
            lookup: { _ in ReadOutput("null") }, send: { _ in
                sent.continuation.finish()
                for await _ in reply.stream {}
                return receipt
            })
        let model = try await model(planning: before) { change in
            guard case .taskCreate(_, let id, _, _, _, _, _, _) = change else { throw RegistryQueryError("Expected File") }
            return try await journal.submit(change, requestID: id)
        }
        let filing = Task { try await model.submitTask(.file, request: row.task.name, wave: nil) }
        for await _ in sent.stream {}
        model.selectDashboard("another")
        model.navigation.palette = .newTask
        reply.continuation.finish()
        try await filing.value
        #expect(model.navigation.dashboardID == "another")
        #expect(model.navigation.palette == .newTask)
        let saved = try await journal.pending()
        #expect(saved.count == 1 && saved.first?.destination == "original")
    }
}
#endif
