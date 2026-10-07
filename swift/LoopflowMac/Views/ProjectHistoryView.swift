#if os(macOS)
import Loopflow
import SwiftUI

struct ProjectHistoryView: View {
    let wave: String
    let repo: String
    var sourceReference: String? = nil
    @Environment(\.dismiss) private var dismiss
    @State private var projects: [ProjectPlanningSnapshot] = []
    @State private var selected: String?
    @State private var error: String?
    @State private var loaded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("\(wave) · Projects").font(.headline)
                Spacer()
                Button("Done") { dismiss() }
            }
            if let error { Text(error).foregroundStyle(Color.statusWarning) }
            if !loaded { ProgressView("Reading Projects…") }
            if loaded && projects.isEmpty && error == nil { Text("No Projects in this Wave.") }
            Picker("Project", selection: $selected) {
                ForEach(projects) { project in
                    Text("\(project.name) · \(project.status.rawValue)").tag(Optional(project.id))
                }
            }
            ScrollView {
                if let project = projects.first(where: { $0.id == selected }) {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Workflow: \(project.workflow)")
                        ForEach(project.krs) { kr in
                            Label(kr.text, systemImage: kr.holds ? "checkmark.circle.fill" : "circle")
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .padding(24).frame(minWidth: 500, minHeight: 420)
        .task {
            do {
                let status = try await RegistryQueryLocal.shared.status(wave: wave, cwd: repo)
                projects = status.projects.items
                error = status.projects.unavailableReason
                selected = projects.first(where: { $0.id == sourceReference || $0.slug == sourceReference || (sourceReference != nil && $0.workId == sourceReference) })?.id
                    ?? status.currentProject?.id ?? projects.first?.id
            } catch { self.error = error.localizedDescription }
            loaded = true
        }
    }
}
#endif
