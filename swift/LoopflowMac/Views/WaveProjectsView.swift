#if os(macOS)
import Loopflow
import SwiftUI

struct WaveProjectsView: View {
    @Environment(\.palette) private var palette
    @Environment(\.dismiss) private var dismiss

    let wave: String
    let repo: String
    var sourceReference: String? = nil

    @State private var projects: WorkEvidence<ProjectPlanningSnapshot>?

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            HStack {
                Text("\(wave) · Projects").font(Typography.textStrong)
                Spacer()
                Button("Done") { dismiss() }.buttonStyle(LoopflowButtonStyle())
            }
            switch projects {
            case nil:
                ProgressView("Reading Projects…")
            case .unavailable(let reason):
                FailureLine("Projects did not load.", machine: reason)
            case .available(let entries, let truncated):
                if entries.isEmpty && !truncated {
                    Text("No Projects in this Wave.").emptyLine()
                } else {
                    ScrollViewReader { proxy in
                        ScrollView {
                            VStack(alignment: .leading, spacing: Spacing.lg) {
                                ForEach(entries) { project in
                                    WaveProjectView(project: project, repo: repo).id(project.id)
                                }
                                if truncated { Text("More Projects are available.").foregroundStyle(palette.warning) }
                            }.frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .onAppear {
                            if let sourceReference,
                               let project = entries.first(where: { $0.matches(reference: sourceReference) }) {
                                proxy.scrollTo(project.id)
                            }
                        }
                    }
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
