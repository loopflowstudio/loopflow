import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Shared Work selection")
struct WorkSelectionTests {
    @Test func viewPlanRetainsOccurrencesAndMissingPrimaryWithoutExecution() async throws {
        let text = String(decoding: try loadFixtureData("view_plan.json"), as: UTF8.self)
        let query = RegistryQuery { args, cwd in
            guard args == ["desktop", "view", "state:backlog", "--plan", "--json"], cwd == "/repo" else {
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

    @Test func queryPlansArriveFromTheirWatch() throws {
        let entries = try #require(JSONSerialization.jsonObject(with: loadFixtureData("watch_frames.json"))
            as? [[String: Any]])
        let entry = try #require(entries.first { $0["part"] as? String == "selections" })
        let frame = try WorkFrame.decode(
            part: .selections, scope: WorkScope(repo: "/repo", headless: false, task: nil, activity: nil),
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

func fixtureViewPlan(owner: WorkWorkspaceKey, members: [WorkViewPlan.Content]) -> WorkViewPlan {
    let refs = members.map { WorkMemberRef(source: [owner.id], work: $0.work) }
    return WorkViewPlan(name: nil, pinned: false, dashboardID: nil, repository: owner.repository,
        selection: WorkSelection(id: owner.id, expression: owner.kind == .repository ? .repository : .wave(owner.id)), savedRevision: nil,
        resolution: WorkSelectionResolution(attention: [], members: refs, unavailable: []),
        arrangement: WorkArrangement(order: refs, hidden: [], mode: .auto),
        projection: WorkArrangementProjection(visible: refs, suppressed: []), slots: DashboardSlotLayout(root: .slot(0)), contents: members)
}
func fixtureViewContent(work: WorkWorkspaceKey, title: String) -> WorkViewPlan.Content {
    WorkViewPlan.Content(paneRevision: nil, face: nil, work: work, title: title,
        states: [.open], primarySession: nil, location: nil, argv: nil, unavailable: [])
}
