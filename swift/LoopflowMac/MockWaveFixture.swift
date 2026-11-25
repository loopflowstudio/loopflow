#if os(macOS)
import Foundation
import Loopflow

/// Fixture data for the `mock-waves` UI-test mode (`AppTestMode.mockWaves`).
enum MockWaveFixture {
    /// A path that need not exist: `mock-waves` gates off the on-disk authored
    /// scan, so nothing reads this directory.
    static let repoPath = "/src/loopflow"

    /// The Wave the detail pane opens to. `LOOPFLOW_UI_TEST_SELECT_BRANCH`
    /// overrides it so a screenshot run can target a specific list state.
    static var selectedWaveName: String {
        AppTestMode.selectBranch ?? "infrastructure"
    }

    /// Which detail-pane state the selected Wave renders: `loading` holds the
    /// pre-snapshot state so the plan pane shows its loading affordance;
    /// `error` discards volatile detail and leaves the authored plan under the
    /// "showing cached plan · live status unavailable" footer. A screenshot run
    /// picks one with `LOOPFLOW_UI_TEST_DETAIL_STATE`.
    enum DetailState: String {
        case loading
        case error
    }

    static var detailState: DetailState {
        let raw = ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_DETAIL_STATE"]
        return raw.flatMap(DetailState.init(rawValue:)) ?? .loading
    }

    private static let portfolio: [(wave: Wave, objective: String)] = [
        (Wave(id: "wave-1", name: "infrastructure", repo: repoPath, status: .ready, activeTasks: 1),
         "Make releases boring."),
        (Wave(id: "wave-2", name: "intelligence", repo: repoPath, status: .ready, activeTasks: 2),
         "Loopflow gets sharper from its own evidence."),
        (Wave(id: "wave-3", name: "feedback", repo: repoPath, status: .ready, activeTasks: 0),
         "Every failure becomes one focused piece of work."),
        (Wave(id: "wave-4", name: "cadenza", repo: repoPath, status: .ready, activeTasks: 0, parentWaveId: "wave-1"),
         "Keep the wave's rhythm honest."),
    ]

    static let waves = portfolio.map(\.wave)

    static let plans = Dictionary(uniqueKeysWithValues: portfolio.map { wave, objective in
        let project: WorkReading<ProjectPlanningSnapshot?> = wave.name == "infrastructure"
            ? infrastructureProject : .available(nil)
        return (wave.id, WavePlan(objective: objective, project: project))
    })

    private static let infrastructureProject: WorkReading<ProjectPlanningSnapshot?> = {
        let json = #"""
            {
              "id": "project-1", "tracker_id": null, "sync": {"actions": []},
              "slug": "release-feedback", "name": "Release feedback", "status": "started", "url": null,
              "unbound": "not in Linear yet; `lf wave create release-feedback` makes it"
            }
            """#
        do {
            let project = try JSONDecoder().decode(ProjectPlanningSnapshot.self, from: Data(json.utf8))
            return .available(project)
        } catch {
            return .unavailable(reason: error.localizedDescription)
        }
    }()

    /// The detail reading a `mock-waves` capture renders in the given state,
    /// plus whether the pane still awaits its first live read (the loading
    /// affordance stays on screen only while awaiting). Pure so the screenshot
    /// states are covered without launching the app.
    static func detailReading(state: DetailState) -> (reading: WaveDetailReading, awaitingFirstRead: Bool) {
        (WaveDetailReading(), state == .loading)
    }
}
#endif
