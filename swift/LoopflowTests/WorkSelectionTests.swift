import Foundation
import Testing
@testable import Loopflow

@Suite("Shared Work selection")
struct WorkSelectionTests {
    private func fixture(_ name: String) throws -> Data {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        return try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/\(name).json"))
    }

    @Test func viewPlanRetainsOccurrencesAndMissingPrimaryWithoutExecution() async throws {
        let text = String(decoding: try fixture("view_plan"), as: UTF8.self)
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

    @Test func queryPlansArriveWithTheSharedPlanningFrame() throws {
        let frames = try JSONDecoder().decode([WorkFrame].self, from: fixture("work_frame"))
        guard case .planning(let planning?) = frames[0].content else {
            Issue.record("Missing planning frame"); return
        }
        #expect(planning.selections?.first?.dashboardID == "picked")
        #expect(planning.selections?.first?.projection.suppressed.count == 1)
    }

    @Test func dashboardKeepsIdentitySeparateFromItsQuery() async throws {
        let bytes = try fixture("dashboard")
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
        let bytes = try fixture("pane_layout")
        let layout = try JSONDecoder().decode(WorkPaneLayout.self, from: bytes)
        #expect(layout.drafts.first?.text == "unsaved edits")
        #expect(layout.drafts.first?.baseContent == "base")
        #expect(layout.focused == "pane-1")
        #expect(layout.zoomed == "pane-2")
        #expect(layout.hidden == ["pane-4"])
        let encoded = try JSONEncoder().encode(layout)
        #expect(try JSONSerialization.jsonObject(with: encoded) as? NSDictionary == JSONSerialization.jsonObject(with: bytes) as? NSDictionary)
    }
}

func fixtureViewPlan(owner: WorkWorkspaceKey, members: [WorkViewPlan.Content]) -> WorkViewPlan {
    let refs = members.map { WorkMemberRef(source: [owner.id], work: $0.work) }
    return WorkViewPlan(name: nil, pinned: false, dashboardID: nil, repository: owner.repository,
        observedAt: 0, selection: WorkSelection(id: owner.id, expression: owner.kind == .repository ? .repository : .wave(owner.id)), savedRevision: nil,
        resolution: WorkSelectionResolution(attention: [], members: refs, unavailable: []),
        arrangement: WorkArrangement(tree: nil, hidden: [], mode: .auto),
        projection: WorkArrangementProjection(visible: nil, suppressed: []), contents: members)
}
func fixtureViewContent(work: WorkWorkspaceKey, title: String) -> WorkViewPlan.Content {
    WorkViewPlan.Content(paneRevision: nil, face: nil, work: work, title: title,
        states: [.open], primarySession: nil, location: nil, argv: nil, unavailable: [])
}
