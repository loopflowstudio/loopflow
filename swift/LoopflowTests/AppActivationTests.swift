#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("First-launch activation")
@MainActor
struct AppActivationTests {
    @Test("setup shows progress, surfaces a refusal, and a retry opens the window")
    func failureAndRetry() async throws {
        let helper = HeldActivation()
        let activation = AppActivation(activate: { try await helper.run() })
        let opens = Counter()
        let view = WorkspaceLaunchView(activation: activation) { opens.count += 1 }
        let resumed = Task { await activation.untilActivated() }

        let first = Task { await view.launch() }
        await helper.started()
        #expect(try view.inspect().find(text: "Setting up Loopflow…").string() == "Setting up Loopflow…")
        await helper.finish(failing: "Drag Loopflow into Applications, eject the disk image, then open Loopflow from Applications.")
        await first.value
        _ = try view.inspect().find(text: "Drag Loopflow into Applications, eject the disk image, then open Loopflow from Applications.")
        _ = try view.inspect().find(button: "Retry")
        #expect(opens.count == 0)

        let retry = Task { await view.launch() }
        await helper.started()
        let retryProgress = try? view.inspect().find(text: "Setting up Loopflow…").string()
        await helper.finish(failing: nil)
        await retry.value
        await resumed.value
        #expect(retryProgress == "Setting up Loopflow…")
        #expect(activation.state == .activated)
        #expect(opens.count == 1)
        #expect(await helper.runs == 2)

        // Settled: later windows open without running the helper again.
        await view.launch()
        #expect(await helper.runs == 2)
        #expect(opens.count == 2)
    }

    @Test("a launch with nothing to activate opens directly")
    func nothingToActivate() async throws {
        let activation = AppActivation(activate: nil)
        let opens = Counter()
        let view = WorkspaceLaunchView(activation: activation) { opens.count += 1 }
        #expect(try view.inspect().find(text: "Opening repository…").string() == "Opening repository…")
        await view.launch()
        await activation.untilActivated()
        #expect(opens.count == 1)
    }
}

@MainActor
private final class Counter {
    var count = 0
}

/// The bundled helper, held until the test finishes each run.
private actor HeldActivation {
    private(set) var runs = 0
    private var pending: CheckedContinuation<String?, Never>?
    private var observer: CheckedContinuation<Void, Never>?

    func run() async throws {
        runs += 1
        let failure = await withCheckedContinuation { continuation in
            pending = continuation
            observer?.resume()
            observer = nil
        }
        if let failure { throw LocalLfError(errorDescription: failure) }
    }

    func started() async {
        if pending != nil { return }
        await withCheckedContinuation { observer = $0 }
    }

    func finish(failing failure: String?) {
        pending?.resume(returning: failure)
        pending = nil
    }
}
#endif
