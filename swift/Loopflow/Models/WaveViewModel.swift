import Foundation
import SwiftUI

public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave
    public var content: WaveContent?
    public var plan: WavePlan?
    public var pid: Int?

    public init(
        api: Wave,
        content: WaveContent? = nil,
        plan: WavePlan? = nil,
        pid: Int? = nil
    ) {
        self.api = api
        self.content = content
        self.plan = plan
        self.pid = pid
    }

    public var id: String { api.id }

    public var name: String {
        get { api.name }
        set { api.name = newValue }
    }

    public var repo: String {
        get { api.repo }
        set { api.repo = newValue }
    }

    public var direction: [String] {
        get { api.direction }
        set { api.direction = newValue }
    }

    public var area: [String] {
        get { api.area }
        set { api.area = newValue }
    }

    public var agent: String? {
        get { api.agent }
        set { api.agent = newValue }
    }

    public var skillAgents: [String: String]? {
        get { api.skillAgents }
        set { api.skillAgents = newValue }
    }

    public var triggers: [Trigger] {
        get { api.triggers }
        set { api.triggers = newValue }
    }

    /// First active trigger, if any.
    public var trigger: Trigger? { triggers.first }

    public var status: WaveStatus {
        get { api.status }
        set { api.status = newValue }
    }

    public var iteration: Int {
        get { api.iteration }
        set { api.iteration = newValue }
    }

    public var createdAt: Date? {
        get { api.createdAt }
        set { api.createdAt = newValue }
    }

    public var shortId: String { String(id.prefix(7)) }

    public var displayName: String {
        if !name.isEmpty { return name }
        return area.first.map { $0 == "." ? "root" : $0 } ?? "root"
    }

    /// First line of the vision section — the tagline.
    public var visionTagline: String? {
        guard let vision = content?.vision else { return nil }
        let firstLine = vision.components(separatedBy: .newlines)
            .first { !$0.trimmingCharacters(in: .whitespaces).isEmpty }?
            .trimmingCharacters(in: .whitespaces)
        return firstLine
    }

    public var areaDisplay: String {
        if area.isEmpty { return "" }
        return area.first == "." ? "." : area.joined(separator: ", ")
    }

    public var directionDisplay: String {
        direction.isEmpty ? "" : direction.joined(separator: ", ")
    }

    public var statusText: String {
        switch status {
        case .running: return "Running"
        case .waiting: return "Waiting"
        case .idle: return "Idle"
        case .failed: return "Failed"
        case .paused: return "Paused"
        }
    }

    public var iterationText: String {
        iteration > 0 ? "iter \(iteration)" : ""
    }

    public var detailText: String {
        var parts: [String] = []
        if !areaDisplay.isEmpty { parts.append(areaDisplay) }
        if let t = trigger { parts.append(t.signal.rawValue) }
        return parts.joined(separator: " · ")
    }

    public var triggerText: String {
        trigger?.description ?? "manual"
    }

    public var hasActiveTrigger: Bool { !triggers.isEmpty }

    public var statusIndicator: (icon: String, color: Color) {
        switch status {
        case .running:
            return ("circle.fill", .statusSuccess)
        case .waiting:
            return ("circle.lefthalf.filled", .statusWarning)
        case .failed:
            return ("xmark.circle.fill", .statusError)
        case .idle:
            return ("circle", .statusNeutral)
        case .paused:
            return ("pause.circle", .statusNeutral)
        }
    }
}
