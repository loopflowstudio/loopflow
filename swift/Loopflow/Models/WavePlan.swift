import Foundation

public struct WavePlan: Sendable, Hashable {
    public var objective: String
    public var project: WorkReading<ProjectPlanningSnapshot?>

    public init(objective: String, project: WorkReading<ProjectPlanningSnapshot?> = .unavailable(reason: "Project planning has not been read")) {
        self.objective = objective
        self.project = project
    }
}
