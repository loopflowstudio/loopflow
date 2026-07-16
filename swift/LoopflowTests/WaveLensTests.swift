import Testing
@testable import Loopflow

@Suite("Wave Lens")
struct WaveLensTests {
    @Test("green when a body is live")
    func greenWhenLive() {
        let lens = WaveLens.provisional(live: true, status: .idle, activeTasks: 0, activeProjects: 0)
        #expect(lens.color == .green)
        #expect(lens.reason.contains("Running"))
    }

    @Test("green when running even without an explicit live flag")
    func greenWhenRunning() {
        let lens = WaveLens.provisional(live: false, status: .running, activeTasks: 0, activeProjects: 0)
        #expect(lens.color == .green)
    }

    @Test("red when stopped with outstanding work")
    func redWhenOutstanding() {
        let lens = WaveLens.provisional(live: false, status: .idle, activeTasks: 2, activeProjects: 1)
        #expect(lens.color == .red)
        #expect(lens.reason.contains("3"))
    }

    @Test("black when off and clean")
    func blackWhenClean() {
        let lens = WaveLens.provisional(live: false, status: .idle, activeTasks: 0, activeProjects: 0)
        #expect(lens.color == .black)
        #expect(!lens.color.isLit)
    }

    @Test("every lens carries a reason for VoiceOver")
    func everyLensHasReason() {
        for lens in [
            WaveLens.provisional(live: true, status: .running, activeTasks: 0, activeProjects: 0),
            WaveLens.provisional(live: false, status: .idle, activeTasks: 1, activeProjects: 0),
            WaveLens.provisional(live: false, status: .idle, activeTasks: 0, activeProjects: 0),
        ] {
            #expect(!lens.reason.isEmpty)
        }
    }
}
