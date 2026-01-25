// UI tests for AgentRow view.

import SwiftUI
import Testing
import ViewInspector
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("Agent Row View")
struct AgentRowViewTests {
    private func makeAgent(
        name: String = "swift-falcon",
        area: [String]? = ["src/"],
        flow: String = "ship",
        status: AgentStatus = .idle,
        iteration: Int = 0,
        stimulus: Stimulus = Stimulus(kind: .manual)
    ) -> Agent {
        Agent(
            id: "test-agent-id",
            name: name,
            area: area,
            flow: flow,
            repo: "/tmp/repo",
            stimulus: stimulus,
            status: status,
            iteration: iteration
        )
    }

    private func makeRow(
        agent: Agent,
        isSelected: Bool = false,
        liveOutput: [OutputLine] = [],
        pendingPR: (number: Int, url: URL?)? = nil
    ) -> AgentRow {
        AgentRow(
            agent: agent,
            isSelected: isSelected,
            liveOutput: liveOutput,
            pendingPR: pendingPR,
            onSelect: {}
        )
    }

    @Test("Row shows agent display name")
    func showsDisplayName() throws {
        let agent = makeAgent(name: "aurora-melody")
        let row = makeRow(agent: agent)

        let nameText = try row.inspect().find(viewWithAccessibilityIdentifier: "agent-name").text()
        #expect(try nameText.string() == "aurora-melody")
    }

    @Test("Row shows flow badge")
    func showsFlowBadge() throws {
        let agent = makeAgent(flow: "polish")
        let row = makeRow(agent: agent)

        let flowText = try row.inspect().find(viewWithAccessibilityIdentifier: "agent-flow").text()
        #expect(try flowText.string() == "polish")
    }

    @Test("Row shows area in secondary line")
    func showsArea() throws {
        let agent = makeAgent(area: ["lib/core"])
        let row = makeRow(agent: agent)

        let areaText = try row.inspect().find(viewWithAccessibilityIdentifier: "agent-area").text()
        #expect(try areaText.string() == "lib/core")
    }

    @Test("Row shows iteration when positive")
    func showsIteration() throws {
        let agent = makeAgent(iteration: 3)
        let row = makeRow(agent: agent)

        let iterText = try row.inspect().find(viewWithAccessibilityIdentifier: "agent-iteration").text()
        #expect(try iterText.string() == "iter 3")
    }

    @Test("Row shows PR limit text when waiting")
    func showsPRLimitWhenWaiting() throws {
        let agent = makeAgent(status: .waiting)
        let row = makeRow(agent: agent)

        let prLimitText = try row.inspect().find(viewWithAccessibilityIdentifier: "agent-pr-limit").text()
        #expect(try prLimitText.string() == "PR limit")
    }

    @Test("Row shows cron schedule for cron agents")
    func showsCronSchedule() throws {
        let agent = makeAgent(stimulus: Stimulus(kind: .cron, cron: "0 9 * * *"))
        let row = makeRow(agent: agent)

        let cronText = try row.inspect().find(viewWithAccessibilityIdentifier: "agent-cron").text()
        #expect(try cronText.string() == "9am daily")
    }

    @Test("Row shows raw cron when not a known pattern")
    func showsRawCron() throws {
        let agent = makeAgent(stimulus: Stimulus(kind: .cron, cron: "*/15 * * * *"))
        let row = makeRow(agent: agent)

        let cronText = try row.inspect().find(viewWithAccessibilityIdentifier: "agent-cron").text()
        #expect(try cronText.string() == "*/15 * * * *")
    }

}
