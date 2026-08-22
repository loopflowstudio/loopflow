import Foundation
import Testing

@testable import Loopflow
@testable import LoopflowMac

/// The `mock-waves` UI-test fixture must render the same populated surface the
/// registry would — the stable list's lens states and the selected Wave's full
/// detail hierarchy — so the offline screenshot and AttributeGraph cycle capture
/// exercise real data, not an empty shell.
@Suite("Mock wave fixture")
struct MockWaveFixtureTests {
    @Test("live UI-test mode keeps production registry reads")
    func liveModeUsesRegistry() {
        #expect(AppTestMode.live.bypassesRegistry == false)
        #expect(AppTestMode.mockWaves.bypassesRegistry)
        #expect(AppTestMode.emptyWorkspaces.bypassesRegistry)
    }

    @Test("the stable list carries one Wave per lens state, plus a child")
    func listLensStates() {
        let byName = Dictionary(uniqueKeysWithValues: MockWaveFixture.waves.map { ($0.name, $0) })

        func lens(_ name: String) -> WaveLens {
            WaveViewModel(api: byName[name]!, isRegistered: true).lens
        }

        #expect(lens("infrastructure").color == .green)   // a live body
        #expect(lens("intelligence").color == .red)       // stopped with active work
        #expect(lens("feedback").color == .black)          // off and clean
        #expect(lens("cadenza").color == .green)
        #expect(byName["cadenza"]?.parentWaveId == "wave-1")  // future-ancestry indentation
    }

    @Test("the selected Wave decodes into the populated detail hierarchy with verbatim lenses")
    func selectedDetailHierarchy() throws {
        let detail = try #require(MockWaveFixture.selectedWaveDetail())
        let workMap = detail.workMap

        #expect(workMap.objective == "Make releases boring.")
        let project = try #require(workMap.projects.first)
        #expect(project.project.krs.count == 1)
        #expect(project.tasks.filter { !$0.task.completed }.count == 2)
        #expect(project.runtime?.lastFailure?.message.contains("credential") == true)

        // Project row lens folds its Tasks' attention (runtime not running):
        // the red Task outranks the black one.
        #expect(WaveLens.forProject(runtime: project.runtime, tasks: project.tasks).color == .red)

        // Task rows: the shared attention level verbatim.
        let byId = Dictionary(uniqueKeysWithValues: project.tasks.map { ($0.task.identifier, $0) })
        #expect(WaveLens.forTask(try #require(byId["INF-123"]).attention).color == .red)
        #expect(WaveLens.forTask(try #require(byId["INF-124"]).attention).color == .black)
    }

    // The Proof requires the screenshot fixture to cover empty, loading, error,
    // selected, and future-child indentation. Empty is the `empty-workspaces`
    // mode; selected and indentation are proven above. These cover the detail
    // states the mock surface drives with `LOOPFLOW_UI_TEST_DETAIL_STATE`.

    @Test("the selected detail state renders the populated hierarchy, no longer awaiting")
    func selectedDetailState() {
        let outcome = MockWaveFixture.detailReading(
            waveName: MockWaveFixture.detailWaveName,
            state: .selected
        )
        #expect(outcome.reading.snapshot != nil)
        #expect(outcome.reading.errorMessage == nil)
        #expect(outcome.awaitingFirstRead == false)
    }

    @Test("the loading detail state withholds the snapshot and keeps the loading affordance")
    func loadingDetailState() {
        let outcome = MockWaveFixture.detailReading(
            waveName: MockWaveFixture.detailWaveName,
            state: .loading
        )
        #expect(outcome.reading.snapshot == nil)
        #expect(outcome.reading.errorMessage == nil)
        #expect(outcome.awaitingFirstRead)  // loading affordance stays on screen
    }

    @Test("the error detail state preserves the last-good detail under a framed footer")
    func errorDetailState() {
        let outcome = MockWaveFixture.detailReading(
            waveName: MockWaveFixture.detailWaveName,
            state: .error
        )
        // The cached detail survives the failed refresh (PR #932 behavior)...
        #expect(outcome.reading.snapshot?.wave.name == "infrastructure")
        // ...framed as a quiet footer reason, never a raw error dominating the pane.
        #expect(outcome.reading.errorMessage == "Wave status unavailable: the local registry is unreachable")
        #expect(outcome.awaitingFirstRead == false)
    }

    @Test("detailState reads LOOPFLOW_UI_TEST_DETAIL_STATE, defaulting to selected")
    func detailStateParsing() {
        #expect(MockWaveFixture.DetailState(rawValue: "loading") == .loading)
        #expect(MockWaveFixture.DetailState(rawValue: "error") == .error)
        #expect(MockWaveFixture.DetailState(rawValue: "selected") == .selected)
        #expect(MockWaveFixture.DetailState(rawValue: "nonsense") == nil)
    }
}
