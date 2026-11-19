import Foundation

public struct WavePlan: Sendable, Hashable {
    public var objective: String
    public var projects: WorkEvidence<ProjectPlanningSnapshot>

    public init(objective: String, projects: WorkEvidence<ProjectPlanningSnapshot> = .unavailable(reason: "Project planning has not been read")) {
        self.objective = objective
        self.projects = projects
    }
}
