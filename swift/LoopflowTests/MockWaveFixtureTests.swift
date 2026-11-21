import Foundation
import Testing

@testable import Loopflow
@testable import LoopflowMac

@Suite("Mock wave fixture")
struct MockWaveFixtureTests {
    @Test("live UI-test mode keeps production registry reads")
    func liveModeUsesRegistry() {
        #expect(AppTestMode.live.bypassesRegistry == false)
        #expect(AppTestMode.mockWaves.bypassesRegistry)
        #expect(AppTestMode.emptyWorkspaces.bypassesRegistry)
    }

    @Test("the stable list preserves active work and Wave ancestry")
    func listLensStates() {
        let byName = Dictionary(uniqueKeysWithValues: MockWaveFixture.waves.map { ($0.name, $0) })

        func lens(_ name: String) -> WaveLens {
            WaveViewModel(api: byName[name]!).lens
        }

        #expect(lens("infrastructure").color == .unknown)
        #expect(lens("intelligence").color == .unknown)
        #expect(lens("feedback").color == .black)          // off and clean
        #expect(lens("cadenza").color == .black)
        #expect(byName["cadenza"]?.parentWaveId == "wave-1")  // future-ancestry indentation
    }

    @Test("the empty-workspaces fixture decodes as current wire types")
    @MainActor
    func emptyWorkspacesDecode() throws {
        #expect(try WorkFixture.emptyRoadmap().waves.isEmpty)
        #expect(try WorkFixture.emptyWorkActivity().items.isEmpty)
    }

    // Indentation is proven above.
    // These cover the detail states the mock surface drives with
    // `LOOPFLOW_UI_TEST_DETAIL_STATE`.

    @Test("the loading detail state withholds the snapshot and keeps the loading affordance")
    func loadingDetailState() {
        let outcome = MockWaveFixture.detailReading(state: .loading)
        #expect(outcome.reading.snapshot == nil)
        #expect(outcome.awaitingFirstRead)  // loading affordance stays on screen
    }

    @Test("the error detail state discards volatile detail under a framed footer")
    func errorDetailState() {
        let outcome = MockWaveFixture.detailReading(state: .error)
        // The cached authored plan remains outside the reading; no stale
        // operational evidence shows.
        #expect(outcome.reading.snapshot == nil)
        #expect(outcome.awaitingFirstRead == false)
    }

    @Test("detailState reads LOOPFLOW_UI_TEST_DETAIL_STATE")
    func detailStateParsing() {
        #expect(MockWaveFixture.DetailState(rawValue: "loading") == .loading)
        #expect(MockWaveFixture.DetailState(rawValue: "error") == .error)
        #expect(MockWaveFixture.DetailState(rawValue: "selected") == nil)
        #expect(MockWaveFixture.DetailState(rawValue: "nonsense") == nil)
    }
}
