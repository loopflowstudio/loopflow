// Tests for Agent model and Stimulus struct.

import Foundation
import SwiftUI
import Testing
@testable import LoopflowCore

@Suite("Agent Model")
struct AgentModelTests {

    // MARK: - Display Name

    @Test("displayName uses name when set")
    func displayNameUsesName() {
        let agent = Agent(
            id: "test-id",
            name: "swift-falcon",
            repo: "/tmp/repo"
        )

        #expect(agent.displayName == "swift-falcon")
    }

    @Test("displayName generates from area and flow when name is empty")
    func displayNameGeneratesFromConfig() {
        let agent = Agent(
            id: "test-id",
            name: "",
            flow: "ship",
            area: ["src/auth"],
            repo: "/tmp/repo"
        )

        #expect(agent.displayName == "src/auth · ship")
    }

    @Test("displayName shows 'root' for dot area")
    func displayNameRootForDotArea() {
        let agent = Agent(
            id: "test-id",
            name: "",
            flow: "debug",
            area: ["."],
            repo: "/tmp/repo"
        )

        #expect(agent.displayName == "root · debug")
    }

    @Test("displayName shows default flow when empty")
    func displayNameDefaultFlow() {
        let agent = Agent(
            id: "test-id",
            name: "",
            flow: "",
            area: nil,
            repo: "/tmp/repo"
        )

        #expect(agent.displayName == "root · default")
    }

    // MARK: - Status Indicator

    @Test("statusIndicator returns green circle for running")
    func statusIndicatorRunning() {
        let agent = Agent(id: "test", repo: "/tmp", status: .running)
        let indicator = agent.statusIndicator

        #expect(indicator.icon == "circle.fill")
        #expect(indicator.color == .green)
    }

    @Test("statusIndicator returns yellow half-circle for waiting")
    func statusIndicatorWaiting() {
        let agent = Agent(id: "test", repo: "/tmp", status: .waiting)
        let indicator = agent.statusIndicator

        #expect(indicator.icon == "circle.lefthalf.filled")
        #expect(indicator.color == .yellow)
    }

    @Test("statusIndicator returns gray circle for idle")
    func statusIndicatorIdle() {
        let agent = Agent(id: "test", repo: "/tmp", status: .idle)
        let indicator = agent.statusIndicator

        #expect(indicator.icon == "circle")
        #expect(indicator.color == .gray)
    }

    @Test("statusIndicator returns clock for idle cron agent")
    func statusIndicatorIdleCron() {
        let agent = Agent(
            id: "test",
            repo: "/tmp",
            stimulus: Stimulus(kind: .cron, cron: "0 9 * * *"),
            status: .idle
        )
        let indicator = agent.statusIndicator

        #expect(indicator.icon == "clock")
        #expect(indicator.color == .gray)
    }

    @Test("statusIndicator returns red X for error")
    func statusIndicatorError() {
        let agent = Agent(id: "test", repo: "/tmp", status: .error)
        let indicator = agent.statusIndicator

        #expect(indicator.icon == "xmark.circle.fill")
        #expect(indicator.color == .red)
    }

    @Test("statusIndicator returns green checkmark for completed")
    func statusIndicatorCompleted() {
        let agent = Agent(id: "test", repo: "/tmp", status: .completed)
        let indicator = agent.statusIndicator

        #expect(indicator.icon == "checkmark.circle.fill")
        #expect(indicator.color == .green)
    }

    // MARK: - Computed Properties

    @Test("areaDisplay joins multiple areas")
    func areaDisplayJoins() {
        let agent = Agent(
            id: "test",
            area: ["src/", "lib/"],
            repo: "/tmp"
        )

        #expect(agent.areaDisplay == "src/, lib/")
    }

    @Test("areaDisplay returns dot for root area")
    func areaDisplayDot() {
        let agent = Agent(id: "test", area: ["."], repo: "/tmp")

        #expect(agent.areaDisplay == ".")
    }

    @Test("areaDisplay returns empty for nil area")
    func areaDisplayNil() {
        let agent = Agent(id: "test", area: nil, repo: "/tmp")

        #expect(agent.areaDisplay == "")
    }

    @Test("goalDisplay joins multiple goals")
    func goalDisplayJoins() {
        let agent = Agent(
            id: "test",
            goal: ["product-engineer", "designer"],
            repo: "/tmp"
        )

        #expect(agent.goalDisplay == "product-engineer, designer")
    }

    @Test("goalDisplay returns empty for nil goal")
    func goalDisplayNil() {
        let agent = Agent(id: "test", goal: nil, repo: "/tmp")

        #expect(agent.goalDisplay == "")
    }

    @Test("flowDisplay returns flow name")
    func flowDisplayName() {
        let agent = Agent(id: "test", flow: "polish", repo: "/tmp")

        #expect(agent.flowDisplay == "polish")
    }

    @Test("flowDisplay returns ship for empty flow")
    func flowDisplayDefault() {
        let agent = Agent(id: "test", flow: "", repo: "/tmp")

        #expect(agent.flowDisplay == "ship")
    }

    @Test("shortId returns first 7 characters")
    func shortIdTruncates() {
        let agent = Agent(id: "abcdefghijklmnop", repo: "/tmp")

        #expect(agent.shortId == "abcdefg")
    }

    // MARK: - isConfigured

    @Test("isConfigured returns true when area is set")
    func isConfiguredWithArea() {
        let agent = Agent(id: "test", area: ["src/"], repo: "/tmp")

        #expect(agent.isConfigured == true)
    }

    @Test("isConfigured returns false when area is nil")
    func isConfiguredWithoutArea() {
        let agent = Agent(id: "test", area: nil, repo: "/tmp")

        #expect(agent.isConfigured == false)
    }

    // MARK: - Detail Text

    @Test("detailText combines area, flow, and stimulus")
    func detailTextCombines() {
        let agent = Agent(
            id: "test",
            flow: "ship",
            area: ["src/"],
            repo: "/tmp",
            stimulus: Stimulus(kind: .loop)
        )

        #expect(agent.detailText == "src/ · ship · loop")
    }

    @Test("detailText omits manual stimulus")
    func detailTextOmitsManual() {
        let agent = Agent(
            id: "test",
            flow: "debug",
            area: ["."],
            repo: "/tmp",
            stimulus: Stimulus(kind: .manual)
        )

        #expect(agent.detailText == ". · debug")
    }

    // MARK: - Iteration Text

    @Test("iterationText shows iter count when positive")
    func iterationTextPositive() {
        let agent = Agent(id: "test", repo: "/tmp", iteration: 5)

        #expect(agent.iterationText == "iter 5")
    }

    @Test("iterationText is empty when zero")
    func iterationTextZero() {
        let agent = Agent(id: "test", repo: "/tmp", iteration: 0)

        #expect(agent.iterationText == "")
    }
}

@Suite("Stimulus")
struct StimulusTests {

    @Test("description returns kind for non-cron")
    func descriptionNonCron() {
        #expect(Stimulus(kind: .manual).description == "manual")
        #expect(Stimulus(kind: .once).description == "once")
        #expect(Stimulus(kind: .loop).description == "loop")
        #expect(Stimulus(kind: .watch).description == "watch")
    }

    @Test("description includes cron expression")
    func descriptionCron() {
        let stimulus = Stimulus(kind: .cron, cron: "0 9 * * *")

        #expect(stimulus.description == "cron(0 9 * * *)")
    }

    @Test("icon returns correct SF Symbol for each kind")
    func iconForKind() {
        #expect(Stimulus(kind: .manual).icon == "circle")
        #expect(Stimulus(kind: .once).icon == "play.circle")
        #expect(Stimulus(kind: .loop).icon == "circle.fill")
        #expect(Stimulus(kind: .watch).icon == "eye.circle")
        #expect(Stimulus(kind: .cron).icon == "clock")
    }
}

@Suite("AgentStatus")
struct AgentStatusTests {

    @Test("color returns correct SwiftUI color")
    func colorForStatus() {
        #expect(AgentStatus.running.color == .green)
        #expect(AgentStatus.waiting.color == .yellow)
        #expect(AgentStatus.idle.color == .gray)
        #expect(AgentStatus.completed.color == .green)
        #expect(AgentStatus.error.color == .red)
    }

    @Test("icon returns correct SF Symbol")
    func iconForStatus() {
        #expect(AgentStatus.running.icon == "circle.fill")
        #expect(AgentStatus.waiting.icon == "circle.lefthalf.filled")
        #expect(AgentStatus.idle.icon == "circle")
        #expect(AgentStatus.completed.icon == "checkmark.circle.fill")
        #expect(AgentStatus.error.icon == "xmark.circle.fill")
    }
}
