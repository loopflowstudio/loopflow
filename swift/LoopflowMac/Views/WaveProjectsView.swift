#if os(macOS)
import Loopflow
import SwiftUI

struct WaveProjectsView: View {
    @Environment(\.dismiss) private var dismiss

    let wave: String
    let repo: String

    @State private var projects: WorkEvidence<ProjectPlanningSnapshot>?

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            HStack {
                Text("\(wave) · Project").font(Typography.textStrong)
                Spacer()
                Button("Done") { dismiss() }.buttonStyle(LoopflowButtonStyle())
            }
            switch projects {
            case nil:
                ProgressView("Reading the Project…")
            case .unavailable(let reason):
                FailureLine("The Project did not load.", machine: reason)
            case .available(let entries, _):
                if let project = entries.first {
                    ScrollView {
                        WaveProjectView(project: project, repo: repo)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                } else {
                    Text("This Wave has no Project yet.").emptyLine()
                }
            }
        }
        .padding(Spacing.xxl).frame(minWidth: 500, minHeight: 420, alignment: .topLeading)
        .task {
            do {
                projects = try await RegistryQueryLocal.shared.status(wave: wave, cwd: repo).projects
            } catch {
                projects = .unavailable(reason: error.localizedDescription)
            }
        }
    }
}
#endif
