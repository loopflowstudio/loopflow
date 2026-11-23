import Loopflow
import SwiftUI

struct SettingsTargetPicker: View {
    let model: ScopedSettingsModel
    let checkout: String
    @State private var expanded = false
    @State private var skills: [String] = []
    @State private var graphs: [String] = []
    @State private var graph: String?
    @State private var steps: [ConfigurationInspection] = []
    @State private var error: String?

    var body: some View {
        DisclosureGroup("Skill or Graph step", isExpanded: $expanded) {
            Menu("Inspect Skill") {
                ForEach(skills, id: \.self) { name in Button(name) { model.target = .skill(name) } }
            }
            Picker("Graph", selection: $graph) {
                Text("Choose Graph").tag(String?.none)
                ForEach(graphs, id: \.self) { Text($0).tag(Optional($0)) }
            }
            if graph != nil {
                Menu("Inspect Graph step") {
                    ForEach(Array(steps.enumerated()), id: \.offset) { _, inspection in
                        if case .object(let occurrence) = inspection.occurrence,
                           case .string(let graph) = occurrence["graph"],
                           case .string(let step) = occurrence["id"] == .null ? occurrence["address"] : occurrence["id"] {
                            Button("\(inspection.skill ?? step) · \(graph) / \(step)") { model.target = .step(graph: graph, step: step) }
                        }
                    }
                }
            }
            if let error { Text(error) }
        }
        .task(id: expanded ? checkout : nil) {
            guard expanded else { return }
            struct Entry: Decodable { let name: String; let kind: String }
            do {
                let entries = try await ServerClient.read(.skillList(directory: checkout, path: []), location: model.location).decode([Entry].self)
                if Task.isCancelled { return }
                skills = entries.filter { $0.kind == "skill" }.map(\.name).sorted()
                graphs = entries.filter { $0.kind == "graph" }.map(\.name).sorted()
                error = nil
            } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
        }
        .task(id: graph) {
            steps = []
            guard let graph else { return }
            do {
                let value = try await ServerClient.read(.configSettings(directory: checkout, wave: model.wave, skill: nil,
                    graph: graph, step: nil), location: model.location).decode([ConfigurationInspection].self)
                if !Task.isCancelled { steps = value; error = nil }
            } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
        }
    }
}
