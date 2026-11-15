import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Shared Work selection")
struct WorkSelectionTests {
    @Test func typedFieldSupportKeepsSQLiteIndependentOfUnavailableActivity() throws {
        let data = try loadFixtureData("membership_fields.json")
        let facts = try JSONDecoder().decode([WorkMembershipPlan.Facts].self, from: data)
        #expect(facts[0].parent.support?.revisions == ["planning": 7])
        #expect(facts[0].parent.support?.observations == .sqlite)
        #expect(facts[0].activity.support?.observations == .unavailable)
        guard case .captured(let evidence) = facts[1].activity.support?.observations else {
            Issue.record("Empty validated capture must remain distinct from unavailable"); return
        }
        #expect(evidence.dependencies.isEmpty)
        #expect(try JSONDecoder().decode([WorkMembershipPlan.Facts].self, from: JSONEncoder().encode(facts)) == facts)
        for field in ["parent", "lifecycle", "activity"] {
            var raw = try #require((JSONSerialization.jsonObject(with: data) as? [[String: Any]])?.first)
            var value = try #require(raw[field] as? [String: Any])
            value.removeValue(forKey: "support"); raw[field] = value
            let selected = try JSONSerialization.data(withJSONObject: raw)
            #expect(throws: DecodingError.self) { try JSONDecoder().decode(WorkMembershipPlan.Facts.self, from: selected) }
        }
    }

    @Test func viewPlanRetainsOccurrencesAndMissingPrimaryWithoutExecution() async throws {
        let text = String(decoding: try loadFixtureData("view_plan.json"), as: UTF8.self)
        let query = RegistryQuery { args, cwd in
            guard args == ["desktop", "view", "show", "state:backlog", "--json"], cwd == "/repo" else {
                throw RegistryQueryError("Unexpected command while planning")
            }
            return text
        }
        let plan = try await query.viewPlan(expression: "state:backlog", repoPath: "/repo")
        #expect(plan.contents.first?.title == "Backlog")
        #expect(plan.contents.first?.primarySession == nil)
        #expect(plan.contents.first?.argv == nil)
        #expect(plan.resolution.members.count == 2)
        #expect(plan.projection.suppressed.first?.winner == plan.resolution.members.first)
        #expect(plan.arrangement.mode == .touched)
        #expect(plan.savedRevision == 3)
        let restored = try JSONDecoder().decode(WorkViewPlan.self, from: JSONEncoder().encode(plan))
        #expect(restored == plan)
    }

    @Test func everyStateRustSendsDecodes() throws {
        let states = try JSONDecoder().decode(
            [WorkSelectionState].self, from: loadFixtureData("selection_states.json"))
        #expect(states.contains(.working) && states.contains(.autonomous))
    }

    @Test func queryPlansArriveFromTheirSubscription() throws {
        let entries = try #require(JSONSerialization.jsonObject(with: loadFixtureData("subscribe_frames.json"))
            as? [[String: Any]])
        let entry = try #require(entries.first { $0["part"] as? String == "selections" })
        let frame = try WorkFrame.decode(
            part: .selections, scope: WorkScope(repo: "/repo", headless: false, task: nil, activity: nil, taskListWave: nil),
            sequence: 1, line: JSONSerialization.data(withJSONObject: try #require(entry["frame"])))
        guard case .selections(let plans?) = frame.content else {
            Issue.record("Missing Dashboard plans"); return
        }
        #expect(plans.first?.dashboardID == "picked")
        #expect(plans.first?.projection.suppressed.count == 1)
    }

    @Test func dashboardKeepsIdentitySeparateFromItsQuery() async throws {
        let bytes = try loadFixtureData("dashboard.json")
        let query = RegistryQuery { _, _ in String(decoding: bytes, as: UTF8.self) }
        let dashboard = try await query.editDashboardQuery(
            id: "picked", expression: "state:backlog", revision: 2, repoPath: "/repo"
        )
        #expect(dashboard.id == "picked")
        #expect(dashboard.id != dashboard.selection.id)
        #expect(dashboard.revision == 3)
        let encoded = try JSONEncoder().encode(dashboard)
        #expect(try JSONSerialization.jsonObject(with: encoded) as? NSDictionary == JSONSerialization.jsonObject(with: bytes) as? NSDictionary)
    }

    @Test func portablePaneRecordRetainsDraftAndZoomSeparatelyFromFocus() throws {
        let bytes = try loadFixtureData("pane_layout.json")
        let layout = try JSONDecoder().decode(WorkPaneLayout.self, from: bytes)
        #expect(layout.drafts.first?.text == "unsaved edits")
        #expect(layout.drafts.first?.baseContent == "base")
        #expect(layout.focused == "pane-0")
        #expect(layout.zoomed == "pane-2")
        #expect(layout.hidden == ["pane-1"])
        if case .split(_, _, _, let second) = layout.tree,
           case .split(_, _, _, let browser) = second,
           case .pane(_, .browser(_, let tabs, _)) = browser {
            #expect(tabs.contains(.task(id: "task")))
            #expect(tabs.contains(.changes(id: "changes-parent", comparison: .workingTree(base: "parent"))))
        } else { Issue.record("Expected retained browser tools") }
        let encoded = try JSONEncoder().encode(layout)
        #expect(try JSONSerialization.jsonObject(with: encoded) as? NSDictionary == JSONSerialization.jsonObject(with: bytes) as? NSDictionary)
    }

    @Test("A delivered Wave link selects that Wave in its repository window") @MainActor
    func deliveredWaveLinkSelectsWave() async throws {
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixture))
        let wave = try #require(roadmap.waves.first)
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Selection must not read") }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(roadmap), waves: .available([]), workActivity: .loading, repos: [])
        await model.receiveWorkLink(.wave(id: wave.wave.id, address: "loopflow://loopflow/wave/\(wave.wave.id)"))
        #expect(model.selection == .wave(id: wave.wave.id))
    }
}

func fixtureViewPlan(owner: WorkWorkspaceKey, members: [WorkViewPlan.Content],
                     facts: [WorkMembershipPlan.Facts]? = nil, slots: DashboardSlotLayout = DashboardSlotLayout(root: .slot(0))) -> WorkViewPlan {
    let refs = members.map { WorkMemberRef(source: [owner.id], work: $0.work) }
    return WorkViewPlan(name: nil, pinned: false, dashboardID: nil, repository: owner.repository, repositoryPath: "/src/fixture",
        selection: WorkSelection(id: owner.id, expression: owner.kind == .repository ? .repository : .wave(owner.id)), savedRevision: nil,
        resolution: WorkSelectionResolution(attention: [], members: refs, unavailable: []),
        membership: WorkMembershipPlan(domains: [], effects: [:], root: .members(refs), facts: facts ?? members.map { .init(work: $0.work, name: $0.identifier ?? $0.work.id, parent: .init(wave: nil, support: nil), lifecycle: .init(completedAt: nil, states: $0.states.filter { ![.available, .busy, .blocked].contains($0) }, known: true, support: nil), activity: .init(states: $0.states.filter { [.available, .busy, .blocked].contains($0) }, known: true, support: nil)) }, unavailable: []),
        arrangement: WorkArrangement(order: refs, hidden: [], mode: .auto),
        projection: WorkArrangementProjection(visible: refs, suppressed: []), slots: slots, contents: members)
}
extension WorkModel {
    /// The sidebar's started list as the server reads it for `wave`: each Task with its states, in order.
    func applyStartedFixture(_ tasks: [(id: String, states: [WorkSelectionState])], wave: String? = nil, repository: String = "repo") {
        let members = tasks.map {
            WorkViewPlan.Content(paneRevision: nil, face: nil, work: .init(repository: repository, kind: .task, id: $0.id), title: $0.id,
                states: [.open] + $0.states, identifier: nil, primarySession: nil, location: nil, argv: nil, unavailable: [])
        }
        applyTaskList(fixtureViewPlan(owner: .init(repository: repository, kind: .repository, id: repository), members: members),
                      wave: wave, reason: "")
    }

    func applyStartedFixture(working: [String], autonomous: [String] = [], wave: String? = nil, repository: String = "repo") {
        applyStartedFixture(working.map { ($0, [.working]) } + autonomous.map { ($0, [.autonomous]) }, wave: wave, repository: repository)
    }
}

func fixtureViewContent(work: WorkWorkspaceKey, title: String) -> WorkViewPlan.Content {
    WorkViewPlan.Content(paneRevision: nil, face: nil, work: work, title: title,
        states: [.open], identifier: nil, primarySession: nil, location: nil, argv: nil, unavailable: [])
}

/// A full captured manifest for presentation fixtures, independent of roadmap grouping.
func fixtureInventoryPlan(_ snapshot: RoadmapSnapshot) -> WorkViewPlan {
    let tasks = snapshot.tasks.items
    let contents = tasks.map { task in
        WorkViewPlan.Content(paneRevision: nil, face: nil, work: .init(repository: "repo", kind: .task, id: task.id),
            title: task.task.name, states: Array(task.selectionStates), identifier: task.task.identifier,
            primarySession: nil, location: nil, argv: nil, unavailable: [])
    }
    let facts = tasks.map { task in
        WorkMembershipPlan.Facts(work: .init(repository: "repo", kind: .task, id: task.id), name: task.task.identifier,
            parent: .init(wave: task.task.waveId, support: nil),
            lifecycle: .init(completedAt: nil, states: Array(task.selectionStates).filter { ![.available, .busy, .blocked].contains($0) }, known: true, support: nil),
            activity: .init(states: Array(task.selectionStates).filter { [.available, .busy, .blocked].contains($0) }, known: true, support: nil))
    } + snapshot.waves.map { wave in
        WorkMembershipPlan.Facts(work: .init(repository: "repo", kind: .wave, id: wave.wave.id), name: wave.wave.name,
            parent: .init(wave: wave.wave.parentWaveId, support: nil),
            lifecycle: .init(completedAt: nil, states: [], known: true, support: nil), activity: .init(states: [], known: true, support: nil))
    }
    let template = fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"), members: contents)
    return WorkViewPlan(name: nil, pinned: false, dashboardID: nil, repository: "repo", repositoryPath: "/src/fixture",
        selection: template.selection, savedRevision: nil, resolution: template.resolution,
        membership: WorkMembershipPlan(domains: [], effects: [:], root: .select(source: ["scope"], predicate: .default(wave: nil)),
            facts: facts, unavailable: []), arrangement: template.arrangement, projection: template.projection,
        slots: template.slots, contents: contents)
}
