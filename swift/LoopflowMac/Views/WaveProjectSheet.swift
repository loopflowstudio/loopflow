#if os(macOS)
import Loopflow
import SwiftUI

struct WaveProjectSheet: View {
    @Environment(\.dismiss) private var dismiss

    let wave: String
    let repo: String

    @State private var project: WorkReading<ProjectPlanningSnapshot?>?

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            HStack {
                Text("\(wave) · Project").font(Typography.textStrong)
                Spacer()
                Button("Done") { dismiss() }.buttonStyle(LoopflowButtonStyle())
            }
            switch project {
            case nil:
                ProgressView("Reading the Project…")
            case .unavailable(let reason):
                FailureLine("The Project did not load.", machine: reason)
            case .available(let project?):
                ScrollView {
                    WaveProjectView(project: project, repo: repo)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            case .available(.none):
                Text("This Wave has no Project yet.").emptyLine()
            }
        }
        .padding(Spacing.xxl).frame(minWidth: 500, minHeight: 420, alignment: .topLeading)
        .task {
            do {
                project = try await RegistryQueryLocal.shared.status(wave: wave, cwd: repo).project
            } catch {
                project = .unavailable(reason: error.localizedDescription)
            }
        }
    }
}
#endif
