import Foundation
import Testing

@testable import Loopflow
@testable import LoopflowMac

/// The `mock-waves` UI-test fixture renders the states no data can produce —
/// loading and error — over the stable list. Populated states render from an
/// `lf-world` world in `live` mode.
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
            WaveViewModel(api: byName[name]!, isRegistered: true).lens
        }

        #expect(lens("infrastructure").color == .unknown)
        #expect(lens("intelligence").color == .unknown)
        #expect(lens("feedback").color == .black)          // off and clean
        #expect(lens("cadenza").color == .black)
        #expect(byName["cadenza"]?.parentWaveId == "wave-1")  // future-ancestry indentation
    }

    // Empty is the `empty-workspaces` mode and indentation is proven above.
    // These cover the detail states the mock surface drives with
    // `LOOPFLOW_UI_TEST_DETAIL_STATE`.

    @Test("the loading detail state withholds the snapshot and keeps the loading affordance")
    func loadingDetailState() {
        let outcome = MockWaveFixture.detailReading(state: .loading)
        #expect(outcome.reading.snapshot == nil)
        #expect(outcome.reading.errorMessage == nil)
        #expect(outcome.awaitingFirstRead)  // loading affordance stays on screen
    }

    @Test("the error detail state discards volatile detail under a framed footer")
    func errorDetailState() {
        let outcome = MockWaveFixture.detailReading(state: .error)
        #expect(outcome.reading.snapshot == nil)
        // The cached authored plan remains outside the reading; the failure is
        // framed as a quiet footer reason, never stale operational evidence.
        #expect(outcome.reading.errorMessage == "Wave status unavailable: the local registry is unreachable")
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
