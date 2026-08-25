#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// The fader collapses two evidence channels — process activity and human
/// attention — into four phases, and each phase owns exactly one press verb.
@Suite("Fader switch")
struct FaderSwitchTests {
    @Test("A human-owed stop wins over any sibling activity")
    func humanStopWins() {
        for signal in [PodiumSignalState.off, .producing, .blocked, .waiting, .unknown] {
            #expect(
                ConsoleSignal.phase(humanStop: true, agentRunning: true, signal: signal)
                    == .waiting
            )
        }
    }

    @Test("Process signal maps onto the collapsed phase set")
    func signalCollapse() {
        #expect(ConsoleSignal.phase(humanStop: false, agentRunning: false, signal: .producing) == .producing)
        #expect(ConsoleSignal.phase(humanStop: false, agentRunning: false, signal: .blocked) == .waiting)
        #expect(ConsoleSignal.phase(humanStop: false, agentRunning: false, signal: .waiting) == .starting)
        #expect(ConsoleSignal.phase(humanStop: false, agentRunning: false, signal: .off) == .off)
        #expect(ConsoleSignal.phase(humanStop: false, agentRunning: false, signal: .unknown) == .off)
    }

    @Test("A spun-up but quiet agent reads as starting, not off")
    func quietAgentIsStarting() {
        #expect(ConsoleSignal.phase(humanStop: false, agentRunning: true, signal: .off) == .starting)
        #expect(ConsoleSignal.phase(humanStop: false, agentRunning: true, signal: .unknown) == .starting)
    }

    @Test("Each phase owns one press verb")
    func verbs() {
        #expect(FaderPhase.off.verb == "Start")
        #expect(FaderPhase.starting.verb == "Stop")
        #expect(FaderPhase.producing.verb == "Stop")
        #expect(FaderPhase.waiting.verb == "Resolve")
    }

    @Test("Press-to-start follows the server's recommended move")
    func taskStartFollowsRecommendation() throws {
        let fixture = try ConsoleFixture.load()
        let tasks = Dictionary(
            uniqueKeysWithValues: fixture.roadmap.waves
                .flatMap(\.projects.items)
                .flatMap(\.tasks)
                .map { ($0.id, $0) }
        )

        // W2-156 has no Work yet: the press runs `lf task run`.
        let available = try #require(tasks["issue-available"])
        #expect(available.runtime == nil)
        #expect(ConsoleSignal.taskStart(available) == .run)

        // A completed Task offers no start at all — its fader is evidence only.
        let completed = try #require(tasks.values.first { $0.task.completed })
        #expect(ConsoleSignal.taskStart(completed) == nil)
    }
}

private struct ConsoleFixture {
    let roadmap: RoadmapSnapshot

    static func load(sourceFile: String = #filePath) throws -> ConsoleFixture {
        let fixtures = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let data = try Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))
        return ConsoleFixture(roadmap: try JSONDecoder().decode(RoadmapSnapshot.self, from: data))
    }
}
#endif
